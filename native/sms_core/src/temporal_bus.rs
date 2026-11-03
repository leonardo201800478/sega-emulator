//! Transactional SMS memory-bus adapter for the Scheduler-driven path.
//!
//! Memory reads and writes become visible only when their scheduled completion
//! event is dispatched. A completion is latched until the CPU acknowledges it,
//! preventing a second dispatch from repeating a write side effect.

use core::fmt;

use crate::{
    CartridgeImage, SmsCartridgeRamFile, SmsCartridgeRamFileError, SmsCoreError, SmsMemory, SmsTemporalCoordinator, SmsTemporalCoordinatorError,
    SmsTemporalCoordinatorState, SmsTemporalEvent,
};
use sega_temporal_core::MachineTime;
use sega_z80_cpu::{Z80BusOperation, Z80BusRequest};

pub const SMS_TEMPORAL_BUS_STATE_VERSION: u16 = 4;

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
    let tstates = elapsed / scale;
    (tstates <= u128::from(u32::MAX)).then_some(tstates)
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
    pub request_started_at: Option<MachineTime>,
    pub completion: Option<SmsBusCompletion>,
    pub pending_internal_cycle: Option<u64>,
    pub internal_cycle_started_at: Option<MachineTime>,
    pub internal_completion: Option<SmsInternalCycleCompletion>,
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
    request_started_at: Option<MachineTime>,
    completion: Option<SmsBusCompletion>,
    pending_internal_cycle: Option<u64>,
    internal_cycle_started_at: Option<MachineTime>,
    internal_completion: Option<SmsInternalCycleCompletion>,
}

impl SmsTemporalBus {
    /// Construct a bus from a cartridge and an explicit master-tick/T-state scale.
    pub fn new(
        rom: impl Into<Vec<u8>>,
        master_ticks_per_tstate: u64,
    ) -> Result<Self, SmsTemporalBusError> {
        Self::new_with_cartridge_ram(rom, None, master_ticks_per_tstate)
    }

    /// Construct a temporal bus with explicitly supplied cartridge SRAM.
    pub fn new_with_cartridge_ram(
        rom: impl Into<Vec<u8>>,
        cartridge_ram: Option<Vec<u8>>,
        master_ticks_per_tstate: u64,
    ) -> Result<Self, SmsTemporalBusError> {
        let cartridge = CartridgeImage::new(rom)?;
        let memory = SmsMemory::with_bios_and_cartridge_ram(cartridge, None, cartridge_ram)?;
        let coordinator = SmsTemporalCoordinator::new(master_ticks_per_tstate)?;
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
            || self.internal_completion.is_some()
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
        let started_at = self.coordinator.now();
        let deadline = self
            .coordinator
            .schedule_internal_cycle(continuation_id, tstates)?;
        self.pending_internal_cycle = Some(continuation_id);
        self.internal_cycle_started_at = Some(started_at);
        Ok(deadline)
    }

    /// Dispatch the next scheduled boundary and apply the memory side effect once.
    ///
    /// A read samples memory at this completion boundary. A write mutates memory
    /// at this completion boundary. The response is latched until acknowledged.
    pub fn dispatch_next(&mut self) -> Result<Option<SmsTemporalDispatch>, SmsTemporalBusError> {
        // Dispatched boundaries remain latched until the CPU accepts them.
        if let Some(completion) = self.completion {
            return Ok(Some(SmsTemporalDispatch::BusCompletion(completion)));
        }
        if let Some(completion) = self.internal_completion {
            return Ok(Some(SmsTemporalDispatch::InternalCycleComplete(completion)));
        }
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
                let completion = SmsInternalCycleCompletion {
                    continuation_id,
                    completed_at: dispatch.event.timestamp,
                };
                self.internal_completion = Some(completion);
                Ok(Some(SmsTemporalDispatch::InternalCycleComplete(completion)))
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
        self.request_started_at = None;
        Ok(completion)
    }

    /// Acknowledge an internal interval only after the CPU continuation commits.
    pub fn acknowledge_internal_completion(
        &mut self,
        continuation_id: u64,
    ) -> Result<SmsInternalCycleCompletion, SmsTemporalBusError> {
        let completion = self
            .internal_completion
            .ok_or(SmsTemporalBusError::NoPendingRequest)?;
        if completion.continuation_id != continuation_id
            || self.pending_internal_cycle != Some(continuation_id)
        {
            return Err(SmsTemporalBusError::InvalidState);
        }
        self.internal_completion = None;
        self.pending_internal_cycle = None;
        self.internal_cycle_started_at = None;
        Ok(completion)
    }

    /// Return the currently outstanding CPU request, if any.
    pub const fn pending_request(&self) -> Option<Z80BusRequest> {
        self.pending_request
    }

    /// Timestamp at which the outstanding bus request was scheduled.
    pub const fn request_started_at(&self) -> Option<MachineTime> {
        self.request_started_at
    }

    /// Timestamp at which the outstanding internal interval was scheduled.
    pub const fn internal_cycle_started_at(&self) -> Option<MachineTime> {
        self.internal_cycle_started_at
    }

    /// Return the latched bus completion, if dispatch has reached its deadline.
    pub const fn completion(&self) -> Option<SmsBusCompletion> {
        self.completion
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

    /// Restore the configured mapper SRAM from a validated .srm image.
    pub fn load_cartridge_ram_file(
        &mut self,
        save_file: &SmsCartridgeRamFile,
    ) -> Result<bool, SmsCartridgeRamFileError> {
        save_file.load_into(&mut self.memory)
    }

    /// Persist the mapper SRAM without performing I/O on CPU bus transactions.
    pub fn save_cartridge_ram_file(
        &self,
        save_file: &SmsCartridgeRamFile,
    ) -> Result<(), SmsCartridgeRamFileError> {
        save_file.save_from_memory(&self.memory)
    }

    /// Export memory, scheduler, and in-flight response state.
    pub fn export_state(&self) -> SmsTemporalBusState {
        SmsTemporalBusState {
            format_version: SMS_TEMPORAL_BUS_STATE_VERSION,
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

    /// Restore only when the scheduler event and bus transaction agree.
    pub fn restore_state(state: SmsTemporalBusState) -> Result<Self, SmsTemporalBusError> {
        if state.format_version != SMS_TEMPORAL_BUS_STATE_VERSION {
            return Err(SmsTemporalBusError::InvalidState);
        }
        let coordinator = SmsTemporalCoordinator::restore_state(state.coordinator.clone())?;
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
                if pending_events.len() == 1
                    && pending_events[0].payload
                        == (SmsTemporalEvent::BusCompletion {
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
                if request.id == completion.request_id
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
                        == (SmsTemporalEvent::InternalCycleComplete { continuation_id })
                    && started_at <= now
                    && pending_events[0].timestamp >= now
                    && valid_internal_deadline(started_at, pending_events[0].timestamp, scale) => {}
            (None, None, None, Some(continuation_id), Some(started_at), Some(completion))
                if continuation_id == completion.continuation_id
                    && pending_events.is_empty()
                    && completion.completed_at == now
                    && last_dispatch == Some(completion.completed_at)
                    && valid_internal_deadline(started_at, completion.completed_at, scale) => {}
            _ => return Err(SmsTemporalBusError::InvalidState),
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
    use super::{SmsTemporalBus, SmsTemporalBusError, SmsTemporalDispatch};
    use sega_temporal_core::MachineTime;
    use sega_z80_cpu::{Z80BusOperation, Z80BusRequest};

    fn request(id: u64, operation: Z80BusOperation, address: u16, tstates: u8) -> Z80BusRequest {
        Z80BusRequest {
            id,
            operation,
            address,
            base_tstates: tstates,
        }
    }

    fn patterned_rom(bank_count: usize) -> Vec<u8> {
        let mut rom = vec![0; bank_count * 0x4000];
        for bank in 0..bank_count {
            let start = bank * 0x4000;
            rom[start..start + 0x4000].fill(bank as u8);
        }
        rom
    }

    #[test]
    fn sega_mapper_writes_take_effect_at_scheduler_deadline_and_reads_see_selected_bank() {
        let mut bus = SmsTemporalBus::new(patterned_rom(4), 3).unwrap();
        let cases = [
            (10, 0xFFFD, 0x0400, 0, 0xDFFD),
            (11, 0xFFFE, 0x4000, 1, 0xDFFE),
            (12, 0xFFFF, 0x8000, 2, 0xDFFF),
        ];

        for (request_id, register, banked_address, slot, ram_mirror) in cases {
            let before = bus.memory().selected_banks();
            assert_eq!(bus.memory().read(banked_address), before[slot]);

            let write_request = request(
                request_id,
                Z80BusOperation::MemoryWrite { value: 3 },
                register,
                3,
            );
            let started_at = bus.now();
            let deadline = bus.begin_request(write_request, 2).unwrap();
            assert_eq!(
                deadline.checked_duration_since(started_at),
                Some(15),
                "three nominal T-states plus two fixture WAIT T-states at three master ticks each"
            );

            // Snapshot/restore before dispatch must not expose the mapper write.
            let mut restored = SmsTemporalBus::restore_state(bus.export_state()).unwrap();
            assert_eq!(restored.memory().selected_banks(), before);
            assert_eq!(restored.memory().read(banked_address), before[slot]);
            assert_eq!(
                restored.memory().read(ram_mirror),
                0,
                "the mirrored work-RAM byte must remain unchanged until the mapper write deadline"
            );

            let Some(SmsTemporalDispatch::BusCompletion(completion)) =
                restored.dispatch_next().unwrap()
            else {
                panic!("expected mapper-register write completion");
            };
            assert_eq!(completion.request_id, request_id);
            assert_eq!(completion.completed_at, deadline);
            assert_eq!(restored.memory().selected_banks()[slot], 3);
            assert_eq!(
                restored.memory().mapper_register_write_count_for_test(),
                request_id - 9,
                "each Sega mapper register write is applied exactly once"
            );
            assert_eq!(restored.memory().read(ram_mirror), 3);
            assert_eq!(
                restored.memory().read(0x03FF),
                0,
                "the Sega mapper's first 1 KiB remains fixed to bank zero"
            );
            assert_eq!(restored.memory().read(banked_address), 3);

            restored.acknowledge_completion(request_id).unwrap();
            let read_request = request(
                request_id + 100,
                Z80BusOperation::MemoryRead,
                banked_address,
                3,
            );
            restored.begin_request(read_request, 0).unwrap();
            let Some(SmsTemporalDispatch::BusCompletion(read_completion)) =
                restored.dispatch_next().unwrap()
            else {
                panic!("expected selected-bank read completion");
            };
            assert_eq!(read_completion.data, Some(3));
            restored.acknowledge_completion(read_request.id).unwrap();

            // Continue the next register case from the restored timeline.
            bus = restored;
        }
    }

    #[test]
    fn mapper_write_completion_remains_latched_across_restore_without_reapplying_effect() {
        let mut bus = SmsTemporalBus::new(patterned_rom(4), 3).unwrap();
        let request = request(20, Z80BusOperation::MemoryWrite { value: 3 }, 0xFFFD, 3);
        bus.begin_request(request, 1).unwrap();
        let first = bus.dispatch_next().unwrap().unwrap();
        let SmsTemporalDispatch::BusCompletion(completion) = first else {
            panic!("expected mapper-register write completion");
        };
        assert_eq!(bus.memory().selected_banks(), [3, 1, 2]);
        assert_eq!(bus.memory().read(0x0400), 3);
        assert_eq!(bus.memory().mapper_register_write_count_for_test(), 1);

        let mut restored = SmsTemporalBus::restore_state(bus.export_state()).unwrap();
        let latched_state = restored.export_state();
        assert_eq!(restored.dispatch_next().unwrap(), Some(first));
        assert_eq!(
            restored.export_state(),
            latched_state,
            "a latched completion must not dispatch the scheduler event or mutate memory twice"
        );
        assert_eq!(restored.memory().selected_banks(), [3, 1, 2]);
        assert_eq!(restored.memory().read(0x0400), 3);
        assert_eq!(restored.memory().mapper_register_write_count_for_test(), 1);
        assert_eq!(restored.acknowledge_completion(20).unwrap(), completion);
        assert_eq!(restored.dispatch_next().unwrap(), None);
        assert_eq!(restored.memory().selected_banks(), [3, 1, 2]);
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
        let SmsTemporalDispatch::BusCompletion(completion) = bus.dispatch_next().unwrap().unwrap()
        else {
            panic!("expected bus completion");
        };
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
        let SmsTemporalDispatch::BusCompletion(completion) = bus.dispatch_next().unwrap().unwrap()
        else {
            panic!("expected bus completion");
        };
        let mut restored = SmsTemporalBus::restore_state(bus.export_state()).unwrap();
        assert_eq!(restored.memory().read(0xC002), 0x99);
        assert_eq!(restored.acknowledge_completion(4).unwrap(), completion);
        assert_eq!(restored.memory().read(0xC002), 0x99);
    }

    #[test]
    fn snapshot_restore_preserves_cartridge_sram_and_mapper_selection() {
        let mut bus =
            SmsTemporalBus::new_with_cartridge_ram(patterned_rom(4), Some(vec![0; 0x8000]), 3)
                .unwrap();
        bus.write_memory_for_test(0xFFFC, 0x0C);
        bus.write_memory_for_test(0x8000, 0xA7);
        let restored = SmsTemporalBus::restore_state(bus.export_state()).unwrap();
        assert_eq!(restored.memory().mapper_control(), 0x0C);
        assert_eq!(restored.memory().read(0x8000), 0xA7);
        assert_eq!(restored.memory().cartridge_ram().unwrap().len(), 0x8000);
    }

    #[test]
    fn cartridge_sram_write_is_applied_at_the_mapper_bus_deadline_and_survives_restore() {
        let mut bus =
            SmsTemporalBus::new_with_cartridge_ram(patterned_rom(4), Some(vec![0; 0x2000]), 3)
                .unwrap();
        let mapper_enable = request(31, Z80BusOperation::MemoryWrite { value: 0x08 }, 0xFFFC, 3);
        bus.begin_request(mapper_enable, 0).unwrap();
        bus.dispatch_next().unwrap().unwrap();
        bus.acknowledge_completion(31).unwrap();

        let ram_write = request(32, Z80BusOperation::MemoryWrite { value: 0xA6 }, 0x8000, 3);
        bus.begin_request(ram_write, 0).unwrap();
        assert_eq!(
            bus.memory().read(0x8000),
            0,
            "enabled SRAM remains unchanged before the scheduled write dispatch"
        );
        let completion = bus.dispatch_next().unwrap().unwrap();
        let mut restored = SmsTemporalBus::restore_state(bus.export_state()).unwrap();
        assert_eq!(restored.memory().read(0x8000), 0xA6);
        assert_eq!(restored.dispatch_next().unwrap(), Some(completion));
        restored.acknowledge_completion(32).unwrap();
    }

    #[test]
    fn restore_rejects_bus_deadlines_shorter_than_nominal_cycle() {
        let mut bus = SmsTemporalBus::new(vec![0; 0x4000], 3).unwrap();
        let request = request(1, Z80BusOperation::OpcodeFetch, 0, 4);
        bus.begin_request(request, 0).unwrap();

        let mut state = bus.export_state();
        state.coordinator.scheduler.event_queue.pending[0].timestamp = MachineTime::from_ticks(3);
        assert_eq!(
            SmsTemporalBus::restore_state(state).err(),
            Some(SmsTemporalBusError::InvalidState)
        );
    }

    #[test]
    fn restore_rejects_bus_deadlines_not_aligned_to_tstate_scale() {
        let mut bus = SmsTemporalBus::new(vec![0; 0x4000], 3).unwrap();
        let request = request(1, Z80BusOperation::OpcodeFetch, 0, 4);
        bus.begin_request(request, 0).unwrap();

        let mut state = bus.export_state();
        state.coordinator.scheduler.event_queue.pending[0].timestamp = MachineTime::from_ticks(13);
        assert_eq!(
            SmsTemporalBus::restore_state(state).err(),
            Some(SmsTemporalBusError::InvalidState)
        );
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
    fn internal_completion_is_latched_until_acknowledged_and_survives_restore() {
        let mut bus = SmsTemporalBus::new(vec![0; 0x4000], 3).unwrap();
        let deadline = bus.schedule_internal_cycle(77, 1).unwrap();
        let first = bus.dispatch_next().unwrap().unwrap();
        assert_eq!(
            first,
            SmsTemporalDispatch::InternalCycleComplete(super::SmsInternalCycleCompletion {
                continuation_id: 77,
                completed_at: deadline,
            })
        );
        assert_eq!(bus.dispatch_next().unwrap(), Some(first));

        let mut restored = SmsTemporalBus::restore_state(bus.export_state()).unwrap();
        assert_eq!(restored.dispatch_next().unwrap(), Some(first));
        assert_eq!(
            restored.acknowledge_internal_completion(77).unwrap(),
            match first {
                SmsTemporalDispatch::InternalCycleComplete(completion) => completion,
                _ => unreachable!(),
            }
        );
        assert_eq!(restored.dispatch_next().unwrap(), None);
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
