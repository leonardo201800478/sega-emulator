//! Versioned, deterministic binary codec for machine-scheduler state.
//!
//! The wire format is explicit and independent of Serde. Every u128 temporal
//! scalar is encoded as exactly 16 unsigned big-endian bytes. Event payloads
//! are delegated to a caller-supplied codec and length-prefixed.

use std::fmt;

use sega_temporal_core::{
    EventQueueState, EventQueueWatermark, HardwarePhase, MachineTime, PlatformPriority,
    ScheduledEvent,
};

use crate::{
    ClockDomainId, ClockDomainState, SCHEDULER_STATE_FORMAT_VERSION, SchedulerState,
};

/// Eight-byte identifier at the beginning of every scheduler-state frame.
pub const SCHEDULER_STATE_MAGIC: [u8; 8] = *b"SEGSCHD\0";

const CLOCK_DOMAIN_WIRE_SIZE: usize = 2 + 8 + 8 + (16 * 5);
const EVENT_FIXED_WIRE_SIZE: usize = 16 + 2 + 2 + 8 + 4;

/// Errors returned by the scheduler-state wire codec.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SchedulerStateCodecError {
    InvalidMagic,
    UnsupportedVersion { expected: u16, actual: u16 },
    UnsupportedPayloadVersion { expected: u16, actual: u16 },
    StateVersionMismatch { expected: u16, actual: u16 },
    Truncated,
    TrailingData,
    LengthOverflow,
    CollectionTooLarge,
    PayloadTooLarge,
    AllocationFailed,
    InvalidOptionTag(u8),
    Payload(String),
}

impl fmt::Display for SchedulerStateCodecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidMagic => write!(f, "scheduler-state frame has an invalid magic value"),
            Self::UnsupportedVersion { expected, actual } => write!(
                f,
                "scheduler-state codec version {actual} is unsupported (expected {expected})"
            ),
            Self::UnsupportedPayloadVersion { expected, actual } => write!(
                f,
                "event payload codec version {actual} is unsupported (expected {expected})"
            ),
            Self::StateVersionMismatch { expected, actual } => write!(
                f,
                "scheduler-state DTO version {actual} does not match codec version {expected}"
            ),
            Self::Truncated => write!(f, "scheduler-state frame is truncated"),
            Self::TrailingData => write!(f, "scheduler-state frame contains trailing data"),
            Self::LengthOverflow => write!(f, "scheduler-state frame length overflow"),
            Self::CollectionTooLarge => {
                write!(f, "scheduler-state collection exceeds the wire count limit")
            }
            Self::PayloadTooLarge => {
                write!(f, "event payload exceeds the wire length limit")
            }
            Self::AllocationFailed => write!(f, "scheduler-state codec allocation failed"),
            Self::InvalidOptionTag(tag) => {
                write!(f, "scheduler-state frame contains invalid option tag {tag}")
            }
            Self::Payload(message) => write!(f, "event payload codec failed: {message}"),
        }
    }
}

impl std::error::Error for SchedulerStateCodecError {}

/// Encodes and decodes application-defined event payloads.
///
/// The scheduler owns event ordering and framing, not device-specific payload
/// schemas. Implementations must be deterministic and reject malformed
/// payload bytes rather than silently normalizing them.
pub trait EventPayloadCodec<T> {
    /// Stable version of this payload representation.
    const VERSION: u16;

    /// Error type produced by the payload implementation.
    type Error: fmt::Display;

    /// Encode one payload into its canonical byte representation.
    fn encode_payload(&self, payload: &T) -> Result<Vec<u8>, Self::Error>;

    /// Decode exactly one payload from its length-delimited byte slice.
    fn decode_payload(&self, bytes: &[u8]) -> Result<T, Self::Error>;
}

/// Encode a scheduler DTO using the versioned canonical wire format.
///
/// Frame layout:
/// magic[8] | state_version[u16 BE] | payload_version[u16 BE] | body_length[u64 BE] | body.
///
/// Collection counts and payload lengths are unsigned 32-bit big-endian.
/// All temporal u128 values are exactly 16-byte unsigned big-endian values.
/// The function rejects a DTO carrying a schema version other than the
/// currently supported version.
pub fn encode_scheduler_state<T, C>(
    state: &SchedulerState<T>,
    payload_codec: &C,
) -> Result<Vec<u8>, SchedulerStateCodecError>
where
    C: EventPayloadCodec<T>,
{
    if state.format_version != SCHEDULER_STATE_FORMAT_VERSION {
        return Err(SchedulerStateCodecError::StateVersionMismatch {
            expected: SCHEDULER_STATE_FORMAT_VERSION,
            actual: state.format_version,
        });
    }

    let mut body = Writer::default();
    body.write_u128(state.now.ticks())?;
    body.write_count(state.clock_domains.len())?;

    for domain in &state.clock_domains {
        body.write_u16(domain.id.0)?;
        body.write_u64(domain.ratio_numerator)?;
        body.write_u64(domain.ratio_denominator)?;
        body.write_u128(domain.current_time.ticks())?;
        body.write_u128(domain.total_cycles)?;
        body.write_u128(domain.phase)?;
        body.write_u128(domain.registration_start.ticks())?;
        body.write_u128(domain.initial_phase)?;
    }

    body.write_count(state.event_queue.pending.len())?;
    body.write_u64(state.event_queue.next_sequence)?;
    match state.event_queue.last_dispatched {
        None => body.write_u8(0)?,
        Some(watermark) => {
            body.write_u8(1)?;
            write_watermark(&mut body, watermark)?;
        }
    }

    for event in &state.event_queue.pending {
        body.write_u128(event.timestamp.ticks())?;
        body.write_u16(event.hardware_phase.value())?;
        body.write_u16(event.priority.value())?;
        body.write_u64(event.sequence)?;

        let payload = payload_codec
            .encode_payload(&event.payload)
            .map_err(|error| SchedulerStateCodecError::Payload(error.to_string()))?;
        let payload_len = u32::try_from(payload.len())
            .map_err(|_| SchedulerStateCodecError::PayloadTooLarge)?;
        body.write_u32(payload_len)?;
        body.write_bytes(&payload)?;
    }

    body.write_u64(state.delta_event_limit)?;
    match state.last_dispatch_timestamp {
        None => body.write_u8(0)?,
        Some(timestamp) => {
            body.write_u8(1)?;
            body.write_u128(timestamp.ticks())?;
        }
    }
    body.write_u64(state.dispatched_at_timestamp)?;

    let body_len = u64::try_from(body.bytes.len())
        .map_err(|_| SchedulerStateCodecError::LengthOverflow)?;
    let mut frame = Writer::default();
    frame.write_bytes(&SCHEDULER_STATE_MAGIC)?;
    frame.write_u16(SCHEDULER_STATE_FORMAT_VERSION)?;
    frame.write_u16(C::VERSION)?;
    frame.write_u64(body_len)?;
    frame.write_bytes(&body.bytes)?;
    Ok(frame.bytes)
}

/// Decode one complete scheduler-state frame into its persistence DTO.
///
/// This function validates framing, version, option tags, field widths,
/// collection boundaries, payload lengths and complete input consumption.
/// Cross-component scheduler invariants remain the responsibility of
/// MachineScheduler::restore_state.
pub fn decode_scheduler_state<T, C>(
    bytes: &[u8],
    payload_codec: &C,
) -> Result<SchedulerState<T>, SchedulerStateCodecError>
where
    C: EventPayloadCodec<T>,
{
    let mut frame = Reader::new(bytes);
    if frame.read_array::<8>()? != SCHEDULER_STATE_MAGIC {
        return Err(SchedulerStateCodecError::InvalidMagic);
    }
    let version = frame.read_u16()?;
    if version != SCHEDULER_STATE_FORMAT_VERSION {
        return Err(SchedulerStateCodecError::UnsupportedVersion {
            expected: SCHEDULER_STATE_FORMAT_VERSION,
            actual: version,
        });
    }
    let payload_version = frame.read_u16()?;
    if payload_version != C::VERSION {
        return Err(SchedulerStateCodecError::UnsupportedPayloadVersion {
            expected: C::VERSION,
            actual: payload_version,
        });
    }
    let body_len = usize::try_from(frame.read_u64()?)
        .map_err(|_| SchedulerStateCodecError::LengthOverflow)?;
    let body_bytes = frame.read_bytes(body_len)?;
    if !frame.is_finished() {
        return Err(SchedulerStateCodecError::TrailingData);
    }

    let mut reader = Reader::new(body_bytes);
    let now = MachineTime::from_ticks(reader.read_u128()?);

    let domain_count = reader.read_u32()?;
    let domain_count_usize = usize::try_from(domain_count)
        .map_err(|_| SchedulerStateCodecError::LengthOverflow)?;
    if domain_count_usize > reader.remaining() / CLOCK_DOMAIN_WIRE_SIZE {
        return Err(SchedulerStateCodecError::Truncated);
    }
    let mut clock_domains = Vec::new();
    clock_domains
        .try_reserve_exact(domain_count_usize)
        .map_err(|_| SchedulerStateCodecError::AllocationFailed)?;
    for _ in 0..domain_count {
        clock_domains.push(ClockDomainState {
            id: ClockDomainId(reader.read_u16()?),
            ratio_numerator: reader.read_u64()?,
            ratio_denominator: reader.read_u64()?,
            current_time: MachineTime::from_ticks(reader.read_u128()?),
            total_cycles: reader.read_u128()?,
            phase: reader.read_u128()?,
            registration_start: MachineTime::from_ticks(reader.read_u128()?),
            initial_phase: reader.read_u128()?,
        });
    }

    let pending_count = reader.read_u32()?;
    let pending_count_usize = usize::try_from(pending_count)
        .map_err(|_| SchedulerStateCodecError::LengthOverflow)?;
    if pending_count_usize > reader.remaining() / EVENT_FIXED_WIRE_SIZE {
        return Err(SchedulerStateCodecError::Truncated);
    }
    let next_sequence = reader.read_u64()?;
    let last_dispatched = match reader.read_u8()? {
        0 => None,
        1 => Some(read_watermark(&mut reader)?),
        tag => return Err(SchedulerStateCodecError::InvalidOptionTag(tag)),
    };

    let mut pending = Vec::new();
    pending
        .try_reserve_exact(pending_count_usize)
        .map_err(|_| SchedulerStateCodecError::AllocationFailed)?;
    for _ in 0..pending_count {
        let timestamp = MachineTime::from_ticks(reader.read_u128()?);
        let hardware_phase = HardwarePhase::new(reader.read_u16()?);
        let priority = PlatformPriority::new(reader.read_u16()?);
        let sequence = reader.read_u64()?;
        let payload_len = usize::try_from(reader.read_u32()?)
            .map_err(|_| SchedulerStateCodecError::LengthOverflow)?;
        let payload_bytes = reader.read_bytes(payload_len)?;
        let payload = payload_codec
            .decode_payload(payload_bytes)
            .map_err(|error| SchedulerStateCodecError::Payload(error.to_string()))?;
        pending.push(ScheduledEvent {
            timestamp,
            hardware_phase,
            priority,
            sequence,
            payload,
        });
    }

    let delta_event_limit = reader.read_u64()?;
    let last_dispatch_timestamp = match reader.read_u8()? {
        0 => None,
        1 => Some(MachineTime::from_ticks(reader.read_u128()?)),
        tag => return Err(SchedulerStateCodecError::InvalidOptionTag(tag)),
    };
    let dispatched_at_timestamp = reader.read_u64()?;

    if !reader.is_finished() {
        return Err(SchedulerStateCodecError::TrailingData);
    }

    Ok(SchedulerState {
        format_version: version,
        now,
        clock_domains,
        event_queue: EventQueueState {
            pending,
            next_sequence,
            last_dispatched,
        },
        delta_event_limit,
        last_dispatch_timestamp,
        dispatched_at_timestamp,
    })
}

fn write_watermark(
    writer: &mut Writer,
    watermark: EventQueueWatermark,
) -> Result<(), SchedulerStateCodecError> {
    writer.write_u128(watermark.timestamp.ticks())?;
    writer.write_u16(watermark.hardware_phase.value())?;
    writer.write_u16(watermark.priority.value())?;
    writer.write_u64(watermark.sequence)
}

fn read_watermark(
    reader: &mut Reader<'_>,
) -> Result<EventQueueWatermark, SchedulerStateCodecError> {
    Ok(EventQueueWatermark {
        timestamp: MachineTime::from_ticks(reader.read_u128()?),
        hardware_phase: HardwarePhase::new(reader.read_u16()?),
        priority: PlatformPriority::new(reader.read_u16()?),
        sequence: reader.read_u64()?,
    })
}

#[derive(Default)]
struct Writer {
    bytes: Vec<u8>,
}

impl Writer {
    fn write_bytes(&mut self, bytes: &[u8]) -> Result<(), SchedulerStateCodecError> {
        self.bytes
            .try_reserve(bytes.len())
            .map_err(|_| SchedulerStateCodecError::AllocationFailed)?;
        self.bytes.extend_from_slice(bytes);
        Ok(())
    }

    fn write_u8(&mut self, value: u8) -> Result<(), SchedulerStateCodecError> {
        self.write_bytes(&[value])
    }

    fn write_u16(&mut self, value: u16) -> Result<(), SchedulerStateCodecError> {
        self.write_bytes(&value.to_be_bytes())
    }

    fn write_u32(&mut self, value: u32) -> Result<(), SchedulerStateCodecError> {
        self.write_bytes(&value.to_be_bytes())
    }

    fn write_u64(&mut self, value: u64) -> Result<(), SchedulerStateCodecError> {
        self.write_bytes(&value.to_be_bytes())
    }

    fn write_u128(&mut self, value: u128) -> Result<(), SchedulerStateCodecError> {
        self.write_bytes(&value.to_be_bytes())
    }

    fn write_count(&mut self, count: usize) -> Result<(), SchedulerStateCodecError> {
        let count =
            u32::try_from(count).map_err(|_| SchedulerStateCodecError::CollectionTooLarge)?;
        self.write_u32(count)
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Reader<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn read_bytes(&mut self, length: usize) -> Result<&'a [u8], SchedulerStateCodecError> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or(SchedulerStateCodecError::LengthOverflow)?;
        let bytes = self
            .bytes
            .get(self.offset..end)
            .ok_or(SchedulerStateCodecError::Truncated)?;
        self.offset = end;
        Ok(bytes)
    }

    fn read_array<const N: usize>(&mut self) -> Result<[u8; N], SchedulerStateCodecError> {
        self.read_bytes(N)?
            .try_into()
            .map_err(|_| SchedulerStateCodecError::Truncated)
    }

    fn read_u8(&mut self) -> Result<u8, SchedulerStateCodecError> {
        Ok(self.read_array::<1>()?[0])
    }

    fn read_u16(&mut self) -> Result<u16, SchedulerStateCodecError> {
        Ok(u16::from_be_bytes(self.read_array()?))
    }

    fn read_u32(&mut self) -> Result<u32, SchedulerStateCodecError> {
        Ok(u32::from_be_bytes(self.read_array()?))
    }

    fn read_u64(&mut self) -> Result<u64, SchedulerStateCodecError> {
        Ok(u64::from_be_bytes(self.read_array()?))
    }

    fn read_u128(&mut self) -> Result<u128, SchedulerStateCodecError> {
        Ok(u128::from_be_bytes(self.read_array()?))
    }

    fn remaining(&self) -> usize {
        self.bytes.len() - self.offset
    }

    fn is_finished(&self) -> bool {
        self.offset == self.bytes.len()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        decode_scheduler_state, encode_scheduler_state, EventPayloadCodec,
        SchedulerStateCodecError, SCHEDULER_STATE_MAGIC,
    };
    use sega_temporal_core::{
        EventQueueState, EventQueueWatermark, HardwarePhase, MachineTime, PlatformPriority,
        ScheduledEvent,
    };

    use crate::{
        ClockDomainId, ClockDomainState, MachineScheduler, SCHEDULER_STATE_FORMAT_VERSION,
        SchedulerError, SchedulerState,
    };

    struct U32PayloadCodec;

    impl EventPayloadCodec<u32> for U32PayloadCodec {
        const VERSION: u16 = 1;

        type Error = &'static str;

        fn encode_payload(&self, payload: &u32) -> Result<Vec<u8>, Self::Error> {
            Ok(payload.to_be_bytes().to_vec())
        }

        fn decode_payload(&self, bytes: &[u8]) -> Result<u32, Self::Error> {
            let array: [u8; 4] = bytes.try_into().map_err(|_| "expected four payload bytes")?;
            Ok(u32::from_be_bytes(array))
        }
    }

    fn state_fixture() -> SchedulerState<u32> {
        SchedulerState {
            format_version: SCHEDULER_STATE_FORMAT_VERSION,
            now: MachineTime::from_ticks(5),
            clock_domains: vec![ClockDomainState {
                id: ClockDomainId(2),
                ratio_numerator: 1,
                ratio_denominator: 1,
                current_time: MachineTime::from_ticks(5),
                total_cycles: 5,
                phase: 0,
                registration_start: MachineTime::ZERO,
                initial_phase: 0,
            }],
            event_queue: EventQueueState {
                pending: vec![ScheduledEvent {
                    timestamp: MachineTime::from_ticks(5),
                    hardware_phase: HardwarePhase::new(2),
                    priority: PlatformPriority::new(3),
                    sequence: 0,
                    payload: 0x1234_5678,
                }],
                next_sequence: 1,
                last_dispatched: None,
            },
            delta_event_limit: 8,
            last_dispatch_timestamp: None,
            dispatched_at_timestamp: 0,
        }
    }

    #[test]
    fn scheduler_state_codec_round_trips_all_fields() {
        let state = state_fixture();
        let encoded = encode_scheduler_state(&state, &U32PayloadCodec).unwrap();
        let decoded = decode_scheduler_state(&encoded, &U32PayloadCodec).unwrap();
        assert_eq!(decoded, state);
        let restored = MachineScheduler::restore_state(decoded).unwrap();
        assert_eq!(restored.export_state(), state);
    }

    #[test]
    fn codec_decode_leaves_semantic_validation_to_atomic_scheduler_restore() {
        let mut invalid = state_fixture();
        invalid.delta_event_limit = 0;
        let encoded = encode_scheduler_state(&invalid, &U32PayloadCodec).unwrap();
        let decoded = decode_scheduler_state(&encoded, &U32PayloadCodec).unwrap();
        assert!(matches!(
            MachineScheduler::restore_state(decoded),
            Err(SchedulerError::ZeroDeltaEventLimit)
        ));
    }

    #[test]
    fn codec_round_trips_present_watermarks_and_dispatch_history() {
        let mut state = state_fixture();
        state.event_queue.pending[0].sequence = 1;
        state.event_queue.next_sequence = 2;
        state.event_queue.last_dispatched = Some(EventQueueWatermark {
            timestamp: MachineTime::from_ticks(4),
            hardware_phase: HardwarePhase::new(1),
            priority: PlatformPriority::new(1),
            sequence: 0,
        });
        state.last_dispatch_timestamp = Some(MachineTime::from_ticks(4));
        state.dispatched_at_timestamp = 1;

        let encoded = encode_scheduler_state(&state, &U32PayloadCodec).unwrap();
        let decoded = decode_scheduler_state(&encoded, &U32PayloadCodec).unwrap();
        assert_eq!(decoded, state);
        let restored = MachineScheduler::restore_state(decoded).unwrap();
        assert_eq!(restored.export_state(), state);
    }

    #[test]
    fn u128_temporal_values_use_exact_unsigned_big_endian_bytes() {
        let mut state = state_fixture();
        state.now = MachineTime::from_ticks(u128::MAX);
        state.clock_domains[0].current_time = MachineTime::from_ticks(u128::MAX);
        state.clock_domains[0].total_cycles = u128::MAX;
        state.event_queue.pending[0].timestamp = MachineTime::from_ticks(u128::MAX);

        let encoded = encode_scheduler_state(&state, &U32PayloadCodec).unwrap();
        assert_eq!(&encoded[..8], &SCHEDULER_STATE_MAGIC);
        assert_eq!(&encoded[20..36], &[0xff; 16]);
        let decoded = decode_scheduler_state(&encoded, &U32PayloadCodec).unwrap();
        assert_eq!(decoded.now.ticks(), u128::MAX);
        assert_eq!(decoded.clock_domains[0].total_cycles, u128::MAX);
        assert_eq!(decoded.clock_domains[0].current_time.ticks(), u128::MAX);
        assert_eq!(decoded.event_queue.pending[0].timestamp.ticks(), u128::MAX);
    }

    #[test]
    fn codec_rejects_unknown_version_invalid_magic_and_state_version() {
        let state = state_fixture();
        let mut encoded = encode_scheduler_state(&state, &U32PayloadCodec).unwrap();
        encoded[0] ^= 0xff;
        assert_eq!(
            decode_scheduler_state(&encoded, &U32PayloadCodec),
            Err(SchedulerStateCodecError::InvalidMagic)
        );

        let mut encoded = encode_scheduler_state(&state, &U32PayloadCodec).unwrap();
        encoded[8..10].copy_from_slice(&u16::MAX.to_be_bytes());
        assert_eq!(
            decode_scheduler_state(&encoded, &U32PayloadCodec),
            Err(SchedulerStateCodecError::UnsupportedVersion {
                expected: SCHEDULER_STATE_FORMAT_VERSION,
                actual: u16::MAX,
            })
        );

        let mut encoded = encode_scheduler_state(&state, &U32PayloadCodec).unwrap();
        encoded[10..12].copy_from_slice(&u16::MAX.to_be_bytes());
        assert_eq!(
            decode_scheduler_state(&encoded, &U32PayloadCodec),
            Err(SchedulerStateCodecError::UnsupportedPayloadVersion {
                expected: U32PayloadCodec::VERSION,
                actual: u16::MAX,
            })
        );

        let mut invalid_state = state;
        invalid_state.format_version += 1;
        assert_eq!(
            encode_scheduler_state(&invalid_state, &U32PayloadCodec),
            Err(SchedulerStateCodecError::StateVersionMismatch {
                expected: SCHEDULER_STATE_FORMAT_VERSION,
                actual: SCHEDULER_STATE_FORMAT_VERSION + 1,
            })
        );
    }

    #[test]
    fn codec_rejects_truncated_and_trailing_frames() {
        let encoded = encode_scheduler_state(&state_fixture(), &U32PayloadCodec).unwrap();
        assert_eq!(
            decode_scheduler_state(&encoded[..encoded.len() - 1], &U32PayloadCodec),
            Err(SchedulerStateCodecError::Truncated)
        );

        let mut trailing = encoded;
        trailing.push(0);
        assert_eq!(
            decode_scheduler_state(&trailing, &U32PayloadCodec),
            Err(SchedulerStateCodecError::TrailingData)
        );
    }

    #[test]
    fn codec_rejects_invalid_option_tags_and_malformed_payloads() {
        let encoded = encode_scheduler_state(&state_fixture(), &U32PayloadCodec).unwrap();
        let watermark_tag_offset = 20 + 16 + 4 + CLOCK_DOMAIN_SIZE_FOR_TEST + 4 + 8;
        let mut invalid_tag = encoded.clone();
        invalid_tag[watermark_tag_offset] = 2;
        assert_eq!(
            decode_scheduler_state(&invalid_tag, &U32PayloadCodec),
            Err(SchedulerStateCodecError::InvalidOptionTag(2))
        );

        let mut malformed_payload = encoded;
        let payload_length_offset = 20 + 16 + 4 + CLOCK_DOMAIN_SIZE_FOR_TEST + 4 + 8 + 1 + 16 + 2 + 2 + 8;
        malformed_payload[payload_length_offset..payload_length_offset + 4]
            .copy_from_slice(&3_u32.to_be_bytes());
        assert!(matches!(
            decode_scheduler_state(&malformed_payload, &U32PayloadCodec),
            Err(SchedulerStateCodecError::Payload(_))
        ));
    }

    #[test]
    fn codec_rejects_payload_lengths_beyond_frame_boundary() {
        let mut encoded = encode_scheduler_state(&state_fixture(), &U32PayloadCodec).unwrap();
        let payload_length_offset = 20 + 16 + 4 + CLOCK_DOMAIN_SIZE_FOR_TEST + 4 + 8 + 1 + 16 + 2 + 2 + 8;
        encoded[payload_length_offset..payload_length_offset + 4]
            .copy_from_slice(&u32::MAX.to_be_bytes());
        assert_eq!(
            decode_scheduler_state(&encoded, &U32PayloadCodec),
            Err(SchedulerStateCodecError::Truncated)
        );
    }

    const CLOCK_DOMAIN_SIZE_FOR_TEST: usize = 98;
}
