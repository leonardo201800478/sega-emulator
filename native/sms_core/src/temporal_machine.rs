//! First scheduler-driven SMS Z80/memory vertical slice.
//!
//! This composition uses the resumable Z80 subset, the mapper-backed SMS
//! memory adapter, and one machine scheduler. It intentionally excludes the
//! legacy scanline VDP, PSG, ports, interrupts, DMA and physical WAIT wiring.

use core::fmt;

use crate::{SmsTemporalBus, SmsTemporalBusError, SmsTemporalBusState, SmsTemporalDispatch};
use sega_temporal_core::MachineTime;
use sega_z80_cpu::{
    Z80MicroSequencer, Z80MicroSequencerError, Z80MicroSequencerPhase, Z80MicroSequencerState,
    Z80MicroStep,
};

pub const SMS_TEMPORAL_MACHINE_STATE_VERSION: u16 = 1;

/// Versioned state spanning the CPU continuation and SMS memory/scheduler.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmsTemporalMachineState {
    pub format_version: u16,
    pub cpu: Z80MicroSequencerState,
    pub bus: SmsTemporalBusState,
    pub next_internal_cycle_id: u64,
    pub pending_internal_cycle_id: Option<u64>,
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
    next_internal_cycle_id: u64,
    pending_internal_cycle_id: Option<u64>,
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
            next_internal_cycle_id: 0,
            pending_internal_cycle_id: None,
        })
    }

    /// Start the next instruction and expose its first bus request.
    pub fn begin_instruction(&mut self) -> Result<Z80MicroStep, SmsTemporalMachineError> {
        Ok(self.cpu.begin_instruction()?)
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
        if !matches!(
            self.cpu.export_state().phase,
            Z80MicroSequencerPhase::MemoryIncDecInternal { .. }
        ) {
            return Err(SmsTemporalMachineError::InvalidState);
        }
        if self.pending_internal_cycle_id.is_some() {
            return Err(SmsTemporalMachineError::InvalidState);
        }
        let continuation_id = self.next_internal_cycle_id;
        let next_internal_cycle_id = continuation_id
            .checked_add(1)
            .ok_or(SmsTemporalMachineError::InternalCycleIdExhausted)?;
        let deadline = self.bus.schedule_internal_cycle(continuation_id, tstates)?;
        self.next_internal_cycle_id = next_internal_cycle_id;
        self.pending_internal_cycle_id = Some(continuation_id);
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
                let response = self.bus.acknowledge_completion(completion.request_id)?;
                Ok(self
                    .cpu
                    .complete_bus_cycle(response.request_id, response.data)?)
            }
            SmsTemporalDispatch::InternalCycleComplete(completion) => {
                if self.pending_internal_cycle_id != Some(completion.continuation_id) {
                    return Err(SmsTemporalMachineError::InvalidState);
                }
                self.pending_internal_cycle_id = None;
                Ok(self.cpu.complete_internal_cycle()?)
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
            bus: self.bus.export_state(),
            next_internal_cycle_id: self.next_internal_cycle_id,
            pending_internal_cycle_id: self.pending_internal_cycle_id,
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
        Ok(Self {
            cpu,
            bus,
            next_internal_cycle_id: state.next_internal_cycle_id,
            pending_internal_cycle_id: state.pending_internal_cycle_id,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{SmsTemporalMachine, SmsTemporalMachineError};
    use sega_temporal_core::MachineTime;
    use sega_z80_cpu::{Z80MicroSequencerPhase, Z80MicroStep};

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
    fn fixture_wait_is_counted_in_cpu_tstates_and_machine_time() {
        let mut machine = SmsTemporalMachine::new(vec![0x3E, 0x42], 3).unwrap();
        retired(machine.step_instruction(1).unwrap(), 0x3E, 9);
        assert_eq!(machine.now(), MachineTime::from_ticks(27));
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
        assert_eq!(temporal.memory().read(initial_hl), bus.memory[usize::from(initial_hl)]);
        assert_eq!(temporal.now(), MachineTime::from_ticks(u128::from(expected_tstates) * 3));
    }

    #[test]
    fn supported_microsequencer_slice_matches_atomic_reference() {
        assert_atomic_differential(&[0x00], 0, 0, 0, 0, 0x00);
        assert_atomic_differential(&[0x3E, 0x42], 0, 0, 0, 0, 0x3E);
        assert_atomic_differential(&[0x77], 0x5A, 0, 0xC000, 0, 0x77);
        assert_atomic_differential(&[0x34], 0, 0, 0xC000, 0x7F, 0x34);
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
}
