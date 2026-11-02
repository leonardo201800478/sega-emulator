//! First scheduler-driven SMS Z80/memory vertical slice.
//!
//! This composition uses the resumable Z80 subset, the mapper-backed SMS
//! memory adapter, and one machine scheduler. It intentionally excludes the
//! legacy scanline VDP, PSG, ports, interrupts, DMA and physical WAIT wiring.

use core::fmt;

use crate::{SmsTemporalBus, SmsTemporalBusError, SmsTemporalBusState, SmsTemporalDispatch};
use sega_temporal_core::{
    BusContractError, BusCycleKind, BusDirection, BusOperationResult, BusPhase, BusTransaction,
    MCycleIndex, MachineTime, TStateIndex,
};
use sega_z80_cpu::{
    Z80BusOperation, Z80MicroSequencer, Z80MicroSequencerError, Z80MicroSequencerPhase,
    Z80MicroSequencerState, Z80MicroStep,
};

pub const SMS_TEMPORAL_MACHINE_STATE_VERSION: u16 = 6;

/// Runtime event trace emitted from actual Scheduler/bus dispatch boundaries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmsTemporalTraceEvent {
    BusCompleted {
        timestamp: MachineTime,
        request_id: u64,
        operation: Z80BusOperation,
        address: u16,
        base_tstates: u8,
        wait_tstates: u32,
        data: Option<u8>,
    },
    InternalCycleCompleted {
        timestamp: MachineTime,
        continuation_id: u64,
        tstates: u32,
    },
}

/// Versioned state spanning the CPU continuation and SMS memory/scheduler.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmsTemporalMachineState {
    pub format_version: u16,
    pub cpu: Z80MicroSequencerState,
    /// Absolute machine timestamp at which the active instruction began.
    pub instruction_started_at: Option<MachineTime>,
    pub bus: SmsTemporalBusState,
    pub next_internal_cycle_id: u64,
    pub pending_internal_cycle_id: Option<u64>,
    pub pending_internal_tstates: Option<u32>,
}

fn elapsed_tstates(
    started_at: MachineTime,
    boundary: MachineTime,
    master_ticks_per_tstate: u64,
) -> Option<u32> {
    let scale = u128::from(master_ticks_per_tstate);
    if scale == 0 {
        return None;
    }
    let elapsed = boundary.checked_duration_since(started_at)?;
    if elapsed % scale != 0 {
        return None;
    }
    u32::try_from(elapsed / scale).ok()
}

fn instruction_timeline_is_valid(state: &SmsTemporalMachineState) -> bool {
    if matches!(state.cpu.phase, Z80MicroSequencerPhase::Ready) {
        return state.instruction_started_at.is_none();
    }
    let Some(instruction_started_at) = state.instruction_started_at else {
        return false;
    };
    let cpu_tstates = state.cpu.instruction_tstates;
    let (boundary, expected_tstates) = match (
        state.bus.pending_request,
        state.bus.completion,
        state.bus.pending_internal_cycle,
        state.bus.internal_completion,
    ) {
        (Some(_), None, None, None) => {
            let Some(request_started_at) = state.bus.request_started_at else {
                return false;
            };
            (request_started_at, cpu_tstates)
        }
        (Some(request), Some(completion), None, None) => {
            if request.id != completion.request_id {
                return false;
            }
            let Some(current_cycle_tstates) =
                u32::from(request.base_tstates).checked_add(state.cpu.handshake.wait_tstates)
            else {
                return false;
            };
            let Some(expected_tstates) = cpu_tstates.checked_add(current_cycle_tstates) else {
                return false;
            };
            (completion.completed_at, expected_tstates)
        }
        (None, None, Some(_), None) => {
            let Some(internal_started_at) = state.bus.internal_cycle_started_at else {
                return false;
            };
            (internal_started_at, cpu_tstates)
        }
        (None, None, Some(continuation_id), Some(completion)) => {
            if continuation_id != completion.continuation_id {
                return false;
            }
            let Some(internal_tstates) = state.pending_internal_tstates else {
                return false;
            };
            let Some(expected_tstates) = cpu_tstates.checked_add(internal_tstates) else {
                return false;
            };
            (completion.completed_at, expected_tstates)
        }
        (None, None, None, None)
            if matches!(
                state.cpu.phase,
                Z80MicroSequencerPhase::MemoryIncDecInternal { .. }
            ) =>
        {
            (state.bus.coordinator.scheduler.now, cpu_tstates)
        }
        _ => return false,
    };
    elapsed_tstates(
        instruction_started_at,
        boundary,
        state.bus.coordinator.master_ticks_per_tstate,
    ) == Some(expected_tstates)
}

/// Errors from the first integrated SMS temporal execution path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SmsTemporalMachineError {
    Bus(SmsTemporalBusError),
    Cpu(Z80MicroSequencerError),
    InvalidState,
    NoScheduledEvent,
    InternalCycleIdExhausted,
}

impl fmt::Display for SmsTemporalMachineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bus(error) => write!(f, "SMS temporal bus failed: {error}"),
            Self::Cpu(error) => write!(f, "SMS resumable Z80 failed: {error:?}"),
            Self::InvalidState => f.write_str("SMS temporal machine snapshot is inconsistent"),
            Self::NoScheduledEvent => f.write_str("SMS temporal machine has no event to dispatch"),
            Self::InternalCycleIdExhausted => {
                f.write_str("SMS internal-cycle continuation ID exhausted")
            }
        }
    }
}

impl std::error::Error for SmsTemporalMachineError {}

impl From<SmsTemporalBusError> for SmsTemporalMachineError {
    fn from(error: SmsTemporalBusError) -> Self {
        Self::Bus(error)
    }
}

impl From<Z80MicroSequencerError> for SmsTemporalMachineError {
    fn from(error: Z80MicroSequencerError) -> Self {
        Self::Cpu(error)
    }
}

/// SMS Z80 + mapper memory + single scheduler timeline.
#[derive(Debug, Clone)]
pub struct SmsTemporalMachine {
    cpu: Z80MicroSequencer,
    bus: SmsTemporalBus,
    instruction_started_at: Option<MachineTime>,
    next_internal_cycle_id: u64,
    pending_internal_cycle_id: Option<u64>,
    pending_internal_tstates: Option<u32>,
    trace_enabled: bool,
    trace_events: Vec<SmsTemporalTraceEvent>,
}

impl SmsTemporalMachine {
    /// Construct the temporal slice from a cartridge and explicit time scale.
    pub fn new(
        rom: impl Into<Vec<u8>>,
        master_ticks_per_tstate: u64,
    ) -> Result<Self, SmsTemporalMachineError> {
        Ok(Self {
            cpu: Z80MicroSequencer::default(),
            bus: SmsTemporalBus::new(rom, master_ticks_per_tstate)?,
            instruction_started_at: None,
            next_internal_cycle_id: 0,
            pending_internal_cycle_id: None,
            pending_internal_tstates: None,
            trace_enabled: false,
            trace_events: Vec::new(),
        })
    }

    /// Start the next instruction and expose its first bus request.
    pub fn begin_instruction(&mut self) -> Result<Z80MicroStep, SmsTemporalMachineError> {
        let step = self.cpu.begin_instruction()?;
        self.instruction_started_at = Some(self.bus.now());
        Ok(step)
    }

    /// Describe the pending CPU bus request using the shared temporal contract.
    ///
    /// The timestamp is the scheduler's current machine time (transaction start);
    /// the CPU remains the sole owner of continuation and request identity.
    pub fn pending_bus_transaction(&self) -> Result<Option<BusTransaction>, BusContractError> {
        let Some(request) = self.cpu.pending_request() else {
            return Ok(None);
        };
        let state = self.cpu.export_state();
        let (kind, direction, data) = match request.operation {
            Z80BusOperation::OpcodeFetch => (BusCycleKind::OpcodeFetch, BusDirection::Read, None),
            Z80BusOperation::MemoryRead => (BusCycleKind::MemoryRead, BusDirection::Read, None),
            Z80BusOperation::MemoryWrite { value } => {
                (BusCycleKind::MemoryWrite, BusDirection::Write, Some(value))
            }
            Z80BusOperation::PortRead => (BusCycleKind::IoRead, BusDirection::Read, None),
            Z80BusOperation::PortWrite { value } => {
                (BusCycleKind::IoWrite, BusDirection::Write, Some(value))
            }
        };
        let phase = if self.bus.completion().is_some() {
            BusPhase::Complete
        } else {
            BusPhase::Setup
        };
        // BusTransaction.timestamp is the request/start timestamp in both
        // phases. The completion boundary is exposed separately as a result.
        let timestamp = self
            .bus
            .request_started_at()
            .unwrap_or_else(|| self.bus.now());
        BusTransaction::new(
            timestamp,
            MCycleIndex::new(state.mcycle_index),
            TStateIndex::new(state.tstate_index),
            phase,
            kind,
            direction,
            Some(request.address),
            data,
            u16::from(request.base_tstates),
        )
        .map(Some)
    }

    /// Return the completed result for the currently latched bus transaction.
    ///
    /// The request timestamp remains on BusTransaction; the result carries
    /// the distinct completion timestamp required by the temporal contract.
    pub fn pending_bus_operation_result(
        &self,
    ) -> Option<BusOperationResult<Option<u8>, SmsTemporalBusError>> {
        self.bus
            .completion()
            .map(|completion| BusOperationResult::Completed {
                value: completion.data,
                completed_at: completion.completed_at,
            })
    }

    /// Describe the currently exposed CPU-internal interval using the shared contract.
    pub fn pending_internal_transaction(&self) -> Result<Option<BusTransaction>, BusContractError> {
        let state = self.cpu.export_state();
        if !matches!(
            state.phase,
            Z80MicroSequencerPhase::MemoryIncDecInternal { .. }
        ) {
            return Ok(None);
        }
        let timestamp = self
            .bus
            .internal_cycle_started_at()
            .unwrap_or_else(|| self.bus.now());
        BusTransaction::new(
            timestamp,
            MCycleIndex::new(state.mcycle_index),
            TStateIndex::new(state.tstate_index),
            BusPhase::Internal,
            BusCycleKind::Internal,
            BusDirection::None,
            None,
            None,
            1,
        )
        .map(Some)
    }
    /// Schedule the CPU's current pending bus request with fixture WAIT states.
    pub fn schedule_pending_bus(
        &mut self,
        fixture_wait_tstates: u32,
    ) -> Result<MachineTime, SmsTemporalMachineError> {
        let request = self
            .cpu
            .pending_request()
            .ok_or(SmsTemporalMachineError::InvalidState)?;
        if self.bus.pending_request().is_some() {
            return Err(SmsTemporalMachineError::InvalidState);
        }
        let cpu_before = self.cpu.export_state();
        self.cpu.insert_wait_tstates(fixture_wait_tstates)?;
        match self.bus.begin_request(request, fixture_wait_tstates) {
            Ok(deadline) => Ok(deadline),
            Err(error) => {
                self.cpu = Z80MicroSequencer::restore_state(cpu_before)?;
                Err(error.into())
            }
        }
    }

    /// Schedule the current exposed internal CPU interval.
    pub fn schedule_internal_cycle(
        &mut self,
        tstates: u32,
    ) -> Result<MachineTime, SmsTemporalMachineError> {
        let cpu_state = self.cpu.export_state();
        if !matches!(
            cpu_state.phase,
            Z80MicroSequencerPhase::MemoryIncDecInternal { .. }
        ) {
            return Err(SmsTemporalMachineError::InvalidState);
        }
        if self.pending_internal_cycle_id.is_some() || tstates != 1 {
            // The current resumable slice exposes exactly one internal T-state
            // for INC/DEC (HL). Reject caller-supplied durations that would make
            // scheduler time diverge from the CPU's checked instruction counter.
            return Err(SmsTemporalMachineError::InvalidState);
        }
        if cpu_state.instruction_tstates == u32::MAX {
            return Err(SmsTemporalMachineError::Cpu(
                Z80MicroSequencerError::InstructionTStateOverflow,
            ));
        }
        if cpu_state.handshake.next_request_id == u64::MAX {
            return Err(SmsTemporalMachineError::Cpu(
                Z80MicroSequencerError::Handshake(
                    sega_z80_cpu::Z80BusHandshakeError::RequestIdExhausted,
                ),
            ));
        }
        let continuation_id = self.next_internal_cycle_id;
        let next_internal_cycle_id = continuation_id
            .checked_add(1)
            .ok_or(SmsTemporalMachineError::InternalCycleIdExhausted)?;
        let deadline = self.bus.schedule_internal_cycle(continuation_id, tstates)?;
        self.next_internal_cycle_id = next_internal_cycle_id;
        self.pending_internal_cycle_id = Some(continuation_id);
        self.pending_internal_tstates = Some(tstates);
        Ok(deadline)
    }

    /// Dispatch one event and resume the matching CPU continuation.
    pub fn dispatch_next(&mut self) -> Result<Z80MicroStep, SmsTemporalMachineError> {
        let dispatch = self
            .bus
            .dispatch_next()?
            .ok_or(SmsTemporalMachineError::NoScheduledEvent)?;
        match dispatch {
            SmsTemporalDispatch::BusCompletion(completion) => {
                let trace_data = if self.trace_enabled {
                    let request = self
                        .bus
                        .pending_request()
                        .ok_or(SmsTemporalMachineError::InvalidState)?;
                    Some((request, self.cpu.export_state().handshake.wait_tstates))
                } else {
                    None
                };

                // Validate and apply the CPU continuation on a clone before
                // consuming the latched bus completion. If CPU completion fails
                // (for example, checked T-state overflow), the bus response and
                // its already-applied memory effect remain pending for restore/retry.
                let mut next_cpu = self.cpu.clone();
                let step = next_cpu.complete_bus_cycle(completion.request_id, completion.data)?;
                self.bus.acknowledge_completion(completion.request_id)?;
                self.cpu = next_cpu;
                if matches!(step, Z80MicroStep::InstructionRetired { .. }) {
                    self.instruction_started_at = None;
                }

                if let Some((request, wait_tstates)) = trace_data {
                    self.trace_events.push(SmsTemporalTraceEvent::BusCompleted {
                        timestamp: completion.completed_at,
                        request_id: request.id,
                        operation: request.operation,
                        address: request.address,
                        base_tstates: request.base_tstates,
                        wait_tstates,
                        data: completion.data,
                    });
                }
                Ok(step)
            }
            SmsTemporalDispatch::InternalCycleComplete(completion) => {
                if self.pending_internal_cycle_id != Some(completion.continuation_id) {
                    return Err(SmsTemporalMachineError::InvalidState);
                }
                let tstates = self
                    .pending_internal_tstates
                    .ok_or(SmsTemporalMachineError::InvalidState)?;

                // Prepare the CPU transition before acknowledging the scheduler
                // event. On failure, both the bus completion and continuation
                // metadata remain available for snapshot/restore/retry.
                let mut next_cpu = self.cpu.clone();
                let step = next_cpu.complete_internal_cycle()?;
                self.bus
                    .acknowledge_internal_completion(completion.continuation_id)?;

                self.cpu = next_cpu;
                self.pending_internal_tstates = None;
                self.pending_internal_cycle_id = None;
                if self.trace_enabled {
                    self.trace_events
                        .push(SmsTemporalTraceEvent::InternalCycleCompleted {
                            timestamp: completion.completed_at,
                            continuation_id: completion.continuation_id,
                            tstates,
                        });
                }
                Ok(step)
            }
        }
    }

    /// Run the next instruction to retirement, using deterministic fixture WAIT.
    pub fn step_instruction(
        &mut self,
        fixture_wait_tstates: u32,
    ) -> Result<Z80MicroStep, SmsTemporalMachineError> {
        let mut step = self.begin_instruction()?;
        loop {
            step = match step {
                Z80MicroStep::BusRequest(_) => {
                    self.schedule_pending_bus(fixture_wait_tstates)?;
                    self.dispatch_next()?
                }
                Z80MicroStep::InternalCycle { tstates } => {
                    self.schedule_internal_cycle(tstates)?;
                    self.dispatch_next()?
                }
                retired @ Z80MicroStep::InstructionRetired { .. } => return Ok(retired),
            };
        }
    }

    /// Resume an already active instruction from its current safe point.
    pub fn resume_instruction(
        &mut self,
        fixture_wait_tstates: u32,
    ) -> Result<Z80MicroStep, SmsTemporalMachineError> {
        let mut step = if let Some(request) = self.cpu.pending_request() {
            Z80MicroStep::BusRequest(request)
        } else if matches!(
            self.cpu.export_state().phase,
            Z80MicroSequencerPhase::MemoryIncDecInternal { .. }
        ) {
            Z80MicroStep::InternalCycle { tstates: 1 }
        } else {
            return Err(SmsTemporalMachineError::InvalidState);
        };
        loop {
            step = match step {
                Z80MicroStep::BusRequest(_) => {
                    if self.bus.pending_request().is_none() {
                        self.schedule_pending_bus(fixture_wait_tstates)?;
                    }
                    self.dispatch_next()?
                }
                Z80MicroStep::InternalCycle { tstates } => {
                    if self.pending_internal_cycle_id.is_none() {
                        self.schedule_internal_cycle(tstates)?;
                    }
                    self.dispatch_next()?
                }
                retired @ Z80MicroStep::InstructionRetired { .. } => return Ok(retired),
            };
        }
    }

    /// Enable or disable allocation-backed runtime trace capture.
    pub fn set_trace_enabled(&mut self, enabled: bool) {
        self.trace_enabled = enabled;
        if !enabled {
            self.trace_events.clear();
        }
    }

    /// Drain trace events captured from actual scheduler dispatches.
    pub fn take_trace(&mut self) -> Vec<SmsTemporalTraceEvent> {
        core::mem::take(&mut self.trace_events)
    }

    /// Current CPU state (read-only).
    pub const fn cpu(&self) -> &Z80MicroSequencer {
        &self.cpu
    }

    /// Mutable CPU access for initial register setup and deterministic tests.
    pub fn cpu_mut(&mut self) -> &mut Z80MicroSequencer {
        &mut self.cpu
    }

    /// Current mapper-backed memory (read-only).
    pub const fn memory(&self) -> &crate::SmsMemory {
        self.bus.memory()
    }

    /// Current absolute machine time.
    pub const fn now(&self) -> MachineTime {
        self.bus.now()
    }

    /// Export CPU, pending transactions, memory and scheduler state.
    pub fn export_state(&self) -> SmsTemporalMachineState {
        SmsTemporalMachineState {
            format_version: SMS_TEMPORAL_MACHINE_STATE_VERSION,
            cpu: self.cpu.export_state(),
            instruction_started_at: self.instruction_started_at,
            bus: self.bus.export_state(),
            next_internal_cycle_id: self.next_internal_cycle_id,
            pending_internal_cycle_id: self.pending_internal_cycle_id,
            pending_internal_tstates: self.pending_internal_tstates,
        }
    }

    /// Restore the complete first vertical slice after cross-validating CPU and bus state.
    pub fn restore_state(state: SmsTemporalMachineState) -> Result<Self, SmsTemporalMachineError> {
        if state.format_version != SMS_TEMPORAL_MACHINE_STATE_VERSION {
            return Err(SmsTemporalMachineError::InvalidState);
        }
        let cpu = Z80MicroSequencer::restore_state(state.cpu.clone())?;
        let bus = SmsTemporalBus::restore_state(state.bus.clone())?;
        if cpu.pending_request() != state.bus.pending_request
            || state.pending_internal_cycle_id != state.bus.pending_internal_cycle
            || state.pending_internal_cycle_id.is_some() != state.pending_internal_tstates.is_some()
            || state.pending_internal_tstates == Some(0)
            || state
                .pending_internal_cycle_id
                .is_some_and(|id| id >= state.next_internal_cycle_id)
        {
            return Err(SmsTemporalMachineError::InvalidState);
        }
        if state.pending_internal_cycle_id.is_some()
            && !matches!(
                state.cpu.phase,
                Z80MicroSequencerPhase::MemoryIncDecInternal { .. }
            )
        {
            return Err(SmsTemporalMachineError::InvalidState);
        }
        // A restored internal-cycle event must still be completable by the CPU.
        // These values are rejected by schedule_internal_cycle() before creating
        // the event; accepting them from a snapshot would create an unrecoverable
        // continuation even though the Scheduler event is already pending.
        if state.pending_internal_cycle_id.is_some()
            && (state.cpu.instruction_tstates == u32::MAX
                || state.cpu.handshake.next_request_id == u64::MAX)
        {
            return Err(SmsTemporalMachineError::InvalidState);
        }
        if let Some(request) = state.bus.pending_request {
            let started_at = state
                .bus
                .request_started_at
                .ok_or(SmsTemporalMachineError::InvalidState)?;
            let completed_at = state
                .bus
                .completion
                .map(|completion| completion.completed_at)
                .or_else(|| {
                    state
                        .bus
                        .coordinator
                        .scheduler
                        .event_queue
                        .pending
                        .first()
                        .map(|event| event.timestamp)
                })
                .ok_or(SmsTemporalMachineError::InvalidState)?;
            let scale = u128::from(state.bus.coordinator.master_ticks_per_tstate);
            let elapsed = completed_at
                .checked_duration_since(started_at)
                .ok_or(SmsTemporalMachineError::InvalidState)?;
            if scale == 0 || elapsed % scale != 0 {
                return Err(SmsTemporalMachineError::InvalidState);
            }
            let total_tstates = elapsed / scale;
            let wait_tstates = total_tstates
                .checked_sub(u128::from(request.base_tstates))
                .ok_or(SmsTemporalMachineError::InvalidState)?;
            if wait_tstates != u128::from(state.cpu.handshake.wait_tstates) {
                return Err(SmsTemporalMachineError::InvalidState);
            }
        }
        if state.pending_internal_cycle_id.is_some() {
            if state.pending_internal_tstates != Some(1) {
                return Err(SmsTemporalMachineError::InvalidState);
            }
            let started_at = state
                .bus
                .internal_cycle_started_at
                .ok_or(SmsTemporalMachineError::InvalidState)?;
            let completed_at = state
                .bus
                .internal_completion
                .map(|completion| completion.completed_at)
                .or_else(|| {
                    state
                        .bus
                        .coordinator
                        .scheduler
                        .event_queue
                        .pending
                        .first()
                        .map(|event| event.timestamp)
                })
                .ok_or(SmsTemporalMachineError::InvalidState)?;
            let scale = u128::from(state.bus.coordinator.master_ticks_per_tstate);
            let expected = started_at
                .checked_add(scale)
                .ok_or(SmsTemporalMachineError::InvalidState)?;
            if completed_at != expected {
                return Err(SmsTemporalMachineError::InvalidState);
            }
        }
        if !instruction_timeline_is_valid(&state) {
            return Err(SmsTemporalMachineError::InvalidState);
        }
        Ok(Self {
            cpu,
            bus,
            instruction_started_at: state.instruction_started_at,
            next_internal_cycle_id: state.next_internal_cycle_id,
            pending_internal_cycle_id: state.pending_internal_cycle_id,
            pending_internal_tstates: state.pending_internal_tstates,
            trace_enabled: false,
            trace_events: Vec::new(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{SmsTemporalBus, SmsTemporalMachine, SmsTemporalMachineError};
    use sega_temporal_core::BusOperationResult;
    use sega_temporal_core::{
        BusCycleKind, BusDirection, BusPhase, MCycleIndex, MachineTime, TStateIndex,
    };
    use sega_z80_cpu::{
        Z80MicroSequencer, Z80MicroSequencerError, Z80MicroSequencerPhase, Z80MicroStep,
    };

    fn retired(step: Z80MicroStep, opcode: u8, tstates: u32) {
        assert_eq!(step, Z80MicroStep::InstructionRetired { opcode, tstates });
    }

    #[test]
    fn nop_runs_on_the_scheduler_timeline_for_four_tstates() {
        let mut machine = SmsTemporalMachine::new(vec![0x00; 0x4000], 3).unwrap();
        retired(machine.step_instruction(0).unwrap(), 0x00, 4);
        assert_eq!(machine.now(), MachineTime::from_ticks(12));
        assert_eq!(machine.cpu().registers.pc, 1);
    }

    #[test]
    fn immediate_load_uses_two_scheduled_bus_transactions() {
        let mut machine = SmsTemporalMachine::new(vec![0x3E, 0x42], 3).unwrap();
        retired(machine.step_instruction(0).unwrap(), 0x3E, 7);
        assert_eq!(machine.cpu().registers.a, 0x42);
        assert_eq!(machine.now(), MachineTime::from_ticks(21));
    }

    #[test]
    fn integrated_snapshot_restores_scheduled_internal_cycle_and_pending_write() {
        let mut machine = SmsTemporalMachine::new(vec![0x34; 0x4000], 3).unwrap();
        machine.cpu_mut().registers.set_hl(0xC000);
        machine.begin_instruction().unwrap();
        machine.schedule_pending_bus(0).unwrap();
        let read = machine.dispatch_next().unwrap();
        assert!(matches!(read, Z80MicroStep::BusRequest(_)));
        machine.schedule_pending_bus(0).unwrap();
        assert_eq!(
            machine.dispatch_next().unwrap(),
            Z80MicroStep::InternalCycle { tstates: 1 }
        );
        machine.schedule_internal_cycle(1).unwrap();

        let mut restored = SmsTemporalMachine::restore_state(machine.export_state()).unwrap();
        retired(restored.resume_instruction(0).unwrap(), 0x34, 11);
        assert_eq!(restored.memory().read(0xC000), 1);
        assert_eq!(restored.now(), MachineTime::from_ticks(33));
    }

    #[test]
    fn fixture_wait_is_counted_in_cpu_tstates_and_machine_time() {
        let mut machine = SmsTemporalMachine::new(vec![0x3E, 0x42], 3).unwrap();
        retired(machine.step_instruction(1).unwrap(), 0x3E, 9);
        assert_eq!(machine.now(), MachineTime::from_ticks(27));
    }

    #[test]
    fn overflow_keeps_completed_bus_write_latched_for_snapshot_restore_and_retry() {
        let mut machine = SmsTemporalMachine::new(vec![0x77; 0x4000], 3).unwrap();
        machine.cpu_mut().registers.a = 0x5A;
        machine.cpu_mut().registers.set_hl(0xC000);
        machine.begin_instruction().unwrap();
        machine.schedule_pending_bus(0).unwrap();
        assert!(matches!(
            machine.dispatch_next().unwrap(),
            Z80MicroStep::BusRequest(_)
        ));
        machine.schedule_pending_bus(0).unwrap();

        let mut state = machine.export_state();
        state.cpu.instruction_tstates = u32::MAX - 2;
        // Fault-inject the CPU counter after restoring its own DTO. The machine
        // restore gate rejects this impossible CPU/Scheduler combination; this
        // test bypasses that gate to exercise atomic dispatch failure handling.
        let cpu = Z80MicroSequencer::restore_state(state.cpu).unwrap();
        let bus = SmsTemporalBus::restore_state(state.bus).unwrap();
        let mut machine = SmsTemporalMachine {
            cpu,
            bus,
            instruction_started_at: state.instruction_started_at,
            next_internal_cycle_id: state.next_internal_cycle_id,
            pending_internal_cycle_id: state.pending_internal_cycle_id,
            pending_internal_tstates: state.pending_internal_tstates,
            trace_enabled: false,
            trace_events: Vec::new(),
        };
        assert!(matches!(
            machine.dispatch_next(),
            Err(SmsTemporalMachineError::Cpu(
                Z80MicroSequencerError::InstructionTStateOverflow
            ))
        ));
        let completed_transaction = machine.pending_bus_transaction().unwrap().unwrap();
        assert_eq!(completed_transaction.phase(), BusPhase::Complete);
        assert_eq!(
            completed_transaction.timestamp(),
            MachineTime::from_ticks(12)
        );
        let completed_at = machine.now();
        assert_eq!(
            machine.pending_bus_operation_result(),
            Some(BusOperationResult::Completed {
                value: None,
                completed_at,
            })
        );

        let mut state = machine.export_state();
        assert_eq!(state.bus.completion.unwrap().request_id, 1);
        assert_eq!(state.bus.pending_request.unwrap().id, 1);
        assert_eq!(machine.memory().read(0xC000), 0x5A);

        // Restore a valid CPU counter while preserving the already-committed,
        // unacknowledged bus completion. Resume must consume it, not dispatch
        // the memory write a second time.
        // Preserve the four T-states already consumed by M1 before the
        // pending M2 memory write; restore_state validates this phase minimum.
        state.cpu.instruction_tstates = 4;
        let mut restored = SmsTemporalMachine::restore_state(state).unwrap();
        retired(restored.resume_instruction(0).unwrap(), 0x77, 7);
        assert_eq!(restored.memory().read(0xC000), 0x5A);
        assert!(restored.export_state().bus.completion.is_none());
    }

    #[test]
    fn restore_rejects_internal_cycle_states_that_cannot_complete() {
        let mut machine = SmsTemporalMachine::new(vec![0x34; 0x4000], 3).unwrap();
        machine.cpu_mut().registers.set_hl(0xC000);
        machine.begin_instruction().unwrap();
        machine.schedule_pending_bus(0).unwrap();
        assert!(matches!(
            machine.dispatch_next().unwrap(),
            Z80MicroStep::BusRequest(_)
        ));
        machine.schedule_pending_bus(0).unwrap();
        assert_eq!(
            machine.dispatch_next().unwrap(),
            Z80MicroStep::InternalCycle { tstates: 1 }
        );
        machine.schedule_internal_cycle(1).unwrap();

        let valid_state = machine.export_state();

        let mut overflowing_cpu = valid_state.clone();
        overflowing_cpu.cpu.instruction_tstates = u32::MAX;
        assert!(SmsTemporalMachine::restore_state(overflowing_cpu).is_err());

        let mut exhausted_request_ids = valid_state;
        exhausted_request_ids.cpu.handshake.next_request_id = u64::MAX;
        assert!(SmsTemporalMachine::restore_state(exhausted_request_ids).is_err());
    }

    #[test]
    fn memory_write_is_committed_at_the_scheduled_bus_completion() {
        let mut machine = SmsTemporalMachine::new(vec![0x77; 0x4000], 3).unwrap();
        machine.cpu_mut().registers.a = 0x5A;
        machine.cpu_mut().registers.set_hl(0xC000);
        retired(machine.step_instruction(0).unwrap(), 0x77, 7);
        assert_eq!(machine.memory().read(0xC000), 0x5A);
        assert_eq!(machine.now(), MachineTime::from_ticks(21));
    }

    #[test]
    fn internal_cycle_rejects_duration_that_disagrees_with_cpu_phase() {
        let mut machine = SmsTemporalMachine::new(vec![0x34; 0x4000], 3).unwrap();
        machine.cpu_mut().registers.set_hl(0xC000);
        machine.begin_instruction().unwrap();
        machine.schedule_pending_bus(0).unwrap();
        machine.dispatch_next().unwrap();
        machine.schedule_pending_bus(0).unwrap();
        assert_eq!(
            machine.dispatch_next().unwrap(),
            Z80MicroStep::InternalCycle { tstates: 1 }
        );

        let before = machine.export_state();
        let now = machine.now();
        assert_eq!(
            machine.schedule_internal_cycle(2),
            Err(SmsTemporalMachineError::InvalidState)
        );
        assert_eq!(machine.export_state(), before);
        assert_eq!(machine.now(), now);

        machine.schedule_internal_cycle(1).unwrap();
        assert!(matches!(
            machine.dispatch_next().unwrap(),
            Z80MicroStep::BusRequest(_)
        ));
        machine.schedule_pending_bus(0).unwrap();
        retired(machine.dispatch_next().unwrap(), 0x34, 11);
    }

    #[test]
    fn inc_hl_runs_its_internal_cycle_as_a_scheduler_event() {
        let mut machine = SmsTemporalMachine::new(vec![0x34; 0x4000], 3).unwrap();
        machine.cpu_mut().registers.set_hl(0xC000);
        retired(machine.step_instruction(0).unwrap(), 0x34, 11);
        assert_eq!(machine.memory().read(0xC000), 1);
        assert_eq!(machine.now(), MachineTime::from_ticks(33));
    }

    #[test]
    fn snapshot_restore_resumes_pending_fetch_with_wait_without_repeating_it() {
        let mut machine = SmsTemporalMachine::new(vec![0x00; 0x4000], 3).unwrap();
        assert!(matches!(
            machine.begin_instruction().unwrap(),
            Z80MicroStep::BusRequest(_)
        ));
        machine.schedule_pending_bus(1).unwrap();

        let restored = SmsTemporalMachine::restore_state(machine.export_state()).unwrap();
        let mut restored = restored;
        retired(restored.resume_instruction(0).unwrap(), 0x00, 5);
        assert_eq!(restored.cpu().registers.pc, 1);
        assert_eq!(restored.now(), MachineTime::from_ticks(15));
    }

    #[test]
    fn replay_from_identical_initial_state_is_deterministic() {
        let rom = vec![0x3E, 0x42, 0x77, 0x00];
        let mut left = SmsTemporalMachine::new(rom.clone(), 3).unwrap();
        let mut right = SmsTemporalMachine::new(rom, 3).unwrap();
        for _ in 0..3 {
            left.step_instruction(1).unwrap();
            right.step_instruction(1).unwrap();
        }
        assert_eq!(left.export_state(), right.export_state());
        assert_eq!(left.now(), right.now());
        assert_eq!(left.memory().work_ram(), right.memory().work_ram());
    }

    #[test]
    fn restore_rejects_bus_deadline_that_disagrees_with_cpu_wait_counter() {
        let mut machine = SmsTemporalMachine::new(vec![0x00; 0x4000], 3).unwrap();
        machine.begin_instruction().unwrap();
        machine.schedule_pending_bus(1).unwrap();

        let mut state = machine.export_state();
        state.bus.coordinator.scheduler.event_queue.pending[0].timestamp =
            MachineTime::from_ticks(12);
        assert!(SmsTemporalMachine::restore_state(state).is_err());
    }

    #[test]
    fn restore_rejects_internal_cycle_duration_other_than_one_tstate() {
        let mut machine = SmsTemporalMachine::new(vec![0x34; 0x4000], 3).unwrap();
        machine.cpu_mut().registers.set_hl(0xC000);
        machine.begin_instruction().unwrap();
        machine.schedule_pending_bus(0).unwrap();
        machine.dispatch_next().unwrap();
        machine.schedule_pending_bus(0).unwrap();
        assert_eq!(
            machine.dispatch_next().unwrap(),
            Z80MicroStep::InternalCycle { tstates: 1 }
        );
        machine.schedule_internal_cycle(1).unwrap();

        let mut state = machine.export_state();
        state.pending_internal_tstates = Some(2);
        state.bus.coordinator.scheduler.event_queue.pending[0].timestamp =
            MachineTime::from_ticks(27);
        assert!(SmsTemporalMachine::restore_state(state).is_err());
    }

    #[test]
    fn restore_rejects_cpu_bus_request_mismatch() {
        let mut machine = SmsTemporalMachine::new(vec![0x00; 0x4000], 3).unwrap();
        machine.begin_instruction().unwrap();
        let mut state = machine.export_state();
        state.bus.pending_request = None;
        assert!(matches!(
            SmsTemporalMachine::restore_state(state),
            Err(SmsTemporalMachineError::InvalidState)
        ));
    }

    #[derive(Clone)]
    struct FlatBus {
        memory: Vec<u8>,
    }

    impl FlatBus {
        fn new(rom: &[u8]) -> Self {
            let mut memory = vec![0; 0x10000];
            memory[..rom.len()].copy_from_slice(rom);
            Self { memory }
        }
    }

    impl crate::CpuBus for FlatBus {
        fn read_memory(&mut self, address: u16) -> u8 {
            self.memory[usize::from(address)]
        }

        fn write_memory(&mut self, address: u16, value: u8) {
            self.memory[usize::from(address)] = value;
        }
    }

    fn assert_atomic_differential(
        rom: &[u8],
        initial_a: u8,
        initial_f: u8,
        initial_hl: u16,
        initial_ram: u8,
        expected_opcode: u8,
    ) {
        let mut temporal = SmsTemporalMachine::new(rom.to_vec(), 3).unwrap();
        temporal.cpu_mut().registers.a = initial_a;
        temporal.cpu_mut().registers.f = initial_f;
        temporal.cpu_mut().registers.set_hl(initial_hl);
        if initial_hl >= 0xC000 {
            temporal.bus.write_memory_for_test(initial_hl, initial_ram);
        }

        let mut atomic = crate::Z80::default();
        atomic.registers.a = initial_a;
        atomic.registers.f = initial_f;
        atomic.registers.set_hl(initial_hl);
        let mut bus = FlatBus::new(rom);
        if initial_hl >= 0xC000 {
            bus.memory[usize::from(initial_hl)] = initial_ram;
        }

        let expected_tstates = atomic.step(&mut bus).unwrap();
        retired(
            temporal.step_instruction(0).unwrap(),
            expected_opcode,
            expected_tstates,
        );
        assert_eq!(temporal.cpu().registers.a, atomic.registers.a);
        assert_eq!(temporal.cpu().registers.f, atomic.registers.f);
        assert_eq!(temporal.cpu().registers.bc(), atomic.registers.bc());
        assert_eq!(temporal.cpu().registers.de(), atomic.registers.de());
        assert_eq!(temporal.cpu().registers.hl(), atomic.registers.hl());
        assert_eq!(temporal.cpu().registers.sp, atomic.registers.sp);
        assert_eq!(temporal.cpu().registers.pc, atomic.registers.pc);
        assert_eq!(temporal.cpu().refresh, atomic.r);
        assert_eq!(
            temporal.memory().read(initial_hl),
            bus.memory[usize::from(initial_hl)]
        );
        assert_eq!(
            temporal.now(),
            MachineTime::from_ticks(u128::from(expected_tstates) * 3)
        );
    }

    #[test]
    fn pending_request_maps_to_shared_temporal_contract_without_duplicate_state() {
        let mut machine = SmsTemporalMachine::new(vec![0x00; 0x4000], 3).unwrap();
        machine.begin_instruction().unwrap();
        let transaction = machine.pending_bus_transaction().unwrap().unwrap();
        assert_eq!(transaction.timestamp(), MachineTime::ZERO);
        assert_eq!(transaction.cycle_index(), MCycleIndex::new(0));
        assert_eq!(transaction.t_state_index(), TStateIndex::new(0));
        assert_eq!(transaction.phase(), BusPhase::Setup);
        assert_eq!(transaction.kind(), BusCycleKind::OpcodeFetch);
        assert_eq!(transaction.direction(), BusDirection::Read);
        assert_eq!(transaction.address(), Some(0));
        assert_eq!(transaction.data(), None);
        assert_eq!(transaction.duration_tstates(), 4);
        assert_eq!(
            machine.pending_bus_transaction().unwrap(),
            Some(transaction)
        );
        machine.schedule_pending_bus(0).unwrap();
        assert_eq!(
            machine.pending_bus_transaction().unwrap(),
            Some(transaction)
        );
    }

    #[test]
    fn internal_mcycle_maps_to_shared_temporal_contract() {
        let mut machine = SmsTemporalMachine::new(vec![0x34; 0x4000], 3).unwrap();
        machine.cpu_mut().registers.set_hl(0xC000);
        machine.begin_instruction().unwrap();
        machine.schedule_pending_bus(0).unwrap();
        assert!(matches!(
            machine.dispatch_next().unwrap(),
            Z80MicroStep::BusRequest(_)
        ));
        machine.schedule_pending_bus(0).unwrap();
        assert_eq!(
            machine.dispatch_next().unwrap(),
            Z80MicroStep::InternalCycle { tstates: 1 }
        );
        let transaction = machine.pending_internal_transaction().unwrap().unwrap();
        assert_eq!(transaction.cycle_index(), MCycleIndex::new(1));
        assert_eq!(transaction.t_state_index(), TStateIndex::new(3));
        assert_eq!(transaction.phase(), BusPhase::Internal);
        assert_eq!(transaction.kind(), BusCycleKind::Internal);
        assert_eq!(transaction.direction(), BusDirection::None);
        assert_eq!(transaction.address(), None);
        assert_eq!(transaction.data(), None);
        assert_eq!(transaction.duration_tstates(), 1);
    }
    #[test]
    fn internal_transaction_keeps_start_timestamp_while_completion_is_latched() {
        let mut machine = SmsTemporalMachine::new(vec![0x34; 0x4000], 3).unwrap();
        machine.cpu_mut().registers.set_hl(0xC000);
        machine.begin_instruction().unwrap();
        machine.schedule_pending_bus(0).unwrap();
        assert!(matches!(
            machine.dispatch_next().unwrap(),
            Z80MicroStep::BusRequest(_)
        ));
        machine.schedule_pending_bus(0).unwrap();
        assert_eq!(
            machine.dispatch_next().unwrap(),
            Z80MicroStep::InternalCycle { tstates: 1 }
        );

        let started_at = machine.now();
        machine.schedule_internal_cycle(1).unwrap();
        // Preserve the safe point between Scheduler dispatch and CPU acknowledgement.
        machine.bus.dispatch_next().unwrap().unwrap();
        assert!(machine.now() > started_at);

        let transaction = machine.pending_internal_transaction().unwrap().unwrap();
        assert_eq!(transaction.timestamp(), started_at);
    }

    #[test]
    fn supported_microsequencer_slice_matches_atomic_reference() {
        assert_atomic_differential(&[0x00], 0, 0, 0, 0, 0x00);
        assert_atomic_differential(&[0x3E, 0x42], 0, 0, 0, 0, 0x3E);
        assert_atomic_differential(&[0x36, 0xA5], 0, 0, 0xC000, 0, 0x36);
        assert_atomic_differential(&[0x7E], 0, 0, 0xC000, 0xA5, 0x7E);
        assert_atomic_differential(&[0x77], 0x5A, 0, 0xC000, 0, 0x77);
        assert_atomic_differential(&[0x78], 0x33, 0, 0, 0, 0x78);
        assert_atomic_differential(&[0x04], 0, 0x01, 0, 0, 0x04);
        assert_atomic_differential(&[0x34], 0, 0x01, 0xC000, 0x7F, 0x34);
        assert_atomic_differential(&[0x35], 0, 0x01, 0xC000, 0x80, 0x35);
    }

    #[test]
    fn internal_cycle_preflights_request_id_exhaustion_before_scheduling() {
        let mut machine = SmsTemporalMachine::new(vec![0x34; 0x4000], 3).unwrap();
        machine.cpu_mut().registers.set_hl(0xC000);
        let fetch = match machine.begin_instruction().unwrap() {
            Z80MicroStep::BusRequest(request) => request,
            other => panic!("expected fetch, got {other:?}"),
        };
        machine.schedule_pending_bus(0).unwrap();
        let read = match machine.dispatch_next().unwrap() {
            Z80MicroStep::BusRequest(request) => request,
            other => panic!("expected memory read, got {other:?}"),
        };
        machine.schedule_pending_bus(0).unwrap();
        assert_eq!(
            machine.dispatch_next().unwrap(),
            Z80MicroStep::InternalCycle { tstates: 1 }
        );

        let mut state = machine.export_state();
        state.cpu.handshake.next_request_id = u64::MAX;
        let mut restored = SmsTemporalMachine::restore_state(state).unwrap();
        let before = restored.export_state();
        assert_eq!(
            restored.schedule_internal_cycle(1),
            Err(SmsTemporalMachineError::Cpu(
                Z80MicroSequencerError::Handshake(
                    sega_z80_cpu::Z80BusHandshakeError::RequestIdExhausted
                )
            ))
        );
        assert_eq!(restored.export_state(), before);
        assert_eq!(restored.cpu().pending_request(), None);
        assert!(fetch.id < read.id);
    }

    #[test]
    fn runtime_trace_records_real_bus_and_internal_dispatch_boundaries() {
        let mut machine = SmsTemporalMachine::new(vec![0x34; 0x4000], 3).unwrap();
        machine.cpu_mut().registers.set_hl(0xC000);
        machine.set_trace_enabled(true);
        retired(machine.step_instruction(0).unwrap(), 0x34, 11);

        let trace = machine.take_trace();
        assert_eq!(trace.len(), 4);
        assert!(matches!(
            trace[0],
            super::SmsTemporalTraceEvent::BusCompleted {
                timestamp,
                operation: sega_z80_cpu::Z80BusOperation::OpcodeFetch,
                base_tstates: 4,
                wait_tstates: 0,
                data: Some(0x34),
                ..
            } if timestamp == MachineTime::from_ticks(12)
        ));
        assert!(matches!(
            trace[1],
            super::SmsTemporalTraceEvent::BusCompleted {
                timestamp,
                operation: sega_z80_cpu::Z80BusOperation::MemoryRead,
                base_tstates: 3,
                data: Some(0),
                ..
            } if timestamp == MachineTime::from_ticks(21)
        ));
        assert!(matches!(
            trace[2],
            super::SmsTemporalTraceEvent::InternalCycleCompleted {
                timestamp,
                tstates: 1,
                ..
            } if timestamp == MachineTime::from_ticks(24)
        ));
        assert!(matches!(
            trace[3],
            super::SmsTemporalTraceEvent::BusCompleted {
                timestamp,
                operation: sega_z80_cpu::Z80BusOperation::MemoryWrite { value: 1 },
                base_tstates: 3,
                data: None,
                ..
            } if timestamp == MachineTime::from_ticks(33)
        ));
        assert!(machine.take_trace().is_empty());
    }

    #[test]
    fn exported_phase_is_ready_after_retirement() {
        let mut machine = SmsTemporalMachine::new(vec![0x00; 0x4000], 3).unwrap();
        machine.step_instruction(0).unwrap();
        assert_eq!(
            machine.export_state().cpu.phase,
            Z80MicroSequencerPhase::Ready
        );
    }
    #[test]
    fn restore_rejects_instruction_tstates_ahead_of_scheduler_before_pending_bus_cycle() {
        let mut machine = SmsTemporalMachine::new(vec![0x3E, 0x42], 3).unwrap();
        machine.begin_instruction().unwrap();
        machine.schedule_pending_bus(0).unwrap();
        assert!(matches!(
            machine.dispatch_next().unwrap(),
            Z80MicroStep::BusRequest(_)
        ));
        machine.schedule_pending_bus(0).unwrap();

        let mut state = machine.export_state();
        assert_eq!(state.cpu.instruction_tstates, 4);
        state.cpu.instruction_tstates = 5;
        assert!(matches!(
            SmsTemporalMachine::restore_state(state),
            Err(SmsTemporalMachineError::InvalidState)
        ));
    }

    #[test]
    fn restore_requires_instruction_start_timestamp_for_active_cpu() {
        let mut machine = SmsTemporalMachine::new(vec![0x00; 0x4000], 3).unwrap();
        machine.begin_instruction().unwrap();
        machine.schedule_pending_bus(0).unwrap();
        let mut state = machine.export_state();
        state.instruction_started_at = None;
        assert!(matches!(
            SmsTemporalMachine::restore_state(state),
            Err(SmsTemporalMachineError::InvalidState)
        ));
    }
    #[test]
    fn mapper_write_snapshot_restore_replays_identical_temporal_trace() {
        let mut rom = vec![0; 4 * 0x4000];
        for bank in 0..4 {
            let start = bank * 0x4000;
            rom[start..start + 0x4000].fill(bank as u8);
        }
        // LD (HL),A writes A to the Sega mapper's first bank register.
        rom[0] = 0x77;

        let mut original = SmsTemporalMachine::new(rom, 3).unwrap();
        original.cpu_mut().registers.a = 3;
        original.cpu_mut().registers.set_hl(0xFFFD);
        original.set_trace_enabled(true);
        assert!(matches!(
            original.begin_instruction().unwrap(),
            Z80MicroStep::BusRequest(_)
        ));
        original.schedule_pending_bus(0).unwrap();
        assert!(matches!(
            original.dispatch_next().unwrap(),
            Z80MicroStep::BusRequest(_)
        ));
        // Compare only the mapper write after the opcode-fetch trace boundary.
        assert_eq!(original.take_trace().len(), 1);

        original.schedule_pending_bus(2).unwrap();
        assert_eq!(original.memory().selected_banks(), [0, 1, 2]);
        assert_eq!(original.memory().read(0x0400), 0);

        let mut restored = SmsTemporalMachine::restore_state(original.export_state()).unwrap();
        restored.set_trace_enabled(true);

        let original_step = original.dispatch_next().unwrap();
        let restored_step = restored.dispatch_next().unwrap();
        retired(original_step, 0x77, 9);
        retired(restored_step, 0x77, 9);

        assert_eq!(original.memory().selected_banks(), [3, 1, 2]);
        assert_eq!(restored.memory().selected_banks(), [3, 1, 2]);
        assert_eq!(original.memory().read(0x0400), 3);
        assert_eq!(restored.memory().read(0x0400), 3);
        assert_eq!(original.memory().read(0x03FF), 0);
        assert_eq!(restored.memory().read(0x03FF), 0);
        assert_eq!(original.memory().read(0xDFFD), 3);
        assert_eq!(restored.memory().read(0xDFFD), 3);

        let original_trace = original.take_trace();
        let restored_trace = restored.take_trace();
        assert_eq!(original_trace, restored_trace);
        assert_eq!(original_trace.len(), 1);
        assert!(matches!(
            original_trace[0],
            super::SmsTemporalTraceEvent::BusCompleted {
                timestamp,
                operation: sega_z80_cpu::Z80BusOperation::MemoryWrite { value: 3 },
                address: 0xFFFD,
                base_tstates: 3,
                wait_tstates: 2,
                data: None,
                ..
            } if timestamp == MachineTime::from_ticks(27)
        ));
    }
}
