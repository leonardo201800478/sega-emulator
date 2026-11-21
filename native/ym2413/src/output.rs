//! YM2413 digital output-pin routing and per-slot pulse-width contract.
//!
//! The Yamaha manual identifies MO as the melody output and RO as the rhythm
//! output. Hardware measurements report that each selected channel remains
//! active for three input clocks within its four-clock slot, then returns to
//! the neutral output level for the remaining clock.
//!
//! This module deliberately models only the slot-level routing and measured
//! pulse width. It does not choose the pulse's exact sub-clock phase, define
//! the neutral electrical voltage, convert the operator's one's-complement
//! i16 representation into a raw nine-bit DAC word, or model the analog
//! source-follower/load circuitry. Those details remain separate evidence
//! questions. The supplied Ym2413DacCode is the project's reconstructed
//! signed measurement-domain level, not a raw DAC input word.

use crate::rhythm_output::{
    Ym2413DacCode, Ym2413RhythmOutputDelay, Ym2413RhythmOutputDelayState, Ym2413RhythmOutputError,
    Ym2413RhythmOutputEvent, Ym2413RhythmOutputSlotResult,
};
use crate::slot::{
    YM2413_RHYTHM_OUTPUT_MAP, YM2413_SLOT_COUNT, Ym2413Operator, Ym2413RhythmVoice, slot_descriptor,
};

/// Number of input clocks in one serial operator slot.
pub const YM2413_OUTPUT_SLOT_CLOCKS: u8 = 4;
/// Measured number of clocks for which a selected output level remains active.
pub const YM2413_OUTPUT_ACTIVE_CLOCKS: u8 = 3;
/// Measured neutral interval remaining in each four-clock slot.
pub const YM2413_OUTPUT_NEUTRAL_CLOCKS: u8 =
    YM2413_OUTPUT_SLOT_CLOCKS - YM2413_OUTPUT_ACTIVE_CLOCKS;

/// Convert a Nuked-OPLL cycle to the measured MO carrier-slot coordinate.
///
/// The base `OPLL_Channel` melody window is active on cycles 0..2, 6..8
/// and 12..14. These correspond to measured carrier slots 3..5, 9..11
/// and 15..17. Rhythm Mode's six-channel restriction is a separate persistent
/// mode gate; Nuked's transient `rm_enable` write pipeline is not that gate.
pub const fn mo_output_slot_for_nuked_cycle(cycle: u8) -> Option<u8> {
    if cycle < YM2413_SLOT_COUNT as u8 {
        Some((cycle + 3) % YM2413_SLOT_COUNT as u8)
    } else {
        None
    }
}

/// Convert a Nuked-OPLL cycle to the measured RO occurrence-slot coordinate.
///
/// The conditional RO schedule in the pinned Nuked model maps cycles
/// 16, 17, 0..5, 9 and 10 to the measured original/repeat RO positions using
/// an offset of 15. Nuked marks this RO gate as possibly incorrect, so this
/// is a software schedule correspondence, not physical timing certification.
pub const fn ro_output_slot_for_nuked_cycle(cycle: u8) -> Option<u8> {
    if cycle < YM2413_SLOT_COUNT as u8 {
        Some((cycle + 15) % YM2413_SLOT_COUNT as u8)
    } else {
        None
    }
}

/// Physical analog output pin selected by the YM2413 output routing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ym2413OutputPin {
    /// Melody output pin.
    Mo,
    /// Rhythm output pin.
    Ro,
}

/// Logical source associated with one output pulse.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ym2413OutputSource {
    /// Zero-based melodic channel index (0..=8).
    MelodyChannel(u8),
    /// Rhythm voice routed to RO.
    RhythmVoice(Ym2413RhythmVoice),
}

/// Result of adapting a rhythm-retention slot result to the output-pin layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ym2413OutputSlotResult {
    /// The slot does not emit a rhythm value.
    NoOutput,
    /// A retained or original rhythm value is routed to RO.
    Pulse(Ym2413OutputPulse),
    /// A delayed-repeat slot was reached before its source value was observed.
    RepeatNotInitialized {
        slot_index: u8,
        voice: Ym2413RhythmVoice,
    },
}

/// One slot's measured-domain output pulse.
///
/// The pulse width is fixed by the documented/observed three-active-clock
/// interval. Its exact position among the four clocks is intentionally not
/// represented because the available evidence does not establish that
/// sub-clock alignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ym2413OutputPulse {
    pub pin: Ym2413OutputPin,
    pub slot_index: u8,
    pub source: Ym2413OutputSource,
    pub level: Ym2413DacCode,
}

impl Ym2413OutputPulse {
    /// Return the measured active duration in input clocks.
    pub const fn active_input_clocks(self) -> u8 {
        YM2413_OUTPUT_ACTIVE_CLOCKS
    }

    /// Return the complete slot duration in input clocks.
    pub const fn slot_input_clocks(self) -> u8 {
        YM2413_OUTPUT_SLOT_CLOCKS
    }

    /// Return the neutral interval in input clocks after the active window.
    pub const fn neutral_input_clocks(self) -> u8 {
        YM2413_OUTPUT_NEUTRAL_CLOCKS
    }
}

/// Errors from the slot-level MO/RO routing contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ym2413OutputRoutingError {
    /// The slot index is outside the 18-slot rotation.
    InvalidSlotIndex(u8),
    /// A melodic carrier slot was serviced without a supplied output level.
    MissingMelodyLevel { slot_index: u8, channel: u8 },
    /// A level was supplied at a slot that does not emit melodic output.
    UnexpectedMelodyLevel { slot_index: u8 },
    /// A rhythm event does not match the published slot/voice/repeat map.
    InvalidRhythmEvent(Ym2413RhythmOutputEvent),
}

impl core::fmt::Display for Ym2413OutputRoutingError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidSlotIndex(slot) => {
                write!(f, "invalid YM2413 output slot index: {slot}")
            }
            Self::MissingMelodyLevel {
                slot_index,
                channel,
            } => write!(
                f,
                "missing YM2413 MO level at slot {slot_index} for channel {channel}"
            ),
            Self::UnexpectedMelodyLevel { slot_index } => write!(
                f,
                "unexpected YM2413 MO level at non-melodic-output slot {slot_index}"
            ),
            Self::InvalidRhythmEvent(event) => write!(
                f,
                "invalid YM2413 RO event at slot {} for {:?}",
                event.slot_index, event.voice
            ),
        }
    }
}

impl std::error::Error for Ym2413OutputRoutingError {}

/// Resolve the MO output event for one slot.
///
/// In melodic mode, all nine carrier slots emit to MO. In Rhythm Mode, only
/// channels 1..=6 remain melodic, so carrier slots for channels 7..=9 do not
/// emit a melody event. A zero-valued level must be supplied explicitly for a
/// silent carrier slot; None means the caller did not provide the slot result.
///
/// This function does not evaluate operators or advance clocks.
pub fn melody_output_for_slot(
    slot_index: u8,
    rhythm_mode_enabled: bool,
    level: Option<Ym2413DacCode>,
) -> Result<Option<Ym2413OutputPulse>, Ym2413OutputRoutingError> {
    let descriptor = slot_descriptor(slot_index)
        .ok_or(Ym2413OutputRoutingError::InvalidSlotIndex(slot_index))?;
    let is_melody_output = descriptor.operator == Ym2413Operator::Carrier
        && (!rhythm_mode_enabled || descriptor.channel < 6);

    if !is_melody_output {
        return if level.is_some() {
            Err(Ym2413OutputRoutingError::UnexpectedMelodyLevel { slot_index })
        } else {
            Ok(None)
        };
    }

    let level = level.ok_or(Ym2413OutputRoutingError::MissingMelodyLevel {
        slot_index,
        channel: descriptor.channel,
    })?;

    Ok(Some(Ym2413OutputPulse {
        pin: Ym2413OutputPin::Mo,
        slot_index,
        source: Ym2413OutputSource::MelodyChannel(descriptor.channel),
        level,
    }))
}

/// Route one already-retained rhythm output event to RO.
///
/// The event is checked against the measured original/repeat slot map. This
/// does not synthesize the voice, perform the operator-to-DAC conversion, or
/// advance the rhythm retention state.
pub fn rhythm_output_event(
    event: Ym2413RhythmOutputEvent,
) -> Result<Ym2413OutputPulse, Ym2413OutputRoutingError> {
    let valid = YM2413_RHYTHM_OUTPUT_MAP.iter().any(|entry| {
        entry.slot_index == event.slot_index
            && entry.voice == event.voice
            && entry.is_delayed_repeat == event.is_delayed_repeat
    });
    if !valid || usize::from(event.slot_index) >= YM2413_SLOT_COUNT {
        return Err(Ym2413OutputRoutingError::InvalidRhythmEvent(event));
    }

    Ok(Ym2413OutputPulse {
        pin: Ym2413OutputPin::Ro,
        slot_index: event.slot_index,
        source: Ym2413OutputSource::RhythmVoice(event.voice),
        level: event.code,
    })
}

/// Convert a rhythm retention result to an RO pulse when a value was emitted.
///
/// Uninitialized repeats remain a distinct result for diagnostics and reset-state
/// investigations; neither case is silently converted to digital zero.
pub fn rhythm_slot_result(
    result: Ym2413RhythmOutputSlotResult,
) -> Result<Ym2413OutputSlotResult, Ym2413OutputRoutingError> {
    match result {
        Ym2413RhythmOutputSlotResult::Emitted(event) => {
            rhythm_output_event(event).map(Ym2413OutputSlotResult::Pulse)
        }
        Ym2413RhythmOutputSlotResult::NoRhythmOutput => Ok(Ym2413OutputSlotResult::NoOutput),
        Ym2413RhythmOutputSlotResult::RepeatNotInitialized { slot_index, voice } => {
            Ok(Ym2413OutputSlotResult::RepeatNotInitialized { slot_index, voice })
        }
    }
}

/// Snapshot format for the combined Rhythm Mode MO/RO output service.
pub const YM2413_RHYTHM_OUTPUT_SERVICE_STATE_VERSION: u16 = 2;

/// Complete save-state data for the Rhythm Mode output service.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ym2413RhythmOutputServiceState {
    pub format_version: u16,
    pub rhythm_delay: Ym2413RhythmOutputDelayState,
}

/// Independent MO and RO results for one completed Rhythm Mode slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ym2413RhythmOutputFrame {
    pub slot_index: u8,
    /// Melody output for channels 0..5. None means MO has no pulse in this slot.
    pub mo: Option<Ym2413OutputPulse>,
    /// Rhythm output or explicit absence/uninitialized-repeat result.
    pub ro: Ym2413OutputSlotResult,
}

/// Output-pin results for one Nuked-compatible serial cycle.
///
/// MO and RO retain their own measured slot coordinates because their
/// cycle-to-slot transforms differ. The values are already reconstructed
/// measured-domain levels; this frame does not model the previous-`ch_out`
/// latch, pin-neutral code or output-buffer delay.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ym2413OutputCycleFrame {
    pub cycle: u8,
    pub mo_slot_index: u8,
    pub ro_slot_index: u8,
    pub mo: Option<Ym2413OutputPulse>,
    pub ro: Ym2413OutputSlotResult,
}

/// Errors from the serial Rhythm Mode MO/RO output service.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ym2413RhythmOutputServiceError {
    InvalidState,
    InvalidNukedCycle(u8),
    Retention(Ym2413RhythmOutputError),
    Routing(Ym2413OutputRoutingError),
}

impl core::fmt::Display for Ym2413RhythmOutputServiceError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidState => f.write_str("invalid YM2413 Rhythm Mode output-service state"),
            Self::InvalidNukedCycle(cycle) => {
                write!(f, "invalid Nuked-OPLL output cycle: {cycle}")
            }
            Self::Retention(error) => write!(f, "YM2413 rhythm retention error: {error:?}"),
            Self::Routing(error) => write!(f, "YM2413 output routing error: {error}"),
        }
    }
}

impl std::error::Error for Ym2413RhythmOutputServiceError {}

impl From<Ym2413RhythmOutputError> for Ym2413RhythmOutputServiceError {
    fn from(value: Ym2413RhythmOutputError) -> Self {
        Self::Retention(value)
    }
}

impl From<Ym2413OutputRoutingError> for Ym2413RhythmOutputServiceError {
    fn from(value: Ym2413OutputRoutingError) -> Self {
        Self::Routing(value)
    }
}

/// Serial output stage for the six melodic and five repeated Rhythm Mode outputs.
///
/// The caller invokes process_slot exactly once per completed slot in the 18-slot rotation.
/// The two input levels are already reconstructed measurement-domain values:
/// this service never converts the signed operator intermediate to a DAC word.
/// It also does not advance the chip clock, choose the three-clock pulse's
/// sub-clock phase, or combine MO and RO into PCM.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ym2413RhythmOutputService {
    rhythm_delay: Ym2413RhythmOutputDelay,
}

impl Ym2413RhythmOutputService {
    /// Restore caller-supplied retention state without guessing physical reset contents.
    pub const fn with_initial_state(
        state: Ym2413RhythmOutputServiceState,
    ) -> Result<Self, Ym2413RhythmOutputServiceError> {
        if state.format_version != YM2413_RHYTHM_OUTPUT_SERVICE_STATE_VERSION {
            return Err(Ym2413RhythmOutputServiceError::InvalidState);
        }
        let rhythm_delay = match Ym2413RhythmOutputDelay::with_initial_state(state.rhythm_delay) {
            Ok(delay) => delay,
            Err(error) => return Err(Ym2413RhythmOutputServiceError::Retention(error)),
        };
        Ok(Self { rhythm_delay })
    }

    /// Export the complete MO/RO service state for deterministic save states.
    pub const fn export_state(&self) -> Ym2413RhythmOutputServiceState {
        Ym2413RhythmOutputServiceState {
            format_version: YM2413_RHYTHM_OUTPUT_SERVICE_STATE_VERSION,
            rhythm_delay: self.rhythm_delay.export_state(),
        }
    }

    /// Route one Nuked-compatible cycle through the independently mapped MO and RO slots.
    ///
    /// MO uses `(cycle + 3) % 18`; RO uses `(cycle + 15) % 18`.
    /// The rhythm retention cursor therefore follows RO's slot order, and
    /// its initial `next_slot_index` must match the first RO slot to process.
    ///
    /// Supply melody_level only when the mapped MO slot is an active carrier
    /// in the selected persistent mode. Supply original_rhythm_level only
    /// when the mapped RO slot is 13..17. Both inputs must already be in the
    /// signed measured-output domain. The caller owns the prior-`ch_out`
    /// latch, delayed mode-write state, pin-neutral value and output-buffer
    /// timing; this method only maps slots and applies measured retention.
    pub fn process_nuked_cycle(
        &mut self,
        cycle: u8,
        rhythm_mode_enabled: bool,
        melody_level: Option<Ym2413DacCode>,
        original_rhythm_level: Option<Ym2413DacCode>,
    ) -> Result<Ym2413OutputCycleFrame, Ym2413RhythmOutputServiceError> {
        let mo_slot_index = mo_output_slot_for_nuked_cycle(cycle)
            .ok_or(Ym2413RhythmOutputServiceError::InvalidNukedCycle(cycle))?;
        let ro_slot_index = ro_output_slot_for_nuked_cycle(cycle)
            .ok_or(Ym2413RhythmOutputServiceError::InvalidNukedCycle(cycle))?;

        // Validate MO placement before mutating the RO retention cursor.
        let mo = melody_output_for_slot(mo_slot_index, rhythm_mode_enabled, melody_level)?;
        let rhythm_result = self
            .rhythm_delay
            .process_slot(ro_slot_index, original_rhythm_level)?;
        let ro = rhythm_slot_result(rhythm_result)?;

        Ok(Ym2413OutputCycleFrame {
            cycle,
            mo_slot_index,
            ro_slot_index,
            mo,
            ro,
        })
    }

    /// Route one completed measured slot to MO and RO using a shared slot index.
    ///
    /// This lower-level API is retained for callers that already operate in
    /// measured-slot coordinates. Nuked-cycle callers must use
    /// `process_nuked_cycle` because the two pin offsets differ.
    ///
    /// Supply melody_level only at MO carrier slots 3, 4, 5, 9, 10 and 11.
    /// Supply original_rhythm_level only at source slots 13..17. The five
    /// repeated RO occurrences are read from retained state at slots 0, 1, 2,
    /// 6 and 7. Both values must already be in Ym2413DacCode's measured domain.
    pub fn process_slot(
        &mut self,
        slot_index: u8,
        melody_level: Option<Ym2413DacCode>,
        original_rhythm_level: Option<Ym2413DacCode>,
    ) -> Result<Ym2413RhythmOutputFrame, Ym2413RhythmOutputServiceError> {
        // Validate MO placement before mutating the rhythm retention cursor.
        let mo = melody_output_for_slot(slot_index, true, melody_level)?;
        let rhythm_result = self
            .rhythm_delay
            .process_slot(slot_index, original_rhythm_level)?;
        let ro = rhythm_slot_result(rhythm_result)?;
        Ok(Ym2413RhythmOutputFrame { slot_index, mo, ro })
    }
}

#[cfg(test)]
mod tests {
    use super::{
        YM2413_OUTPUT_ACTIVE_CLOCKS, YM2413_OUTPUT_NEUTRAL_CLOCKS, YM2413_OUTPUT_SLOT_CLOCKS,
        Ym2413OutputPin, Ym2413OutputRoutingError, Ym2413OutputSlotResult, Ym2413OutputSource,
        melody_output_for_slot, mo_output_slot_for_nuked_cycle, rhythm_output_event,
        rhythm_slot_result, ro_output_slot_for_nuked_cycle,
    };
    use crate::rhythm_output::{
        Ym2413DacCode, Ym2413RhythmOutputError, Ym2413RhythmOutputEvent,
        Ym2413RhythmOutputSlotResult,
    };
    use crate::slot::Ym2413RhythmVoice;

    fn level(value: i16) -> Ym2413DacCode {
        Ym2413DacCode::new(value).unwrap()
    }

    fn service_state(next_slot_index: u8) -> super::Ym2413RhythmOutputServiceState {
        super::Ym2413RhythmOutputServiceState {
            format_version: super::YM2413_RHYTHM_OUTPUT_SERVICE_STATE_VERSION,
            rhythm_delay: crate::rhythm_output::Ym2413RhythmOutputDelayState {
                format_version: crate::rhythm_output::YM2413_RHYTHM_OUTPUT_DELAY_STATE_VERSION,
                next_slot_index,
                retained_values: [None; 5],
            },
        }
    }

    fn service_level(slot: u8) -> Option<Ym2413DacCode> {
        match slot {
            3..=5 | 9..=11 => Some(level(i16::from(slot) - 20)),
            _ => None,
        }
    }

    fn original_rhythm_level(slot: u8) -> Option<Ym2413DacCode> {
        match slot {
            13 => Some(level(21)),
            14 => Some(level(-34)),
            15 => Some(level(55)),
            16 => Some(level(-89)),
            17 => Some(level(144)),
            _ => None,
        }
    }

    fn process_service_slot(
        service: &mut super::Ym2413RhythmOutputService,
        slot: u8,
    ) -> super::Ym2413RhythmOutputFrame {
        service
            .process_slot(slot, service_level(slot), original_rhythm_level(slot))
            .unwrap()
    }

    #[test]
    fn rhythm_output_service_routes_mo_and_ro_independently_and_repeats_retained_codes() {
        let mut service =
            super::Ym2413RhythmOutputService::with_initial_state(service_state(13)).unwrap();
        let mut frames = Vec::new();
        for slot in (13..18).chain(0..13) {
            frames.push(process_service_slot(&mut service, slot));
        }

        let melody_slots = [3, 4, 5, 9, 10, 11];
        let rhythm_slots = [13, 14, 15, 16, 17, 0, 1, 2, 6, 7];
        for frame in &frames {
            assert_eq!(
                frame.mo.is_some(),
                melody_slots.contains(&frame.slot_index),
                "MO slot {}",
                frame.slot_index
            );
            assert_eq!(
                matches!(frame.ro, Ym2413OutputSlotResult::Pulse(_)),
                rhythm_slots.contains(&frame.slot_index),
                "RO slot {}",
                frame.slot_index
            );
            assert!(
                !(frame.mo.is_some() && matches!(frame.ro, Ym2413OutputSlotResult::Pulse(_))),
                "MO and RO must remain independent at slot {}",
                frame.slot_index
            );
        }

        let repeats: Vec<_> = frames
            .iter()
            .filter_map(|frame| match frame.ro {
                Ym2413OutputSlotResult::Pulse(pulse) if pulse.slot_index < 13 => Some(pulse),
                _ => None,
            })
            .collect();
        let expected = [
            (0, Ym2413RhythmVoice::HiHat, 21),
            (1, Ym2413RhythmVoice::TomTom, -34),
            (2, Ym2413RhythmVoice::BassDrum, 55),
            (6, Ym2413RhythmVoice::SnareDrum, -89),
            (7, Ym2413RhythmVoice::TopCymbal, 144),
        ];
        assert_eq!(repeats.len(), expected.len());
        for (pulse, (slot, voice, value)) in repeats.into_iter().zip(expected) {
            assert_eq!(pulse.pin, Ym2413OutputPin::Ro);
            assert_eq!(pulse.slot_index, slot);
            assert_eq!(pulse.source, Ym2413OutputSource::RhythmVoice(voice));
            assert_eq!(pulse.level, level(value));
            assert_eq!(pulse.active_input_clocks(), 3);
            assert_eq!(pulse.neutral_input_clocks(), 1);
        }
    }

    #[test]
    fn rhythm_output_service_preserves_negative_zero_on_ro_repeat() {
        let mut service =
            super::Ym2413RhythmOutputService::with_initial_state(service_state(13)).unwrap();
        let negative_zero = Ym2413DacCode::negative_zero();
        let original = service.process_slot(13, None, Some(negative_zero)).unwrap();
        let Ym2413OutputSlotResult::Pulse(original_pulse) = original.ro else {
            panic!("slot 13 must emit the original HH value");
        };
        assert_eq!(original_pulse.pin, Ym2413OutputPin::Ro);
        assert!(original_pulse.level.is_negative_zero());

        for (slot, value) in [(14, 1), (15, -1), (16, 2), (17, -2)] {
            service
                .process_slot(slot, None, Some(level(value)))
                .unwrap();
        }
        let snapshot = service.export_state();
        let mut restored = super::Ym2413RhythmOutputService::with_initial_state(snapshot).unwrap();
        let repeated = restored.process_slot(0, None, None).unwrap();
        let Ym2413OutputSlotResult::Pulse(repeat_pulse) = repeated.ro else {
            panic!("slot 0 must emit the retained HH value");
        };
        assert_eq!(repeat_pulse.pin, Ym2413OutputPin::Ro);
        assert_eq!(repeat_pulse.level, negative_zero);
        assert!(repeat_pulse.level.is_negative_zero());
    }

    #[test]
    fn rhythm_output_service_snapshot_restore_preserves_pin_events_across_rotation() {
        let mut continuous =
            super::Ym2413RhythmOutputService::with_initial_state(service_state(13)).unwrap();
        let mut split =
            super::Ym2413RhythmOutputService::with_initial_state(service_state(13)).unwrap();

        let mut continuous_frames = Vec::new();
        for slot in (13..18).chain(0..13) {
            continuous_frames.push(process_service_slot(&mut continuous, slot));
        }

        let mut prefix = Vec::new();
        for slot in 13..18 {
            prefix.push(process_service_slot(&mut split, slot));
        }
        let snapshot = split.export_state();
        let mut restored = super::Ym2413RhythmOutputService::with_initial_state(snapshot).unwrap();
        let mut resumed = Vec::new();
        for slot in 0..13 {
            resumed.push(process_service_slot(&mut restored, slot));
        }

        assert_eq!(prefix, continuous_frames[..5]);
        assert_eq!(resumed, continuous_frames[5..]);
        assert_eq!(restored.export_state(), continuous.export_state());
    }

    #[test]
    fn rhythm_output_service_rejects_invalid_mo_or_rhythm_inputs_transactionally() {
        let mut service =
            super::Ym2413RhythmOutputService::with_initial_state(service_state(13)).unwrap();
        let before = service.export_state();
        assert_eq!(
            service.process_slot(13, Some(level(1)), Some(level(2))),
            Err(super::Ym2413RhythmOutputServiceError::Routing(
                Ym2413OutputRoutingError::UnexpectedMelodyLevel { slot_index: 13 }
            ))
        );
        assert_eq!(service.export_state(), before);

        assert_eq!(
            service.process_slot(13, None, None),
            Err(super::Ym2413RhythmOutputServiceError::Retention(
                Ym2413RhythmOutputError::MissingOriginalSample {
                    slot_index: 13,
                    voice: Ym2413RhythmVoice::HiHat,
                }
            ))
        );
        assert_eq!(service.export_state(), before);
    }

    #[test]
    fn nuked_cycle_maps_to_measured_mo_carrier_slots() {
        let expected = [3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 0, 1, 2];
        for (cycle, expected_slot) in expected.into_iter().enumerate() {
            assert_eq!(
                mo_output_slot_for_nuked_cycle(cycle as u8),
                Some(expected_slot)
            );
        }
        assert_eq!(mo_output_slot_for_nuked_cycle(18), None);
        assert_eq!(mo_output_slot_for_nuked_cycle(u8::MAX), None);
    }

    #[test]
    fn nuked_cycle_maps_to_measured_ro_occurrence_slots() {
        let expected = [15, 16, 17, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14];
        for (cycle, expected_slot) in expected.into_iter().enumerate() {
            assert_eq!(
                ro_output_slot_for_nuked_cycle(cycle as u8),
                Some(expected_slot)
            );
        }
        assert_eq!(ro_output_slot_for_nuked_cycle(18), None);
        assert_eq!(ro_output_slot_for_nuked_cycle(u8::MAX), None);
    }

    #[test]
    fn independent_cycle_maps_align_measured_rhythm_mo_and_ro_positions() {
        let mo_cycles = [0, 1, 2, 6, 7, 8];
        let expected_mo_slots = [3, 4, 5, 9, 10, 11];
        for (cycle, slot) in mo_cycles.into_iter().zip(expected_mo_slots) {
            assert_eq!(mo_output_slot_for_nuked_cycle(cycle), Some(slot));
        }

        let ro_cycles = [16, 17, 0, 1, 2, 3, 4, 5, 9, 10];
        let expected_ro_slots = [13, 14, 15, 16, 17, 0, 1, 2, 6, 7];
        for (cycle, slot) in ro_cycles.into_iter().zip(expected_ro_slots) {
            assert_eq!(ro_output_slot_for_nuked_cycle(cycle), Some(slot));
        }
    }

    #[test]
    fn nuked_cycle_output_service_uses_independent_pin_slots_and_ro_retention_cursor() {
        let mut service =
            super::Ym2413RhythmOutputService::with_initial_state(service_state(15)).unwrap();
        let mut frames = Vec::new();

        for cycle in 0..18_u8 {
            let mo_slot = mo_output_slot_for_nuked_cycle(cycle).unwrap();
            let ro_slot = ro_output_slot_for_nuked_cycle(cycle).unwrap();
            let melody_level = if [3, 4, 5, 9, 10, 11].contains(&mo_slot) {
                Some(level(i16::from(mo_slot)))
            } else {
                None
            };
            let rhythm_level = if (13..=17).contains(&ro_slot) {
                Some(level(i16::from(ro_slot)))
            } else {
                None
            };
            frames.push(
                service
                    .process_nuked_cycle(cycle, true, melody_level, rhythm_level)
                    .unwrap(),
            );
        }

        assert_eq!(frames.len(), 18);
        for frame in &frames {
            assert_eq!(
                frame.mo_slot_index,
                mo_output_slot_for_nuked_cycle(frame.cycle).unwrap()
            );
            assert_eq!(
                frame.ro_slot_index,
                ro_output_slot_for_nuked_cycle(frame.cycle).unwrap()
            );
        }

        let cycle_zero = frames[0];
        assert_eq!(cycle_zero.mo_slot_index, 3);
        assert!(cycle_zero.mo.is_some());
        assert_eq!(cycle_zero.ro_slot_index, 15);
        assert!(matches!(
            cycle_zero.ro,
            Ym2413OutputSlotResult::Pulse(pulse)
                if pulse.source == Ym2413OutputSource::RhythmVoice(Ym2413RhythmVoice::BassDrum)
        ));

        // The first rotation begins at RO slot 15, so HH/TOM repeats at
        // slots 0/1 precede their source slots 13/14 and remain uninitialized.
        assert!(matches!(
            frames[3].ro,
            Ym2413OutputSlotResult::RepeatNotInitialized {
                slot_index: 0,
                voice: Ym2413RhythmVoice::HiHat
            }
        ));
        assert!(matches!(
            frames[4].ro,
            Ym2413OutputSlotResult::RepeatNotInitialized {
                slot_index: 1,
                voice: Ym2413RhythmVoice::TomTom
            }
        ));
    }

    #[test]
    fn nuked_cycle_output_service_rejects_invalid_cycle_without_mutating_state() {
        let mut service =
            super::Ym2413RhythmOutputService::with_initial_state(service_state(15)).unwrap();
        let before = service.export_state();
        assert_eq!(
            service.process_nuked_cycle(18, true, None, None),
            Err(super::Ym2413RhythmOutputServiceError::InvalidNukedCycle(18))
        );
        assert_eq!(service.export_state(), before);
    }

    #[test]
    fn measured_output_window_is_three_active_clocks_in_each_four_clock_slot() {
        assert_eq!(YM2413_OUTPUT_ACTIVE_CLOCKS, 3);
        assert_eq!(YM2413_OUTPUT_NEUTRAL_CLOCKS, 1);
        assert_eq!(
            YM2413_OUTPUT_ACTIVE_CLOCKS + YM2413_OUTPUT_NEUTRAL_CLOCKS,
            YM2413_OUTPUT_SLOT_CLOCKS
        );
    }

    #[test]
    fn melodic_mode_routes_all_nine_carrier_slots_to_mo() {
        let expected = [
            (3, 0),
            (4, 1),
            (5, 2),
            (9, 3),
            (10, 4),
            (11, 5),
            (15, 6),
            (16, 7),
            (17, 8),
        ];

        for (slot, channel) in expected {
            let pulse = melody_output_for_slot(slot, false, Some(level(channel as i16))).unwrap();
            assert_eq!(
                pulse,
                Some(super::Ym2413OutputPulse {
                    pin: Ym2413OutputPin::Mo,
                    slot_index: slot,
                    source: Ym2413OutputSource::MelodyChannel(channel),
                    level: level(channel as i16),
                })
            );
        }
    }

    #[test]
    fn rhythm_mode_routes_only_the_first_six_melodic_channels_to_mo() {
        for slot in [3, 4, 5, 9, 10, 11] {
            assert!(
                melody_output_for_slot(slot, true, Some(level(12)))
                    .unwrap()
                    .is_some()
            );
        }
        for slot in [15, 16, 17] {
            assert_eq!(melody_output_for_slot(slot, true, None), Ok(None));
        }
    }

    #[test]
    fn non_carrier_slots_do_not_emit_melody_and_reject_supplied_levels() {
        for slot in [0, 1, 2, 6, 7, 8, 12, 13, 14] {
            assert_eq!(melody_output_for_slot(slot, false, None), Ok(None));
            assert_eq!(
                melody_output_for_slot(slot, false, Some(level(1))),
                Err(Ym2413OutputRoutingError::UnexpectedMelodyLevel { slot_index: slot })
            );
        }
    }

    #[test]
    fn melody_carrier_slots_require_explicit_levels_including_silence() {
        assert_eq!(
            melody_output_for_slot(3, false, None),
            Err(Ym2413OutputRoutingError::MissingMelodyLevel {
                slot_index: 3,
                channel: 0,
            })
        );
        assert_eq!(
            melody_output_for_slot(18, false, None),
            Err(Ym2413OutputRoutingError::InvalidSlotIndex(18))
        );
    }

    #[test]
    fn measured_rhythm_original_and_repeat_events_route_to_ro() {
        let events = [
            (0, Ym2413RhythmVoice::HiHat, true),
            (1, Ym2413RhythmVoice::TomTom, true),
            (2, Ym2413RhythmVoice::BassDrum, true),
            (6, Ym2413RhythmVoice::SnareDrum, true),
            (7, Ym2413RhythmVoice::TopCymbal, true),
            (13, Ym2413RhythmVoice::HiHat, false),
            (14, Ym2413RhythmVoice::TomTom, false),
            (15, Ym2413RhythmVoice::BassDrum, false),
            (16, Ym2413RhythmVoice::SnareDrum, false),
            (17, Ym2413RhythmVoice::TopCymbal, false),
        ];

        for (slot, voice, is_delayed_repeat) in events {
            let event = Ym2413RhythmOutputEvent {
                slot_index: slot,
                voice,
                is_delayed_repeat,
                code: level(-37),
            };
            let pulse = rhythm_output_event(event).unwrap();
            assert_eq!(pulse.pin, Ym2413OutputPin::Ro);
            assert_eq!(pulse.slot_index, slot);
            assert_eq!(pulse.source, Ym2413OutputSource::RhythmVoice(voice));
            assert_eq!(pulse.level, level(-37));
            assert_eq!(pulse.active_input_clocks(), 3);
            assert_eq!(pulse.neutral_input_clocks(), 1);
        }
    }

    #[test]
    fn invalid_rhythm_events_are_rejected() {
        let event = Ym2413RhythmOutputEvent {
            slot_index: 12,
            voice: Ym2413RhythmVoice::HiHat,
            is_delayed_repeat: false,
            code: level(0),
        };
        assert_eq!(
            rhythm_output_event(event),
            Err(Ym2413OutputRoutingError::InvalidRhythmEvent(event))
        );
    }

    #[test]
    fn rhythm_slot_result_preserves_uninitialized_repeat_as_no_pulse() {
        assert_eq!(
            rhythm_slot_result(Ym2413RhythmOutputSlotResult::RepeatNotInitialized {
                slot_index: 0,
                voice: Ym2413RhythmVoice::HiHat,
            }),
            Ok(Ym2413OutputSlotResult::RepeatNotInitialized {
                slot_index: 0,
                voice: Ym2413RhythmVoice::HiHat,
            })
        );
        assert_eq!(
            rhythm_slot_result(Ym2413RhythmOutputSlotResult::NoRhythmOutput),
            Ok(Ym2413OutputSlotResult::NoOutput)
        );
    }
}
