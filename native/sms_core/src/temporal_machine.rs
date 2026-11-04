//! First scheduler-driven SMS Z80/memory vertical slice.
//!
//! This composition uses the resumable Z80 subset, the mapper-backed SMS
//! memory/I/O adapter, and one machine scheduler. Immediate port transactions
//! support controller and memory-control effects; VDP/PSG devices remain
//! unconnected (open-bus reads and bounded write diagnostics). Interrupts, DMA
//! and physical WAIT wiring remain outside this temporal slice.

use core::fmt;

use crate::{
    SmsTemporalBus, SmsTemporalBusError, SmsTemporalBusState, SmsTemporalDispatch, SmsTemporalM1,
    SmsTemporalM1Error, SmsTemporalM1State,
};
use sega_temporal_core::{
    BusContractError, BusCycleKind, BusDirection, BusOperationResult, BusPhase, BusTransaction,
    MCycleIndex, MachineTime, TStateIndex,
};
use sega_z80_cpu::{
    Z80BusOperation, Z80M1Phase, Z80M1Pins, Z80MicroSequencer, Z80MicroSequencerError,
    Z80MicroSequencerPhase, Z80MicroSequencerState, Z80MicroStep,
};

pub const SMS_TEMPORAL_MACHINE_STATE_VERSION: u16 = 21;

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
    M1TStateCompleted {
        timestamp: MachineTime,
        phase: Z80M1Phase,
        pins: Z80M1Pins,
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
    pub m1: Option<SmsTemporalM1State>,
    pub m1_fixture_wait_tstates: Option<u32>,
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
        return state.instruction_started_at.is_none()
            && state.m1.is_none()
            && state.m1_fixture_wait_tstates.is_none();
    }
    let Some(instruction_started_at) = state.instruction_started_at else {
        return false;
    };
    if let Some(m1) = state.m1 {
        if state.bus.pending_request.is_some()
            || state.bus.completion.is_some()
            || state.bus.pending_internal_cycle.is_some()
            || state.bus.internal_completion.is_some()
            || state.m1_fixture_wait_tstates.is_none()
            || state.cpu.handshake.wait_tstates != state.m1_fixture_wait_tstates.unwrap_or(0)
        {
            return false;
        }
        let waits = m1.cycle.wait_tstates;
        let current_m1_tstates = match m1.cycle.phase {
            Z80M1Phase::T1 => 0,
            Z80M1Phase::T2 => 1,
            Z80M1Phase::Wait => 2_u32.saturating_add(waits),
            Z80M1Phase::T3 => 2_u32.saturating_add(waits),
            Z80M1Phase::T4 => 3_u32.saturating_add(waits),
            Z80M1Phase::Complete => 4_u32.saturating_add(waits),
        };
        let Some(expected_tstates) = state
            .cpu
            .instruction_tstates
            .checked_add(current_m1_tstates)
        else {
            return false;
        };
        return elapsed_tstates(
            instruction_started_at,
            state.bus.coordinator.scheduler.now,
            state.bus.coordinator.master_ticks_per_tstate,
        ) == Some(expected_tstates);
    }
    if state.m1_fixture_wait_tstates.is_some() {
        return false;
    }
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
        (None, None, None, None) if state.cpu.phase.is_internal_cycle() => {
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
    M1(SmsTemporalM1Error),
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
            Self::M1(error) => write!(f, "SMS M1 continuation failed: {error:?}"),
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

impl From<SmsTemporalM1Error> for SmsTemporalMachineError {
    fn from(error: SmsTemporalM1Error) -> Self {
        Self::M1(error)
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
    m1: Option<SmsTemporalM1>,
    m1_fixture_wait_tstates: Option<u32>,
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
            m1: None,
            m1_fixture_wait_tstates: None,
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
            Z80BusOperation::InterruptAcknowledge => (
                BusCycleKind::InterruptAcknowledge,
                BusDirection::Read,
                None,
            ),
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
            if request.operation == Z80BusOperation::InterruptAcknowledge {
                None
            } else {
                Some(request.address)
            },
            data,
            u16::from(request.base_tstates)
                + if request.operation == Z80BusOperation::InterruptAcknowledge {
                    2
                } else {
                    0
                },
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
        if !state.phase.is_internal_cycle() {
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
    ///
    /// Opcode fetches use the explicit four-phase M1 model; data accesses retain
    /// the transactional memory-cycle path.
    pub fn schedule_pending_bus(
        &mut self,
        fixture_wait_tstates: u32,
    ) -> Result<MachineTime, SmsTemporalMachineError> {
        let request = self
            .cpu
            .pending_request()
            .ok_or(SmsTemporalMachineError::InvalidState)?;
        if request.operation == Z80BusOperation::OpcodeFetch {
            if self.bus.pending_request().is_some()
                || self.bus.completion().is_some()
                || self.pending_internal_cycle_id.is_some()
            {
                return Err(SmsTemporalMachineError::InvalidState);
            }
            if let Some(m1) = self.m1.as_mut() {
                if self.m1_fixture_wait_tstates != Some(fixture_wait_tstates)
                    || m1.export_state().pending_continuation_id.is_some()
                {
                    return Err(SmsTemporalMachineError::InvalidState);
                }
                m1.schedule_next_tstate(self.bus.coordinator_mut())?;
                return Ok(m1.export_state().pending_deadline.unwrap());
            }

            let mut next_cpu = self.cpu.clone();
            next_cpu.insert_wait_tstates(fixture_wait_tstates)?;
            let mut m1 = SmsTemporalM1::new(request.address, next_cpu.i, next_cpu.refresh);
            m1.schedule_next_tstate(self.bus.coordinator_mut())?;
            let deadline = m1
                .export_state()
                .pending_deadline
                .ok_or(SmsTemporalMachineError::InvalidState)?;
            self.cpu = next_cpu;
            self.m1 = Some(m1);
            self.m1_fixture_wait_tstates = Some(fixture_wait_tstates);
            return Ok(deadline);
        }

        if self.bus.pending_request().is_some() || self.m1.is_some() {
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
        if !cpu_state.phase.is_internal_cycle() {
            return Err(SmsTemporalMachineError::InvalidState);
        }
        if self.pending_internal_cycle_id.is_some()
            || self.bus.pending_request().is_some()
            || self.bus.completion().is_some()
            || self.m1.is_some()
            || tstates != 1
        {
            // The sequencer currently exposes internal work one T-state at a
            // time. Accept every explicitly classified internal continuation,
            // but reject aggregated durations that could skip a CPU phase edge.
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

    fn finish_m1_fetch(&mut self) -> Result<Z80MicroStep, SmsTemporalMachineError> {
        let m1 = self.m1.ok_or(SmsTemporalMachineError::InvalidState)?;
        if m1.phase() != Z80M1Phase::Complete {
            return Err(SmsTemporalMachineError::InvalidState);
        }
        let opcode = m1.opcode().ok_or(SmsTemporalMachineError::InvalidState)?;
        let request = self
            .cpu
            .pending_request()
            .filter(|request| request.operation == Z80BusOperation::OpcodeFetch)
            .ok_or(SmsTemporalMachineError::InvalidState)?;
        let mut next_cpu = self.cpu.clone();
        let step = next_cpu.complete_bus_cycle(request.id, Some(opcode))?;
        self.cpu = next_cpu;
        self.m1 = None;
        self.m1_fixture_wait_tstates = None;
        if matches!(
            step,
            Z80MicroStep::InstructionRetired { .. } | Z80MicroStep::InterruptServiced { .. }
        ) {
            self.instruction_started_at = None;
        }
        Ok(step)
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
                if matches!(
                    step,
                    Z80MicroStep::InstructionRetired { .. }
                        | Z80MicroStep::InterruptServiced { .. }
                ) {
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
            SmsTemporalDispatch::M1TStateComplete {
                continuation_id,
                completed_at,
            } => {
                let mut m1 = self.m1.ok_or(SmsTemporalMachineError::InvalidState)?;
                let phase_before = m1.phase();
                let pins = m1.pins();
                let m1_state = m1.export_state();
                if m1_state.pending_continuation_id != Some(continuation_id)
                    || m1_state.pending_deadline != Some(completed_at)
                {
                    return Err(SmsTemporalMachineError::InvalidState);
                }
                let total_waits = self
                    .m1_fixture_wait_tstates
                    .ok_or(SmsTemporalMachineError::InvalidState)?;
                let wait_asserted = match phase_before {
                    Z80M1Phase::T2 => total_waits > 0,
                    Z80M1Phase::Wait => m1_state.cycle.wait_tstates.saturating_add(1) < total_waits,
                    _ => false,
                };

                // The opcode is sampled at the T3 completion boundary, before
                // the M1 model advances to T4. Memory is not read at T1/T2/T4.
                if phase_before == Z80M1Phase::T3 {
                    let opcode = self.bus.memory().read(pins.address);
                    m1.sample_opcode(opcode)?;
                }
                let next_phase = m1.complete_tstate(
                    crate::SmsTemporalEvent::M1TStateComplete { continuation_id },
                    completed_at,
                    wait_asserted,
                )?;

                if self.trace_enabled {
                    self.trace_events
                        .push(SmsTemporalTraceEvent::M1TStateCompleted {
                            timestamp: completed_at,
                            phase: phase_before,
                            pins,
                        });
                }

                self.m1 = Some(m1);
                if next_phase == Z80M1Phase::Complete {
                    self.finish_m1_fetch()
                } else {
                    let request = self
                        .cpu
                        .pending_request()
                        .ok_or(SmsTemporalMachineError::InvalidState)?;
                    Ok(Z80MicroStep::BusRequest(request))
                }
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
                acknowledged @ Z80MicroStep::InterruptAcknowledge { .. } => {
                    return Ok(acknowledged);
                }
                serviced @ Z80MicroStep::InterruptServiced { .. } => return Ok(serviced),
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
        } else if self.cpu.export_state().phase.is_internal_cycle() {
            Z80MicroStep::InternalCycle { tstates: 1 }
        } else if let Z80MicroSequencerPhase::InterruptAcknowledgeComplete { vector } =
            self.cpu.export_state().phase
        {
            return Ok(Z80MicroStep::InterruptAcknowledge {
                vector,
                tstates: self.cpu.export_state().instruction_tstates,
            });
        } else {
            return Err(SmsTemporalMachineError::InvalidState);
        };
        loop {
            step = match step {
                Z80MicroStep::BusRequest(_) => {
                    if self
                        .m1
                        .as_ref()
                        .is_some_and(|m1| m1.phase() == Z80M1Phase::Complete)
                    {
                        self.finish_m1_fetch()?
                    } else {
                        if self.m1.is_some() {
                            if self.m1.as_ref().is_some_and(|m1| {
                                m1.export_state().pending_continuation_id.is_none()
                            }) {
                                let m1_waits =
                                    self.m1_fixture_wait_tstates.unwrap_or(fixture_wait_tstates);
                                self.schedule_pending_bus(m1_waits)?;
                            }
                        } else if self.bus.pending_request().is_none() {
                            self.schedule_pending_bus(fixture_wait_tstates)?;
                        }
                        self.dispatch_next()?
                    }
                }
                Z80MicroStep::InternalCycle { tstates } => {
                    if self.pending_internal_cycle_id.is_none() {
                        self.schedule_internal_cycle(tstates)?;
                    }
                    self.dispatch_next()?
                }
                retired @ Z80MicroStep::InstructionRetired { .. } => return Ok(retired),
                acknowledged @ Z80MicroStep::InterruptAcknowledge { .. } => {
                    return Ok(acknowledged);
                }
                serviced @ Z80MicroStep::InterruptServiced { .. } => return Ok(serviced),
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

    /// Latch an NMI input edge for acceptance at the next CPU instruction boundary.
    /// This signal update does not advance machine time.
    pub fn request_nmi(&mut self) {
        self.cpu.request_nmi();
    }

    /// Set the external byte driven during an INT acknowledge cycle.
    /// This signal update does not advance machine time.
    pub fn set_interrupt_vector(&mut self, vector: u8) {
        self.bus.set_interrupt_vector(vector);
    }

    /// Drive the level-sensitive INT input and its acknowledge vector.
    /// None deasserts INT. Signal changes do not advance machine time.
    pub fn set_int_line(&mut self, vector: Option<u8>) {
        self.cpu.set_int_line(vector);
        if let Some(vector) = vector {
            self.bus.set_interrupt_vector(vector);
        }
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
            m1: self.m1.map(|m1| m1.export_state()),
            m1_fixture_wait_tstates: self.m1_fixture_wait_tstates,
        }
    }

    /// Restore the complete first vertical slice after cross-validating CPU and bus state.
    pub fn restore_state(state: SmsTemporalMachineState) -> Result<Self, SmsTemporalMachineError> {
        if state.format_version != SMS_TEMPORAL_MACHINE_STATE_VERSION {
            return Err(SmsTemporalMachineError::InvalidState);
        }
        let cpu = Z80MicroSequencer::restore_state(state.cpu.clone())?;
        let bus = SmsTemporalBus::restore_state(state.bus.clone())?;
        let m1 = state.m1.map(SmsTemporalM1::restore_state).transpose()?;
        let expected_bus_request = if m1.is_some() {
            None
        } else {
            cpu.pending_request()
        };
        if expected_bus_request != state.bus.pending_request
            || state.pending_internal_cycle_id != state.bus.pending_internal_cycle
            || state.pending_internal_cycle_id.is_some() != state.pending_internal_tstates.is_some()
            || state.pending_internal_tstates == Some(0)
            || state
                .pending_internal_cycle_id
                .is_some_and(|id| id >= state.next_internal_cycle_id)
        {
            return Err(SmsTemporalMachineError::InvalidState);
        }
        if state.pending_internal_cycle_id.is_some() && !state.cpu.phase.is_internal_cycle() {
            return Err(SmsTemporalMachineError::InvalidState);
        }
        match (m1, state.m1_fixture_wait_tstates) {
            (Some(m1_cycle), Some(total_waits)) => {
                let request = cpu
                    .pending_request()
                    .filter(|request| request.operation == Z80BusOperation::OpcodeFetch)
                    .ok_or(SmsTemporalMachineError::InvalidState)?;
                let m1_state = m1_cycle.export_state();
                let events = &state.bus.coordinator.scheduler.event_queue.pending;
                if state.bus.pending_request.is_some()
                    || state.pending_internal_cycle_id.is_some()
                    || state.cpu.handshake.wait_tstates != total_waits
                    || m1_state.cycle.address != request.address
                    || m1_state.cycle.refresh_after_m1
                        != ((cpu.refresh & 0x80) | (cpu.refresh.wrapping_add(1) & 0x7F))
                {
                    return Err(SmsTemporalMachineError::InvalidState);
                }
                match (m1_state.pending_continuation_id, m1_state.pending_deadline) {
                    (Some(id), Some(deadline))
                        if events.len() == 1
                            && events[0].payload
                                == (crate::SmsTemporalEvent::M1TStateComplete {
                                    continuation_id: id,
                                })
                            && events[0].timestamp == deadline => {}
                    (None, None) if events.is_empty() => {}
                    _ => return Err(SmsTemporalMachineError::InvalidState),
                }
            }
            (None, None) => {}
            _ => return Err(SmsTemporalMachineError::InvalidState),
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
            let automatic_wait_tstates = if request.operation == Z80BusOperation::InterruptAcknowledge {
                2
            } else {
                0
            };
            let wait_tstates = total_tstates
                .checked_sub(u128::from(request.base_tstates))
                .and_then(|waits| waits.checked_sub(automatic_wait_tstates))
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
            m1,
            m1_fixture_wait_tstates: state.m1_fixture_wait_tstates,
            trace_enabled: false,
            trace_events: Vec::new(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{SmsTemporalMachine, SmsTemporalMachineError};
    use sega_temporal_core::{
        BusCycleKind, BusDirection, BusPhase, MCycleIndex, MachineTime, TStateIndex,
    };
    use sega_z80_cpu::{
        Z80MicroSequencerError, Z80MicroSequencerPhase, Z80MicroStep, Z80Registers,
    };

    fn retired(step: Z80MicroStep, opcode: u8, tstates: u32) {
        assert_eq!(step, Z80MicroStep::InstructionRetired { opcode, tstates });
    }

    fn complete_m1_fetch(
        machine: &mut SmsTemporalMachine,
        fixture_wait_tstates: u32,
    ) -> Z80MicroStep {
        loop {
            let m1 = machine.export_state().m1;
            if m1.is_none() || m1.is_some_and(|state| state.pending_continuation_id.is_none()) {
                machine.schedule_pending_bus(fixture_wait_tstates).unwrap();
            }
            let step = machine.dispatch_next().unwrap();
            if machine.export_state().m1.is_none() {
                return step;
            }
        }
    }

    #[test]
    fn int_acknowledge_is_a_distinct_temporal_transaction_and_restores_waits() {
        let mut machine = SmsTemporalMachine::new(vec![0x00; 0x4000], 3).unwrap();
        machine.cpu_mut().iff1 = true;
        machine.cpu_mut().iff2 = true;
        machine.set_int_line(Some(0xCF));

        let Z80MicroStep::BusRequest(request) = machine.begin_instruction().unwrap() else {
            panic!("eligible INT must start an acknowledge request");
        };
        assert_eq!(
            request.operation,
            sega_z80_cpu::Z80BusOperation::InterruptAcknowledge
        );
        let transaction = machine.pending_bus_transaction().unwrap().unwrap();
        assert_eq!(transaction.kind(), BusCycleKind::InterruptAcknowledge);
        assert_eq!(transaction.direction(), BusDirection::Read);
        assert_eq!(transaction.address(), None);
        assert_eq!(transaction.data(), None);
        assert_eq!(transaction.duration_tstates(), 5);

        machine.schedule_pending_bus(1).unwrap();
        let snapshot = machine.export_state();
        let mut restored = SmsTemporalMachine::restore_state(snapshot).unwrap();
        assert_eq!(
            restored.dispatch_next().unwrap(),
            Z80MicroStep::InterruptAcknowledge {
                vector: 0xCF,
                tstates: 6,
            }
        );
        assert_eq!(restored.now(), MachineTime::from_ticks(18));
        assert_eq!(
            restored.cpu().export_state().phase,
            Z80MicroSequencerPhase::InterruptAcknowledgeComplete { vector: 0xCF }
        );
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
        let read = complete_m1_fetch(&mut machine, 0);
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
    fn non_base_internal_phase_runs_and_restores_through_generic_scheduler_path() {
        let mut machine = SmsTemporalMachine::new(vec![0xED, 0x47], 3).unwrap();
        machine.cpu_mut().registers.a = 0xA5;
        machine.begin_instruction().unwrap();

        assert!(matches!(
            complete_m1_fetch(&mut machine, 0),
            Z80MicroStep::BusRequest(_)
        ));
        assert_eq!(
            complete_m1_fetch(&mut machine, 0),
            Z80MicroStep::InternalCycle { tstates: 1 }
        );
        assert!(machine.cpu().export_state().phase.is_internal_cycle());

        machine.schedule_internal_cycle(1).unwrap();
        let mut restored = SmsTemporalMachine::restore_state(machine.export_state()).unwrap();
        retired(restored.resume_instruction(0).unwrap(), 0x47, 9);
        assert_eq!(restored.cpu().i, 0xA5);
        assert_eq!(restored.now(), MachineTime::from_ticks(27));
    }

    #[test]
    fn fixture_wait_is_counted_in_cpu_tstates_and_machine_time() {
        let mut machine = SmsTemporalMachine::new(vec![0x3E, 0x42], 3).unwrap();
        retired(machine.step_instruction(1).unwrap(), 0x3E, 9);
        assert_eq!(machine.now(), MachineTime::from_ticks(27));
    }

    #[test]
    fn unsupported_ed_decode_resumes_after_prefix_snapshot_and_retries_deterministically() {
        let mut rom = vec![0; 0x4000];
        rom[..2].copy_from_slice(&[0xED, 0x00]);
        let mut machine = SmsTemporalMachine::new(rom, 3).unwrap();
        machine.begin_instruction().unwrap();

        // Complete the ED prefix M1 and schedule the sub-opcode M1.
        assert!(matches!(
            complete_m1_fetch(&mut machine, 0),
            Z80MicroStep::BusRequest(_)
        ));
        machine.schedule_pending_bus(0).unwrap();

        let prefix_snapshot = machine.export_state();
        assert_eq!(prefix_snapshot.cpu.instruction_tstates, 4);
        assert_eq!(
            prefix_snapshot.cpu.phase,
            Z80MicroSequencerPhase::OpcodeFetch
        );
        assert_eq!(
            prefix_snapshot.cpu.prefix_state,
            sega_z80_cpu::Z80PrefixState::Ed
        );

        let mut restored = SmsTemporalMachine::restore_state(prefix_snapshot.clone()).unwrap();
        let first_error = restored.resume_instruction(0).unwrap_err();
        assert!(matches!(
            first_error,
            SmsTemporalMachineError::Cpu(Z80MicroSequencerError::UnsupportedOpcode {
                opcode: 0x00,
                ..
            })
        ));

        let mut retried = SmsTemporalMachine::restore_state(prefix_snapshot).unwrap();
        let retry_error = retried.resume_instruction(0).unwrap_err();
        assert_eq!(retry_error, first_error);
    }

    #[test]
    fn restore_rejects_internal_cycle_states_that_cannot_complete() {
        let mut machine = SmsTemporalMachine::new(vec![0x34; 0x4000], 3).unwrap();
        machine.cpu_mut().registers.set_hl(0xC000);
        machine.begin_instruction().unwrap();
        assert!(matches!(
            complete_m1_fetch(&mut machine, 0),
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
        complete_m1_fetch(&mut machine, 0);
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
    fn base_inc_bc_internal_tstates_are_scheduler_driven_and_restorable() {
        let mut machine = SmsTemporalMachine::new(vec![0x03; 0x4000], 3).unwrap();
        machine.cpu_mut().registers.set_bc(0xFFFF);
        machine.begin_instruction().unwrap();
        assert_eq!(
            complete_m1_fetch(&mut machine, 0),
            Z80MicroStep::InternalCycle { tstates: 1 }
        );

        machine.schedule_internal_cycle(1).unwrap();
        let mut restored = SmsTemporalMachine::restore_state(machine.export_state()).unwrap();
        retired(restored.resume_instruction(0).unwrap(), 0x03, 6);
        assert_eq!(restored.cpu().registers.bc(), 0x0000);
        assert_eq!(restored.now(), MachineTime::from_ticks(18));
    }

    #[test]
    fn base_add_hl_bc_crosses_documented_mcycle_boundary_and_restores() {
        let mut machine = SmsTemporalMachine::new(vec![0x09; 0x4000], 3).unwrap();
        machine.cpu_mut().registers.set_hl(0x7FFF);
        machine.cpu_mut().registers.set_bc(0xFFFF);
        machine.begin_instruction().unwrap();
        assert_eq!(
            complete_m1_fetch(&mut machine, 0),
            Z80MicroStep::InternalCycle { tstates: 1 }
        );

        machine.schedule_internal_cycle(1).unwrap();
        assert_eq!(
            machine.dispatch_next().unwrap(),
            Z80MicroStep::InternalCycle { tstates: 1 }
        );
        let mut restored = SmsTemporalMachine::restore_state(machine.export_state()).unwrap();
        retired(restored.resume_instruction(0).unwrap(), 0x09, 11);
        assert_eq!(restored.cpu().registers.hl(), 0x7FFE);
        assert_eq!(restored.cpu().registers.f & 0x11, 0x11);
        assert_eq!(restored.now(), MachineTime::from_ticks(33));
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
        complete_m1_fetch(&mut machine, 0);
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
        assert!(matches!(
            complete_m1_fetch(&mut machine, 0),
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
        assert!(matches!(
            complete_m1_fetch(&mut machine, 0),
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
    fn alu_register_matrix_matches_atomic_reference_through_scheduler() {
        for operation in 0_u8..8 {
            for source in [0_u8, 1, 2, 3, 4, 5, 7] {
                let opcode = 0x80 | (operation << 3) | source;
                let mut rom = vec![0; 0x4000];
                rom[0] = opcode;
                let initial_registers = Z80Registers {
                    a: 0x7F,
                    f: 0x01,
                    b: 0x01,
                    c: 0x02,
                    d: 0x03,
                    e: 0x04,
                    h: 0x05,
                    l: 0x06,
                    ..Z80Registers::default()
                };

                let mut temporal = SmsTemporalMachine::new(rom.clone(), 3).unwrap();
                temporal.cpu_mut().registers = initial_registers;
                let mut atomic = crate::Z80::default();
                atomic.registers = initial_registers;
                let mut bus = FlatBus::new(&rom);

                let expected_tstates = atomic.step(&mut bus).unwrap();
                retired(
                    temporal.step_instruction(0).unwrap(),
                    opcode,
                    expected_tstates,
                );

                assert_eq!(
                    temporal.cpu().registers,
                    atomic.registers,
                    "opcode {opcode:02X}"
                );
                assert_eq!(temporal.cpu().refresh, atomic.r, "opcode {opcode:02X}");
                assert_eq!(
                    temporal.now(),
                    MachineTime::from_ticks(u128::from(expected_tstates) * 3),
                    "opcode {opcode:02X}"
                );
            }
        }
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
        let read = match complete_m1_fetch(&mut machine, 0) {
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
        assert_eq!(trace.len(), 7);
        assert!(matches!(
            trace[0],
            super::SmsTemporalTraceEvent::M1TStateCompleted {
                timestamp,
                phase: sega_z80_cpu::Z80M1Phase::T1,
                ..
            } if timestamp == MachineTime::from_ticks(3)
        ));
        assert!(matches!(
            trace[3],
            super::SmsTemporalTraceEvent::M1TStateCompleted {
                timestamp,
                phase: sega_z80_cpu::Z80M1Phase::T4,
                ..
            } if timestamp == MachineTime::from_ticks(12)
        ));
        assert!(matches!(
            trace[4],
            super::SmsTemporalTraceEvent::BusCompleted {
                timestamp,
                operation: sega_z80_cpu::Z80BusOperation::MemoryRead,
                base_tstates: 3,
                data: Some(0),
                ..
            } if timestamp == MachineTime::from_ticks(21)
        ));
        assert!(matches!(
            trace[5],
            super::SmsTemporalTraceEvent::InternalCycleCompleted {
                timestamp,
                tstates: 1,
                ..
            } if timestamp == MachineTime::from_ticks(24)
        ));
        assert!(matches!(
            trace[6],
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
        assert!(matches!(
            complete_m1_fetch(&mut machine, 0),
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
        assert!(matches!(
            complete_m1_fetch(&mut original, 0),
            Z80MicroStep::BusRequest(_)
        ));
        // The four M1 phase boundaries are captured separately from the mapper write.
        assert_eq!(original.take_trace().len(), 4);

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

    #[test]
    fn alu_immediate_and_hl_matrices_match_atomic_reference_through_scheduler() {
        for operation in 0_u8..8 {
            let immediate_opcode = 0xC6 | (operation << 3);
            let memory_opcode = 0x86 | (operation << 3);
            let operand = 0x13;
            let initial_registers = Z80Registers {
                a: 0x27,
                f: 0x01,
                ..Z80Registers::default()
            };

            let mut immediate_rom = vec![0; 0x4000];
            immediate_rom[0] = immediate_opcode;
            immediate_rom[1] = operand;
            let mut immediate_temporal = SmsTemporalMachine::new(immediate_rom.clone(), 3).unwrap();
            immediate_temporal.cpu_mut().registers = initial_registers;
            let mut immediate_atomic = crate::Z80::default();
            immediate_atomic.registers = initial_registers;
            let mut immediate_bus = FlatBus::new(&immediate_rom);
            let expected_immediate_tstates = immediate_atomic.step(&mut immediate_bus).unwrap();
            retired(
                immediate_temporal.step_instruction(0).unwrap(),
                immediate_opcode,
                expected_immediate_tstates,
            );
            assert_eq!(
                immediate_temporal.cpu().registers,
                immediate_atomic.registers
            );
            assert_eq!(immediate_temporal.cpu().refresh, immediate_atomic.r);
            assert_eq!(
                immediate_temporal.now(),
                MachineTime::from_ticks(u128::from(expected_immediate_tstates) * 3)
            );

            let mut memory_rom = vec![0; 0x4000];
            memory_rom[0] = memory_opcode;
            let mut memory_temporal = SmsTemporalMachine::new(memory_rom.clone(), 3).unwrap();
            let mut memory_registers = initial_registers;
            memory_registers.set_hl(0xC123);
            memory_temporal.cpu_mut().registers = memory_registers;
            memory_temporal.bus.write_memory_for_test(0xC123, operand);
            let mut memory_atomic = crate::Z80::default();
            memory_atomic.registers = memory_registers;
            let mut memory_bus = FlatBus::new(&memory_rom);
            memory_bus.memory[0xC123] = operand;
            let expected_memory_tstates = memory_atomic.step(&mut memory_bus).unwrap();
            retired(
                memory_temporal.step_instruction(0).unwrap(),
                memory_opcode,
                expected_memory_tstates,
            );
            assert_eq!(memory_temporal.cpu().registers, memory_atomic.registers);
            assert_eq!(memory_temporal.cpu().refresh, memory_atomic.r);
            assert_eq!(
                memory_temporal.now(),
                MachineTime::from_ticks(u128::from(expected_memory_tstates) * 3)
            );
        }
    }

    #[test]
    fn jp_hl_and_ex_de_hl_match_atomic_reference_through_scheduler() {
        for (opcode, de, hl) in [(0xE9, 0x2345, 0xC123), (0xEB, 0x2345, 0x6789)] {
            let mut rom = vec![0; 0x4000];
            rom[0] = opcode;
            let initial_registers = Z80Registers {
                a: 0xA5,
                f: 0xD7,
                b: 0x11,
                c: 0x22,
                d: (de >> 8) as u8,
                e: de as u8,
                h: (hl >> 8) as u8,
                l: hl as u8,
                sp: 0x7788,
                pc: 0,
            };

            let mut temporal = SmsTemporalMachine::new(rom.clone(), 3).unwrap();
            temporal.cpu_mut().registers = initial_registers;
            let mut atomic = crate::Z80::default();
            atomic.registers = initial_registers;
            let mut atomic_bus = FlatBus::new(&rom);
            let expected_tstates = atomic.step(&mut atomic_bus).unwrap();

            retired(
                temporal.step_instruction(0).unwrap(),
                opcode,
                expected_tstates,
            );
            assert_eq!(expected_tstates, 4);
            assert_eq!(temporal.cpu().registers, atomic.registers);
            assert_eq!(temporal.cpu().refresh, atomic.r);
            assert_eq!(temporal.cpu().q, atomic.q);
            assert_eq!(
                temporal.now(),
                MachineTime::from_ticks(u128::from(expected_tstates) * 3)
            );
        }
    }

    #[test]
    fn ld_register_pair_immediate_matrix_matches_atomic_reference_through_scheduler() {
        for (opcode, low, high) in [
            (0x01, 0x34, 0x12),
            (0x11, 0x78, 0x56),
            (0x21, 0xBC, 0x9A),
            (0x31, 0xF0, 0xDE),
        ] {
            let mut rom = vec![0; 0x4000];
            rom[0] = opcode;
            rom[1] = low;
            rom[2] = high;
            let initial_registers = Z80Registers {
                a: 0xA5,
                f: 0xD7,
                b: 0x11,
                c: 0x22,
                d: 0x33,
                e: 0x44,
                h: 0x55,
                l: 0x66,
                sp: 0x7788,
                pc: 0,
            };

            let mut temporal = SmsTemporalMachine::new(rom.clone(), 3).unwrap();
            temporal.cpu_mut().registers = initial_registers;
            let mut atomic = crate::Z80::default();
            atomic.registers = initial_registers;
            let mut atomic_bus = FlatBus::new(&rom);
            let expected_tstates = atomic.step(&mut atomic_bus).unwrap();

            retired(
                temporal.step_instruction(0).unwrap(),
                opcode,
                expected_tstates,
            );
            assert_eq!(expected_tstates, 10);
            assert_eq!(temporal.cpu().registers, atomic.registers);
            assert_eq!(temporal.cpu().refresh, atomic.r);
            assert_eq!(temporal.cpu().q, atomic.q);
            assert_eq!(
                temporal.now(),
                MachineTime::from_ticks(u128::from(expected_tstates) * 3)
            );
        }
    }

    #[test]
    fn ld_register_pair_immediate_wait_states_follow_scheduler_deadlines() {
        let mut machine = SmsTemporalMachine::new(vec![0x21, 0x34, 0x12], 3).unwrap();
        retired(machine.step_instruction(1).unwrap(), 0x21, 13);
        assert_eq!(machine.cpu().registers.hl(), 0x1234);
        assert_eq!(machine.cpu().registers.pc, 3);
        assert_eq!(machine.now(), MachineTime::from_ticks(39));
    }

    #[test]
    fn accumulator_rotate_daa_and_cpl_match_atomic_reference_through_scheduler() {
        for (opcode, accumulator, flags) in [
            (0x07, 0x81, 0xC4), // RLCA: bit 7 wraps into Carry
            (0x0F, 0x01, 0xC4), // RRCA: bit 0 wraps into Carry
            (0x17, 0x81, 0x01), // RLA: old Carry enters bit 0
            (0x1F, 0x01, 0x01), // RRA: old Carry enters bit 7
            (0x27, 0x0A, 0x00), // DAA after addition
            (0x27, 0x9A, 0x00), // DAA sets decimal carry
            (0x27, 0x10, 0x12), // DAA after subtraction with half-borrow
            (0x2F, 0x55, 0xC5), // CPL preserves S/Z/PV/C
        ] {
            assert_atomic_differential(&[opcode], accumulator, flags, 0, 0, opcode);
        }
    }
    #[test]
    fn nop_opcode_fetch_uses_four_scheduler_driven_m1_tstates() {
        let mut machine = SmsTemporalMachine::new(vec![0x00; 0x4000], 3).unwrap();
        machine.set_trace_enabled(true);
        retired(machine.step_instruction(0).unwrap(), 0x00, 4);
        assert_eq!(machine.now(), MachineTime::from_ticks(12));
        assert_eq!(machine.cpu().refresh, 1);
        let trace = machine.take_trace();
        let m1_events = trace
            .iter()
            .filter(|event| {
                matches!(
                    event,
                    super::SmsTemporalTraceEvent::M1TStateCompleted { .. }
                )
            })
            .count();
        assert_eq!(m1_events, 4);
        assert!(machine.export_state().m1.is_none());
    }

    #[test]
    fn opcode_fetch_wait_states_are_scheduler_tw_events() {
        let mut machine = SmsTemporalMachine::new(vec![0x00; 0x4000], 2).unwrap();
        retired(machine.step_instruction(2).unwrap(), 0x00, 6);
        assert_eq!(machine.now(), MachineTime::from_ticks(12));
        assert_eq!(machine.cpu().refresh, 1);
    }

    #[test]
    fn m1_snapshot_restores_pending_tstate_and_retires_exactly_once() {
        let mut machine = SmsTemporalMachine::new(vec![0x00; 0x4000], 3).unwrap();
        machine.begin_instruction().unwrap();
        machine.schedule_pending_bus(1).unwrap();
        assert!(matches!(
            machine.dispatch_next().unwrap(),
            Z80MicroStep::BusRequest(_)
        ));
        machine.schedule_pending_bus(1).unwrap();
        let snapshot = machine.export_state();
        assert!(snapshot.m1.is_some());
        let mut restored = SmsTemporalMachine::restore_state(snapshot).unwrap();
        retired(restored.resume_instruction(1).unwrap(), 0x00, 5);
        assert_eq!(restored.now(), MachineTime::from_ticks(15));
        assert_eq!(restored.cpu().registers.pc, 1);
        assert!(restored.export_state().m1.is_none());
    }
}

#[cfg(test)]
mod base_absolute_transfer_temporal_tests {
    use super::SmsTemporalMachine;
    use sega_temporal_core::MachineTime;
    use sega_z80_cpu::Z80MicroStep;

    #[test]
    fn absolute_memory_transfers_run_through_sms_scheduler_and_preserve_timing() {
        let mut store_hl = SmsTemporalMachine::new(vec![0x22, 0x00, 0xC0], 3).unwrap();
        store_hl.cpu_mut().registers.set_hl(0xBEEF);
        assert!(matches!(
            store_hl.step_instruction(0).unwrap(),
            Z80MicroStep::InstructionRetired {
                opcode: 0x22,
                tstates: 16
            }
        ));
        let state = store_hl.export_state();
        assert_eq!(state.bus.memory.read(0xC000), 0xEF);
        assert_eq!(state.bus.memory.read(0xC001), 0xBE);
        assert_eq!(store_hl.now(), MachineTime::from_ticks(48));

        let mut load_hl = SmsTemporalMachine::new(vec![0x2A, 0x03, 0x00, 0x34, 0x12], 3).unwrap();
        assert!(matches!(
            load_hl.step_instruction(0).unwrap(),
            Z80MicroStep::InstructionRetired {
                opcode: 0x2A,
                tstates: 16
            }
        ));
        assert_eq!(load_hl.cpu().registers.hl(), 0x1234);
        assert_eq!(load_hl.now(), MachineTime::from_ticks(48));

        let mut store_a = SmsTemporalMachine::new(vec![0x32, 0x00, 0xC0], 3).unwrap();
        store_a.cpu_mut().registers.a = 0xA7;
        assert!(matches!(
            store_a.step_instruction(0).unwrap(),
            Z80MicroStep::InstructionRetired {
                opcode: 0x32,
                tstates: 13
            }
        ));
        assert_eq!(store_a.export_state().bus.memory.read(0xC000), 0xA7);
        assert_eq!(store_a.now(), MachineTime::from_ticks(39));

        let mut load_a = SmsTemporalMachine::new(vec![0x3A, 0x03, 0x00, 0x5A], 3).unwrap();
        assert!(matches!(
            load_a.step_instruction(0).unwrap(),
            Z80MicroStep::InstructionRetired {
                opcode: 0x3A,
                tstates: 13
            }
        ));
        assert_eq!(load_a.cpu().registers.a, 0x5A);
        assert_eq!(load_a.now(), MachineTime::from_ticks(39));
    }
}

#[cfg(test)]
mod base_relative_branch_temporal_tests {
    use super::SmsTemporalMachine;
    use sega_temporal_core::MachineTime;
    use sega_z80_cpu::Z80MicroStep;

    #[test]
    fn relative_branches_advance_through_scheduler_with_nominal_timing() {
        let mut jr = SmsTemporalMachine::new(vec![0x18, 0x02, 0x00, 0x00], 3).unwrap();
        assert_eq!(
            jr.step_instruction(0).unwrap(),
            Z80MicroStep::InstructionRetired {
                opcode: 0x18,
                tstates: 12,
            }
        );
        assert_eq!(jr.cpu().registers.pc, 4);
        assert_eq!(jr.cpu().wz, 4);
        assert_eq!(jr.now(), MachineTime::from_ticks(36));

        let mut djnz_taken = SmsTemporalMachine::new(vec![0x10, 0xFE], 3).unwrap();
        djnz_taken.cpu_mut().registers.b = 2;
        assert_eq!(
            djnz_taken.step_instruction(0).unwrap(),
            Z80MicroStep::InstructionRetired {
                opcode: 0x10,
                tstates: 13,
            }
        );
        assert_eq!(djnz_taken.cpu().registers.b, 1);
        assert_eq!(djnz_taken.cpu().registers.pc, 0);
        assert_eq!(djnz_taken.cpu().wz, 0);
        assert_eq!(djnz_taken.now(), MachineTime::from_ticks(39));

        let mut djnz_not_taken = SmsTemporalMachine::new(vec![0x10, 0xFE], 3).unwrap();
        djnz_not_taken.cpu_mut().registers.b = 1;
        djnz_not_taken.cpu_mut().wz = 0xA328;
        assert_eq!(
            djnz_not_taken.step_instruction(0).unwrap(),
            Z80MicroStep::InstructionRetired {
                opcode: 0x10,
                tstates: 8,
            }
        );
        assert_eq!(djnz_not_taken.cpu().registers.b, 0);
        assert_eq!(djnz_not_taken.cpu().registers.pc, 2);
        assert_eq!(djnz_not_taken.cpu().wz, 0xA328);
        assert_eq!(djnz_not_taken.now(), MachineTime::from_ticks(24));
    }

    #[test]
    fn relative_branch_internal_safe_point_restores_with_scheduler_state() {
        let mut machine = SmsTemporalMachine::new(vec![0x18, 0x02, 0x00, 0x00], 3).unwrap();
        let first = machine.begin_instruction().unwrap();
        assert!(matches!(first, Z80MicroStep::BusRequest(_)));

        let mut after_m1 = first;
        for _ in 0..4 {
            machine.schedule_pending_bus(0).unwrap();
            after_m1 = machine.dispatch_next().unwrap();
        }
        assert!(matches!(after_m1, Z80MicroStep::BusRequest(_)));

        machine.schedule_pending_bus(0).unwrap();
        let after_displacement = machine.dispatch_next().unwrap();
        assert_eq!(
            after_displacement,
            Z80MicroStep::InternalCycle { tstates: 1 }
        );

        let snapshot = machine.export_state();
        let mut restored = SmsTemporalMachine::restore_state(snapshot).unwrap();
        assert_eq!(
            restored.resume_instruction(0).unwrap(),
            Z80MicroStep::InstructionRetired {
                opcode: 0x18,
                tstates: 12,
            }
        );
        assert_eq!(restored.cpu().registers.pc, 4);
        assert_eq!(restored.now(), MachineTime::from_ticks(36));
    }
}

#[cfg(test)]
mod base_absolute_jump_temporal_tests {
    use super::SmsTemporalMachine;
    use sega_temporal_core::MachineTime;
    use sega_z80_cpu::Z80MicroStep;

    #[test]
    fn absolute_jumps_run_through_sms_scheduler_with_ten_tstates() {
        let mut jp = SmsTemporalMachine::new(vec![0xC3, 0x03, 0x00, 0x00, 0x00], 3).unwrap();
        assert_eq!(
            jp.step_instruction(0).unwrap(),
            Z80MicroStep::InstructionRetired {
                opcode: 0xC3,
                tstates: 10,
            }
        );
        assert_eq!(jp.cpu().registers.pc, 3);
        assert_eq!(jp.cpu().wz, 3);
        assert_eq!(jp.now(), MachineTime::from_ticks(30));

        let mut jp_not_taken =
            SmsTemporalMachine::new(vec![0xC2, 0x04, 0x00, 0x00, 0x00], 3).unwrap();
        jp_not_taken.cpu_mut().registers.f = 0x40;
        assert_eq!(
            jp_not_taken.step_instruction(0).unwrap(),
            Z80MicroStep::InstructionRetired {
                opcode: 0xC2,
                tstates: 10,
            }
        );
        assert_eq!(jp_not_taken.cpu().registers.pc, 3);
        assert_eq!(jp_not_taken.cpu().wz, 4);
        assert_eq!(jp_not_taken.now(), MachineTime::from_ticks(30));
    }

    #[test]
    fn absolute_jump_pending_immediate_read_restores_with_scheduler_state() {
        let mut machine = SmsTemporalMachine::new(vec![0xC3, 0x03, 0x00, 0x00], 3).unwrap();
        let first = machine.begin_instruction().unwrap();
        assert!(matches!(first, Z80MicroStep::BusRequest(_)));

        let mut after_m1 = first;
        for _ in 0..4 {
            machine.schedule_pending_bus(0).unwrap();
            after_m1 = machine.dispatch_next().unwrap();
        }
        assert!(matches!(after_m1, Z80MicroStep::BusRequest(_)));

        machine.schedule_pending_bus(0).unwrap();
        let after_low = machine.dispatch_next().unwrap();
        assert!(matches!(after_low, Z80MicroStep::BusRequest(_)));

        machine.schedule_pending_bus(0).unwrap();
        let snapshot = machine.export_state();
        let mut restored = SmsTemporalMachine::restore_state(snapshot).unwrap();
        assert_eq!(
            restored.resume_instruction(0).unwrap(),
            Z80MicroStep::InstructionRetired {
                opcode: 0xC3,
                tstates: 10,
            }
        );
        assert_eq!(restored.cpu().registers.pc, 3);
        assert_eq!(restored.now(), MachineTime::from_ticks(30));
    }
}

#[cfg(test)]
mod base_stack_temporal_tests {
    use super::SmsTemporalMachine;
    use sega_temporal_core::MachineTime;
    use sega_z80_cpu::Z80MicroStep;

    #[test]
    fn push_and_pop_use_scheduler_bus_cycles_and_nominal_timing() {
        let mut push = SmsTemporalMachine::new(vec![0xC5], 3).unwrap();
        push.cpu_mut().registers.b = 0x12;
        push.cpu_mut().registers.c = 0x34;
        push.cpu_mut().registers.sp = 0xC100;
        assert_eq!(
            push.step_instruction(0).unwrap(),
            Z80MicroStep::InstructionRetired {
                opcode: 0xC5,
                tstates: 11,
            }
        );
        assert_eq!(push.cpu().registers.sp, 0xC0FE);
        assert_eq!(push.bus.memory().read(0xC0FF), 0x12);
        assert_eq!(push.bus.memory().read(0xC0FE), 0x34);
        assert_eq!(push.now(), MachineTime::from_ticks(33));

        let mut pop = SmsTemporalMachine::new(vec![0xF1], 3).unwrap();
        pop.cpu_mut().registers.sp = 0xC0FE;
        pop.bus.write_memory_for_test(0xC0FE, 0x34);
        pop.bus.write_memory_for_test(0xC0FF, 0x12);
        assert_eq!(
            pop.step_instruction(0).unwrap(),
            Z80MicroStep::InstructionRetired {
                opcode: 0xF1,
                tstates: 10,
            }
        );
        assert_eq!(pop.cpu().registers.sp, 0xC100);
        assert_eq!(pop.cpu().registers.a, 0x12);
        assert_eq!(pop.cpu().registers.f, 0x34);
        assert_eq!(pop.now(), MachineTime::from_ticks(30));
    }

    #[test]
    fn push_internal_safe_point_restores_with_scheduler_state() {
        let mut machine = SmsTemporalMachine::new(vec![0xC5], 3).unwrap();
        machine.cpu_mut().registers.set_bc(0x1234);
        machine.cpu_mut().registers.sp = 0xC100;
        let first = machine.begin_instruction().unwrap();
        assert!(matches!(first, Z80MicroStep::BusRequest(_)));

        let mut after_m1 = first;
        for _ in 0..4 {
            machine.schedule_pending_bus(0).unwrap();
            after_m1 = machine.dispatch_next().unwrap();
        }
        assert_eq!(after_m1, Z80MicroStep::InternalCycle { tstates: 1 });

        let snapshot = machine.export_state();
        let mut restored = SmsTemporalMachine::restore_state(snapshot).unwrap();
        assert_eq!(
            restored.resume_instruction(0).unwrap(),
            Z80MicroStep::InstructionRetired {
                opcode: 0xC5,
                tstates: 11,
            }
        );
        assert_eq!(restored.cpu().registers.sp, 0xC0FE);
        assert_eq!(restored.bus.memory().read(0xC0FF), 0x12);
        assert_eq!(restored.bus.memory().read(0xC0FE), 0x34);
        assert_eq!(restored.now(), MachineTime::from_ticks(33));
    }
}

#[cfg(test)]
mod base_call_rst_temporal_tests {
    use super::SmsTemporalMachine;
    use sega_temporal_core::MachineTime;
    use sega_z80_cpu::Z80MicroStep;

    fn advance_m1(machine: &mut SmsTemporalMachine) -> Z80MicroStep {
        let mut step = machine.begin_instruction().unwrap();
        for _ in 0..4 {
            machine.schedule_pending_bus(0).unwrap();
            step = machine.dispatch_next().unwrap();
        }
        step
    }

    #[test]
    fn call_and_rst_use_scheduler_stack_transactions_and_timing() {
        let mut call = SmsTemporalMachine::new(vec![0xCD, 0x03, 0x00], 3).unwrap();
        call.cpu_mut().registers.sp = 0xC100;
        assert_eq!(
            call.step_instruction(0).unwrap(),
            Z80MicroStep::InstructionRetired {
                opcode: 0xCD,
                tstates: 17,
            }
        );
        assert_eq!(call.cpu().registers.pc, 3);
        assert_eq!(call.cpu().wz, 3);
        assert_eq!(call.cpu().registers.sp, 0xC0FE);
        assert_eq!(call.bus.memory().read(0xC0FF), 0x00);
        assert_eq!(call.bus.memory().read(0xC0FE), 0x03);
        assert_eq!(call.now(), MachineTime::from_ticks(51));

        let mut call_not_taken = SmsTemporalMachine::new(vec![0xC4, 0x04, 0x00], 3).unwrap();
        call_not_taken.cpu_mut().registers.sp = 0xC100;
        call_not_taken.cpu_mut().registers.f = 0x40;
        assert_eq!(
            call_not_taken.step_instruction(0).unwrap(),
            Z80MicroStep::InstructionRetired {
                opcode: 0xC4,
                tstates: 10,
            }
        );
        assert_eq!(call_not_taken.cpu().registers.pc, 3);
        assert_eq!(call_not_taken.cpu().wz, 4);
        assert_eq!(call_not_taken.cpu().registers.sp, 0xC100);
        assert_eq!(call_not_taken.now(), MachineTime::from_ticks(30));

        let mut rst = SmsTemporalMachine::new(vec![0xFF], 3).unwrap();
        rst.cpu_mut().registers.sp = 0xC100;
        assert_eq!(
            rst.step_instruction(0).unwrap(),
            Z80MicroStep::InstructionRetired {
                opcode: 0xFF,
                tstates: 11,
            }
        );
        assert_eq!(rst.cpu().registers.pc, 0x0038);
        assert_eq!(rst.cpu().wz, 0x0038);
        assert_eq!(rst.cpu().registers.sp, 0xC0FE);
        assert_eq!(rst.bus.memory().read(0xC0FF), 0x00);
        assert_eq!(rst.bus.memory().read(0xC0FE), 0x01);
        assert_eq!(rst.now(), MachineTime::from_ticks(33));
    }

    #[test]
    fn call_and_rst_internal_safe_points_restore_with_scheduler_state() {
        let mut call = SmsTemporalMachine::new(vec![0xCD, 0x03, 0x00], 3).unwrap();
        call.cpu_mut().registers.sp = 0xC100;
        assert!(matches!(advance_m1(&mut call), Z80MicroStep::BusRequest(_)));
        call.schedule_pending_bus(0).unwrap();
        assert!(matches!(
            call.dispatch_next().unwrap(),
            Z80MicroStep::BusRequest(_)
        ));
        call.schedule_pending_bus(0).unwrap();
        assert_eq!(
            call.dispatch_next().unwrap(),
            Z80MicroStep::InternalCycle { tstates: 1 }
        );
        let snapshot = call.export_state();
        let mut restored_call = SmsTemporalMachine::restore_state(snapshot).unwrap();
        assert_eq!(
            restored_call.resume_instruction(0).unwrap(),
            Z80MicroStep::InstructionRetired {
                opcode: 0xCD,
                tstates: 17,
            }
        );
        assert_eq!(restored_call.cpu().registers.pc, 3);
        assert_eq!(restored_call.now(), MachineTime::from_ticks(51));

        let mut rst = SmsTemporalMachine::new(vec![0xFF], 3).unwrap();
        rst.cpu_mut().registers.sp = 0xC100;
        assert_eq!(
            advance_m1(&mut rst),
            Z80MicroStep::InternalCycle { tstates: 1 }
        );
        let snapshot = rst.export_state();
        let mut restored_rst = SmsTemporalMachine::restore_state(snapshot).unwrap();
        assert_eq!(
            restored_rst.resume_instruction(0).unwrap(),
            Z80MicroStep::InstructionRetired {
                opcode: 0xFF,
                tstates: 11,
            }
        );
        assert_eq!(restored_rst.cpu().registers.pc, 0x0038);
        assert_eq!(restored_rst.now(), MachineTime::from_ticks(33));
    }
}

#[cfg(test)]
mod base_return_temporal_tests {
    use super::SmsTemporalMachine;
    use sega_temporal_core::MachineTime;
    use sega_z80_cpu::Z80MicroStep;

    fn advance_m1(machine: &mut SmsTemporalMachine) -> Z80MicroStep {
        let mut step = machine.begin_instruction().unwrap();
        for _ in 0..4 {
            machine.schedule_pending_bus(0).unwrap();
            step = machine.dispatch_next().unwrap();
        }
        step
    }

    #[test]
    fn ret_and_conditional_ret_use_scheduler_stack_reads_and_timing() {
        let mut ret = SmsTemporalMachine::new(vec![0xC9], 3).unwrap();
        ret.cpu_mut().registers.sp = 0xC0FE;
        ret.bus.write_memory_for_test(0xC0FE, 0x34);
        ret.bus.write_memory_for_test(0xC0FF, 0x12);
        assert_eq!(
            ret.step_instruction(0).unwrap(),
            Z80MicroStep::InstructionRetired {
                opcode: 0xC9,
                tstates: 10,
            }
        );
        assert_eq!(ret.cpu().registers.pc, 0x1234);
        assert_eq!(ret.cpu().wz, 0x1234);
        assert_eq!(ret.cpu().registers.sp, 0xC100);
        assert_eq!(ret.now(), MachineTime::from_ticks(30));

        let mut ret_taken = SmsTemporalMachine::new(vec![0xC0], 3).unwrap();
        ret_taken.cpu_mut().registers.sp = 0xC0FE;
        ret_taken.cpu_mut().registers.f = 0x00;
        ret_taken.bus.write_memory_for_test(0xC0FE, 0x34);
        ret_taken.bus.write_memory_for_test(0xC0FF, 0x12);
        assert_eq!(
            ret_taken.step_instruction(0).unwrap(),
            Z80MicroStep::InstructionRetired {
                opcode: 0xC0,
                tstates: 11,
            }
        );
        assert_eq!(ret_taken.cpu().registers.pc, 0x1234);
        assert_eq!(ret_taken.cpu().wz, 0x1234);
        assert_eq!(ret_taken.cpu().registers.sp, 0xC100);
        assert_eq!(ret_taken.now(), MachineTime::from_ticks(33));

        let mut ret_not_taken = SmsTemporalMachine::new(vec![0xC0], 3).unwrap();
        ret_not_taken.cpu_mut().registers.sp = 0xC0FE;
        ret_not_taken.cpu_mut().registers.f = 0x40;
        ret_not_taken.cpu_mut().wz = 0xA328;
        assert_eq!(
            ret_not_taken.step_instruction(0).unwrap(),
            Z80MicroStep::InstructionRetired {
                opcode: 0xC0,
                tstates: 5,
            }
        );
        assert_eq!(ret_not_taken.cpu().registers.pc, 1);
        assert_eq!(ret_not_taken.cpu().wz, 0xA328);
        assert_eq!(ret_not_taken.cpu().registers.sp, 0xC0FE);
        assert_eq!(ret_not_taken.now(), MachineTime::from_ticks(15));
    }

    #[test]
    fn conditional_return_internal_safe_point_restores_with_scheduler_state() {
        let mut machine = SmsTemporalMachine::new(vec![0xC0], 3).unwrap();
        machine.cpu_mut().registers.sp = 0xC0FE;
        machine.cpu_mut().registers.f = 0x00;
        machine.bus.write_memory_for_test(0xC0FE, 0x34);
        machine.bus.write_memory_for_test(0xC0FF, 0x12);
        assert_eq!(
            advance_m1(&mut machine),
            Z80MicroStep::InternalCycle { tstates: 1 }
        );

        let snapshot = machine.export_state();
        let mut restored = SmsTemporalMachine::restore_state(snapshot).unwrap();
        assert_eq!(
            restored.resume_instruction(0).unwrap(),
            Z80MicroStep::InstructionRetired {
                opcode: 0xC0,
                tstates: 11,
            }
        );
        assert_eq!(restored.cpu().registers.pc, 0x1234);
        assert_eq!(restored.now(), MachineTime::from_ticks(33));
    }
}

#[cfg(test)]
mod base_exchange_interrupt_control_temporal_tests {
    use super::SmsTemporalMachine;
    use sega_temporal_core::MachineTime;
    use sega_z80_cpu::Z80MicroStep;

    #[test]
    fn exchange_and_interrupt_control_advance_four_scheduler_tstates() {
        let mut ex_af = SmsTemporalMachine::new(vec![0x08], 3).unwrap();
        ex_af.cpu_mut().registers.a = 0x12;
        ex_af.cpu_mut().registers.f = 0xA5;
        ex_af.cpu_mut().alternate.a = 0x34;
        ex_af.cpu_mut().alternate.f = 0x56;
        assert_eq!(
            ex_af.step_instruction(0).unwrap(),
            Z80MicroStep::InstructionRetired {
                opcode: 0x08,
                tstates: 4,
            }
        );
        assert_eq!(ex_af.cpu().registers.a, 0x34);
        assert_eq!(ex_af.cpu().registers.f, 0x56);
        assert_eq!(ex_af.now(), MachineTime::from_ticks(12));

        let mut exx = SmsTemporalMachine::new(vec![0xD9], 3).unwrap();
        exx.cpu_mut().registers.set_bc(0x1234);
        exx.cpu_mut().registers.set_de(0x5678);
        exx.cpu_mut().registers.set_hl(0x9ABC);
        exx.cpu_mut().alternate.b = 0xAB;
        exx.cpu_mut().alternate.c = 0xCD;
        exx.cpu_mut().alternate.d = 0xEF;
        exx.cpu_mut().alternate.e = 0x01;
        exx.cpu_mut().alternate.h = 0x23;
        exx.cpu_mut().alternate.l = 0x45;
        assert_eq!(
            exx.step_instruction(0).unwrap(),
            Z80MicroStep::InstructionRetired {
                opcode: 0xD9,
                tstates: 4,
            }
        );
        assert_eq!(exx.cpu().registers.bc(), 0xABCD);
        assert_eq!(exx.cpu().registers.de(), 0xEF01);
        assert_eq!(exx.cpu().registers.hl(), 0x2345);
        assert_eq!(exx.now(), MachineTime::from_ticks(12));

        let mut di = SmsTemporalMachine::new(vec![0xF3], 3).unwrap();
        assert_eq!(
            di.step_instruction(0).unwrap(),
            Z80MicroStep::InstructionRetired {
                opcode: 0xF3,
                tstates: 4,
            }
        );
        assert!(!di.cpu().iff1);
        assert!(!di.cpu().iff2);
        assert_eq!(di.cpu().ei_delay, 0);
        assert_eq!(di.now(), MachineTime::from_ticks(12));

        let mut ei = SmsTemporalMachine::new(vec![0xFB, 0x00], 3).unwrap();
        assert_eq!(
            ei.step_instruction(0).unwrap(),
            Z80MicroStep::InstructionRetired {
                opcode: 0xFB,
                tstates: 4,
            }
        );
        assert!(ei.cpu().iff1);
        assert!(ei.cpu().iff2);
        assert_eq!(ei.cpu().ei_delay, 1);
        assert_eq!(
            ei.step_instruction(0).unwrap(),
            Z80MicroStep::InstructionRetired {
                opcode: 0x00,
                tstates: 4,
            }
        );
        assert_eq!(ei.cpu().ei_delay, 0);
        assert_eq!(ei.now(), MachineTime::from_ticks(24));
    }
}

#[cfg(test)]
mod base_exchange_stack_temporal_tests {
    use super::SmsTemporalMachine;
    use sega_temporal_core::MachineTime;
    use sega_z80_cpu::Z80MicroStep;

    fn advance_m1(machine: &mut SmsTemporalMachine) -> Z80MicroStep {
        let mut step = machine.begin_instruction().unwrap();
        for _ in 0..4 {
            machine.schedule_pending_bus(0).unwrap();
            step = machine.dispatch_next().unwrap();
        }
        step
    }

    #[test]
    fn ex_sp_hl_uses_five_scheduler_machine_cycles() {
        let mut machine = SmsTemporalMachine::new(vec![0xE3], 3).unwrap();
        machine.cpu_mut().registers.sp = 0xC100;
        machine.cpu_mut().registers.set_hl(0xABCD);
        machine.bus.write_memory_for_test(0xC100, 0x34);
        machine.bus.write_memory_for_test(0xC101, 0x12);

        assert_eq!(
            machine.step_instruction(0).unwrap(),
            Z80MicroStep::InstructionRetired {
                opcode: 0xE3,
                tstates: 19,
            }
        );
        assert_eq!(machine.cpu().registers.hl(), 0x1234);
        assert_eq!(machine.cpu().wz, 0x1234);
        assert_eq!(machine.cpu().registers.sp, 0xC100);
        assert_eq!(machine.bus.memory().read(0xC100), 0xCD);
        assert_eq!(machine.bus.memory().read(0xC101), 0xAB);
        assert_eq!(machine.now(), MachineTime::from_ticks(57));
    }

    #[test]
    fn ex_sp_hl_internal_safe_point_restores_with_scheduler_state() {
        let mut machine = SmsTemporalMachine::new(vec![0xE3], 3).unwrap();
        machine.cpu_mut().registers.sp = 0xC100;
        machine.cpu_mut().registers.set_hl(0xABCD);
        machine.bus.write_memory_for_test(0xC100, 0x34);
        machine.bus.write_memory_for_test(0xC101, 0x12);
        assert!(matches!(
            advance_m1(&mut machine),
            Z80MicroStep::BusRequest(_)
        ));

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

        let snapshot = machine.export_state();
        let mut restored = SmsTemporalMachine::restore_state(snapshot).unwrap();
        assert_eq!(
            restored.resume_instruction(0).unwrap(),
            Z80MicroStep::InstructionRetired {
                opcode: 0xE3,
                tstates: 19,
            }
        );
        assert_eq!(restored.cpu().registers.hl(), 0x1234);
        assert_eq!(restored.cpu().wz, 0x1234);
        assert_eq!(restored.bus.memory().read(0xC100), 0xCD);
        assert_eq!(restored.bus.memory().read(0xC101), 0xAB);
        assert_eq!(restored.now(), MachineTime::from_ticks(57));
    }
}

#[cfg(test)]
mod base_immediate_io_temporal_tests {
    use super::SmsTemporalMachine;
    use crate::input::ControllerButtons;
    use sega_temporal_core::MachineTime;
    use sega_z80_cpu::Z80MicroStep;

    fn advance_m1(machine: &mut SmsTemporalMachine) -> Z80MicroStep {
        let mut step = machine.begin_instruction().unwrap();
        for _ in 0..4 {
            machine.schedule_pending_bus(0).unwrap();
            step = machine.dispatch_next().unwrap();
        }
        step
    }

    #[test]
    fn immediate_in_reads_controller_and_out_updates_io_control() {
        let mut input = SmsTemporalMachine::new(vec![0xDB, 0xDC], 3).unwrap();
        input.cpu_mut().registers.a = 0x12;
        input.cpu_mut().registers.f = 0xA5;
        input
            .bus
            .io_mut()
            .set_controller(
                1,
                ControllerButtons {
                    up: true,
                    ..ControllerButtons::default()
                },
            )
            .unwrap();
        assert_eq!(
            input.step_instruction(0).unwrap(),
            Z80MicroStep::InstructionRetired {
                opcode: 0xDB,
                tstates: 11,
            }
        );
        assert_eq!(input.cpu().registers.a, 0xFE);
        assert_eq!(input.cpu().registers.f, 0xA5);
        assert_eq!(input.cpu().wz, 0x12DD);
        assert_eq!(input.now(), MachineTime::from_ticks(33));

        let mut output = SmsTemporalMachine::new(vec![0xD3, 0x3F], 3).unwrap();
        output.cpu_mut().registers.a = 0xD5;
        assert_eq!(
            output.step_instruction(0).unwrap(),
            Z80MicroStep::InstructionRetired {
                opcode: 0xD3,
                tstates: 11,
            }
        );
        assert_eq!(output.bus.io().io_control(), 0xD5);
        assert_eq!(output.cpu().wz, 0xD540);
        assert_eq!(output.bus.port_write_trace().len(), 1);
        assert_eq!(output.bus.port_write_trace()[0].port, 0xD53F);
        assert_eq!(output.bus.port_write_trace()[0].value, 0xD5);
        assert_eq!(
            output.bus.port_write_trace()[0].completed_at,
            MachineTime::from_ticks(33)
        );
        assert_eq!(output.now(), MachineTime::from_ticks(33));

        let mut memory_control = SmsTemporalMachine::new(vec![0xD3, 0x3E], 3).unwrap();
        memory_control.cpu_mut().registers.a = 0x55;
        assert_eq!(
            memory_control.step_instruction(0).unwrap(),
            Z80MicroStep::InstructionRetired {
                opcode: 0xD3,
                tstates: 11,
            }
        );
        assert_eq!(memory_control.bus.memory().control_port(), 0x55);
    }

    #[test]
    fn immediate_in_port_read_pending_state_restores_with_io_context() {
        let mut machine = SmsTemporalMachine::new(vec![0xDB, 0xDC], 3).unwrap();
        machine.cpu_mut().registers.a = 0x12;
        machine
            .bus
            .io_mut()
            .set_controller(
                1,
                ControllerButtons {
                    up: true,
                    ..ControllerButtons::default()
                },
            )
            .unwrap();
        assert!(matches!(
            advance_m1(&mut machine),
            Z80MicroStep::BusRequest(_)
        ));

        machine.schedule_pending_bus(0).unwrap();
        assert!(matches!(
            machine.dispatch_next().unwrap(),
            Z80MicroStep::BusRequest(_)
        ));
        machine.schedule_pending_bus(0).unwrap();

        let snapshot = machine.export_state();
        let mut restored = SmsTemporalMachine::restore_state(snapshot).unwrap();
        assert_eq!(
            restored.resume_instruction(0).unwrap(),
            Z80MicroStep::InstructionRetired {
                opcode: 0xDB,
                tstates: 11,
            }
        );
        assert_eq!(restored.cpu().registers.a, 0xFE);
        assert_eq!(restored.now(), MachineTime::from_ticks(33));
    }
}
