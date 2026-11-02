//! First resumable instruction slice for the shared Z80.
//!
//! This deliberately small sequencer proves real suspend/resume semantics for
//! NOP, LD A,n, LD A,(HL), LD (HL),A, and register-to-register LD. It is independent of the legacy
//! atomic executor; it does not yet cover interrupts, prefixes, HALT, refresh
//! pin phases, or the full architectural state.

use crate::{
    Z80_MICRO_SEQUENCER_STATE_VERSION, Z80BusHandshake, Z80BusHandshakeError, Z80BusOperation,
    Z80BusRequest, Z80Registers,
};

const FLAG_C: u8 = 0x01;
const FLAG_N: u8 = 0x02;
const FLAG_PV: u8 = 0x04;
const FLAG_H: u8 = 0x10;
const FLAG_Z: u8 = 0x40;
const FLAG_S: u8 = 0x80;
const FLAG_XY: u8 = 0x28;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Z80MicroSequencerError {
    Handshake(Z80BusHandshakeError),
    InstructionAlreadyActive,
    NoInstructionActive,
    UnexpectedCompletion,
    InstructionTStateOverflow,
    InvalidState,
    UnsupportedOpcode { opcode: u8, address: u16 },
}

impl From<Z80BusHandshakeError> for Z80MicroSequencerError {
    fn from(value: Z80BusHandshakeError) -> Self {
        Self::Handshake(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Z80MicroSequencerPhase {
    Ready,
    OpcodeFetch,
    ImmediateReadRegister {
        destination: u8,
    },
    ImmediateReadMemoryWrite,
    MemoryReadRegister {
        destination: u8,
    },
    MemoryReadIncDec {
        decrement: bool,
        address: u16,
    },
    MemoryIncDecInternal {
        decrement: bool,
        old: u8,
        address: u16,
    },
    MemoryWrite,
}

/// Observable result after a completed bus transaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Z80MicroStep {
    /// The CPU is suspended until this exact request completes.
    BusRequest(Z80BusRequest),
    /// One instruction has retired. T-states include base cycle lengths and
    /// WAIT states inserted while its bus requests were pending.
    InstructionRetired { opcode: u8, tstates: u32 },
    /// One non-bus internal T-state; advance machine time before resuming.
    InternalCycle { tstates: u32 },
}

/// Logical state needed to resume the currently implemented sequencer slice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Z80MicroSequencerState {
    pub format_version: u16,
    pub registers: Z80Registers,
    pub refresh: u8,
    pub handshake: crate::Z80BusHandshakeState,
    pub phase: Z80MicroSequencerPhase,
    pub opcode: Option<u8>,
    pub instruction_tstates: u32,
    /// Zero-based M-cycle index within the active instruction.
    pub mcycle_index: u32,
    /// Zero-based T-state position at the start of the current M-cycle.
    /// This slice exposes M-cycle boundaries; intra-cycle edges remain unmodeled.
    pub tstate_index: u16,
}

/// A resumable subset of the Z80 instruction engine.
///
/// The scheduler owns elapsed time. This type only exposes ordered bus cycles
/// and consumes their completion responses; it never reads or writes a bus
/// directly. The existing `Z80::step()` remains the full compatibility path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Z80MicroSequencer {
    pub registers: Z80Registers,
    /// Low seven bits count M1 fetches; bit 7 is preserved.
    pub refresh: u8,
    handshake: Z80BusHandshake,
    phase: Z80MicroSequencerPhase,
    opcode: Option<u8>,
    instruction_tstates: u32,
    mcycle_index: u32,
    tstate_index: u16,
}

impl Default for Z80MicroSequencer {
    fn default() -> Self {
        Self {
            registers: Z80Registers::default(),
            refresh: 0,
            handshake: Z80BusHandshake::default(),
            phase: Z80MicroSequencerPhase::Ready,
            opcode: None,
            instruction_tstates: 0,
            mcycle_index: 0,
            tstate_index: 0,
        }
    }
}

impl Z80MicroSequencer {
    /// Export the complete resumable state without advancing emulated time.
    pub fn export_state(&self) -> Z80MicroSequencerState {
        Z80MicroSequencerState {
            format_version: Z80_MICRO_SEQUENCER_STATE_VERSION,
            registers: self.registers,
            refresh: self.refresh,
            handshake: self.handshake.export_state(),
            phase: self.phase,
            opcode: self.opcode,
            instruction_tstates: self.instruction_tstates,
            mcycle_index: self.mcycle_index,
            tstate_index: self.tstate_index,
        }
    }

    /// Restore a resumable state after validating CPU/handshake cross-invariants.
    pub fn restore_state(state: Z80MicroSequencerState) -> Result<Self, Z80MicroSequencerError> {
        if state.format_version != Z80_MICRO_SEQUENCER_STATE_VERSION {
            return Err(Z80MicroSequencerError::InvalidState);
        }
        let handshake = Z80BusHandshake::restore_state(state.handshake)?;
        let pending = handshake.pending_request();
        let valid = match state.phase {
            Z80MicroSequencerPhase::Ready => {
                pending.is_none()
                    && state.opcode.is_none()
                    && state.instruction_tstates == 0
                    && state.mcycle_index == 0
                    && state.tstate_index == 0
            }
            Z80MicroSequencerPhase::OpcodeFetch => {
                pending.is_some_and(|request| request.operation == Z80BusOperation::OpcodeFetch)
                    && state.opcode.is_none()
            }
            Z80MicroSequencerPhase::ImmediateReadRegister { .. }
            | Z80MicroSequencerPhase::ImmediateReadMemoryWrite
            | Z80MicroSequencerPhase::MemoryReadRegister { .. }
            | Z80MicroSequencerPhase::MemoryReadIncDec { .. } => {
                pending.is_some_and(|request| request.operation == Z80BusOperation::MemoryRead)
                    && state.opcode.is_some()
            }
            Z80MicroSequencerPhase::MemoryIncDecInternal { .. } => {
                pending.is_none() && state.opcode.is_some()
            }
            Z80MicroSequencerPhase::MemoryWrite => {
                pending.is_some_and(|request| {
                    matches!(request.operation, Z80BusOperation::MemoryWrite { .. })
                }) && state.opcode.is_some()
            }
        };
        if !valid || state.tstate_index != 0 {
            return Err(Z80MicroSequencerError::InvalidState);
        }
        Ok(Self {
            registers: state.registers,
            refresh: state.refresh,
            handshake,
            phase: state.phase,
            opcode: state.opcode,
            instruction_tstates: state.instruction_tstates,
            mcycle_index: state.mcycle_index,
            tstate_index: state.tstate_index,
        })
    }

    /// Begin the next instruction by issuing its M1 opcode-fetch request.
    /// Architectural PC/R changes occur only when that request completes.
    pub fn begin_instruction(&mut self) -> Result<Z80MicroStep, Z80MicroSequencerError> {
        if self.phase != Z80MicroSequencerPhase::Ready || self.handshake.is_pending() {
            return Err(Z80MicroSequencerError::InstructionAlreadyActive);
        }
        self.instruction_tstates = 0;
        self.mcycle_index = 0;
        self.tstate_index = 0;
        self.opcode = None;
        let request = self
            .handshake
            .begin(Z80BusOperation::OpcodeFetch, self.registers.pc, 4)?;
        self.phase = Z80MicroSequencerPhase::OpcodeFetch;
        Ok(Z80MicroStep::BusRequest(request))
    }

    /// Add one WAIT T-state to the currently outstanding bus transaction.
    pub fn insert_wait_tstate(&mut self) -> Result<(), Z80MicroSequencerError> {
        self.handshake.insert_wait_tstate()?;
        Ok(())
    }

    /// Insert several WAIT T-states atomically into the active bus request.
    pub fn insert_wait_tstates(&mut self, tstates: u32) -> Result<(), Z80MicroSequencerError> {
        self.handshake.insert_wait_tstates(tstates)?;
        Ok(())
    }

    /// Complete the current bus request and either issue the next one or retire.
    ///
    /// Read data is consumed only here, at the completion edge supplied by the
    /// machine scheduler. A rejected response leaves the request outstanding.
    pub fn complete_bus_cycle(
        &mut self,
        request_id: u64,
        data: Option<u8>,
    ) -> Result<Z80MicroStep, Z80MicroSequencerError> {
        if self.phase == Z80MicroSequencerPhase::Ready {
            return Err(Z80MicroSequencerError::NoInstructionActive);
        }
        let request = self
            .handshake
            .pending_request()
            .ok_or(Z80MicroSequencerError::UnexpectedCompletion)?;
        let waits = self.handshake.wait_tstates().unwrap_or(0);
        let next_instruction_tstates = self
            .instruction_tstates
            .checked_add(u32::from(request.base_tstates))
            .and_then(|total| total.checked_add(waits))
            .ok_or(Z80MicroSequencerError::InstructionTStateOverflow)?;
        let response = self.handshake.complete(request_id, data)?;
        self.instruction_tstates = next_instruction_tstates;

        match self.phase {
            Z80MicroSequencerPhase::OpcodeFetch => {
                let opcode = response
                    .data
                    .ok_or(Z80MicroSequencerError::UnexpectedCompletion)?;
                self.registers.pc = self.registers.pc.wrapping_add(1);
                self.refresh = (self.refresh & 0x80) | (self.refresh.wrapping_add(1) & 0x7F);
                self.opcode = Some(opcode);
                match opcode {
                    0x00 => self.retire(opcode),
                    0x06 | 0x0E | 0x16 | 0x1E | 0x26 | 0x2E | 0x3E => {
                        let destination = (opcode >> 3) & 0x07;
                        self.issue_read(
                            Z80MicroSequencerPhase::ImmediateReadRegister { destination },
                            self.registers.pc,
                        )
                    }
                    0x36 => self.issue_read(
                        Z80MicroSequencerPhase::ImmediateReadMemoryWrite,
                        self.registers.pc,
                    ),
                    0x04..=0x3D if (opcode & 0xC7) == 0x04 || (opcode & 0xC7) == 0x05 => {
                        let register = (opcode >> 3) & 0x07;
                        if register == 6 {
                            self.issue_read(
                                Z80MicroSequencerPhase::MemoryReadIncDec {
                                    decrement: opcode & 0x01 != 0,
                                    address: self.registers.hl(),
                                },
                                self.registers.hl(),
                            )
                        } else {
                            let decrement = opcode & 0x01 != 0;
                            let old = self.read_register(register);
                            let value = self.increment_decrement_register(old, decrement);
                            self.write_register(register, value);
                            self.retire(opcode)
                        }
                    }
                    0x40..=0x7F if opcode != 0x76 => {
                        let destination = (opcode >> 3) & 0x07;
                        let source = opcode & 0x07;
                        match (destination, source) {
                            (6, 6) => self.unsupported(opcode),
                            (6, register) => {
                                let value = self.read_register(register);
                                self.issue_write(self.registers.hl(), value)
                            }
                            (register, 6) => self.issue_read(
                                Z80MicroSequencerPhase::MemoryReadRegister {
                                    destination: register,
                                },
                                self.registers.hl(),
                            ),
                            (destination, source) => {
                                let value = self.read_register(source);
                                self.write_register(destination, value);
                                self.retire(opcode)
                            }
                        }
                    }
                    _ => self.unsupported(opcode),
                }
            }
            Z80MicroSequencerPhase::ImmediateReadRegister { destination } => {
                let value = response
                    .data
                    .ok_or(Z80MicroSequencerError::UnexpectedCompletion)?;
                self.registers.pc = self.registers.pc.wrapping_add(1);
                self.write_register(destination, value);
                self.retire(
                    self.opcode
                        .ok_or(Z80MicroSequencerError::UnexpectedCompletion)?,
                )
            }
            Z80MicroSequencerPhase::ImmediateReadMemoryWrite => {
                let value = response
                    .data
                    .ok_or(Z80MicroSequencerError::UnexpectedCompletion)?;
                self.registers.pc = self.registers.pc.wrapping_add(1);
                self.issue_write(self.registers.hl(), value)
            }
            Z80MicroSequencerPhase::MemoryReadRegister { destination } => {
                let value = response
                    .data
                    .ok_or(Z80MicroSequencerError::UnexpectedCompletion)?;
                self.write_register(destination, value);
                self.retire(
                    self.opcode
                        .ok_or(Z80MicroSequencerError::UnexpectedCompletion)?,
                )
            }
            Z80MicroSequencerPhase::MemoryReadIncDec { decrement, address } => {
                let old = response
                    .data
                    .ok_or(Z80MicroSequencerError::UnexpectedCompletion)?;
                self.mcycle_index = self
                    .mcycle_index
                    .checked_add(1)
                    .ok_or(Z80MicroSequencerError::InstructionTStateOverflow)?;
                self.tstate_index = 0;
                self.phase = Z80MicroSequencerPhase::MemoryIncDecInternal {
                    decrement,
                    old,
                    address,
                };
                Ok(Z80MicroStep::InternalCycle { tstates: 1 })
            }
            Z80MicroSequencerPhase::MemoryIncDecInternal { .. } => {
                Err(Z80MicroSequencerError::UnexpectedCompletion)
            }
            Z80MicroSequencerPhase::MemoryWrite => self.retire(
                self.opcode
                    .ok_or(Z80MicroSequencerError::UnexpectedCompletion)?,
            ),
            Z80MicroSequencerPhase::Ready => Err(Z80MicroSequencerError::NoInstructionActive),
        }
    }

    /// Complete the explicitly exposed internal T-state of INC/DEC (HL).
    /// The result/flags are committed at this edge, then the memory write is issued.
    pub fn complete_internal_cycle(&mut self) -> Result<Z80MicroStep, Z80MicroSequencerError> {
        let Z80MicroSequencerPhase::MemoryIncDecInternal {
            decrement,
            old,
            address,
        } = self.phase
        else {
            return Err(Z80MicroSequencerError::UnexpectedCompletion);
        };
        let next_instruction_tstates = self
            .instruction_tstates
            .checked_add(1)
            .ok_or(Z80MicroSequencerError::InstructionTStateOverflow)?;
        let mut next = self.clone();
        let value = next.increment_decrement_register(old, decrement);
        let step = next.issue_write(address, value)?;
        next.instruction_tstates = next_instruction_tstates;
        *self = next;
        Ok(step)
    }

    /// Inspect the pending request without advancing the sequencer.
    pub fn pending_request(&self) -> Option<Z80BusRequest> {
        self.handshake.pending_request()
    }

    /// Whether an instruction is currently suspended or in progress.
    pub fn instruction_active(&self) -> bool {
        self.phase != Z80MicroSequencerPhase::Ready
    }

    fn unsupported(&mut self, opcode: u8) -> Result<Z80MicroStep, Z80MicroSequencerError> {
        let address = self.registers.pc.wrapping_sub(1);
        self.phase = Z80MicroSequencerPhase::Ready;
        self.opcode = None;
        self.instruction_tstates = 0;
        self.mcycle_index = 0;
        self.tstate_index = 0;
        Err(Z80MicroSequencerError::UnsupportedOpcode { opcode, address })
    }

    /// Apply the Z80 8-bit INC/DEC flag rules while preserving Carry.
    fn increment_decrement_register(&mut self, old: u8, decrement: bool) -> u8 {
        let value = if decrement {
            old.wrapping_sub(1)
        } else {
            old.wrapping_add(1)
        };
        let mut flags = (self.registers.f & FLAG_C) | (value & (FLAG_S | FLAG_XY));
        if value == 0 {
            flags |= FLAG_Z;
        }
        if if decrement {
            old & 0x0F == 0
        } else {
            old & 0x0F == 0x0F
        } {
            flags |= FLAG_H;
        }
        if if decrement { old == 0x80 } else { old == 0x7F } {
            flags |= FLAG_PV;
        }
        if decrement {
            flags |= FLAG_N;
        }
        self.registers.f = flags;
        value
    }

    fn read_register(&self, code: u8) -> u8 {
        match code {
            0 => self.registers.b,
            1 => self.registers.c,
            2 => self.registers.d,
            3 => self.registers.e,
            4 => self.registers.h,
            5 => self.registers.l,
            7 => self.registers.a,
            _ => unreachable!("memory operand is not a register"),
        }
    }

    fn write_register(&mut self, code: u8, value: u8) {
        match code {
            0 => self.registers.b = value,
            1 => self.registers.c = value,
            2 => self.registers.d = value,
            3 => self.registers.e = value,
            4 => self.registers.h = value,
            5 => self.registers.l = value,
            7 => self.registers.a = value,
            _ => unreachable!("memory operand is not a register"),
        }
    }

    fn issue_read(
        &mut self,
        phase: Z80MicroSequencerPhase,
        address: u16,
    ) -> Result<Z80MicroStep, Z80MicroSequencerError> {
        self.mcycle_index = self
            .mcycle_index
            .checked_add(1)
            .ok_or(Z80MicroSequencerError::InstructionTStateOverflow)?;
        self.tstate_index = 0;
        let request = self
            .handshake
            .begin(Z80BusOperation::MemoryRead, address, 3)?;
        self.phase = phase;
        Ok(Z80MicroStep::BusRequest(request))
    }

    fn issue_write(
        &mut self,
        address: u16,
        value: u8,
    ) -> Result<Z80MicroStep, Z80MicroSequencerError> {
        self.mcycle_index = self.mcycle_index.checked_add(1)
            .ok_or(Z80MicroSequencerError::InstructionTStateOverflow)?;
        self.tstate_index = 0;
        let request = self
            .handshake
            .begin(Z80BusOperation::MemoryWrite { value }, address, 3)?;
        self.phase = Z80MicroSequencerPhase::MemoryWrite;
        Ok(Z80MicroStep::BusRequest(request))
    }

    fn retire(&mut self, opcode: u8) -> Result<Z80MicroStep, Z80MicroSequencerError> {
        let tstates = self.instruction_tstates;
        self.phase = Z80MicroSequencerPhase::Ready;
        self.opcode = None;
        self.instruction_tstates = 0;
        self.mcycle_index = 0;
        self.tstate_index = 0;
        Ok(Z80MicroStep::InstructionRetired { opcode, tstates })
    }
}

#[cfg(test)]
mod tests {
    use super::{
        FLAG_C, FLAG_H, FLAG_N, FLAG_PV, FLAG_S, FLAG_XY, FLAG_Z, Z80MicroSequencer,
        Z80MicroSequencerError, Z80MicroSequencerPhase, Z80MicroStep,
    };
    use crate::{Z80BusOperation, Z80Registers};

    fn request(step: Z80MicroStep) -> crate::Z80BusRequest {
        match step {
            Z80MicroStep::BusRequest(request) => request,
            other => panic!("expected bus request, got {other:?}"),
        }
    }

    fn retired(step: Z80MicroStep, opcode: u8, tstates: u32) {
        assert_eq!(step, Z80MicroStep::InstructionRetired { opcode, tstates });
    }

    #[test]
    fn nop_suspends_until_fetch_completion() {
        let mut cpu = Z80MicroSequencer::default();
        let fetch = request(cpu.begin_instruction().unwrap());
        assert_eq!(fetch.operation, Z80BusOperation::OpcodeFetch);
        assert_eq!(fetch.address, 0);
        assert_eq!(cpu.registers.pc, 0);
        retired(
            cpu.complete_bus_cycle(fetch.id, Some(0x00)).unwrap(),
            0x00,
            4,
        );
        assert_eq!(cpu.registers.pc, 1);
        assert_eq!(cpu.refresh, 1);
        assert!(!cpu.instruction_active());
    }

    #[test]
    fn immediate_load_updates_pc_and_a_only_after_operand_read() {
        let mut cpu = Z80MicroSequencer::default();
        let fetch = request(cpu.begin_instruction().unwrap());
        let operand = request(cpu.complete_bus_cycle(fetch.id, Some(0x3E)).unwrap());
        assert_eq!(operand.operation, Z80BusOperation::MemoryRead);
        assert_eq!(operand.address, 1);
        assert_eq!(cpu.registers.pc, 1);
        assert_eq!(cpu.registers.a, 0);
        retired(
            cpu.complete_bus_cycle(operand.id, Some(0x42)).unwrap(),
            0x3E,
            7,
        );
        assert_eq!(cpu.registers.pc, 2);
        assert_eq!(cpu.registers.a, 0x42);
    }

    #[test]
    fn load_a_from_hl_commits_read_data_on_completion_and_counts_waits() {
        let mut cpu = Z80MicroSequencer {
            registers: Z80Registers {
                h: 0xC0,
                l: 0x10,
                ..Z80Registers::default()
            },
            ..Z80MicroSequencer::default()
        };
        let fetch = request(cpu.begin_instruction().unwrap());
        let read = request(cpu.complete_bus_cycle(fetch.id, Some(0x7E)).unwrap());
        assert_eq!(read.address, 0xC010);
        assert_eq!(cpu.registers.a, 0);
        cpu.insert_wait_tstate().unwrap();
        cpu.insert_wait_tstate().unwrap();
        retired(
            cpu.complete_bus_cycle(read.id, Some(0xA5)).unwrap(),
            0x7E,
            9,
        );
        assert_eq!(cpu.registers.a, 0xA5);
    }

    #[test]
    fn load_hl_from_a_exposes_write_without_committing_early() {
        let mut cpu = Z80MicroSequencer {
            registers: Z80Registers {
                a: 0x5A,
                h: 0xC1,
                l: 0x23,
                ..Z80Registers::default()
            },
            ..Z80MicroSequencer::default()
        };
        let fetch = request(cpu.begin_instruction().unwrap());
        let write = request(cpu.complete_bus_cycle(fetch.id, Some(0x77)).unwrap());
        assert_eq!(
            write.operation,
            Z80BusOperation::MemoryWrite { value: 0x5A }
        );
        assert_eq!(write.address, 0xC123);
        retired(cpu.complete_bus_cycle(write.id, None).unwrap(), 0x77, 7);
    }

    #[test]
    fn register_to_register_load_retires_after_m1_without_extra_bus_cycle() {
        let mut cpu = Z80MicroSequencer {
            registers: Z80Registers {
                b: 0xA5,
                ..Z80Registers::default()
            },
            ..Z80MicroSequencer::default()
        };
        let fetch = request(cpu.begin_instruction().unwrap());
        retired(
            cpu.complete_bus_cycle(fetch.id, Some(0x78)).unwrap(),
            0x78,
            4,
        );
        assert_eq!(cpu.registers.a, 0xA5);
        assert_eq!(cpu.registers.b, 0xA5);
        assert!(cpu.pending_request().is_none());
    }

    #[test]
    fn ld_register_immediate_commits_only_after_operand_read() {
        let mut cpu = Z80MicroSequencer::default();
        let fetch = request(cpu.begin_instruction().unwrap());
        let operand = request(cpu.complete_bus_cycle(fetch.id, Some(0x0E)).unwrap());
        assert_eq!(operand.address, 1);
        assert_eq!(cpu.registers.c, 0);
        retired(
            cpu.complete_bus_cycle(operand.id, Some(0x91)).unwrap(),
            0x0E,
            7,
        );
        assert_eq!(cpu.registers.c, 0x91);
        assert_eq!(cpu.registers.pc, 2);
    }

    #[test]
    fn ld_hl_immediate_uses_operand_read_then_memory_write() {
        let mut cpu = Z80MicroSequencer {
            registers: Z80Registers {
                h: 0xC0,
                l: 0x10,
                ..Z80Registers::default()
            },
            ..Z80MicroSequencer::default()
        };
        let fetch = request(cpu.begin_instruction().unwrap());
        let operand = request(cpu.complete_bus_cycle(fetch.id, Some(0x36)).unwrap());
        assert_eq!(operand.address, 1);
        let write = request(cpu.complete_bus_cycle(operand.id, Some(0xA5)).unwrap());
        assert_eq!(write.address, 0xC010);
        assert_eq!(
            write.operation,
            Z80BusOperation::MemoryWrite { value: 0xA5 }
        );
        assert_eq!(cpu.registers.pc, 2);
        retired(cpu.complete_bus_cycle(write.id, None).unwrap(), 0x36, 10);
    }

    #[test]
    fn ld_register_from_hl_uses_a_separate_memory_read_cycle() {
        let mut cpu = Z80MicroSequencer {
            registers: Z80Registers {
                h: 0xC0,
                l: 0x10,
                ..Z80Registers::default()
            },
            ..Z80MicroSequencer::default()
        };
        let fetch = request(cpu.begin_instruction().unwrap());
        let read = request(cpu.complete_bus_cycle(fetch.id, Some(0x46)).unwrap());
        assert_eq!(read.address, 0xC010);
        assert_eq!(read.operation, Z80BusOperation::MemoryRead);
        assert_eq!(cpu.registers.b, 0);
        retired(
            cpu.complete_bus_cycle(read.id, Some(0xA6)).unwrap(),
            0x46,
            7,
        );
        assert_eq!(cpu.registers.b, 0xA6);
    }

    #[test]
    fn ld_hl_from_register_uses_a_separate_memory_write_cycle() {
        let mut cpu = Z80MicroSequencer {
            registers: Z80Registers {
                c: 0x5A,
                h: 0xC1,
                l: 0x23,
                ..Z80Registers::default()
            },
            ..Z80MicroSequencer::default()
        };
        let fetch = request(cpu.begin_instruction().unwrap());
        let write = request(cpu.complete_bus_cycle(fetch.id, Some(0x71)).unwrap());
        assert_eq!(write.address, 0xC123);
        assert_eq!(
            write.operation,
            Z80BusOperation::MemoryWrite { value: 0x5A }
        );
        retired(cpu.complete_bus_cycle(write.id, None).unwrap(), 0x71, 7);
    }

    #[test]
    fn halt_opcode_is_not_decoded_as_a_register_load() {
        let mut cpu = Z80MicroSequencer::default();
        let fetch = request(cpu.begin_instruction().unwrap());
        assert!(matches!(
            cpu.complete_bus_cycle(fetch.id, Some(0x76)),
            Err(Z80MicroSequencerError::UnsupportedOpcode { opcode: 0x76, .. })
        ));
    }

    #[test]
    fn register_load_does_not_modify_flags() {
        let mut cpu = Z80MicroSequencer {
            registers: Z80Registers {
                f: 0xD7,
                h: 0x12,
                ..Z80Registers::default()
            },
            ..Z80MicroSequencer::default()
        };
        let fetch = request(cpu.begin_instruction().unwrap());
        let read = request(cpu.complete_bus_cycle(fetch.id, Some(0x46)).unwrap());
        assert_eq!(cpu.registers.f, 0xD7);
        assert_eq!(read.address, 0x1200);
        let retired = cpu.complete_bus_cycle(read.id, Some(0xA5)).unwrap();
        assert_eq!(
            retired,
            Z80MicroStep::InstructionRetired {
                opcode: 0x46,
                tstates: 7
            }
        );
        assert_eq!(cpu.registers.f, 0xD7);
    }

    #[test]
    fn wrong_request_id_does_not_advance_architectural_state() {
        let mut cpu = Z80MicroSequencer::default();
        let fetch = request(cpu.begin_instruction().unwrap());
        assert!(matches!(
            cpu.complete_bus_cycle(fetch.id + 1, Some(0x00)),
            Err(Z80MicroSequencerError::Handshake(_))
        ));
        assert_eq!(cpu.registers.pc, 0);
        assert_eq!(cpu.pending_request(), Some(fetch));
    }

    #[test]
    fn inc_b_wraps_and_sets_half_carry_and_overflow_preserving_carry() {
        let mut cpu = Z80MicroSequencer {
            registers: Z80Registers {
                b: 0x7F,
                f: FLAG_C,
                ..Z80Registers::default()
            },
            ..Z80MicroSequencer::default()
        };
        let fetch = request(cpu.begin_instruction().unwrap());
        retired(
            cpu.complete_bus_cycle(fetch.id, Some(0x04)).unwrap(),
            0x04,
            4,
        );
        assert_eq!(cpu.registers.b, 0x80);
        assert_eq!(cpu.registers.f, FLAG_S | FLAG_H | FLAG_PV | FLAG_C);
    }

    #[test]
    fn dec_a_wraps_and_sets_subtract_half_carry_and_overflow() {
        let mut cpu = Z80MicroSequencer {
            registers: Z80Registers {
                a: 0x80,
                f: FLAG_C,
                ..Z80Registers::default()
            },
            ..Z80MicroSequencer::default()
        };
        let fetch = request(cpu.begin_instruction().unwrap());
        retired(
            cpu.complete_bus_cycle(fetch.id, Some(0x3D)).unwrap(),
            0x3D,
            4,
        );
        assert_eq!(cpu.registers.a, 0x7F);
        assert_eq!(
            cpu.registers.f,
            (0x7F & FLAG_XY) | FLAG_H | FLAG_PV | FLAG_N | FLAG_C
        );
    }

    #[test]
    fn inc_zero_sets_zero_flag_and_preserves_carry() {
        let mut cpu = Z80MicroSequencer {
            registers: Z80Registers {
                c: 0xFF,
                f: FLAG_C | FLAG_N,
                ..Z80Registers::default()
            },
            ..Z80MicroSequencer::default()
        };
        let fetch = request(cpu.begin_instruction().unwrap());
        retired(
            cpu.complete_bus_cycle(fetch.id, Some(0x0C)).unwrap(),
            0x0C,
            4,
        );
        assert_eq!(cpu.registers.c, 0);
        assert_eq!(cpu.registers.f, FLAG_C | FLAG_Z | FLAG_H);
    }

    #[test]
    fn inc_hl_uses_read_internal_cycle_and_write_for_eleven_tstates() {
        let mut cpu = Z80MicroSequencer {
            registers: Z80Registers {
                h: 0xC0,
                l: 0x10,
                f: FLAG_C,
                ..Z80Registers::default()
            },
            ..Z80MicroSequencer::default()
        };
        let fetch = request(cpu.begin_instruction().unwrap());
        let read = request(cpu.complete_bus_cycle(fetch.id, Some(0x34)).unwrap());
        assert_eq!(read.operation, Z80BusOperation::MemoryRead);
        assert_eq!(read.address, 0xC010);
        cpu.insert_wait_tstate().unwrap();
        let internal = cpu.complete_bus_cycle(read.id, Some(0x7F)).unwrap();
        assert_eq!(internal, Z80MicroStep::InternalCycle { tstates: 1 });
        assert_eq!(cpu.registers.f, FLAG_C);
        let write = request(cpu.complete_internal_cycle().unwrap());
        assert_eq!(write.address, 0xC010);
        assert_eq!(
            write.operation,
            Z80BusOperation::MemoryWrite { value: 0x80 }
        );
        assert_eq!(cpu.registers.f, FLAG_S | FLAG_H | FLAG_PV | FLAG_C);
        retired(cpu.complete_bus_cycle(write.id, None).unwrap(), 0x34, 12);
    }

    #[test]
    fn inc_hl_keeps_captured_address_when_hl_changes_during_suspension() {
        let mut cpu = Z80MicroSequencer {
            registers: Z80Registers {
                h: 0x12,
                l: 0x34,
                f: FLAG_C,
                ..Z80Registers::default()
            },
            ..Z80MicroSequencer::default()
        };
        let fetch = request(cpu.begin_instruction().unwrap());
        let read = request(cpu.complete_bus_cycle(fetch.id, Some(0x34)).unwrap());
        assert_eq!(read.address, 0x1234);
        assert_eq!(
            cpu.complete_bus_cycle(read.id, Some(0x41)).unwrap(),
            Z80MicroStep::InternalCycle { tstates: 1 }
        );

        cpu.registers.h = 0xAB;
        cpu.registers.l = 0xCD;
        let write = request(cpu.complete_internal_cycle().unwrap());
        assert_eq!(write.address, 0x1234);
        assert_eq!(
            write.operation,
            Z80BusOperation::MemoryWrite { value: 0x42 }
        );
        retired(cpu.complete_bus_cycle(write.id, None).unwrap(), 0x34, 11);
    }

    #[test]
    fn dec_hl_sets_flags_and_writes_result_after_internal_cycle() {
        let mut cpu = Z80MicroSequencer {
            registers: Z80Registers {
                h: 0x12,
                l: 0x34,
                f: FLAG_C,
                ..Z80Registers::default()
            },
            ..Z80MicroSequencer::default()
        };
        let fetch = request(cpu.begin_instruction().unwrap());
        let read = request(cpu.complete_bus_cycle(fetch.id, Some(0x35)).unwrap());
        assert_eq!(read.address, 0x1234);
        assert_eq!(
            cpu.complete_bus_cycle(read.id, Some(0x80)).unwrap(),
            Z80MicroStep::InternalCycle { tstates: 1 }
        );
        let write = request(cpu.complete_internal_cycle().unwrap());
        assert_eq!(
            write.operation,
            Z80BusOperation::MemoryWrite { value: 0x7F }
        );
        assert_eq!(
            cpu.registers.f,
            (0x7F & FLAG_XY) | FLAG_H | FLAG_PV | FLAG_N | FLAG_C
        );
        retired(cpu.complete_bus_cycle(write.id, None).unwrap(), 0x35, 11);
    }

    #[test]
    fn inc_hl_memory_form_is_decoded_as_memory_read_not_register_inc() {
        let mut cpu = Z80MicroSequencer {
            registers: Z80Registers {
                h: 0x12,
                l: 0x34,
                ..Z80Registers::default()
            },
            ..Z80MicroSequencer::default()
        };
        let fetch = request(cpu.begin_instruction().unwrap());
        let read = request(cpu.complete_bus_cycle(fetch.id, Some(0x34)).unwrap());
        assert_eq!(read.operation, Z80BusOperation::MemoryRead);
        assert_eq!(read.address, 0x1234);
        assert_eq!(cpu.registers.h, 0x12);
        assert_eq!(cpu.registers.l, 0x34);
    }

    #[test]
    fn inc_hl_exposes_stable_mcycle_positions_across_bus_and_internal_phases() {
        let mut cpu = Z80MicroSequencer::default();
        let fetch = request(cpu.begin_instruction().unwrap());
        assert_eq!(
            (cpu.export_state().mcycle_index, cpu.export_state().tstate_index),
            (0, 0)
        );

        let read = request(cpu.complete_bus_cycle(fetch.id, Some(0x34)).unwrap());
        assert_eq!(
            (cpu.export_state().mcycle_index, cpu.export_state().tstate_index),
            (1, 0)
        );

        assert_eq!(
            cpu.complete_bus_cycle(read.id, Some(0x7F)).unwrap(),
            Z80MicroStep::InternalCycle { tstates: 1 }
        );
        assert_eq!(
            (cpu.export_state().mcycle_index, cpu.export_state().tstate_index),
            (2, 0)
        );

        let write = request(cpu.complete_internal_cycle().unwrap());
        assert_eq!(
            (cpu.export_state().mcycle_index, cpu.export_state().tstate_index),
            (3, 0)
        );
        retired(cpu.complete_bus_cycle(write.id, None).unwrap(), 0x34, 11);
        assert_eq!(
            (cpu.export_state().mcycle_index, cpu.export_state().tstate_index),
            (0, 0)
        );
    }

    #[test]
    fn instruction_tstate_overflow_keeps_bus_request_pending() {
        let mut cpu = Z80MicroSequencer::default();
        let fetch = request(cpu.begin_instruction().unwrap());
        cpu.instruction_tstates = u32::MAX;

        assert_eq!(
            cpu.complete_bus_cycle(fetch.id, Some(0x00)),
            Err(Z80MicroSequencerError::InstructionTStateOverflow)
        );
        assert_eq!(cpu.pending_request(), Some(fetch));
        assert_eq!(cpu.registers.pc, 0);
        assert_eq!(cpu.instruction_tstates, u32::MAX);
    }

    #[test]
    fn snapshot_restore_preserves_pending_fetch_and_wait_state() {
        let mut cpu = Z80MicroSequencer::default();
        let fetch = request(cpu.begin_instruction().unwrap());
        cpu.insert_wait_tstate().unwrap();

        let mut restored = Z80MicroSequencer::restore_state(cpu.export_state()).unwrap();
        assert_eq!(restored.export_state(), cpu.export_state());
        assert_eq!(restored.pending_request(), Some(fetch));
        assert_eq!(
            restored.complete_bus_cycle(fetch.id, Some(0x00)).unwrap(),
            Z80MicroStep::InstructionRetired {
                opcode: 0x00,
                tstates: 5
            }
        );
    }

    #[test]
    fn snapshot_restore_preserves_internal_incdec_safe_point() {
        let mut cpu = Z80MicroSequencer {
            registers: Z80Registers {
                h: 0xC0,
                l: 0x10,
                ..Z80Registers::default()
            },
            ..Z80MicroSequencer::default()
        };
        let fetch = request(cpu.begin_instruction().unwrap());
        let read = request(cpu.complete_bus_cycle(fetch.id, Some(0x34)).unwrap());
        assert_eq!(
            cpu.complete_bus_cycle(read.id, Some(0x7F)).unwrap(),
            Z80MicroStep::InternalCycle { tstates: 1 }
        );

        let mut restored = Z80MicroSequencer::restore_state(cpu.export_state()).unwrap();
        assert_eq!(restored.export_state(), cpu.export_state());
        let write = request(restored.complete_internal_cycle().unwrap());
        assert_eq!(write.address, 0xC010);
        assert_eq!(
            restored.complete_bus_cycle(write.id, None).unwrap(),
            Z80MicroStep::InstructionRetired {
                opcode: 0x34,
                tstates: 11
            }
        );
    }

    #[test]
    fn failed_internal_write_allocation_does_not_consume_internal_tstate() {
        let mut cpu = Z80MicroSequencer::default();
        let fetch = request(cpu.begin_instruction().unwrap());
        let read = request(cpu.complete_bus_cycle(fetch.id, Some(0x34)).unwrap());
        assert_eq!(
            cpu.complete_bus_cycle(read.id, Some(0x7F)).unwrap(),
            Z80MicroStep::InternalCycle { tstates: 1 }
        );
        let mut state = cpu.export_state();
        state.handshake.next_request_id = u64::MAX;
        let mut restored = Z80MicroSequencer::restore_state(state).unwrap();
        let before = restored.export_state();

        assert_eq!(
            restored.complete_internal_cycle(),
            Err(Z80MicroSequencerError::Handshake(
                crate::Z80BusHandshakeError::RequestIdExhausted
            ))
        );
        assert_eq!(restored.export_state(), before);
    }

    #[test]
    fn snapshot_restore_rejects_phase_handshake_mismatch() {
        let mut cpu = Z80MicroSequencer::default();
        cpu.begin_instruction().unwrap();
        let mut state = cpu.export_state();
        state.phase = Z80MicroSequencerPhase::MemoryWrite;

        assert_eq!(
            Z80MicroSequencer::restore_state(state),
            Err(Z80MicroSequencerError::InvalidState)
        );
    }

    #[test]
    fn unsupported_opcode_does_not_leave_a_phantom_pending_request() {
        let mut cpu = Z80MicroSequencer::default();
        let fetch = request(cpu.begin_instruction().unwrap());
        assert!(matches!(
            cpu.complete_bus_cycle(fetch.id, Some(0xFF)),
            Err(Z80MicroSequencerError::UnsupportedOpcode { opcode: 0xFF, .. })
        ));
        assert!(!cpu.instruction_active());
        assert!(cpu.pending_request().is_none());
    }
}
