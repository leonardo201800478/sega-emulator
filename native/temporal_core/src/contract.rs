//! Shared temporal transaction contracts for CPU, bus and machine scheduling.
//!
//! These are protocol/data types only. They do not execute a CPU cycle, advance a
//! device, assign SMS1 electrical timing, or define hardware arbitration.

use core::fmt;

use crate::MachineTime;

/// Logical kind of a CPU bus cycle. This is not an electrical pin waveform.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BusCycleKind {
    OpcodeFetch,
    MemoryRead,
    MemoryWrite,
    IoRead,
    IoWrite,
    InterruptAcknowledge,
    Refresh,
    Internal,
}

/// Zero-based M-cycle position in the current CPU continuation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MCycleIndex(u32);

impl MCycleIndex {
    pub const fn new(value: u32) -> Self { Self(value) }
    pub const fn value(self) -> u32 { self.0 }
}

/// Zero-based T-state position within one M-cycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TStateIndex(u16);

impl TStateIndex {
    pub const fn new(value: u16) -> Self { Self(value) }
    pub const fn value(self) -> u16 { self.0 }
}

/// Direction of a bus transaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BusDirection {
    Read,
    Write,
    None,
}

/// Abstract phase within a logical bus cycle.
///
/// These labels describe protocol progress, not a measured half-clock phase.
/// A machine profile must provide any hardware-specific edge mapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BusPhase {
    Setup,
    Sample,
    Wait,
    Hold,
    Complete,
    Internal,
}

/// Immutable description of one logical bus transaction at machine time.
///
/// `t_state_index` is zero-based within the current M-cycle. `cycle_index`
/// is zero-based within the current CPU execution continuation. Optional data
/// is absent for a read until the bus returns its result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BusTransaction {
    timestamp: MachineTime,
    cycle_index: MCycleIndex,
    t_state_index: TStateIndex,
    phase: BusPhase,
    kind: BusCycleKind,
    direction: BusDirection,
    address: Option<u16>,
    data: Option<u8>,
    duration_tstates: u16,
}

impl BusTransaction {
    /// Construct a transaction after validating its logical fields.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        timestamp: MachineTime,
        cycle_index: MCycleIndex,
        t_state_index: TStateIndex,
        phase: BusPhase,
        kind: BusCycleKind,
        direction: BusDirection,
        address: Option<u16>,
        data: Option<u8>,
        duration_tstates: u16,
    ) -> Result<Self, BusContractError> {
        if duration_tstates == 0 {
            return Err(BusContractError::ZeroDuration);
        }
        let valid = match kind {
            BusCycleKind::OpcodeFetch | BusCycleKind::MemoryRead | BusCycleKind::IoRead => {
                direction == BusDirection::Read && address.is_some() && data.is_none()
            }
            BusCycleKind::InterruptAcknowledge => {
                direction == BusDirection::Read && address.is_none() && data.is_none()
            }
            BusCycleKind::Refresh => {
                direction == BusDirection::None && address.is_some() && data.is_none()
            },
            BusCycleKind::MemoryWrite | BusCycleKind::IoWrite => {
                direction == BusDirection::Write && address.is_some() && data.is_some()
            }
            BusCycleKind::Internal => {
                direction == BusDirection::None && address.is_none() && data.is_none()
            }
        };
        if !valid {
            return Err(BusContractError::InconsistentFields);
        }
        Ok(Self {
            timestamp,
            cycle_index,
            t_state_index,
            phase,
            kind,
            direction,
            address,
            data,
            duration_tstates,
        })
    }

    pub const fn timestamp(self) -> MachineTime { self.timestamp }
    pub const fn cycle_index(self) -> MCycleIndex { self.cycle_index }
    pub const fn t_state_index(self) -> TStateIndex { self.t_state_index }
    pub const fn phase(self) -> BusPhase { self.phase }
    pub const fn kind(self) -> BusCycleKind { self.kind }
    pub const fn direction(self) -> BusDirection { self.direction }
    pub const fn address(self) -> Option<u16> { self.address }
    pub const fn data(self) -> Option<u8> { self.data }
    pub const fn duration_tstates(self) -> u16 { self.duration_tstates }
}

/// Result of attempting a temporal bus operation.
///
/// WAIT is a resumable hold at a strictly later machine timestamp. Suspended
/// means the owner must retain its continuation and retry only when explicitly
/// scheduled; it does not imply a busy loop. Fault is a deterministic protocol
/// or device error, not an emulated hardware value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BusOperationResult<T, E> {
    Completed {
        value: T,
        completed_at: MachineTime,
    },
    Wait {
        resume_at: MachineTime,
    },
    Suspended {
        continuation_id: u64,
    },
    Fault(E),
}

impl<T, E> BusOperationResult<T, E> {
    /// Validate that a completed transaction does not finish before it starts.
    pub fn validate_completion(&self, requested_at: MachineTime) -> Result<(), BusContractError> {
        if let Self::Completed { completed_at, .. } = self {
            if *completed_at < requested_at {
                return Err(BusContractError::CompletionBeforeRequest);
            }
        }
        Ok(())
    }

    /// Validate the temporal relation of a WAIT response to its request.
    pub fn validate_wait(&self, requested_at: MachineTime) -> Result<(), BusContractError> {
        if let Self::Wait { resume_at } = self {
            if *resume_at <= requested_at {
                return Err(BusContractError::WaitDoesNotAdvanceTime);
            }
        }
        Ok(())
    }
}

/// Progress report returned by a device after being advanced to a boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeviceProgress {
    reached: MachineTime,
    next_event: Option<MachineTime>,
}

impl DeviceProgress {
    /// Construct progress and reject a next event earlier than the reached time.
    pub fn new(
        reached: MachineTime,
        next_event: Option<MachineTime>,
    ) -> Result<Self, BusContractError> {
        if next_event.is_some_and(|next| next < reached) {
            return Err(BusContractError::RetroactiveEvent);
        }
        Ok(Self { reached, next_event })
    }

    pub const fn reached(self) -> MachineTime { self.reached }
    pub const fn next_event(self) -> Option<MachineTime> { self.next_event }
}

/// Errors detected while validating shared temporal contracts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BusContractError {
    ZeroDuration,
    InconsistentFields,
    WaitDoesNotAdvanceTime,
    RetroactiveEvent,
    CompletionBeforeRequest,
}

impl fmt::Display for BusContractError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroDuration => f.write_str("bus transaction duration must be positive"),
            Self::InconsistentFields => f.write_str("bus transaction fields are inconsistent with its cycle kind"),
            Self::WaitDoesNotAdvanceTime => f.write_str("WAIT resume timestamp must be later than request timestamp"),
            Self::RetroactiveEvent => f.write_str("device reported an event before its reached timestamp"),
            Self::CompletionBeforeRequest => f.write_str("bus operation completed before its request timestamp"),
        }
    }
}

impl std::error::Error for BusContractError {}

#[cfg(test)]
mod tests {
    use super::{
        BusContractError, BusCycleKind, BusDirection, BusOperationResult, BusPhase,
        BusTransaction, DeviceProgress, MCycleIndex, TStateIndex,
    };
    use crate::MachineTime;

    #[test]
    fn transaction_accepts_a_well_formed_memory_read() {
        let transaction = BusTransaction::new(
            MachineTime::from_ticks(12), MCycleIndex::new(2), TStateIndex::new(1), BusPhase::Sample,
            BusCycleKind::MemoryRead, BusDirection::Read, Some(0xC000), None, 3,
        ).unwrap();
        assert_eq!(transaction.timestamp(), MachineTime::from_ticks(12));
        assert_eq!(transaction.address(), Some(0xC000));
        assert_eq!(transaction.data(), None);
    }

    #[test]
    fn transaction_rejects_inconsistent_direction_and_missing_write_data() {
        assert_eq!(
            BusTransaction::new(
                MachineTime::ZERO, MCycleIndex::new(0), TStateIndex::new(0), BusPhase::Setup,
                BusCycleKind::MemoryRead, BusDirection::Write, Some(0), Some(1), 3,
            ),
            Err(BusContractError::InconsistentFields),
        );
        assert_eq!(
            BusTransaction::new(
                MachineTime::ZERO, MCycleIndex::new(0), TStateIndex::new(0), BusPhase::Setup,
                BusCycleKind::MemoryWrite, BusDirection::Write, Some(0), None, 3,
            ),
            Err(BusContractError::InconsistentFields),
        );
    }

    #[test]
    fn transaction_rejects_zero_duration() {
        assert_eq!(
            BusTransaction::new(
                MachineTime::ZERO, MCycleIndex::new(0), TStateIndex::new(0), BusPhase::Setup,
                BusCycleKind::Internal, BusDirection::None, None, None, 0,
            ),
            Err(BusContractError::ZeroDuration),
        );
    }

    #[test]
    fn wait_must_resume_at_a_strictly_later_timestamp() {
        let same_time = BusOperationResult::<(), ()>::Wait { resume_at: MachineTime::ZERO };
        assert_eq!(same_time.validate_wait(MachineTime::ZERO), Err(BusContractError::WaitDoesNotAdvanceTime));
        let later = BusOperationResult::<(), ()>::Wait { resume_at: MachineTime::from_ticks(1) };
        assert_eq!(later.validate_wait(MachineTime::ZERO), Ok(()));
    }

    #[test]
    fn completion_cannot_precede_request() {
        let result = BusOperationResult::<(), ()>::Completed {
            value: (), completed_at: MachineTime::ZERO,
        };
        assert_eq!(result.validate_completion(MachineTime::from_ticks(1)), Err(BusContractError::CompletionBeforeRequest));
    }

    #[test]
    fn refresh_cycle_has_address_but_no_read_or_write_direction() {
        assert!(BusTransaction::new(
            MachineTime::ZERO, MCycleIndex::new(0), TStateIndex::new(0), BusPhase::Setup,
            BusCycleKind::Refresh, BusDirection::None, Some(0x1234), None, 1,
        ).is_ok());
    }

    #[test]
    fn device_cannot_report_an_event_in_the_past() {
        assert_eq!(
            DeviceProgress::new(MachineTime::from_ticks(8), Some(MachineTime::from_ticks(7))),
            Err(BusContractError::RetroactiveEvent),
        );
        assert!(DeviceProgress::new(MachineTime::from_ticks(8), Some(MachineTime::from_ticks(8))).is_ok());
    }
}
