//! Transactional SMS memory-bus adapter for the Scheduler-driven path.
//!
//! Memory reads and writes become visible only when their scheduled completion
//! event is dispatched. A completion is latched until the CPU acknowledges it,
//! preventing a second dispatch from repeating a write side effect.

use core::fmt;

use crate::{
    CartridgeImage, SmsCoreError, SmsMemory, SmsTemporalCoordinator, SmsTemporalCoordinatorError,
    SmsTemporalCoordinatorState, SmsTemporalEvent,
};
use sega_temporal_core::MachineTime;
use sega_z80_cpu::{Z80BusOperation, Z80BusRequest};

pub const SMS_TEMPORAL_BUS_STATE_VERSION: u16 = 2;

/// Result latched at the completion boundary of one memory transaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmsBusCompletion {
    pub request_id: u64,
    pub data: Option<u8>,
    pub completed_at: MachineTime,
}

/// Completion of a CPU-internal interval with no memory-bus transaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmsInternalCycleCompletion {
    pub continuation_id: u64,
    pub completed_at: MachineTime,
}

/// One dispatched event owned by the SMS temporal bus/coordinator boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmsTemporalDispatch {
    BusCompletion(SmsBusCompletion),
    InternalCycleComplete(SmsInternalCycleCompletion),
}

/// Complete in-memory continuation of the transactional bus adapter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmsTemporalBusState {
    pub format_version: u16,
    pub memory: SmsMemory,
    pub coordinator: SmsTemporalCoordinatorState,
    pub pending_request: Option<Z80BusRequest>,
    pub completion: Option<SmsBusCompletion>,
    pub pending_internal_cycle: Option<u64>,
}

/// Errors from the transactional SMS memory-bus adapter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SmsTemporalBusError {
    Core(SmsCoreError),
    Coordinator(SmsTemporalCoordinatorError),
    Busy,
    NoPendingRequest,
    WrongRequestId { expected: u64, received: u64 },
    UnsupportedOperation,
    InvalidState,
}

impl fmt::Display for SmsTemporalBusError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Core(error) => write!(f, "SMS memory initialization failed: {error}"),
            Self::Coordinator(error) => write!(f, "SMS temporal coordinator failed: {error}"),
            Self::Busy => {
                f.write_str("SMS temporal bus already owns an unacknowledged transaction")
            }
            Self::NoPendingRequest => f.write_str("SMS temporal bus has no pending request"),
            Self::WrongRequestId { expected, received } => write!(
                f,
                "SMS temporal bus expected request {expected}, received {received}"
            ),
            Self::UnsupportedOperation => {
                f.write_str("this temporal bus slice supports memory cycles only")
            }
            Self::InvalidState => f.write_str("SMS temporal bus snapshot is inconsistent"),
        }
    }
}

impl std::error::Error for SmsTemporalBusError {}

impl From<SmsCoreError> for SmsTemporalBusError {
    fn from(error: SmsCoreError) -> Self {
        Self::Core(error)
    }
}

impl From<SmsTemporalCoordinatorError> for SmsTemporalBusError {
    fn from(error: SmsTemporalCoordinatorError) -> Self {
        Self::Coordinator(error)
    }
}

/// SMS mapper-backed memory and its scheduler-owned bus completion timeline.
#[derive(Debug, Clone)]
pub struct SmsTemporalBus {
    memory: SmsMemory,
    coordinator: SmsTemporalCoordinator,
    pending_request: Option<Z80BusRequest>,
    completion: Option<SmsBusCompletion>,
    pending_internal_cycle: Option<u64>,
}

impl SmsTemporalBus {
    /// Construct a bus from a cartridge and an explicit master-tick/T-state scale.
    pub fn new(
        rom: impl Into<Vec<u8>>,
        master_ticks_per_tstate: u64,
    ) -> Result<Self, SmsTemporalBusError> {
        let cartridge = CartridgeImage::new(rom)?;
        let memory = SmsMemory::new(cartridge)?;
        let coordinator = SmsTemporalCoordinator::new(master_ticks_per_tstate)?;
        Ok(Self {
            memory,
            coordinator,
            pending_request: None,
            completion: None,
            pending_internal_cycle: None,
        })
    }

    /// Begin one memory request and schedule its completion deadline.
    ///
    /// WAIT is supplied by a deterministic fixture; this API does not sample
    /// or claim a physical SMS WAIT signal.
    pub fn begin_request(
        &mut self,
        request: Z80BusRequest,
        fixture_wait_tstates: u32,
    ) -> Result<MachineTime, SmsTemporalBusError> {
        if self.pending_request.is_some()
            || self.completion.is_some()
            || self.pending_internal_cycle.is_some()
        {
            return Err(SmsTemporalBusError::Busy);
        }
        if !matches!(
            request.operation,
            Z80BusOperation::OpcodeFetch
                | Z80BusOperation::MemoryRead
                | Z80BusOperation::MemoryWrite { .. }
        ) {
            return Err(SmsTemporalBusError::UnsupportedOperation);
        }
        let deadline = self.coordinator.schedule_bus_completion(
            request.id,
            request.base_tstates,
            fixture_wait_tstates,
        )?;
        self.pending_request = Some(request);
        Ok(deadline)
    }

    /// Schedule one internal CPU interval without creating a fake bus access.
    pub fn schedule_internal_cycle(
        &mut self,
        continuation_id: u64,
        tstates: u32,
    ) -> Result<MachineTime, SmsTemporalBusError> {
        if self.pending_request.is_some()
            || self.completion.is_some()
            || self.pending_internal_cycle.is_some()
        {
            return Err(SmsTemporalBusError::Busy);
        }
        let deadline = self
            .coordinator
            .schedule_internal_cycle(continuation_id, tstates)?;
        self.pending_internal_cycle = Some(continuation_id);
        Ok(deadline)
    }

    /// Dispatch the next scheduled boundary and apply the memory side effect once.
    ///
    /// A read samples memory at this completion boundary. A write mutates memory
    /// at this completion boundary. The response is latched until acknowledged.
    pub fn dispatch_next(&mut self) -> Result<Option<SmsTemporalDispatch>, SmsTemporalBusError> {
        let Some(dispatch) = self.coordinator.dispatch_next()? else {
            return Ok(None);
        };
        match dispatch.event.payload {
            SmsTemporalEvent::BusCompletion { request_id } => {
                let request = self
                    .pending_request
                    .ok_or(SmsTemporalBusError::NoPendingRequest)?;
                if request.id != request_id {
                    return Err(SmsTemporalBusError::WrongRequestId {
                        expected: request.id,
                        received: request_id,
                    });
                }
                if self.completion.is_some() {
                    return Err(SmsTemporalBusError::Busy);
                }

                let data = match request.operation {
                    Z80BusOperation::OpcodeFetch | Z80BusOperation::MemoryRead => {
                        Some(self.memory.read(request.address))
                    }
                    Z80BusOperation::MemoryWrite { value } => {
                        self.memory.write(request.address, value);
                        None
                    }
                    Z80BusOperation::PortRead | Z80BusOperation::PortWrite { .. } => {
                        return Err(SmsTemporalBusError::UnsupportedOperation);
                    }
                };
                let completion = SmsBusCompletion {
                    request_id,
                    data,
                    completed_at: dispatch.event.timestamp,
                };
                self.completion = Some(completion);
                Ok(Some(SmsTemporalDispatch::BusCompletion(completion)))
            }
            SmsTemporalEvent::InternalCycleComplete { continuation_id } => {
                if self.pending_internal_cycle != Some(continuation_id) {
                    return Err(SmsTemporalBusError::InvalidState);
                }
                self.pending_internal_cycle = None;
                Ok(Some(SmsTemporalDispatch::InternalCycleComplete(
                    SmsInternalCycleCompletion {
                        continuation_id,
                        completed_at: dispatch.event.timestamp,
                    },
                )))
            }
        }
    }

    /// Consume the latched response exactly once after the CPU accepts it.
    pub fn acknowledge_completion(
        &mut self,
        request_id: u64,
    ) -> Result<SmsBusCompletion, SmsTemporalBusError> {
        let completion = self
            .completion
            .ok_or(SmsTemporalBusError::NoPendingRequest)?;
        if completion.request_id != request_id {
            return Err(SmsTemporalBusError::WrongRequestId {
                expected: completion.request_id,
                received: request_id,
            });
        }
        self.completion = None;
        self.pending_request = None;
        Ok(completion)
    }

    /// Return the currently outstanding CPU request, if any.
    pub const fn pending_request(&self) -> Option<Z80BusRequest> {
        self.pending_request
    }

    /// Return the current machine timestamp.
    pub const fn now(&self) -> MachineTime {
        self.coordinator.now()
    }

    #[cfg(test)]
    pub(crate) fn write_memory_for_test(&mut self, address: u16, value: u8) {
        self.memory.write(address, value);
    }

    /// Read-only access to the mapper-backed memory image.
    pub const fn memory(&self) -> &SmsMemory {
        &self.memory
    }

    /// Export memory, scheduler, and in-flight response state.
    pub fn export_state(&self) -> SmsTemporalBusState {
        SmsTemporalBusState {
            format_version: SMS_TEMPORAL_BUS_STATE_VERSION,
            memory: self.memory.clone(),
            coordinator: self.coordinator.export_state(),
            pending_request: self.pending_request,
            completion: self.completion,
            pending_internal_cycle: self.pending_internal_cycle,
        }
    }

    /// Restore only when the scheduler event and bus transaction agree.
    pub fn restore_state(state: SmsTemporalBusState) -> Result<Self, SmsTemporalBusError> {
        if state.format_version != SMS_TEMPORAL_BUS_STATE_VERSION {
            return Err(SmsTemporalBusError::InvalidState);
        }
        let coordinator = SmsTemporalCoordinator::restore_state(state.coordinator.clone())?;
        let pending_events = &state.coordinator.scheduler.event_queue.pending;
        match (
            state.pending_request,
            state.completion,
            state.pending_internal_cycle,
        ) {
            (None, None, None) if pending_events.is_empty() => {}
            (Some(request), None, None)
                if pending_events.len() == 1
                    && pending_events[0].payload
                        == (SmsTemporalEvent::BusCompletion {
                            request_id: request.id,
                        }) => {}
            (Some(request), Some(completion), None)
                if request.id == completion.request_id && pending_events.is_empty() => {}
            (None, None, Some(continuation_id))
                if pending_events.len() == 1
                    && pending_events[0].payload
                        == (SmsTemporalEvent::InternalCycleComplete { continuation_id }) => {}
            _ => return Err(SmsTemporalBusError::InvalidState),
        }
        Ok(Self {
            memory: state.memory,
            coordinator,
            pending_request: state.pending_request,
            completion: state.completion,
            pending_internal_cycle: state.pending_internal_cycle,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{SmsTemporalBus, SmsTemporalBusError, SmsTemporalDispatch};
    use sega_z80_cpu::{Z80BusOperation, Z80BusRequest};

    fn request(id: u64, operation: Z80BusOperation, address: u16, tstates: u8) -> Z80BusRequest {
        Z80BusRequest {
            id,
            operation,
            address,
            base_tstates: tstates,
        }
    }

    #[test]
    fn memory_read_is_sampled_at_dispatch_and_acknowledged_once() {
        let mut bus = SmsTemporalBus::new(vec![0xA5; 0x4000], 3).unwrap();
        let request = request(1, Z80BusOperation::MemoryRead, 0x0100, 3);
        bus.begin_request(request, 2).unwrap();
        let SmsTemporalDispatch::BusCompletion(completion) = bus.dispatch_next().unwrap().unwrap()
        else {
            panic!("expected bus completion");
        };
        assert_eq!(completion.request_id, 1);
        assert_eq!(completion.data, Some(0xA5));
        assert_eq!(bus.acknowledge_completion(1).unwrap(), completion);
        assert_eq!(
            bus.acknowledge_completion(1),
            Err(SmsTemporalBusError::NoPendingRequest)
        );
    }

    #[test]
    fn memory_write_occurs_only_at_its_scheduled_completion() {
        let mut bus = SmsTemporalBus::new(vec![0; 0x4000], 3).unwrap();
        let request = request(2, Z80BusOperation::MemoryWrite { value: 0x5A }, 0xC000, 3);
        bus.begin_request(request, 0).unwrap();
        assert_eq!(bus.memory().read(0xC000), 0);
        let SmsTemporalDispatch::BusCompletion(completion) = bus.dispatch_next().unwrap().unwrap() else { panic!("expected bus completion"); };
        assert_eq!(completion.data, None);
        assert_eq!(bus.memory().read(0xC000), 0x5A);
        bus.acknowledge_completion(2).unwrap();
        assert_eq!(bus.memory().read(0xC000), 0x5A);
    }

    #[test]
    fn snapshot_restore_preserves_pending_transaction_before_effect() {
        let mut bus = SmsTemporalBus::new(vec![0; 0x4000], 3).unwrap();
        let request = request(3, Z80BusOperation::MemoryWrite { value: 0x77 }, 0xC001, 3);
        bus.begin_request(request, 1).unwrap();
        let mut restored = SmsTemporalBus::restore_state(bus.export_state()).unwrap();
        assert_eq!(restored.memory().read(0xC001), 0);
        let SmsTemporalDispatch::BusCompletion(completion) =
            restored.dispatch_next().unwrap().unwrap()
        else {
            panic!("expected bus completion");
        };
        assert_eq!(restored.memory().read(0xC001), 0x77);
        restored
            .acknowledge_completion(completion.request_id)
            .unwrap();
    }

    #[test]
    fn snapshot_restore_preserves_completed_unacknowledged_write() {
        let mut bus = SmsTemporalBus::new(vec![0; 0x4000], 3).unwrap();
        let request = request(4, Z80BusOperation::MemoryWrite { value: 0x99 }, 0xC002, 3);
        bus.begin_request(request, 0).unwrap();
        let SmsTemporalDispatch::BusCompletion(completion) = bus.dispatch_next().unwrap().unwrap() else { panic!("expected bus completion"); };
        let mut restored = SmsTemporalBus::restore_state(bus.export_state()).unwrap();
        assert_eq!(restored.memory().read(0xC002), 0x99);
        assert_eq!(restored.acknowledge_completion(4).unwrap(), completion);
        assert_eq!(restored.memory().read(0xC002), 0x99);
    }

    #[test]
    fn internal_cycle_dispatch_has_no_memory_side_effect() {
        let mut bus = SmsTemporalBus::new(vec![0; 0x4000], 3).unwrap();
        let deadline = bus.schedule_internal_cycle(77, 1).unwrap();
        let Some(SmsTemporalDispatch::InternalCycleComplete(completion)) =
            bus.dispatch_next().unwrap()
        else {
            panic!("expected internal-cycle completion");
        };
        assert_eq!(completion.continuation_id, 77);
        assert_eq!(completion.completed_at, deadline);
        assert_eq!(bus.memory().read(0xC000), 0);
    }

    #[test]
    fn port_operations_are_rejected_before_scheduling() {
        let mut bus = SmsTemporalBus::new(vec![0; 0x4000], 3).unwrap();
        assert_eq!(
            bus.begin_request(request(5, Z80BusOperation::PortRead, 0x00DC, 4), 0),
            Err(SmsTemporalBusError::UnsupportedOperation)
        );
    }
}
