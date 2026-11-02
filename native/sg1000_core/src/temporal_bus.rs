//! Transactional SG-1000 memory bus for the Scheduler-driven path.
//!
//! Memory reads and writes become visible only at their scheduled completion
//! boundary. Completion remains latched until acknowledgement so redispatch
//! cannot repeat a write side effect.

use core::fmt;

use crate::{MemoryConfig, MemoryError, Sg1000Memory, Sg1000TemporalCoordinator,
    Sg1000TemporalCoordinatorError, Sg1000TemporalCoordinatorState, Sg1000TemporalEvent};
use sega_temporal_core::MachineTime;
use sega_z80_cpu::{Z80BusOperation, Z80BusRequest};

pub const SG1000_TEMPORAL_BUS_STATE_VERSION: u16 = 1;

fn elapsed_tstates(
    started_at: MachineTime,
    completed_at: MachineTime,
    master_ticks_per_tstate: u64,
) -> Option<u128> {
    let scale = u128::from(master_ticks_per_tstate);
    if scale == 0 {
        return None;
    }
    let elapsed = completed_at.checked_duration_since(started_at)?;
    if elapsed == 0 || elapsed % scale != 0 {
        return None;
    }
    Some(elapsed / scale)
}

fn valid_bus_deadline(
    started_at: MachineTime,
    deadline: MachineTime,
    master_ticks_per_tstate: u64,
    base_tstates: u8,
) -> bool {
    elapsed_tstates(started_at, deadline, master_ticks_per_tstate)
        .is_some_and(|total| total >= u128::from(base_tstates))
}

fn valid_internal_deadline(
    started_at: MachineTime,
    deadline: MachineTime,
    master_ticks_per_tstate: u64,
) -> bool {
    elapsed_tstates(started_at, deadline, master_ticks_per_tstate).is_some()
}

fn is_supported_memory_operation(operation: Z80BusOperation) -> bool {
    matches!(
        operation,
        Z80BusOperation::OpcodeFetch
            | Z80BusOperation::MemoryRead
            | Z80BusOperation::MemoryWrite { .. }
    )
}

/// One memory-bus response latched at its scheduler deadline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sg1000BusCompletion {
    pub request_id: u64,
    pub data: Option<u8>,
    pub completed_at: MachineTime,
}

/// Completion of a CPU-internal interval without a memory access.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sg1000InternalCycleCompletion {
    pub continuation_id: u64,
    pub completed_at: MachineTime,
}

/// One event dispatched by the SG-1000 temporal bus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sg1000TemporalDispatch {
    BusCompletion(Sg1000BusCompletion),
    InternalCycleComplete(Sg1000InternalCycleCompletion),
}

/// Complete in-memory continuation of the temporal memory bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sg1000TemporalBusState {
    pub format_version: u16,
    pub memory: Sg1000Memory,
    pub coordinator: Sg1000TemporalCoordinatorState,
    pub pending_request: Option<Z80BusRequest>,
    pub request_started_at: Option<MachineTime>,
    pub completion: Option<Sg1000BusCompletion>,
    pub pending_internal_cycle: Option<u64>,
    pub internal_cycle_started_at: Option<MachineTime>,
    pub internal_completion: Option<Sg1000InternalCycleCompletion>,
}

/// Errors from the SG-1000 transactional memory bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Sg1000TemporalBusError {
    Memory(MemoryError),
    Coordinator(Sg1000TemporalCoordinatorError),
    Busy,
    NoPendingRequest,
    WrongRequestId { expected: u64, received: u64 },
    UnsupportedOperation,
    InvalidState,
}

impl fmt::Display for Sg1000TemporalBusError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Memory(error) => write!(f, "SG-1000 memory initialization failed: {error}"),
            Self::Coordinator(error) => write!(f, "SG-1000 temporal coordinator failed: {error}"),
            Self::Busy => f.write_str("SG-1000 temporal bus already owns an unacknowledged event"),
            Self::NoPendingRequest => f.write_str("SG-1000 temporal bus has no pending request"),
            Self::WrongRequestId { expected, received } => write!(
                f,
                "SG-1000 temporal bus expected request {expected}, received {received}"
            ),
            Self::UnsupportedOperation => {
                f.write_str("this SG-1000 temporal bus slice supports memory cycles only")
            }
            Self::InvalidState => f.write_str("SG-1000 temporal bus snapshot is inconsistent"),
        }
    }
}

impl std::error::Error for Sg1000TemporalBusError {}

impl From<MemoryError> for Sg1000TemporalBusError {
    fn from(error: MemoryError) -> Self {
        Self::Memory(error)
    }
}

impl From<Sg1000TemporalCoordinatorError> for Sg1000TemporalBusError {
    fn from(error: Sg1000TemporalCoordinatorError) -> Self {
        Self::Coordinator(error)
    }
}

/// SG-1000 memory and its scheduler-owned transaction timeline.
#[derive(Debug, Clone)]
pub struct Sg1000TemporalBus {
    memory: Sg1000Memory,
    coordinator: Sg1000TemporalCoordinator,
    pending_request: Option<Z80BusRequest>,
    request_started_at: Option<MachineTime>,
    completion: Option<Sg1000BusCompletion>,
    pending_internal_cycle: Option<u64>,
    internal_cycle_started_at: Option<MachineTime>,
    internal_completion: Option<Sg1000InternalCycleCompletion>,
}

impl Sg1000TemporalBus {
    /// Construct the memory bus with an explicit profile and time scale.
    pub fn new(
        rom: impl Into<Vec<u8>>,
        memory_config: MemoryConfig,
        master_ticks_per_tstate: u64,
    ) -> Result<Self, Sg1000TemporalBusError> {
        let rom = rom.into();
        let memory = Sg1000Memory::new(&rom, memory_config)?;
        let coordinator = Sg1000TemporalCoordinator::new(master_ticks_per_tstate)?;
        Ok(Self {
            memory,
            coordinator,
            pending_request: None,
            request_started_at: None,
            completion: None,
            pending_internal_cycle: None,
            internal_cycle_started_at: None,
            internal_completion: None,
        })
    }

    /// Schedule one memory request. WAIT is fixture input, never a physical signal.
    pub fn begin_request(
        &mut self,
        request: Z80BusRequest,
        fixture_wait_tstates: u32,
    ) -> Result<MachineTime, Sg1000TemporalBusError> {
        if self.pending_request.is_some()
            || self.completion.is_some()
            || self.pending_internal_cycle.is_some()
            || self.internal_completion.is_some()
        {
            return Err(Sg1000TemporalBusError::Busy);
        }
        if !is_supported_memory_operation(request.operation) {
            return Err(Sg1000TemporalBusError::UnsupportedOperation);
        }
        let started_at = self.coordinator.now();
        let deadline = self.coordinator.schedule_bus_completion(
            request.id,
            request.base_tstates,
            fixture_wait_tstates,
        )?;
        self.pending_request = Some(request);
        self.request_started_at = Some(started_at);
        Ok(deadline)
    }

    /// Schedule a CPU-internal interval on the same machine timeline.
    pub fn schedule_internal_cycle(
        &mut self,
        continuation_id: u64,
        tstates: u32,
    ) -> Result<MachineTime, Sg1000TemporalBusError> {
        if self.pending_request.is_some()
            || self.completion.is_some()
            || self.pending_internal_cycle.is_some()
            || self.internal_completion.is_some()
        {
            return Err(Sg1000TemporalBusError::Busy);
        }
        let started_at = self.coordinator.now();
        let deadline = self
            .coordinator
            .schedule_internal_cycle(continuation_id, tstates)?;
        self.pending_internal_cycle = Some(continuation_id);
        self.internal_cycle_started_at = Some(started_at);
        Ok(deadline)
    }

    /// Dispatch the next deadline and apply memory side effects exactly once.
    pub fn dispatch_next(
        &mut self,
    ) -> Result<Option<Sg1000TemporalDispatch>, Sg1000TemporalBusError> {
        if let Some(completion) = self.completion {
            return Ok(Some(Sg1000TemporalDispatch::BusCompletion(completion)));
        }
        if let Some(completion) = self.internal_completion {
            return Ok(Some(Sg1000TemporalDispatch::InternalCycleComplete(completion)));
        }
        let Some(dispatch) = self.coordinator.dispatch_next()? else {
            return Ok(None);
        };
        match dispatch.event.payload {
            Sg1000TemporalEvent::BusCompletion { request_id } => {
                let request = self
                    .pending_request
                    .ok_or(Sg1000TemporalBusError::NoPendingRequest)?;
                if request.id != request_id {
                    return Err(Sg1000TemporalBusError::WrongRequestId {
                        expected: request.id,
                        received: request_id,
                    });
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
                        return Err(Sg1000TemporalBusError::UnsupportedOperation);
                    }
                };
                let completion = Sg1000BusCompletion {
                    request_id,
                    data,
                    completed_at: dispatch.event.timestamp,
                };
                self.completion = Some(completion);
                Ok(Some(Sg1000TemporalDispatch::BusCompletion(completion)))
            }
            Sg1000TemporalEvent::InternalCycleComplete { continuation_id } => {
                if self.pending_internal_cycle != Some(continuation_id) {
                    return Err(Sg1000TemporalBusError::InvalidState);
                }
                let completion = Sg1000InternalCycleCompletion {
                    continuation_id,
                    completed_at: dispatch.event.timestamp,
                };
                self.internal_completion = Some(completion);
                Ok(Some(Sg1000TemporalDispatch::InternalCycleComplete(completion)))
            }
        }
    }

    /// Consume a latched memory response exactly once.
    pub fn acknowledge_completion(
        &mut self,
        request_id: u64,
    ) -> Result<Sg1000BusCompletion, Sg1000TemporalBusError> {
        let completion = self
            .completion
            .ok_or(Sg1000TemporalBusError::NoPendingRequest)?;
        if completion.request_id != request_id {
            return Err(Sg1000TemporalBusError::WrongRequestId {
                expected: completion.request_id,
                received: request_id,
            });
        }
        self.completion = None;
        self.pending_request = None;
        self.request_started_at = None;
        Ok(completion)
    }

    /// Consume an internal interval after the CPU continuation commits.
    pub fn acknowledge_internal_completion(
        &mut self,
        continuation_id: u64,
    ) -> Result<Sg1000InternalCycleCompletion, Sg1000TemporalBusError> {
        let completion = self
            .internal_completion
            .ok_or(Sg1000TemporalBusError::NoPendingRequest)?;
        if completion.continuation_id != continuation_id
            || self.pending_internal_cycle != Some(continuation_id)
        {
            return Err(Sg1000TemporalBusError::InvalidState);
        }
        self.internal_completion = None;
        self.pending_internal_cycle = None;
        self.internal_cycle_started_at = None;
        Ok(completion)
    }

    pub const fn pending_request(&self) -> Option<Z80BusRequest> {
        self.pending_request
    }

    pub const fn request_started_at(&self) -> Option<MachineTime> {
        self.request_started_at
    }

    pub const fn completion(&self) -> Option<Sg1000BusCompletion> {
        self.completion
    }

    pub const fn internal_cycle_started_at(&self) -> Option<MachineTime> {
        self.internal_cycle_started_at
    }

    pub const fn now(&self) -> MachineTime {
        self.coordinator.now()
    }

    pub const fn master_ticks_per_tstate(&self) -> u64 {
        self.coordinator.master_ticks_per_tstate()
    }

    pub const fn memory(&self) -> &Sg1000Memory {
        &self.memory
    }

    #[cfg(test)]
    pub(crate) fn write_memory_for_test(&mut self, address: u16, value: u8) {
        self.memory.write(address, value);
    }

    /// Export memory, Scheduler and all pending transaction state.
    pub fn export_state(&self) -> Sg1000TemporalBusState {
        Sg1000TemporalBusState {
            format_version: SG1000_TEMPORAL_BUS_STATE_VERSION,
            memory: self.memory.clone(),
            coordinator: self.coordinator.export_state(),
            pending_request: self.pending_request,
            request_started_at: self.request_started_at,
            completion: self.completion,
            pending_internal_cycle: self.pending_internal_cycle,
            internal_cycle_started_at: self.internal_cycle_started_at,
            internal_completion: self.internal_completion,
        }
    }

    /// Restore only if the memory transaction and Scheduler event agree.
    pub fn restore_state(
        state: Sg1000TemporalBusState,
    ) -> Result<Self, Sg1000TemporalBusError> {
        if state.format_version != SG1000_TEMPORAL_BUS_STATE_VERSION {
            return Err(Sg1000TemporalBusError::InvalidState);
        }
        let coordinator = Sg1000TemporalCoordinator::restore_state(state.coordinator.clone())?;
        let pending_events = &state.coordinator.scheduler.event_queue.pending;
        let scale = state.coordinator.master_ticks_per_tstate;
        let now = coordinator.now();
        let last_dispatch = state.coordinator.scheduler.last_dispatch_timestamp;

        match (
            state.pending_request,
            state.request_started_at,
            state.completion,
            state.pending_internal_cycle,
            state.internal_cycle_started_at,
            state.internal_completion,
        ) {
            (None, None, None, None, None, None) if pending_events.is_empty() => {}
            (Some(request), Some(started_at), None, None, None, None)
                if is_supported_memory_operation(request.operation)
                    && pending_events.len() == 1
                    && pending_events[0].payload
                        == (Sg1000TemporalEvent::BusCompletion {
                            request_id: request.id,
                        })
                    && started_at <= now
                    && pending_events[0].timestamp >= now
                    && valid_bus_deadline(
                        started_at,
                        pending_events[0].timestamp,
                        scale,
                        request.base_tstates,
                    ) => {}
            (Some(request), Some(started_at), Some(completion), None, None, None)
                if is_supported_memory_operation(request.operation)
                    && request.id == completion.request_id
                    && pending_events.is_empty()
                    && completion.completed_at == now
                    && last_dispatch == Some(completion.completed_at)
                    && valid_bus_deadline(
                        started_at,
                        completion.completed_at,
                        scale,
                        request.base_tstates,
                    ) => {}
            (None, None, None, Some(continuation_id), Some(started_at), None)
                if pending_events.len() == 1
                    && pending_events[0].payload
                        == (Sg1000TemporalEvent::InternalCycleComplete { continuation_id })
                    && started_at <= now
                    && pending_events[0].timestamp >= now
                    && valid_internal_deadline(started_at, pending_events[0].timestamp, scale) => {}
            (None, None, None, Some(continuation_id), Some(started_at), Some(completion))
                if continuation_id == completion.continuation_id
                    && pending_events.is_empty()
                    && completion.completed_at == now
                    && last_dispatch == Some(completion.completed_at)
                    && valid_internal_deadline(started_at, completion.completed_at, scale) => {}
            _ => return Err(Sg1000TemporalBusError::InvalidState),
        }

        Ok(Self {
            memory: state.memory,
            coordinator,
            pending_request: state.pending_request,
            request_started_at: state.request_started_at,
            completion: state.completion,
            pending_internal_cycle: state.pending_internal_cycle,
            internal_cycle_started_at: state.internal_cycle_started_at,
            internal_completion: state.internal_completion,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Sg1000TemporalBus, Sg1000TemporalBusError, Sg1000TemporalDispatch,
    };
    use crate::MemoryConfig;
    use sega_z80_cpu::{Z80BusOperation, Z80BusRequest};
    use sega_temporal_core::MachineTime;

    fn request(id: u64, operation: Z80BusOperation, address: u16, base_tstates: u8) -> Z80BusRequest {
        Z80BusRequest {
            id,
            operation,
            address,
            base_tstates,
        }
    }

    fn dispatch_write(
        bus: &mut Sg1000TemporalBus,
        id: u64,
        address: u16,
        value: u8,
        wait_tstates: u32,
    ) {
        bus.begin_request(
            request(id, Z80BusOperation::MemoryWrite { value }, address, 3),
            wait_tstates,
        )
        .unwrap();
        let Some(Sg1000TemporalDispatch::BusCompletion(completion)) =
            bus.dispatch_next().unwrap()
        else {
            panic!("expected memory write completion");
        };
        assert_eq!(completion.request_id, id);
        assert_eq!(completion.data, None);
        bus.acknowledge_completion(id).unwrap();
    }

    #[test]
    fn memory_write_is_invisible_until_deadline_then_updates_mirrored_ram_once() {
        let mut bus = Sg1000TemporalBus::new(vec![0x00], MemoryConfig::default(), 2).unwrap();
        let deadline = bus
            .begin_request(
                request(1, Z80BusOperation::MemoryWrite { value: 0xA5 }, 0xC000, 3),
                2,
            )
            .unwrap();
        assert_eq!(deadline, MachineTime::from_ticks(10));
        assert_eq!(bus.memory().read(0xC000), 0);
        assert_eq!(bus.memory().read(0xC400), 0);
        assert_eq!(bus.now(), MachineTime::ZERO);

        let restored = Sg1000TemporalBus::restore_state(bus.export_state()).unwrap();
        let mut restored = restored;
        let Some(Sg1000TemporalDispatch::BusCompletion(completion)) =
            restored.dispatch_next().unwrap()
        else {
            panic!("expected memory write completion");
        };
        assert_eq!(completion.completed_at, deadline);
        assert_eq!(restored.memory().read(0xC000), 0xA5);
        assert_eq!(restored.memory().read(0xC400), 0xA5);
        assert_eq!(restored.acknowledge_completion(1).unwrap(), completion);
        assert_eq!(
            restored.acknowledge_completion(1),
            Err(Sg1000TemporalBusError::NoPendingRequest)
        );
        assert!(restored.dispatch_next().unwrap().is_none());
    }

    #[test]
    fn memory_read_samples_the_value_at_dispatch_deadline() {
        let mut bus = Sg1000TemporalBus::new(vec![0x00], MemoryConfig::default(), 3).unwrap();
        bus.begin_request(
            request(2, Z80BusOperation::MemoryRead, 0xC000, 3),
            1,
        )
        .unwrap();
        bus.write_memory_for_test(0xC000, 0x5A);
        let Some(Sg1000TemporalDispatch::BusCompletion(completion)) = bus.dispatch_next().unwrap()
        else {
            panic!("expected memory read completion");
        };
        assert_eq!(completion.data, Some(0x5A));
        bus.acknowledge_completion(2).unwrap();
    }

    #[test]
    fn rom_window_mirrors_and_ignores_scheduled_writes() {
        let mut bus = Sg1000TemporalBus::new(vec![0x42, 0x99], MemoryConfig::default(), 1).unwrap();
        dispatch_write(&mut bus, 3, 0x0000, 0xFF, 0);
        assert_eq!(bus.memory().read(0x0000), 0x42);
        assert_eq!(bus.memory().read(0x0002), 0x42);
        assert_eq!(bus.memory().read(0xBFFE), 0x42);
        assert_eq!(bus.memory().read(0xBFFF), 0x99);
    }

    #[test]
    fn configured_eight_kib_internal_ram_and_cartridge_ram_keep_their_mirrors() {
        let config = MemoryConfig {
            internal_ram_size: 0x2000,
            cartridge_ram_size: 0x0800,
            cartridge_ram_start: 0x8000,
            cartridge_ram_end: 0xBFFF,
        };
        let mut bus = Sg1000TemporalBus::new(vec![0x00], config, 1).unwrap();
        dispatch_write(&mut bus, 4, 0x8000, 0xA7, 0);
        assert_eq!(bus.memory().read(0x8000), 0xA7);
        assert_eq!(bus.memory().read(0xA000), 0xA7);
        dispatch_write(&mut bus, 5, 0xC000, 0x5C, 0);
        assert_eq!(bus.memory().read(0xC000), 0x5C);
        assert_eq!(bus.memory().read(0xE000), 0x5C);
        dispatch_write(&mut bus, 6, 0xBFFF, 0x3C, 0);
        assert_eq!(bus.memory().read(0xBFFF), 0x3C);
        assert_eq!(bus.memory().read(0x87FF), 0x3C);
    }

    #[test]
    fn completion_latched_snapshot_restore_does_not_repeat_memory_write() {
        let mut bus = Sg1000TemporalBus::new(vec![0x00], MemoryConfig::default(), 2).unwrap();
        bus.begin_request(
            request(6, Z80BusOperation::MemoryWrite { value: 0x33 }, 0xC000, 3),
            0,
        )
        .unwrap();
        let first = bus.dispatch_next().unwrap().unwrap();
        let state = bus.export_state();
        let mut restored = Sg1000TemporalBus::restore_state(state.clone()).unwrap();
        assert_eq!(restored.dispatch_next().unwrap(), Some(first));
        assert_eq!(restored.export_state(), state);
        assert_eq!(restored.memory().read(0xC000), 0x33);
        restored.acknowledge_completion(6).unwrap();
        assert!(restored.dispatch_next().unwrap().is_none());
        assert_eq!(
            restored.acknowledge_completion(6),
            Err(Sg1000TemporalBusError::NoPendingRequest)
        );
    }

    #[test]
    fn internal_cycle_is_latched_and_acknowledged_once() {
        let mut bus = Sg1000TemporalBus::new(vec![0x00], MemoryConfig::default(), 2).unwrap();
        let deadline = bus.schedule_internal_cycle(7, 1).unwrap();
        let Some(Sg1000TemporalDispatch::InternalCycleComplete(completion)) =
            bus.dispatch_next().unwrap()
        else {
            panic!("expected internal cycle completion");
        };
        assert_eq!(completion.completed_at, deadline);
        assert_eq!(
            bus.dispatch_next().unwrap(),
            Some(Sg1000TemporalDispatch::InternalCycleComplete(completion))
        );
        bus.acknowledge_internal_completion(7).unwrap();
        assert!(bus.dispatch_next().unwrap().is_none());
    }

    #[test]
    fn port_requests_are_rejected_without_scheduling() {
        let mut bus = Sg1000TemporalBus::new(vec![0x00], MemoryConfig::default(), 2).unwrap();
        let before = bus.export_state();
        assert_eq!(
            bus.begin_request(request(10, Z80BusOperation::PortRead, 0x00DC, 4), 0),
            Err(Sg1000TemporalBusError::UnsupportedOperation)
        );
        assert_eq!(bus.export_state(), before);
    }

    #[test]
    fn restore_rejects_a_port_operation_in_a_memory_transaction_snapshot() {
        let mut bus = Sg1000TemporalBus::new(vec![0x00], MemoryConfig::default(), 2).unwrap();
        bus.begin_request(request(11, Z80BusOperation::MemoryRead, 0x0000, 4), 0)
            .unwrap();
        let mut state = bus.export_state();
        state.pending_request.as_mut().unwrap().operation = Z80BusOperation::PortRead;
        assert_eq!(
            Sg1000TemporalBus::restore_state(state).err(),
            Some(Sg1000TemporalBusError::InvalidState)
        );
    }

    #[test]
    fn restore_rejects_inconsistent_pending_event_and_request() {
        let mut bus = Sg1000TemporalBus::new(vec![0x00], MemoryConfig::default(), 2).unwrap();
        bus.begin_request(
            request(8, Z80BusOperation::MemoryRead, 0x0000, 3),
            0,
        )
        .unwrap();
        let mut state = bus.export_state();
        state.pending_request.as_mut().unwrap().id = 9;
        assert_eq!(
            Sg1000TemporalBus::restore_state(state).err(),
            Some(Sg1000TemporalBusError::InvalidState)
        );
    }
}
