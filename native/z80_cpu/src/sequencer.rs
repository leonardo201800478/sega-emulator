//! First resumable instruction slice for the shared Z80.
//!
//! This deliberately small sequencer proves real suspend/resume semantics for
//! NOP, selected BASE operations, and the initial prefix-fetch pipeline. It is
//! independent of the legacy atomic executor; full prefixed instruction
//! semantics, interrupts, HALT, and the complete architectural state remain
//! outside this sequencer slice.

use crate::alu::{adc8, add8, and8, cp8, cpl8, daa8, or8, rotate_accumulator, sbc8, sub8, xor8};
use crate::{
    Z80_MICRO_SEQUENCER_STATE_VERSION, Z80AlternateRegisters, Z80ArchitecturalState,
    Z80BusHandshake, Z80BusHandshakeError, Z80BusOperation, Z80BusRequest, Z80Registers,
};
use serde::{Deserialize, Serialize};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub enum Z80MicroSequencerPhase {
    Ready,
    OpcodeFetch,
    ImmediateReadRegister {
        destination: u8,
    },
    ImmediateReadRegisterPairLow {
        pair: u8,
    },
    ImmediateReadRegisterPairHigh {
        pair: u8,
        low: u8,
    },
    ImmediateReadMemoryWrite,
    ImmediateReadAlu {
        operation: u8,
    },
    MemoryReadRegister {
        destination: u8,
    },
    MemoryReadAlu {
        operation: u8,
        address: u16,
    },
    MemoryReadIncDec {
        decrement: bool,
        address: u16,
    },
    CbMemoryRead {
        address: u16,
    },
    CbMemoryInternal {
        address: u16,
        result: Option<u8>,
    },
    CbMemoryWrite {
        address: u16,
        value: u8,
    },
    MemoryIncDecInternal {
        decrement: bool,
        old: u8,
        address: u16,
    },
    MemoryWrite {
        address: u16,
        value: u8,
    },
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
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct Z80MicroSequencerState {
    pub format_version: u16,
    pub registers: Z80Registers,
    pub refresh: u8,
    pub q: u8,
    pub q_previous: u8,
    pub handshake: crate::Z80BusHandshakeState,
    pub phase: Z80MicroSequencerPhase,
    pub opcode: Option<u8>,
    pub instruction_tstates: u32,
    /// Zero-based M-cycle index within the active instruction.
    pub mcycle_index: u32,
    /// Zero-based T-state position within the current M-cycle.
    /// This slice exposes selected intra-M-cycle offsets, not electrical edges.
    pub tstate_index: u16,
    pub alternate: Z80AlternateRegisters,
    pub ix: u16,
    pub iy: u16,
    pub i: u8,
    pub iff1: bool,
    pub iff2: bool,
    pub interrupt_mode: u8,
    pub wz: u16,
    pub ei_delay: u8,
    pub nmi_pending: bool,
    pub int_pending: Option<u8>,
    pub p: bool,
    pub halted: bool,
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
    /// Flags written by the current instruction; zero when it does not write F.
    pub q: u8,
    /// Q value captured at the previous instruction boundary.
    pub q_previous: u8,
    pub alternate: Z80AlternateRegisters,
    pub ix: u16,
    pub iy: u16,
    pub i: u8,
    pub iff1: bool,
    pub iff2: bool,
    pub interrupt_mode: u8,
    pub wz: u16,
    pub ei_delay: u8,
    pub nmi_pending: bool,
    pub int_pending: Option<u8>,
    pub p: bool,
    pub halted: bool,
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
            q: 0,
            q_previous: 0,
            alternate: Z80AlternateRegisters::default(),
            ix: 0,
            iy: 0,
            i: 0,
            iff1: false,
            iff2: false,
            interrupt_mode: 0,
            wz: 0,
            ei_delay: 0,
            nmi_pending: false,
            int_pending: None,
            p: false,
            halted: false,
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
            q: self.q,
            q_previous: self.q_previous,
            handshake: self.handshake.export_state(),
            phase: self.phase,
            opcode: self.opcode,
            instruction_tstates: self.instruction_tstates,
            mcycle_index: self.mcycle_index,
            tstate_index: self.tstate_index,
            alternate: self.alternate,
            ix: self.ix,
            iy: self.iy,
            i: self.i,
            iff1: self.iff1,
            iff2: self.iff2,
            interrupt_mode: self.interrupt_mode,
            wz: self.wz,
            ei_delay: self.ei_delay,
            nmi_pending: self.nmi_pending,
            int_pending: self.int_pending,
            p: self.p,
            halted: self.halted,
        }
    }

    /// Export the shared architectural DTO without sequencer continuation data.
    pub fn export_architectural_state(&self) -> Z80ArchitecturalState {
        Z80ArchitecturalState {
            registers: self.registers,
            alternate: self.alternate,
            ix: self.ix,
            iy: self.iy,
            i: self.i,
            r: self.refresh,
            iff1: self.iff1,
            iff2: self.iff2,
            interrupt_mode: self.interrupt_mode,
            wz: self.wz,
            ei_delay: self.ei_delay,
            nmi_pending: self.nmi_pending,
            int_pending: self.int_pending,
            q: self.q,
            q_previous: self.q_previous,
            p: self.p,
            halted: self.halted,
        }
    }

    /// Import architectural state only at a sequencer instruction boundary.
    pub fn import_architectural_state(
        &mut self,
        state: Z80ArchitecturalState,
    ) -> Result<(), Z80MicroSequencerError> {
        state
            .validate()
            .map_err(|_| Z80MicroSequencerError::InvalidState)?;
        if self.phase != Z80MicroSequencerPhase::Ready || self.handshake.is_pending() {
            return Err(Z80MicroSequencerError::InstructionAlreadyActive);
        }
        self.registers = state.registers;
        self.alternate = state.alternate;
        self.ix = state.ix;
        self.iy = state.iy;
        self.i = state.i;
        self.refresh = state.r;
        self.iff1 = state.iff1;
        self.iff2 = state.iff2;
        self.interrupt_mode = state.interrupt_mode;
        self.wz = state.wz;
        self.ei_delay = state.ei_delay;
        self.nmi_pending = state.nmi_pending;
        self.int_pending = state.int_pending;
        self.q = state.q;
        self.q_previous = state.q_previous;
        self.p = state.p;
        self.halted = state.halted;
        Ok(())
    }

    /// Restore a resumable state after validating CPU/handshake cross-invariants.
    pub fn restore_state(state: Z80MicroSequencerState) -> Result<Self, Z80MicroSequencerError> {
        if state.format_version != Z80_MICRO_SEQUENCER_STATE_VERSION {
            return Err(Z80MicroSequencerError::InvalidState);
        }
        let handshake = Z80BusHandshake::restore_state(state.handshake)?;
        if state.interrupt_mode > 2 || state.ei_delay > 1 {
            return Err(Z80MicroSequencerError::InvalidState);
        }
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
                let prefix_state_valid = match state.opcode {
                    None => state.mcycle_index == 0 && state.instruction_tstates == 0,
                    Some(0xCB | 0xED | 0xDD | 0xFD) => {
                        state.mcycle_index >= 1 && state.instruction_tstates >= 4
                    }
                    Some(_) => false,
                };
                prefix_state_valid
                    && pending.is_some_and(|request| {
                        request.operation == Z80BusOperation::OpcodeFetch
                            && request.address == state.registers.pc
                            && request.base_tstates == 4
                    })
            }
            Z80MicroSequencerPhase::ImmediateReadRegister { destination } => {
                let opcode_matches = state.opcode.is_some_and(|opcode| {
                    matches!(opcode, 0x06 | 0x0E | 0x16 | 0x1E | 0x26 | 0x2E | 0x3E)
                        && destination == ((opcode >> 3) & 0x07)
                });
                opcode_matches
                    && pending.is_some_and(|request| {
                        request.operation == Z80BusOperation::MemoryRead
                            && request.address == state.registers.pc
                            && request.base_tstates == 3
                    })
                    && state.instruction_tstates >= 4
            }
            Z80MicroSequencerPhase::ImmediateReadRegisterPairLow { pair } => {
                let opcode_matches = state.opcode.is_some_and(|opcode| {
                    matches!(opcode, 0x01 | 0x11 | 0x21 | 0x31) && pair == ((opcode >> 4) & 0x03)
                });
                opcode_matches
                    && pending.is_some_and(|request| {
                        request.operation == Z80BusOperation::MemoryRead
                            && request.address == state.registers.pc
                            && request.base_tstates == 3
                    })
                    && state.instruction_tstates >= 4
            }
            Z80MicroSequencerPhase::ImmediateReadRegisterPairHigh { pair, .. } => {
                let opcode_matches = state.opcode.is_some_and(|opcode| {
                    matches!(opcode, 0x01 | 0x11 | 0x21 | 0x31) && pair == ((opcode >> 4) & 0x03)
                });
                opcode_matches
                    && pending.is_some_and(|request| {
                        request.operation == Z80BusOperation::MemoryRead
                            && request.address == state.registers.pc
                            && request.base_tstates == 3
                    })
                    && state.instruction_tstates >= 7
            }
            Z80MicroSequencerPhase::ImmediateReadMemoryWrite => {
                state.opcode == Some(0x36)
                    && pending.is_some_and(|request| {
                        request.operation == Z80BusOperation::MemoryRead
                            && request.address == state.registers.pc
                            && request.base_tstates == 3
                    })
                    && state.instruction_tstates >= 4
            }
            Z80MicroSequencerPhase::ImmediateReadAlu { operation } => {
                state.opcode.is_some_and(|opcode| {
                    matches!(
                        opcode,
                        0xC6 | 0xCE | 0xD6 | 0xDE | 0xE6 | 0xEE | 0xF6 | 0xFE
                    ) && operation == ((opcode >> 3) & 0x07)
                }) && pending.is_some_and(|request| {
                    request.operation == Z80BusOperation::MemoryRead
                        && request.address == state.registers.pc
                        && request.base_tstates == 3
                }) && state.instruction_tstates >= 4
            }
            Z80MicroSequencerPhase::MemoryReadRegister { destination } => {
                let expected_address = state.opcode.and_then(|opcode| match opcode {
                    0x0A if destination == 7 => Some(state.registers.bc()),
                    0x1A if destination == 7 => Some(state.registers.de()),
                    0x40..=0x7F
                        if opcode != 0x76
                            && opcode & 0x07 == 0x06
                            && ((opcode >> 3) & 0x07) == destination
                            && destination != 6 =>
                    {
                        Some(state.registers.hl())
                    }
                    _ => None,
                });
                expected_address.is_some_and(|address| {
                    pending.is_some_and(|request| {
                        request.operation == Z80BusOperation::MemoryRead
                            && request.address == address
                            && request.base_tstates == 3
                    })
                }) && state.instruction_tstates >= 4
            }
            Z80MicroSequencerPhase::MemoryReadAlu { operation, address } => {
                state.opcode.is_some_and(|opcode| {
                    (0x80..=0xBF).contains(&opcode)
                        && opcode & 0x07 == 6
                        && operation == ((opcode >> 3) & 0x07)
                }) && pending.is_some_and(|request| {
                    request.operation == Z80BusOperation::MemoryRead
                        && request.address == address
                        && request.base_tstates == 3
                }) && address == state.registers.hl()
                    && state.instruction_tstates >= 4
            }
            Z80MicroSequencerPhase::CbMemoryRead { address } => {
                state.opcode.is_some_and(|opcode| opcode & 0x07 == 0x06)
                    && pending.is_some_and(|request| {
                        request.operation == Z80BusOperation::MemoryRead
                            && request.address == address
                            && request.base_tstates == 3
                    })
                    && address == state.registers.hl()
                    && state.instruction_tstates >= 8
            }
            Z80MicroSequencerPhase::CbMemoryInternal { address, result } => {
                state.opcode.is_some_and(|opcode| {
                    opcode & 0x07 == 0x06 && ((opcode >> 6 == 1) == result.is_none())
                }) && pending.is_none()
                    && address == state.registers.hl()
                    && state.tstate_index >= 3
                    && state.instruction_tstates >= 8 + u32::from(state.tstate_index)
            }
            Z80MicroSequencerPhase::CbMemoryWrite { address, value } => {
                let Some(request) = pending else {
                    return Err(Z80MicroSequencerError::InvalidState);
                };
                state
                    .opcode
                    .is_some_and(|opcode| opcode & 0x07 == 0x06 && opcode >> 6 != 1)
                    && matches!(
                        request.operation,
                        Z80BusOperation::MemoryWrite {
                            value: request_value
                        } if request_value == value
                    )
                    && request.address == address
                    && request.base_tstates == 3
                    && address == state.registers.hl()
                    && state.instruction_tstates >= 12
            }
            Z80MicroSequencerPhase::MemoryReadIncDec { decrement, address } => {
                let expected_opcode = if decrement { 0x35 } else { 0x34 };
                pending.is_some_and(|request| {
                    request.operation == Z80BusOperation::MemoryRead
                        && request.address == address
                        && request.base_tstates == 3
                }) && state.opcode == Some(expected_opcode)
                    && address == state.registers.hl()
                    && state.instruction_tstates >= 4
            }
            Z80MicroSequencerPhase::MemoryIncDecInternal {
                decrement, address, ..
            } => {
                let expected_opcode = if decrement { 0x35 } else { 0x34 };
                pending.is_none()
                    && state.opcode == Some(expected_opcode)
                    && address == state.registers.hl()
                    && state.instruction_tstates >= 4 + u32::from(state.tstate_index)
            }
            Z80MicroSequencerPhase::MemoryWrite { address, value } => {
                let Some(request) = pending else {
                    return Err(Z80MicroSequencerError::InvalidState);
                };
                let (minimum_tstates, expected_address, expected_value) = match state.opcode {
                    Some(0x02) => (4, Some(state.registers.bc()), Some(state.registers.a)),
                    Some(0x12) => (4, Some(state.registers.de()), Some(state.registers.a)),
                    Some(0x77) => (4, Some(state.registers.hl()), None),
                    Some(0x36) => (7, Some(state.registers.hl()), None),
                    Some(0x34 | 0x35) => (8, Some(state.registers.hl()), None),
                    _ => (u32::MAX, None, None),
                };
                let Z80BusOperation::MemoryWrite {
                    value: request_value,
                } = request.operation
                else {
                    return Err(Z80MicroSequencerError::InvalidState);
                };
                request_value == value
                    && request.address == address
                    && expected_address == Some(address)
                    && expected_value.is_none_or(|expected| expected == request_value)
                    && request.base_tstates == 3
                    && state.instruction_tstates >= minimum_tstates
            }
        };
        let indices_valid = match state.phase {
            Z80MicroSequencerPhase::Ready => state.mcycle_index == 0 && state.tstate_index == 0,
            Z80MicroSequencerPhase::OpcodeFetch => {
                state.tstate_index == 0
                    && match state.opcode {
                        None => state.mcycle_index == 0 && state.instruction_tstates == 0,
                        Some(0xCB | 0xED | 0xDD | 0xFD) => {
                            state.mcycle_index >= 1 && state.instruction_tstates >= 4
                        }
                        Some(_) => false,
                    }
            }
            Z80MicroSequencerPhase::ImmediateReadRegister { .. }
            | Z80MicroSequencerPhase::ImmediateReadRegisterPairLow { .. }
            | Z80MicroSequencerPhase::ImmediateReadMemoryWrite
            | Z80MicroSequencerPhase::ImmediateReadAlu { .. }
            | Z80MicroSequencerPhase::MemoryReadRegister { .. }
            | Z80MicroSequencerPhase::MemoryReadAlu { .. }
            | Z80MicroSequencerPhase::MemoryReadIncDec { .. } => {
                state.mcycle_index == 1 && state.tstate_index == 0
            }
            Z80MicroSequencerPhase::CbMemoryRead { .. } => {
                state.mcycle_index == 2 && state.tstate_index == 0
            }
            Z80MicroSequencerPhase::CbMemoryInternal { .. } => {
                state.mcycle_index == 2 && state.tstate_index >= 3
            }
            Z80MicroSequencerPhase::CbMemoryWrite { .. } => {
                state.mcycle_index == 3 && state.tstate_index == 0
            }
            Z80MicroSequencerPhase::ImmediateReadRegisterPairHigh { .. } => {
                state.mcycle_index == 2 && state.tstate_index == 0
            }
            Z80MicroSequencerPhase::MemoryIncDecInternal { .. } => {
                state.mcycle_index == 1
                    && state.tstate_index >= 3
                    && matches!(state.opcode, Some(0x34 | 0x35))
            }
            Z80MicroSequencerPhase::MemoryWrite { .. } => match state.opcode {
                Some(0x02 | 0x12 | 0x77) => {
                    state.mcycle_index == 1
                        && state.tstate_index == 0
                        && state.instruction_tstates >= 4
                }
                Some(0x36) => {
                    state.mcycle_index == 2
                        && state.tstate_index == 0
                        && state.instruction_tstates >= 7
                }
                Some(0x34 | 0x35) => {
                    state.mcycle_index == 2
                        && state.tstate_index == 0
                        && state.instruction_tstates >= 8
                }
                _ => false,
            },
        };
        if !valid || !indices_valid {
            return Err(Z80MicroSequencerError::InvalidState);
        }
        Ok(Self {
            registers: state.registers,
            refresh: state.refresh,
            q: state.q,
            q_previous: state.q_previous,
            alternate: state.alternate,
            ix: state.ix,
            iy: state.iy,
            i: state.i,
            iff1: state.iff1,
            iff2: state.iff2,
            interrupt_mode: state.interrupt_mode,
            wz: state.wz,
            ei_delay: state.ei_delay,
            nmi_pending: state.nmi_pending,
            int_pending: state.int_pending,
            p: state.p,
            halted: state.halted,
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
        self.q_previous = self.q;
        self.q = 0;
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
        // Prepare the complete transition on a copy. Any failure after consuming
        // the handshake (for example, allocating the next request) leaves the
        // caller's continuation byte-for-byte unchanged.
        let mut next = self.clone();
        match next.complete_bus_cycle_inner(request_id, data) {
            Ok(step) => {
                *self = next;
                Ok(step)
            }
            Err(error @ Z80MicroSequencerError::UnsupportedOpcode { .. }) => {
                // Unsupported decode is a terminal, explicit rejection: clear
                // the consumed fetch and return to Ready as the legacy contract requires.
                *self = next;
                Err(error)
            }
            Err(error) => Err(error),
        }
    }

    fn complete_bus_cycle_inner(
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
                let previous_prefix = self.opcode;
                if Self::is_prefix_fetch(previous_prefix, opcode) {
                    self.opcode = Some(opcode);
                    let next_mcycle_index = self
                        .mcycle_index
                        .checked_add(1)
                        .ok_or(Z80MicroSequencerError::InstructionTStateOverflow)?;
                    let request =
                        self.handshake
                            .begin(Z80BusOperation::OpcodeFetch, self.registers.pc, 4)?;
                    self.mcycle_index = next_mcycle_index;
                    self.tstate_index = 0;
                    return Ok(Z80MicroStep::BusRequest(request));
                }
                self.opcode = Some(opcode);
                if previous_prefix == Some(0xCB) {
                    if opcode & 0x07 == 0x06 {
                        return self.issue_read(
                            Z80MicroSequencerPhase::CbMemoryRead {
                                address: self.registers.hl(),
                            },
                            self.registers.hl(),
                        );
                    }
                    self.execute_cb_register(opcode);
                    return self.retire(opcode);
                }
                if previous_prefix.is_some() {
                    return self.unsupported(opcode);
                }
                match opcode {
                    0x00 => self.retire(opcode),
                    0x07 | 0x0F | 0x17 | 0x1F | 0x27 | 0x2F => {
                        self.execute_accumulator_misc(opcode);
                        self.retire(opcode)
                    }
                    0x37 | 0x3F => {
                        self.execute_carry_flag_operation(opcode);
                        self.retire(opcode)
                    }
                    0xE9 => {
                        self.registers.pc = self.registers.hl();
                        self.retire(opcode)
                    }
                    0xEB => {
                        let de = self.registers.de();
                        self.registers.set_de(self.registers.hl());
                        self.registers.set_hl(de);
                        self.retire(opcode)
                    }
                    0x02 | 0x12 => {
                        let address = if opcode == 0x02 {
                            self.registers.bc()
                        } else {
                            self.registers.de()
                        };
                        self.issue_write(address, self.registers.a)
                    }
                    0x0A | 0x1A => {
                        let address = if opcode == 0x0A {
                            self.registers.bc()
                        } else {
                            self.registers.de()
                        };
                        self.issue_read(
                            Z80MicroSequencerPhase::MemoryReadRegister { destination: 7 },
                            address,
                        )
                    }
                    0x01 | 0x11 | 0x21 | 0x31 => self.issue_read(
                        Z80MicroSequencerPhase::ImmediateReadRegisterPairLow {
                            pair: (opcode >> 4) & 0x03,
                        },
                        self.registers.pc,
                    ),
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
                    0xC6 | 0xCE | 0xD6 | 0xDE | 0xE6 | 0xEE | 0xF6 | 0xFE => self.issue_read(
                        Z80MicroSequencerPhase::ImmediateReadAlu {
                            operation: (opcode >> 3) & 0x07,
                        },
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
                    0x80..=0xBF => {
                        let operation = (opcode >> 3) & 0x07;
                        if opcode & 0x07 == 6 {
                            self.issue_read(
                                Z80MicroSequencerPhase::MemoryReadAlu {
                                    operation,
                                    address: self.registers.hl(),
                                },
                                self.registers.hl(),
                            )
                        } else {
                            let operand = self.read_register(opcode & 0x07);
                            self.execute_alu_operation(operation, operand);
                            self.retire(opcode)
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
            Z80MicroSequencerPhase::ImmediateReadRegisterPairLow { pair } => {
                let low = response
                    .data
                    .ok_or(Z80MicroSequencerError::UnexpectedCompletion)?;
                self.registers.pc = self.registers.pc.wrapping_add(1);
                self.issue_read(
                    Z80MicroSequencerPhase::ImmediateReadRegisterPairHigh { pair, low },
                    self.registers.pc,
                )
            }
            Z80MicroSequencerPhase::ImmediateReadRegisterPairHigh { pair, low } => {
                let high = response
                    .data
                    .ok_or(Z80MicroSequencerError::UnexpectedCompletion)?;
                self.registers.pc = self.registers.pc.wrapping_add(1);
                self.write_register_pair(pair, u16::from_le_bytes([low, high]));
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
            Z80MicroSequencerPhase::ImmediateReadAlu { operation } => {
                let operand = response
                    .data
                    .ok_or(Z80MicroSequencerError::UnexpectedCompletion)?;
                self.registers.pc = self.registers.pc.wrapping_add(1);
                self.execute_alu_operation(operation, operand);
                self.retire(
                    self.opcode
                        .ok_or(Z80MicroSequencerError::UnexpectedCompletion)?,
                )
            }
            Z80MicroSequencerPhase::MemoryReadRegister { destination } => {
                let value = response
                    .data
                    .ok_or(Z80MicroSequencerError::UnexpectedCompletion)?;
                let opcode = self
                    .opcode
                    .ok_or(Z80MicroSequencerError::UnexpectedCompletion)?;
                if matches!(opcode, 0x0A | 0x1A) {
                    let address = if opcode == 0x0A {
                        self.registers.bc()
                    } else {
                        self.registers.de()
                    };
                    self.wz = address.wrapping_add(1);
                }
                self.write_register(destination, value);
                self.retire(opcode)
            }
            Z80MicroSequencerPhase::MemoryReadAlu { operation, .. } => {
                let operand = response
                    .data
                    .ok_or(Z80MicroSequencerError::UnexpectedCompletion)?;
                self.execute_alu_operation(operation, operand);
                self.retire(
                    self.opcode
                        .ok_or(Z80MicroSequencerError::UnexpectedCompletion)?,
                )
            }
            Z80MicroSequencerPhase::CbMemoryRead { address } => {
                let value = response
                    .data
                    .ok_or(Z80MicroSequencerError::UnexpectedCompletion)?;
                let opcode = self
                    .opcode
                    .ok_or(Z80MicroSequencerError::UnexpectedCompletion)?;
                let result = self.execute_cb_value(opcode, value, (self.wz >> 8) as u8);
                let wait_offset = u16::try_from(waits)
                    .ok()
                    .and_then(|waits| 3_u16.checked_add(waits))
                    .ok_or(Z80MicroSequencerError::InstructionTStateOverflow)?;
                self.tstate_index = wait_offset;
                self.phase = Z80MicroSequencerPhase::CbMemoryInternal { address, result };
                Ok(Z80MicroStep::InternalCycle { tstates: 1 })
            }
            Z80MicroSequencerPhase::CbMemoryInternal { .. } => {
                Err(Z80MicroSequencerError::UnexpectedCompletion)
            }
            Z80MicroSequencerPhase::CbMemoryWrite { .. } => {
                let opcode = self
                    .opcode
                    .ok_or(Z80MicroSequencerError::UnexpectedCompletion)?;
                self.retire(opcode)
            }
            Z80MicroSequencerPhase::MemoryReadIncDec { decrement, address } => {
                let old = response
                    .data
                    .ok_or(Z80MicroSequencerError::UnexpectedCompletion)?;
                let wait_offset = u16::try_from(waits)
                    .ok()
                    .and_then(|waits| 3_u16.checked_add(waits))
                    .ok_or(Z80MicroSequencerError::InstructionTStateOverflow)?;
                // INC/DEC (HL) has three M-cycles: M1 fetch, M2 read plus
                // its internal T-state, and M3 write. WAIT extends M2, so
                // the internal interval begins at T3 + the M2 WAIT count.
                self.tstate_index = wait_offset;
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
            Z80MicroSequencerPhase::MemoryWrite { address, .. } => {
                let opcode = self
                    .opcode
                    .ok_or(Z80MicroSequencerError::UnexpectedCompletion)?;
                if matches!(opcode, 0x02 | 0x12) {
                    self.wz =
                        (u16::from(self.registers.a) << 8) | (address.wrapping_add(1) & 0x00FF);
                }
                self.retire(opcode)
            }
            Z80MicroSequencerPhase::Ready => Err(Z80MicroSequencerError::NoInstructionActive),
        }
    }

    /// Complete one explicitly exposed internal T-state for memory read/modify/write.
    /// CB BIT (HL) retires here; the other CB memory groups issue their write cycle.
    pub fn complete_internal_cycle(&mut self) -> Result<Z80MicroStep, Z80MicroSequencerError> {
        let next_instruction_tstates = self
            .instruction_tstates
            .checked_add(1)
            .ok_or(Z80MicroSequencerError::InstructionTStateOverflow)?;
        let mut next = self.clone();
        let step = match next.phase {
            Z80MicroSequencerPhase::MemoryIncDecInternal {
                decrement,
                old,
                address,
            } => {
                let value = next.increment_decrement_register(old, decrement);
                let step = next.issue_write(address, value)?;
                next.instruction_tstates = next_instruction_tstates;
                step
            }
            Z80MicroSequencerPhase::CbMemoryInternal { address, result } => {
                next.instruction_tstates = next_instruction_tstates;
                if let Some(value) = result {
                    next.issue_cb_memory_write(address, value)?
                } else {
                    let opcode = next
                        .opcode
                        .ok_or(Z80MicroSequencerError::UnexpectedCompletion)?;
                    next.retire(opcode)?
                }
            }
            _ => return Err(Z80MicroSequencerError::UnexpectedCompletion),
        };
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

    /// Return true when the byte just fetched is another M1 prefix byte.
    ///
    /// DD/FD prefixes may be repeated (the last one wins); ED supersedes an
    /// index prefix. DD/FD CB is intentionally stopped here until its
    /// displacement/non-M1 opcode sequence is implemented.
    fn is_prefix_fetch(previous: Option<u8>, opcode: u8) -> bool {
        match previous {
            None => matches!(opcode, 0xCB | 0xED | 0xDD | 0xFD),
            Some(0xDD | 0xFD) => matches!(opcode, 0xDD | 0xFD | 0xED),
            Some(0xCB | 0xED) => false,
            Some(_) => false,
        }
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

    /// Execute a CB-prefixed operation whose operand is a CPU register.
    /// The caller supplies the undocumented X/Y flag source explicitly.
    fn execute_cb_register(&mut self, opcode: u8) {
        let target = opcode & 0x07;
        let value = self.read_register(target);
        if let Some(result) = self.execute_cb_value(opcode, value, value) {
            self.write_register(target, result);
        }
    }

    /// Apply the semantic portion of a CB operation to a sampled operand.
    /// Returns a value only for groups that write their operand.
    fn execute_cb_value(&mut self, opcode: u8, value: u8, undocumented: u8) -> Option<u8> {
        let group = opcode >> 6;
        let operation = (opcode >> 3) & 0x07;
        match group {
            0 => {
                let old_carry = self.registers.f & FLAG_C != 0;
                let (result, carry) = match operation {
                    0 => (value.rotate_left(1), value & 0x80 != 0),
                    1 => (value.rotate_right(1), value & 0x01 != 0),
                    2 => ((value << 1) | u8::from(old_carry), value & 0x80 != 0),
                    3 => ((value >> 1) | (u8::from(old_carry) << 7), value & 1 != 0),
                    4 => (value << 1, value & 0x80 != 0),
                    5 => ((value >> 1) | (value & 0x80), value & 1 != 0),
                    6 => ((value << 1) | 1, value & 0x80 != 0),
                    _ => (value >> 1, value & 1 != 0),
                };
                let mut flags = result & (FLAG_S | FLAG_XY);
                if result == 0 {
                    flags |= FLAG_Z;
                }
                if result.count_ones().is_multiple_of(2) {
                    flags |= FLAG_PV;
                }
                if carry {
                    flags |= FLAG_C;
                }
                self.set_flags(flags);
                Some(result)
            }
            1 => {
                let mask = 1_u8 << operation;
                let bit_clear = value & mask == 0;
                let mut flags = (self.registers.f & FLAG_C) | FLAG_H | (undocumented & FLAG_XY);
                if bit_clear {
                    flags |= FLAG_Z | FLAG_PV;
                }
                if operation == 7 && !bit_clear {
                    flags |= FLAG_S;
                }
                self.set_flags(flags);
                None
            }
            2 => Some(value & !(1_u8 << operation)),
            _ => Some(value | (1_u8 << operation)),
        }
    }

    /// Execute one register-source BASE ALU operation using the shared ALU.
    ///
    /// Operation field: 0 ADD, 1 ADC, 2 SUB, 3 SBC, 4 AND, 5 XOR, 6 OR, 7 CP.
    /// The caller excludes (HL), which requires a separate scheduled memory read.
    fn execute_alu_operation(&mut self, operation: u8, operand: u8) {
        let accumulator = self.registers.a;
        let carry = self.registers.f & FLAG_C != 0;
        let result = match operation {
            0 => add8(accumulator, operand),
            1 => adc8(accumulator, operand, carry),
            2 => sub8(accumulator, operand),
            3 => sbc8(accumulator, operand, carry),
            4 => and8(accumulator, operand),
            5 => xor8(accumulator, operand),
            6 => or8(accumulator, operand),
            7 => cp8(accumulator, operand),
            _ => unreachable!("three-bit ALU operation field"),
        };
        self.set_flags(result.flags);
        if operation != 7 {
            self.registers.a = result.value;
        }
    }

    /// Execute accumulator-only BASE instructions whose effects commit at M1 completion.
    fn execute_accumulator_misc(&mut self, opcode: u8) {
        let result = match opcode {
            0x07 => rotate_accumulator(self.registers.a, self.registers.f, 0),
            0x0F => rotate_accumulator(self.registers.a, self.registers.f, 1),
            0x17 => rotate_accumulator(self.registers.a, self.registers.f, 2),
            0x1F => rotate_accumulator(self.registers.a, self.registers.f, 3),
            0x27 => daa8(self.registers.a, self.registers.f),
            0x2F => cpl8(self.registers.a, self.registers.f),
            _ => unreachable!("accumulator-only instruction decode"),
        };
        self.registers.a = result.value;
        self.set_flags(result.flags);
    }

    /// Execute SCF/CCF with the previous-instruction Q latch semantics.
    fn execute_carry_flag_operation(&mut self, opcode: u8) {
        let undocumented = (self.registers.a | (self.q_previous ^ self.registers.f)) & FLAG_XY;
        let flags = match opcode {
            0x37 => (self.registers.f & (FLAG_S | FLAG_Z | FLAG_PV)) | undocumented | FLAG_C,
            0x3F => {
                let carry = self.registers.f & FLAG_C != 0;
                (self.registers.f & (FLAG_S | FLAG_Z | FLAG_PV))
                    | undocumented
                    | if carry { FLAG_H } else { FLAG_C }
            }
            _ => unreachable!("SCF/CCF opcode decode"),
        };
        self.set_flags(flags);
    }

    #[inline]
    fn set_flags(&mut self, flags: u8) {
        self.registers.f = flags;
        self.q = flags;
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
        self.set_flags(flags);
        value
    }

    fn write_register_pair(&mut self, pair: u8, value: u16) {
        match pair {
            0 => self.registers.set_bc(value),
            1 => self.registers.set_de(value),
            2 => self.registers.set_hl(value),
            3 => self.registers.sp = value,
            _ => unreachable!("two-bit register-pair field"),
        }
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
        let next_mcycle_index = self
            .mcycle_index
            .checked_add(1)
            .ok_or(Z80MicroSequencerError::InstructionTStateOverflow)?;
        let request = self
            .handshake
            .begin(Z80BusOperation::MemoryRead, address, 3)?;
        self.mcycle_index = next_mcycle_index;
        self.tstate_index = 0;
        self.phase = phase;
        Ok(Z80MicroStep::BusRequest(request))
    }

    fn issue_cb_memory_write(
        &mut self,
        address: u16,
        value: u8,
    ) -> Result<Z80MicroStep, Z80MicroSequencerError> {
        let next_mcycle_index = self
            .mcycle_index
            .checked_add(1)
            .ok_or(Z80MicroSequencerError::InstructionTStateOverflow)?;
        let request = self
            .handshake
            .begin(Z80BusOperation::MemoryWrite { value }, address, 3)?;
        self.mcycle_index = next_mcycle_index;
        self.tstate_index = 0;
        self.phase = Z80MicroSequencerPhase::CbMemoryWrite { address, value };
        Ok(Z80MicroStep::BusRequest(request))
    }

    fn issue_write(
        &mut self,
        address: u16,
        value: u8,
    ) -> Result<Z80MicroStep, Z80MicroSequencerError> {
        let next_mcycle_index = self
            .mcycle_index
            .checked_add(1)
            .ok_or(Z80MicroSequencerError::InstructionTStateOverflow)?;
        let request = self
            .handshake
            .begin(Z80BusOperation::MemoryWrite { value }, address, 3)?;
        self.mcycle_index = next_mcycle_index;
        self.tstate_index = 0;
        self.phase = Z80MicroSequencerPhase::MemoryWrite { address, value };
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

    fn execute_alu_opcode(opcode: u8, registers: Z80Registers) -> Z80MicroSequencer {
        let mut cpu = Z80MicroSequencer {
            registers,
            ..Z80MicroSequencer::default()
        };
        let fetch = request(cpu.begin_instruction().unwrap());
        retired(
            cpu.complete_bus_cycle(fetch.id, Some(opcode)).unwrap(),
            opcode,
            4,
        );
        cpu
    }

    #[test]
    fn alu_register_matrix_executes_all_56_register_source_encodings() {
        for operation in 0_u8..8 {
            for source in [0_u8, 1, 2, 3, 4, 5, 7] {
                let opcode = 0x80 | (operation << 3) | source;
                let cpu = execute_alu_opcode(
                    opcode,
                    Z80Registers {
                        a: 0x22,
                        b: 0x01,
                        c: 0x02,
                        d: 0x03,
                        e: 0x04,
                        h: 0x05,
                        l: 0x06,
                        f: FLAG_C,
                        ..Z80Registers::default()
                    },
                );
                assert!(!cpu.instruction_active(), "opcode {opcode:02X}");
            }
        }
    }

    #[test]
    fn alu_register_operations_match_fixed_result_and_flag_vectors() {
        let cases = [
            (
                0x80,
                Z80Registers {
                    a: 0x7F,
                    b: 0x01,
                    ..Z80Registers::default()
                },
                0x80,
                0x94,
            ),
            (
                0x89,
                Z80Registers {
                    a: 0x7F,
                    c: 0x00,
                    f: FLAG_C,
                    ..Z80Registers::default()
                },
                0x80,
                0x94,
            ),
            (
                0x92,
                Z80Registers {
                    a: 0x00,
                    d: 0x01,
                    ..Z80Registers::default()
                },
                0xFF,
                0xBB,
            ),
            (
                0x9B,
                Z80Registers {
                    a: 0x80,
                    e: 0x00,
                    f: FLAG_C,
                    ..Z80Registers::default()
                },
                0x7F,
                0x3E,
            ),
            (
                0xA4,
                Z80Registers {
                    a: 0xF0,
                    h: 0x0F,
                    ..Z80Registers::default()
                },
                0x00,
                0x54,
            ),
            (
                0xAD,
                Z80Registers {
                    a: 0xAA,
                    l: 0xAA,
                    ..Z80Registers::default()
                },
                0x00,
                0x44,
            ),
            (
                0xB7,
                Z80Registers {
                    a: 0x80,
                    ..Z80Registers::default()
                },
                0x80,
                0x80,
            ),
        ];

        for (opcode, registers, expected_a, expected_f) in cases {
            let cpu = execute_alu_opcode(opcode, registers);
            assert_eq!(cpu.registers.a, expected_a, "opcode {opcode:02X}");
            assert_eq!(cpu.registers.f, expected_f, "opcode {opcode:02X}");
        }

        let compare = execute_alu_opcode(
            0xB8,
            Z80Registers {
                a: 0x10,
                b: 0x28,
                ..Z80Registers::default()
            },
        );
        assert_eq!(compare.registers.a, 0x10, "CP must preserve A");
        assert_eq!(compare.registers.f, 0xBB, "CP copies X/Y from its operand");
    }

    #[test]
    fn alu_register_source_field_selects_the_requested_register() {
        let source_values = [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x10];
        let source_codes = [0_u8, 1, 2, 3, 4, 5, 7];
        for (source, value) in source_codes.into_iter().zip(source_values) {
            let cpu = execute_alu_opcode(
                0x80 | source,
                Z80Registers {
                    a: 0x10,
                    b: 0x01,
                    c: 0x02,
                    d: 0x03,
                    e: 0x04,
                    h: 0x05,
                    l: 0x06,
                    ..Z80Registers::default()
                },
            );
            assert_eq!(
                cpu.registers.a,
                0x10_u8.wrapping_add(value),
                "source {source}"
            );
        }
    }

    #[test]
    fn alu_memory_operand_issues_scheduled_read_instead_of_rejecting_opcode() {
        let mut cpu = Z80MicroSequencer::default();
        let fetch = request(cpu.begin_instruction().unwrap());
        let read = request(cpu.complete_bus_cycle(fetch.id, Some(0x86)).unwrap());
        assert_eq!(read.operation, Z80BusOperation::MemoryRead);
        assert_eq!(read.address, 0);
        assert_eq!(read.base_tstates, 3);
        assert!(matches!(
            cpu.export_state().phase,
            Z80MicroSequencerPhase::MemoryReadAlu {
                operation: 0,
                address: 0
            }
        ));

        retired(
            cpu.complete_bus_cycle(read.id, Some(0x42)).unwrap(),
            0x86,
            7,
        );
        assert_eq!(cpu.registers.a, 0x42);
        assert!(!cpu.instruction_active());
        assert_eq!(cpu.pending_request(), None);
    }

    #[test]
    fn cb_ed_and_index_prefixes_issue_a_second_m1_fetch() {
        for prefix in [0xCB, 0xED, 0xDD, 0xFD] {
            let mut cpu = Z80MicroSequencer::default();
            let first = request(cpu.begin_instruction().unwrap());
            let second = request(cpu.complete_bus_cycle(first.id, Some(prefix)).unwrap());
            assert_eq!(second.operation, Z80BusOperation::OpcodeFetch);
            assert_eq!(second.address, 1);
            assert_eq!(second.base_tstates, 4);
            let state = cpu.export_state();
            assert_eq!(state.opcode, Some(prefix));
            assert_eq!(state.mcycle_index, 1);
            assert_eq!(state.instruction_tstates, 4);
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.refresh, 1);
            assert_eq!(
                Z80MicroSequencer::restore_state(state)
                    .unwrap()
                    .pending_request(),
                Some(second)
            );
        }
    }

    #[test]
    fn repeated_index_prefixes_count_each_m1_and_last_prefix_is_retained() {
        let mut cpu = Z80MicroSequencer::default();
        let first = request(cpu.begin_instruction().unwrap());
        let second = request(cpu.complete_bus_cycle(first.id, Some(0xDD)).unwrap());
        let third = request(cpu.complete_bus_cycle(second.id, Some(0xFD)).unwrap());
        assert_eq!(third.operation, Z80BusOperation::OpcodeFetch);
        assert_eq!(third.address, 2);
        assert_eq!(cpu.refresh, 2);
        assert_eq!(cpu.export_state().opcode, Some(0xFD));
        assert_eq!(cpu.export_state().instruction_tstates, 8);

        let error = cpu.complete_bus_cycle(third.id, Some(0x00)).unwrap_err();
        assert_eq!(
            error,
            Z80MicroSequencerError::UnsupportedOpcode {
                opcode: 0x00,
                address: 2
            }
        );
        assert_eq!(cpu.registers.pc, 3);
        assert_eq!(cpu.refresh, 3);
        assert!(!cpu.instruction_active());
    }

    #[test]
    fn dd_cb_stops_before_treating_displacement_as_an_m1_opcode() {
        let mut cpu = Z80MicroSequencer::default();
        let first = request(cpu.begin_instruction().unwrap());
        let second = request(cpu.complete_bus_cycle(first.id, Some(0xDD)).unwrap());
        let error = cpu.complete_bus_cycle(second.id, Some(0xCB)).unwrap_err();
        assert_eq!(
            error,
            Z80MicroSequencerError::UnsupportedOpcode {
                opcode: 0xCB,
                address: 1
            }
        );
        assert_eq!(cpu.registers.pc, 2);
        assert_eq!(cpu.refresh, 2);
        assert!(!cpu.instruction_active());
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
        assert!(!cpu.instruction_active());
        assert!(cpu.pending_request().is_none());
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
        assert_eq!((cpu.mcycle_index, cpu.tstate_index), (1, 4));
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
            (
                cpu.export_state().mcycle_index,
                cpu.export_state().tstate_index
            ),
            (0, 0)
        );

        let read = request(cpu.complete_bus_cycle(fetch.id, Some(0x34)).unwrap());
        assert_eq!(
            (
                cpu.export_state().mcycle_index,
                cpu.export_state().tstate_index
            ),
            (1, 0)
        );

        assert_eq!(
            cpu.complete_bus_cycle(read.id, Some(0x7F)).unwrap(),
            Z80MicroStep::InternalCycle { tstates: 1 }
        );
        assert_eq!(
            (
                cpu.export_state().mcycle_index,
                cpu.export_state().tstate_index
            ),
            (1, 3)
        );

        let write = request(cpu.complete_internal_cycle().unwrap());
        assert_eq!(
            (
                cpu.export_state().mcycle_index,
                cpu.export_state().tstate_index
            ),
            (2, 0)
        );
        retired(cpu.complete_bus_cycle(write.id, None).unwrap(), 0x34, 11);
        assert_eq!(
            (
                cpu.export_state().mcycle_index,
                cpu.export_state().tstate_index
            ),
            (0, 0)
        );
    }

    #[test]
    fn restore_rejects_nonzero_elapsed_time_during_opcode_fetch() {
        let mut cpu = Z80MicroSequencer::default();
        cpu.begin_instruction().unwrap();
        let mut state = cpu.export_state();
        state.instruction_tstates = 1;

        assert_eq!(
            Z80MicroSequencer::restore_state(state),
            Err(Z80MicroSequencerError::InvalidState)
        );
    }

    #[test]
    fn restore_rejects_memory_read_before_m1_elapsed_time() {
        let mut cpu = Z80MicroSequencer::default();
        let fetch = request(cpu.begin_instruction().unwrap());
        let _read = cpu.complete_bus_cycle(fetch.id, Some(0x3E)).unwrap();
        let mut state = cpu.export_state();
        state.instruction_tstates = 3;

        assert_eq!(
            Z80MicroSequencer::restore_state(state),
            Err(Z80MicroSequencerError::InvalidState)
        );
    }

    #[test]
    fn restore_rejects_opcode_fetch_with_inconsistent_request_metadata() {
        let mut cpu = Z80MicroSequencer::default();
        cpu.begin_instruction().unwrap();

        let mut wrong_duration = cpu.export_state();
        wrong_duration
            .handshake
            .pending_request
            .as_mut()
            .unwrap()
            .base_tstates = 1;
        assert_eq!(
            Z80MicroSequencer::restore_state(wrong_duration),
            Err(Z80MicroSequencerError::InvalidState)
        );

        let mut wrong_address = cpu.export_state();
        wrong_address
            .handshake
            .pending_request
            .as_mut()
            .unwrap()
            .address = 1;
        assert_eq!(
            Z80MicroSequencer::restore_state(wrong_address),
            Err(Z80MicroSequencerError::InvalidState)
        );
    }

    #[test]
    fn restore_rejects_immediate_read_opcode_destination_address_and_duration_mismatch() {
        let mut cpu = Z80MicroSequencer::default();
        let fetch = request(cpu.begin_instruction().unwrap());
        cpu.complete_bus_cycle(fetch.id, Some(0x3E)).unwrap();

        let mut wrong_destination = cpu.export_state();
        wrong_destination.phase = Z80MicroSequencerPhase::ImmediateReadRegister { destination: 0 };
        assert_eq!(
            Z80MicroSequencer::restore_state(wrong_destination),
            Err(Z80MicroSequencerError::InvalidState)
        );

        let mut wrong_address = cpu.export_state();
        wrong_address
            .handshake
            .pending_request
            .as_mut()
            .unwrap()
            .address = 0;
        assert_eq!(
            Z80MicroSequencer::restore_state(wrong_address),
            Err(Z80MicroSequencerError::InvalidState)
        );

        let mut wrong_duration = cpu.export_state();
        wrong_duration
            .handshake
            .pending_request
            .as_mut()
            .unwrap()
            .base_tstates = 1;
        assert_eq!(
            Z80MicroSequencer::restore_state(wrong_duration),
            Err(Z80MicroSequencerError::InvalidState)
        );
    }

    #[test]
    fn restore_rejects_memory_read_register_destination_mismatch() {
        let mut cpu = Z80MicroSequencer::default();
        let fetch = request(cpu.begin_instruction().unwrap());
        cpu.complete_bus_cycle(fetch.id, Some(0x46)).unwrap();

        let mut state = cpu.export_state();
        state.phase = Z80MicroSequencerPhase::MemoryReadRegister { destination: 1 };
        assert_eq!(
            Z80MicroSequencer::restore_state(state),
            Err(Z80MicroSequencerError::InvalidState)
        );
    }

    #[test]
    fn restore_rejects_mcycle_index_outside_the_active_phase() {
        let mut cpu = Z80MicroSequencer::default();
        cpu.begin_instruction().unwrap();
        let mut state = cpu.export_state();
        state.mcycle_index = u32::MAX;

        assert_eq!(
            Z80MicroSequencer::restore_state(state),
            Err(Z80MicroSequencerError::InvalidState)
        );
    }

    #[test]
    fn restore_rejects_internal_tstate_offset_ahead_of_elapsed_time() {
        let mut cpu = Z80MicroSequencer::default();
        let fetch = request(cpu.begin_instruction().unwrap());
        let read = request(cpu.complete_bus_cycle(fetch.id, Some(0x34)).unwrap());
        assert_eq!(
            cpu.complete_bus_cycle(read.id, Some(0x7F)).unwrap(),
            Z80MicroStep::InternalCycle { tstates: 1 }
        );

        let mut state = cpu.export_state();
        state.tstate_index = 4;
        assert_eq!(state.instruction_tstates, 7);
        assert_eq!(
            Z80MicroSequencer::restore_state(state),
            Err(Z80MicroSequencerError::InvalidState)
        );
    }

    #[test]
    fn complete_bus_cycle_is_atomic_when_follow_up_request_allocation_fails() {
        let mut cpu = Z80MicroSequencer::default();
        let fetch = request(cpu.begin_instruction().unwrap());
        let mut state = cpu.export_state();
        state.handshake.next_request_id = u64::MAX;
        cpu = Z80MicroSequencer::restore_state(state).unwrap();
        let before = cpu.export_state();

        assert_eq!(
            cpu.complete_bus_cycle(fetch.id, Some(0x3E)),
            Err(Z80MicroSequencerError::Handshake(
                crate::Z80BusHandshakeError::RequestIdExhausted
            ))
        );
        assert_eq!(cpu.export_state(), before);
        assert_eq!(cpu.pending_request(), Some(fetch));
    }

    #[test]
    fn failed_bus_request_allocation_does_not_advance_mcycle_index() {
        let mut cpu = Z80MicroSequencer::default();
        let mut state = cpu.export_state();
        state.handshake.next_request_id = u64::MAX;
        cpu = Z80MicroSequencer::restore_state(state).unwrap();

        assert_eq!(
            cpu.issue_read(Z80MicroSequencerPhase::OpcodeFetch, 0x1234),
            Err(Z80MicroSequencerError::Handshake(
                crate::Z80BusHandshakeError::RequestIdExhausted
            ))
        );
        assert_eq!((cpu.mcycle_index, cpu.tstate_index), (0, 0));

        assert_eq!(
            cpu.issue_write(0x1234, 0x56),
            Err(Z80MicroSequencerError::Handshake(
                crate::Z80BusHandshakeError::RequestIdExhausted
            ))
        );
        assert_eq!((cpu.mcycle_index, cpu.tstate_index), (0, 0));
        assert!(cpu.pending_request().is_none());
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
    fn restore_rejects_memory_write_payload_mismatch_for_ld_hl_immediate() {
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
        let write = request(cpu.complete_bus_cycle(operand.id, Some(0xA5)).unwrap());
        assert_eq!(
            write.operation,
            Z80BusOperation::MemoryWrite { value: 0xA5 }
        );

        let mut state = cpu.export_state();
        state.handshake.pending_request.as_mut().unwrap().operation =
            Z80BusOperation::MemoryWrite { value: 0x5A };
        assert_eq!(
            Z80MicroSequencer::restore_state(state),
            Err(Z80MicroSequencerError::InvalidState)
        );
    }

    #[test]
    fn restore_rejects_memory_write_payload_mismatch_for_inc_hl() {
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
        let write = request(cpu.complete_internal_cycle().unwrap());
        assert_eq!(
            write.operation,
            Z80BusOperation::MemoryWrite { value: 0x80 }
        );

        let mut state = cpu.export_state();
        state.handshake.pending_request.as_mut().unwrap().operation =
            Z80BusOperation::MemoryWrite { value: 0x81 };
        assert_eq!(
            Z80MicroSequencer::restore_state(state),
            Err(Z80MicroSequencerError::InvalidState)
        );
    }

    #[test]
    fn restore_preserves_captured_ld_hl_a_payload_after_register_change() {
        let mut cpu = Z80MicroSequencer {
            registers: Z80Registers {
                a: 0x5A,
                h: 0xC0,
                l: 0x10,
                ..Z80Registers::default()
            },
            ..Z80MicroSequencer::default()
        };
        let fetch = request(cpu.begin_instruction().unwrap());
        let write = request(cpu.complete_bus_cycle(fetch.id, Some(0x77)).unwrap());
        cpu.registers.a = 0x99;

        let restored = Z80MicroSequencer::restore_state(cpu.export_state()).unwrap();
        assert_eq!(restored.pending_request(), Some(write));
    }

    #[test]
    fn snapshot_restore_rejects_phase_handshake_mismatch() {
        let mut cpu = Z80MicroSequencer::default();
        cpu.begin_instruction().unwrap();
        let mut state = cpu.export_state();
        state.phase = Z80MicroSequencerPhase::MemoryWrite {
            address: 0,
            value: 0,
        };

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
    #[test]
    fn restore_rejects_incdec_read_address_mismatch_with_hl() {
        let mut cpu = Z80MicroSequencer {
            registers: Z80Registers {
                h: 0xC0,
                l: 0x10,
                ..Z80Registers::default()
            },
            ..Z80MicroSequencer::default()
        };
        let fetch = request(cpu.begin_instruction().unwrap());
        cpu.complete_bus_cycle(fetch.id, Some(0x34)).unwrap();
        let mut state = cpu.export_state();
        state.phase = Z80MicroSequencerPhase::MemoryReadIncDec {
            decrement: false,
            address: 0xC011,
        };
        state.handshake.pending_request.as_mut().unwrap().address = 0xC011;
        assert_eq!(
            Z80MicroSequencer::restore_state(state),
            Err(Z80MicroSequencerError::InvalidState)
        );
    }

    #[test]
    fn restore_rejects_incdec_internal_address_mismatch_with_hl() {
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
        let mut state = cpu.export_state();
        state.phase = Z80MicroSequencerPhase::MemoryIncDecInternal {
            decrement: false,
            old: 0x7F,
            address: 0xC011,
        };
        assert_eq!(
            Z80MicroSequencer::restore_state(state),
            Err(Z80MicroSequencerError::InvalidState)
        );
    }

    #[test]
    fn restore_rejects_hl_write_address_mismatch_even_when_request_matches_phase() {
        let mut cpu = Z80MicroSequencer {
            registers: Z80Registers {
                a: 0x5A,
                h: 0xC0,
                l: 0x10,
                ..Z80Registers::default()
            },
            ..Z80MicroSequencer::default()
        };
        let fetch = request(cpu.begin_instruction().unwrap());
        cpu.complete_bus_cycle(fetch.id, Some(0x77)).unwrap();
        let mut state = cpu.export_state();
        state.phase = Z80MicroSequencerPhase::MemoryWrite {
            address: 0xC011,
            value: 0x5A,
        };
        state.handshake.pending_request.as_mut().unwrap().address = 0xC011;
        assert_eq!(
            Z80MicroSequencer::restore_state(state),
            Err(Z80MicroSequencerError::InvalidState)
        );
    }

    #[test]
    fn alu_immediate_uses_scheduled_operand_read_and_retires_in_seven_tstates() {
        let mut cpu = Z80MicroSequencer::default();
        cpu.registers.a = 0x7F;
        let fetch = request(cpu.begin_instruction().unwrap());
        let operand_read = request(cpu.complete_bus_cycle(fetch.id, Some(0xC6)).unwrap());
        assert_eq!(operand_read.operation, Z80BusOperation::MemoryRead);
        assert_eq!(operand_read.address, 1);
        assert_eq!(operand_read.base_tstates, 3);

        retired(
            cpu.complete_bus_cycle(operand_read.id, Some(0x01)).unwrap(),
            0xC6,
            7,
        );
        assert_eq!(cpu.registers.a, 0x80);
        assert_eq!(cpu.registers.f, FLAG_S | FLAG_PV | FLAG_H);
        assert_eq!(cpu.registers.pc, 2);
    }

    #[test]
    fn alu_hl_uses_scheduled_memory_read_and_retires_in_seven_tstates() {
        let mut cpu = Z80MicroSequencer::default();
        cpu.registers.a = 0x20;
        cpu.registers.set_hl(0xC123);
        let fetch = request(cpu.begin_instruction().unwrap());
        let operand_read = request(cpu.complete_bus_cycle(fetch.id, Some(0x86)).unwrap());
        assert_eq!(operand_read.operation, Z80BusOperation::MemoryRead);
        assert_eq!(operand_read.address, 0xC123);
        assert_eq!(operand_read.base_tstates, 3);

        retired(
            cpu.complete_bus_cycle(operand_read.id, Some(0x22)).unwrap(),
            0x86,
            7,
        );
        assert_eq!(cpu.registers.a, 0x42);
        assert_eq!(cpu.registers.pc, 1);
    }

    #[test]
    fn alu_immediate_and_memory_wait_states_are_counted_without_changing_operands() {
        for (opcode, operand, address) in [(0xC6, 0x01, 1), (0x86, 0x01, 0xC000)] {
            let mut cpu = Z80MicroSequencer::default();
            cpu.registers.a = 0x10;
            cpu.registers.set_hl(0xC000);
            let fetch = request(cpu.begin_instruction().unwrap());
            let read = request(cpu.complete_bus_cycle(fetch.id, Some(opcode)).unwrap());
            assert_eq!(read.address, address);
            cpu.insert_wait_tstates(2).unwrap();
            retired(
                cpu.complete_bus_cycle(read.id, Some(operand)).unwrap(),
                opcode,
                9,
            );
            assert_eq!(cpu.registers.a, 0x11);
        }
    }

    #[test]
    fn scf_ccf_use_q_latch_and_preserve_previous_flag_write_across_snapshot() {
        fn execute(cpu: &mut Z80MicroSequencer, opcode: u8) {
            let fetch = request(cpu.begin_instruction().unwrap());
            retired(
                cpu.complete_bus_cycle(fetch.id, Some(opcode)).unwrap(),
                opcode,
                4,
            );
        }

        let mut cpu = Z80MicroSequencer {
            registers: Z80Registers {
                a: 0x00,
                b: 0x28,
                ..Z80Registers::default()
            },
            ..Z80MicroSequencer::default()
        };

        // CP B writes undocumented X/Y from B, while A remains zero.
        execute(&mut cpu, 0xB8);
        assert_eq!(cpu.registers.f, 0xBB);
        assert_eq!(cpu.q, 0xBB);

        // The next instruction boundary moves Q to Q-previous and clears Q.
        let fetch = request(cpu.begin_instruction().unwrap());
        assert_eq!(cpu.q_previous, 0xBB);
        assert_eq!(cpu.q, 0x00);

        // Both Q values must survive a pending M1 snapshot/restore.
        let encoded = cpu.export_state().encode().unwrap();
        let decoded = crate::Z80MicroSequencerState::decode(&encoded).unwrap();
        let mut restored = Z80MicroSequencer::restore_state(decoded).unwrap();
        retired(
            restored.complete_bus_cycle(fetch.id, Some(0x37)).unwrap(),
            0x37,
            4,
        );
        assert_eq!(restored.registers.f, 0x81);
        assert_eq!(restored.q, 0x81);
        assert_eq!(restored.q_previous, 0xBB);

        // CCF consumes the current carry, clears it, and sets H.
        execute(&mut restored, 0x3F);
        assert_eq!(restored.registers.f, 0x90);
        assert_eq!(restored.q, 0x90);
    }

    #[test]
    fn scf_after_non_flag_instruction_uses_retained_f_for_xy() {
        let mut cpu = Z80MicroSequencer {
            registers: Z80Registers {
                f: FLAG_XY,
                ..Z80Registers::default()
            },
            ..Z80MicroSequencer::default()
        };
        let nop = request(cpu.begin_instruction().unwrap());
        retired(cpu.complete_bus_cycle(nop.id, Some(0x00)).unwrap(), 0x00, 4);
        assert_eq!(cpu.q, 0);

        let scf = request(cpu.begin_instruction().unwrap());
        retired(cpu.complete_bus_cycle(scf.id, Some(0x37)).unwrap(), 0x37, 4);
        assert_eq!(cpu.registers.f, FLAG_XY | FLAG_C);
    }

    #[test]
    fn ld_register_pair_immediate_reads_little_endian_word_before_commit() {
        for (opcode, expected_pair) in [
            (0x01, 0x1234_u16),
            (0x11, 0x5678_u16),
            (0x21, 0x9ABC_u16),
            (0x31, 0xDEF0_u16),
        ] {
            let mut cpu = Z80MicroSequencer::default();
            let fetch = request(cpu.begin_instruction().unwrap());
            let low_read = request(cpu.complete_bus_cycle(fetch.id, Some(opcode)).unwrap());
            assert_eq!(low_read.address, 1);
            assert_eq!(low_read.base_tstates, 3);
            assert_eq!(cpu.registers.pc, 1);

            let low = expected_pair as u8;
            let high_read = request(cpu.complete_bus_cycle(low_read.id, Some(low)).unwrap());
            assert_eq!(high_read.address, 2);
            assert_eq!(high_read.base_tstates, 3);
            assert_eq!(cpu.registers.pc, 2);
            assert_eq!(cpu.registers.bc(), 0);
            assert_eq!(cpu.registers.de(), 0);
            assert_eq!(cpu.registers.hl(), 0);
            assert_eq!(cpu.registers.sp, 0);

            let high = (expected_pair >> 8) as u8;
            retired(
                cpu.complete_bus_cycle(high_read.id, Some(high)).unwrap(),
                opcode,
                10,
            );
            match opcode {
                0x01 => assert_eq!(cpu.registers.bc(), expected_pair),
                0x11 => assert_eq!(cpu.registers.de(), expected_pair),
                0x21 => assert_eq!(cpu.registers.hl(), expected_pair),
                0x31 => assert_eq!(cpu.registers.sp, expected_pair),
                _ => unreachable!(),
            }
            assert_eq!(cpu.registers.pc, 3);
            assert_eq!(cpu.pending_request(), None);
        }
    }

    #[test]
    fn ld_register_pair_immediate_waits_extend_both_operand_cycles() {
        let mut cpu = Z80MicroSequencer::default();
        let fetch = request(cpu.begin_instruction().unwrap());
        let low_read = request(cpu.complete_bus_cycle(fetch.id, Some(0x01)).unwrap());
        cpu.insert_wait_tstate().unwrap();
        let high_read = request(cpu.complete_bus_cycle(low_read.id, Some(0x34)).unwrap());
        cpu.insert_wait_tstates(2).unwrap();
        retired(
            cpu.complete_bus_cycle(high_read.id, Some(0x12)).unwrap(),
            0x01,
            13,
        );
        assert_eq!(cpu.registers.bc(), 0x1234);
    }

    #[test]
    fn jp_hl_commits_target_at_m1_completion_without_extra_bus_cycle() {
        let mut cpu = Z80MicroSequencer {
            registers: Z80Registers {
                h: 0x12,
                l: 0x34,
                ..Z80Registers::default()
            },
            ..Z80MicroSequencer::default()
        };
        let fetch = request(cpu.begin_instruction().unwrap());
        retired(
            cpu.complete_bus_cycle(fetch.id, Some(0xE9)).unwrap(),
            0xE9,
            4,
        );
        assert_eq!(cpu.registers.pc, 0x1234);
        assert_eq!(cpu.pending_request(), None);
    }

    #[test]
    fn ex_de_hl_swaps_register_pairs_at_m1_completion() {
        let mut cpu = Z80MicroSequencer {
            registers: Z80Registers {
                d: 0x12,
                e: 0x34,
                h: 0x56,
                l: 0x78,
                ..Z80Registers::default()
            },
            ..Z80MicroSequencer::default()
        };
        let fetch = request(cpu.begin_instruction().unwrap());
        retired(
            cpu.complete_bus_cycle(fetch.id, Some(0xEB)).unwrap(),
            0xEB,
            4,
        );
        assert_eq!(cpu.registers.de(), 0x5678);
        assert_eq!(cpu.registers.hl(), 0x1234);
        assert_eq!(cpu.registers.pc, 1);
        assert_eq!(cpu.pending_request(), None);
    }

    #[test]
    fn accumulator_rotate_daa_and_cpl_retire_after_m1_without_extra_bus_cycles() {
        for opcode in [0x07, 0x0F, 0x17, 0x1F, 0x27, 0x2F] {
            let mut cpu = Z80MicroSequencer::default();
            let fetch = request(cpu.begin_instruction().unwrap());
            retired(
                cpu.complete_bus_cycle(fetch.id, Some(opcode)).unwrap(),
                opcode,
                4,
            );
            assert_eq!(cpu.pending_request(), None, "opcode {opcode:02X}");
            assert!(!cpu.instruction_active(), "opcode {opcode:02X}");
        }
    }
}

#[cfg(test)]
mod architectural_state_tests {
    use super::Z80MicroSequencer;
    use crate::{Z80AlternateRegisters, Z80ArchitecturalState, Z80Registers};

    #[test]
    fn sequencer_import_export_preserves_the_shared_architectural_dto() {
        let state = Z80ArchitecturalState {
            registers: Z80Registers {
                a: 0x12,
                f: 0xA5,
                b: 0x34,
                c: 0x56,
                d: 0x78,
                e: 0x9A,
                h: 0xBC,
                l: 0xDE,
                sp: 0xF012,
                pc: 0x3456,
            },
            alternate: Z80AlternateRegisters {
                a: 0x21,
                f: 0x5A,
                b: 0x43,
                c: 0x65,
                d: 0x87,
                e: 0xA9,
                h: 0xCB,
                l: 0xED,
            },
            ix: 0x1357,
            iy: 0x2468,
            i: 0x81,
            r: 0xFE,
            iff1: true,
            iff2: false,
            interrupt_mode: 2,
            wz: 0xABCD,
            ei_delay: 1,
            nmi_pending: true,
            int_pending: Some(0xCF),
            q: 0x28,
            q_previous: 0xD7,
            p: true,
            halted: true,
        };
        let mut cpu = Z80MicroSequencer::default();
        cpu.import_architectural_state(state).unwrap();
        assert_eq!(cpu.export_architectural_state(), state);
        let snapshot = cpu.export_state();
        let restored = Z80MicroSequencer::restore_state(snapshot).unwrap();
        assert_eq!(restored.export_architectural_state(), state);
    }
}

#[cfg(test)]
mod indirect_bc_de_tests {
    use super::{Z80MicroSequencer, Z80MicroStep};
    use crate::{Z80BusOperation, Z80Registers};

    fn request(step: Z80MicroStep) -> crate::Z80BusRequest {
        match step {
            Z80MicroStep::BusRequest(request) => request,
            other => panic!("expected bus request, got {other:?}"),
        }
    }

    #[test]
    fn ld_a_from_bc_reads_memory_and_updates_wz_without_touching_flags() {
        let mut cpu = Z80MicroSequencer {
            registers: Z80Registers {
                a: 0x11,
                f: 0xD7,
                b: 0xC1,
                c: 0x23,
                ..Z80Registers::default()
            },
            ..Z80MicroSequencer::default()
        };
        let fetch = request(cpu.begin_instruction().unwrap());
        let read = request(cpu.complete_bus_cycle(fetch.id, Some(0x0A)).unwrap());
        assert_eq!(read.operation, Z80BusOperation::MemoryRead);
        assert_eq!(read.address, 0xC123);
        assert_eq!(cpu.registers.a, 0x11);
        let retired = cpu.complete_bus_cycle(read.id, Some(0xA5)).unwrap();
        assert_eq!(
            retired,
            Z80MicroStep::InstructionRetired {
                opcode: 0x0A,
                tstates: 7
            }
        );
        assert_eq!(cpu.registers.a, 0xA5);
        assert_eq!(cpu.registers.f, 0xD7);
        assert_eq!(cpu.wz, 0xC124);
    }

    #[test]
    fn ld_a_from_de_reads_memory_and_updates_wz() {
        let mut cpu = Z80MicroSequencer {
            registers: Z80Registers {
                d: 0xFE,
                e: 0xFF,
                ..Z80Registers::default()
            },
            ..Z80MicroSequencer::default()
        };
        let fetch = request(cpu.begin_instruction().unwrap());
        let read = request(cpu.complete_bus_cycle(fetch.id, Some(0x1A)).unwrap());
        assert_eq!(read.address, 0xFEFF);
        let retired = cpu.complete_bus_cycle(read.id, Some(0x42)).unwrap();
        assert_eq!(
            retired,
            Z80MicroStep::InstructionRetired {
                opcode: 0x1A,
                tstates: 7
            }
        );
        assert_eq!(cpu.registers.a, 0x42);
        assert_eq!(cpu.wz, 0xFF00);
    }

    #[test]
    fn ld_bc_indirect_from_a_writes_memory_and_sets_wz_high_from_a() {
        let mut cpu = Z80MicroSequencer {
            registers: Z80Registers {
                a: 0x5A,
                b: 0xC1,
                c: 0xFF,
                f: 0xD7,
                ..Z80Registers::default()
            },
            ..Z80MicroSequencer::default()
        };
        let fetch = request(cpu.begin_instruction().unwrap());
        let write = request(cpu.complete_bus_cycle(fetch.id, Some(0x02)).unwrap());
        assert_eq!(write.address, 0xC1FF);
        assert_eq!(
            write.operation,
            Z80BusOperation::MemoryWrite { value: 0x5A }
        );
        assert_eq!(cpu.wz, 0);
        let retired = cpu.complete_bus_cycle(write.id, None).unwrap();
        assert_eq!(
            retired,
            Z80MicroStep::InstructionRetired {
                opcode: 0x02,
                tstates: 7
            }
        );
        assert_eq!(cpu.wz, 0x5A00);
        assert_eq!(cpu.registers.f, 0xD7);
    }

    #[test]
    fn ld_de_indirect_from_a_writes_memory() {
        let mut cpu = Z80MicroSequencer {
            registers: Z80Registers {
                a: 0xA5,
                d: 0x12,
                e: 0x34,
                ..Z80Registers::default()
            },
            ..Z80MicroSequencer::default()
        };
        let fetch = request(cpu.begin_instruction().unwrap());
        let write = request(cpu.complete_bus_cycle(fetch.id, Some(0x12)).unwrap());
        assert_eq!(write.address, 0x1234);
        assert_eq!(
            write.operation,
            Z80BusOperation::MemoryWrite { value: 0xA5 }
        );
        assert_eq!(cpu.wz, 0);
        assert_eq!(
            cpu.complete_bus_cycle(write.id, None).unwrap(),
            Z80MicroStep::InstructionRetired {
                opcode: 0x12,
                tstates: 7
            }
        );
        assert_eq!(cpu.wz, 0xA535);
    }

    #[test]
    fn indirect_bc_de_safe_points_survive_snapshot_restore() {
        for opcode in [0x02, 0x12, 0x0A, 0x1A] {
            let mut cpu = Z80MicroSequencer::default();
            let fetch = request(cpu.begin_instruction().unwrap());
            let pending = cpu.complete_bus_cycle(fetch.id, Some(opcode)).unwrap();
            let state = cpu.export_state();
            let restored = Z80MicroSequencer::restore_state(state).unwrap();
            assert_eq!(restored.export_state(), cpu.export_state());
            assert!(matches!(pending, Z80MicroStep::BusRequest(_)));
        }
    }
}

#[cfg(test)]
mod indirect_bc_de_validation_tests {
    use super::{Z80MicroSequencer, Z80MicroSequencerError, Z80MicroStep};
    use crate::{Z80BusOperation, Z80Registers};

    fn request(step: Z80MicroStep) -> crate::Z80BusRequest {
        match step {
            Z80MicroStep::BusRequest(request) => request,
            other => panic!("expected bus request, got {other:?}"),
        }
    }

    #[test]
    fn indirect_memory_transfers_include_wait_states_in_retirement_time() {
        for (opcode, expected_operation) in [
            (0x02, Z80BusOperation::MemoryWrite { value: 0xA5 }),
            (0x12, Z80BusOperation::MemoryWrite { value: 0xA5 }),
            (0x0A, Z80BusOperation::MemoryRead),
            (0x1A, Z80BusOperation::MemoryRead),
        ] {
            let mut cpu = Z80MicroSequencer {
                registers: Z80Registers {
                    a: 0xA5,
                    ..Z80Registers::default()
                },
                ..Z80MicroSequencer::default()
            };
            let fetch = request(cpu.begin_instruction().unwrap());
            let bus = request(cpu.complete_bus_cycle(fetch.id, Some(opcode)).unwrap());
            assert_eq!(bus.operation, expected_operation);
            cpu.insert_wait_tstates(2).unwrap();
            let data = match bus.operation {
                Z80BusOperation::MemoryRead => Some(0x37),
                Z80BusOperation::MemoryWrite { .. } => None,
                _ => unreachable!(),
            };
            assert_eq!(
                cpu.complete_bus_cycle(bus.id, data).unwrap(),
                Z80MicroStep::InstructionRetired { opcode, tstates: 9 }
            );
        }
    }

    #[test]
    fn restore_rejects_indirect_read_with_wrong_effective_address() {
        let mut cpu = Z80MicroSequencer {
            registers: Z80Registers {
                b: 0x12,
                c: 0x34,
                ..Z80Registers::default()
            },
            ..Z80MicroSequencer::default()
        };
        let fetch = request(cpu.begin_instruction().unwrap());
        let _read = request(cpu.complete_bus_cycle(fetch.id, Some(0x0A)).unwrap());
        let mut state = cpu.export_state();
        state.handshake.pending_request.as_mut().unwrap().address = 0x1235;
        assert_eq!(
            Z80MicroSequencer::restore_state(state),
            Err(Z80MicroSequencerError::InvalidState)
        );
    }

    #[test]
    fn restore_rejects_indirect_write_with_wrong_payload() {
        let mut cpu = Z80MicroSequencer {
            registers: Z80Registers {
                a: 0xA5,
                b: 0x12,
                c: 0x34,
                ..Z80Registers::default()
            },
            ..Z80MicroSequencer::default()
        };
        let fetch = request(cpu.begin_instruction().unwrap());
        let _write = request(cpu.complete_bus_cycle(fetch.id, Some(0x02)).unwrap());
        let mut state = cpu.export_state();
        state.handshake.pending_request.as_mut().unwrap().operation =
            Z80BusOperation::MemoryWrite { value: 0xA4 };
        assert_eq!(
            Z80MicroSequencer::restore_state(state),
            Err(Z80MicroSequencerError::InvalidState)
        );
    }
}

#[cfg(test)]
mod indirect_bc_de_differential_tests {
    use super::{Z80MicroSequencer, Z80MicroStep};
    use crate::bus::CpuBus;
    use crate::{Z80, Z80ArchitecturalState, Z80BusOperation, Z80Registers};

    #[derive(Clone)]
    struct TestBus {
        memory: Box<[u8; 0x10000]>,
    }

    impl Default for TestBus {
        fn default() -> Self {
            Self {
                memory: Box::new([0; 0x10000]),
            }
        }
    }

    impl CpuBus for TestBus {
        fn read_memory(&mut self, address: u16) -> u8 {
            self.memory[usize::from(address)]
        }

        fn write_memory(&mut self, address: u16, value: u8) {
            self.memory[usize::from(address)] = value;
        }
    }

    fn service_request(
        cpu: &mut Z80MicroSequencer,
        bus: &mut TestBus,
        step: Z80MicroStep,
    ) -> Z80MicroStep {
        let Z80MicroStep::BusRequest(request) = step else {
            panic!("expected bus request, got {step:?}");
        };
        match request.operation {
            Z80BusOperation::OpcodeFetch | Z80BusOperation::MemoryRead => {
                let value = bus.read_memory(request.address);
                cpu.complete_bus_cycle(request.id, Some(value)).unwrap()
            }
            Z80BusOperation::MemoryWrite { value } => {
                bus.write_memory(request.address, value);
                cpu.complete_bus_cycle(request.id, None).unwrap()
            }
            operation => panic!("unexpected operation: {operation:?}"),
        }
    }

    #[test]
    fn bc_de_indirect_transfers_match_atomic_executor_architecture_and_memory() {
        for (opcode, address, initial_a, memory_value) in [
            (0x02, 0xC123, 0xA5, 0x19),
            (0x12, 0xFEFF, 0x5A, 0x27),
            (0x0A, 0xC123, 0x11, 0xA5),
            (0x1A, 0xFEFF, 0x22, 0x5A),
        ] {
            let mut registers = Z80Registers {
                a: initial_a,
                f: 0xD7,
                pc: 0x0100,
                ..Z80Registers::default()
            };
            match opcode {
                0x02 | 0x0A => registers.set_bc(address),
                0x12 | 0x1A => registers.set_de(address),
                _ => unreachable!(),
            }
            let architectural = Z80ArchitecturalState {
                registers,
                alternate: crate::Z80AlternateRegisters::default(),
                ix: 0,
                iy: 0,
                i: 0,
                r: 0,
                iff1: false,
                iff2: false,
                interrupt_mode: 0,
                wz: 0,
                ei_delay: 0,
                nmi_pending: false,
                int_pending: None,
                q: 0,
                q_previous: 0,
                p: false,
                halted: false,
            };

            let mut atomic = Z80::default();
            atomic.import_architectural_state(architectural).unwrap();
            let mut atomic_bus = TestBus::default();
            atomic_bus.memory[0x0100] = opcode;
            atomic_bus.memory[usize::from(address)] = memory_value;

            let mut sequenced = Z80MicroSequencer::default();
            sequenced.import_architectural_state(architectural).unwrap();
            let mut sequenced_bus = atomic_bus.clone();

            assert_eq!(atomic.step(&mut atomic_bus).unwrap(), 7);
            let fetch = sequenced.begin_instruction().unwrap();
            let memory = service_request(&mut sequenced, &mut sequenced_bus, fetch);
            let retired = service_request(&mut sequenced, &mut sequenced_bus, memory);
            assert_eq!(
                retired,
                Z80MicroStep::InstructionRetired { opcode, tstates: 7 },
                "opcode {opcode:02X}"
            );

            assert_eq!(
                sequenced.export_architectural_state(),
                atomic.export_architectural_state(),
                "architectural state mismatch for opcode {opcode:02X}"
            );
            assert_eq!(
                sequenced_bus.memory.as_ref(),
                atomic_bus.memory.as_ref(),
                "memory mismatch for opcode {opcode:02X}"
            );
        }
    }
}

#[cfg(test)]
mod cb_memory_differential_tests {
    use super::{Z80MicroSequencer, Z80MicroStep};
    use crate::bus::CpuBus;
    use crate::{Z80, Z80ArchitecturalState, Z80BusOperation, Z80Registers};

    #[derive(Clone)]
    struct TestBus {
        memory: Box<[u8; 0x10000]>,
        reads: Vec<u16>,
        writes: Vec<(u16, u8)>,
    }

    impl Default for TestBus {
        fn default() -> Self {
            Self {
                memory: Box::new([0; 0x10000]),
                reads: Vec::new(),
                writes: Vec::new(),
            }
        }
    }

    impl CpuBus for TestBus {
        fn read_memory(&mut self, address: u16) -> u8 {
            self.reads.push(address);
            self.memory[usize::from(address)]
        }

        fn write_memory(&mut self, address: u16, value: u8) {
            self.writes.push((address, value));
            self.memory[usize::from(address)] = value;
        }
    }

    fn request(step: Z80MicroStep) -> crate::Z80BusRequest {
        match step {
            Z80MicroStep::BusRequest(request) => request,
            other => panic!("expected bus request, got {other:?}"),
        }
    }

    fn read_request(
        cpu: &mut Z80MicroSequencer,
        bus: &mut TestBus,
        step: Z80MicroStep,
    ) -> Z80MicroStep {
        let request = request(step);
        assert!(matches!(
            request.operation,
            Z80BusOperation::OpcodeFetch | Z80BusOperation::MemoryRead
        ));
        let value = bus.read_memory(request.address);
        cpu.complete_bus_cycle(request.id, Some(value)).unwrap()
    }

    fn state() -> Z80ArchitecturalState {
        Z80ArchitecturalState {
            registers: Z80Registers {
                a: 0x5A,
                f: 0xD7,
                b: 0x12,
                c: 0x34,
                d: 0x56,
                e: 0x78,
                h: 0xC1,
                l: 0x23,
                pc: 0xFFFE,
                sp: 0xD000,
            },
            alternate: crate::Z80AlternateRegisters {
                a: 0x21,
                f: 0x5A,
                b: 0x43,
                c: 0x65,
                d: 0x87,
                e: 0xA9,
                h: 0xCB,
                l: 0xED,
            },
            ix: 0x1357,
            iy: 0x2468,
            i: 0xA5,
            r: 0xFE,
            iff1: false,
            iff2: false,
            interrupt_mode: 0,
            wz: 0xA328,
            ei_delay: 0,
            nmi_pending: false,
            int_pending: None,
            q: 0x28,
            q_previous: 0x5A,
            p: false,
            halted: false,
        }
    }

    fn roundtrip(cpu: Z80MicroSequencer) -> Z80MicroSequencer {
        Z80MicroSequencer::restore_state(cpu.export_state()).unwrap()
    }

    #[test]
    fn all_32_cb_hl_encodings_match_atomic_state_bus_trace_and_timing() {
        for group in 0_u8..4 {
            for operation in 0_u8..8 {
                let opcode = (group << 6) | (operation << 3) | 0x06;
                for operand in [0x00, 0x81, 0x7F, 0xA5] {
                    let architectural = state();
                    let mut atomic = Z80::default();
                    atomic.import_architectural_state(architectural).unwrap();
                    let mut atomic_bus = TestBus::default();
                    atomic_bus.memory[0xFFFE] = 0xCB;
                    atomic_bus.memory[0xFFFF] = opcode;
                    atomic_bus.memory[0xC123] = operand;

                    let mut sequenced = Z80MicroSequencer::default();
                    sequenced.import_architectural_state(architectural).unwrap();
                    let mut sequenced_bus = atomic_bus.clone();

                    let expected_tstates = if group == 1 { 12 } else { 15 };
                    assert_eq!(
                        atomic.step(&mut atomic_bus),
                        Ok(expected_tstates),
                        "atomic CB {opcode:02X}, operand {operand:02X}"
                    );

                    sequenced = roundtrip(sequenced);
                    let first_m1 = request(sequenced.begin_instruction().unwrap());
                    assert_eq!(first_m1.operation, Z80BusOperation::OpcodeFetch);
                    assert_eq!(first_m1.address, 0xFFFE);
                    let second_m1 = read_request(
                        &mut sequenced,
                        &mut sequenced_bus,
                        Z80MicroStep::BusRequest(first_m1),
                    );
                    sequenced = roundtrip(sequenced);
                    let second_m1 = request(second_m1);
                    assert_eq!(second_m1.operation, Z80BusOperation::OpcodeFetch);
                    assert_eq!(second_m1.address, 0xFFFF);

                    let memory_read = request(read_request(
                        &mut sequenced,
                        &mut sequenced_bus,
                        Z80MicroStep::BusRequest(second_m1),
                    ));
                    assert_eq!(memory_read.operation, Z80BusOperation::MemoryRead);
                    assert_eq!(memory_read.address, 0xC123);
                    sequenced = roundtrip(sequenced);

                    let internal = sequenced_bus.read_memory(memory_read.address);
                    let internal = sequenced
                        .complete_bus_cycle(memory_read.id, Some(internal))
                        .unwrap();
                    assert_eq!(internal, Z80MicroStep::InternalCycle { tstates: 1 });
                    sequenced = roundtrip(sequenced);

                    let after_internal = sequenced.complete_internal_cycle().unwrap();
                    let retired = if group == 1 {
                        after_internal
                    } else {
                        let write = request(after_internal);
                        assert_eq!(
                            write.operation,
                            Z80BusOperation::MemoryWrite {
                                value: atomic_bus.memory[0xC123]
                            }
                        );
                        assert_eq!(write.address, 0xC123);
                        sequenced = roundtrip(sequenced);
                        sequenced_bus.write_memory(
                            write.address,
                            match write.operation {
                                Z80BusOperation::MemoryWrite { value } => value,
                                _ => unreachable!(),
                            },
                        );
                        sequenced.complete_bus_cycle(write.id, None).unwrap()
                    };

                    assert_eq!(
                        retired,
                        Z80MicroStep::InstructionRetired {
                            opcode,
                            tstates: expected_tstates
                        },
                        "sequenced CB {opcode:02X}, operand {operand:02X}"
                    );
                    assert_eq!(sequenced.registers.pc, 0);
                    assert_eq!(sequenced.refresh, 0x80);
                    assert_eq!(
                        sequenced.export_architectural_state(),
                        atomic.export_architectural_state(),
                        "architectural mismatch for CB {opcode:02X}, operand {operand:02X}"
                    );
                    assert_eq!(
                        sequenced_bus.memory.as_ref(),
                        atomic_bus.memory.as_ref(),
                        "memory mismatch for CB {opcode:02X}, operand {operand:02X}"
                    );
                    assert_eq!(
                        sequenced_bus.reads, atomic_bus.reads,
                        "read trace mismatch for CB {opcode:02X}"
                    );
                    assert_eq!(
                        sequenced_bus.writes, atomic_bus.writes,
                        "write trace mismatch for CB {opcode:02X}"
                    );
                    assert_eq!(sequenced.pending_request(), None);
                }
            }
        }
    }

    #[test]
    fn cb_hl_wait_states_and_snapshots_preserve_read_internal_and_write_boundaries() {
        let architectural = state();
        let mut cpu = Z80MicroSequencer::default();
        cpu.import_architectural_state(architectural).unwrap();
        let mut bus = TestBus::default();
        bus.memory[0xFFFE] = 0xCB;
        bus.memory[0xFFFF] = 0x06; // RLC (HL)
        bus.memory[0xC123] = 0x81;

        let first = request(cpu.begin_instruction().unwrap());
        let second = request(read_request(
            &mut cpu,
            &mut bus,
            Z80MicroStep::BusRequest(first),
        ));
        let memory_read = request(read_request(
            &mut cpu,
            &mut bus,
            Z80MicroStep::BusRequest(second),
        ));
        cpu = roundtrip(cpu);
        cpu.insert_wait_tstates(2).unwrap();

        let value = bus.read_memory(memory_read.address);
        assert_eq!(
            cpu.complete_bus_cycle(memory_read.id, Some(value)).unwrap(),
            Z80MicroStep::InternalCycle { tstates: 1 }
        );
        cpu = roundtrip(cpu);
        let write_step = cpu.complete_internal_cycle().unwrap();
        let write = request(write_step);
        assert_eq!(write.address, 0xC123);
        assert_eq!(
            write.operation,
            Z80BusOperation::MemoryWrite { value: 0x03 }
        );
        cpu = roundtrip(cpu);
        cpu.insert_wait_tstates(3).unwrap();
        bus.write_memory(write.address, 0x03);
        assert_eq!(
            cpu.complete_bus_cycle(write.id, None).unwrap(),
            Z80MicroStep::InstructionRetired {
                opcode: 0x06,
                tstates: 20
            }
        );
        assert_eq!(bus.memory[0xC123], 0x03);
    }

    #[test]
    fn cb_bit_hl_waits_and_snapshot_restore_retire_after_one_internal_tstate() {
        let mut cpu = Z80MicroSequencer::default();
        cpu.import_architectural_state(state()).unwrap();
        let mut bus = TestBus::default();
        bus.memory[0xFFFE] = 0xCB;
        bus.memory[0xFFFF] = 0x7E; // BIT 7,(HL)
        bus.memory[0xC123] = 0x80;

        let first = request(cpu.begin_instruction().unwrap());
        let second = request(read_request(
            &mut cpu,
            &mut bus,
            Z80MicroStep::BusRequest(first),
        ));
        let read = request(read_request(
            &mut cpu,
            &mut bus,
            Z80MicroStep::BusRequest(second),
        ));
        cpu = roundtrip(cpu);
        cpu.insert_wait_tstates(2).unwrap();
        let value = bus.read_memory(read.address);
        assert_eq!(
            cpu.complete_bus_cycle(read.id, Some(value)).unwrap(),
            Z80MicroStep::InternalCycle { tstates: 1 }
        );
        cpu = roundtrip(cpu);
        assert_eq!(
            cpu.complete_internal_cycle().unwrap(),
            Z80MicroStep::InstructionRetired {
                opcode: 0x7E,
                tstates: 14
            }
        );
        assert_eq!(cpu.registers.f & 0xE5, 0xA1);
        assert!(bus.writes.is_empty());
    }
}

#[cfg(test)]
mod cb_register_differential_tests {
    use super::{Z80MicroSequencer, Z80MicroStep};
    use crate::bus::CpuBus;
    use crate::{Z80, Z80ArchitecturalState, Z80BusOperation, Z80Registers};

    #[derive(Clone)]
    struct TestBus {
        memory: Box<[u8; 0x10000]>,
    }

    impl Default for TestBus {
        fn default() -> Self {
            Self {
                memory: Box::new([0; 0x10000]),
            }
        }
    }

    impl CpuBus for TestBus {
        fn read_memory(&mut self, address: u16) -> u8 {
            self.memory[usize::from(address)]
        }

        fn write_memory(&mut self, address: u16, value: u8) {
            self.memory[usize::from(address)] = value;
        }
    }

    fn request(step: Z80MicroStep) -> crate::Z80BusRequest {
        match step {
            Z80MicroStep::BusRequest(request) => request,
            other => panic!("expected bus request, got {other:?}"),
        }
    }

    fn initial_state(registers: Z80Registers, q: u8) -> Z80ArchitecturalState {
        Z80ArchitecturalState {
            registers,
            alternate: crate::Z80AlternateRegisters::default(),
            ix: 0x1357,
            iy: 0x2468,
            i: 0xA5,
            r: 0xFE,
            iff1: false,
            iff2: false,
            interrupt_mode: 0,
            wz: 0xA328,
            ei_delay: 0,
            nmi_pending: false,
            int_pending: None,
            q,
            q_previous: 0x5A,
            p: false,
            halted: false,
        }
    }

    #[test]
    fn all_56_cb_register_encodings_match_atomic_architecture_and_memory() {
        for group in 0_u8..4 {
            for operation in 0_u8..8 {
                for target in [0_u8, 1, 2, 3, 4, 5, 7] {
                    let opcode = (group << 6) | (operation << 3) | target;
                    for (operand, flags, q) in
                        [(0x00, 0x01, 0x00), (0x81, 0xD7, 0x28), (0x7F, 0xA5, 0x80)]
                    {
                        let registers = Z80Registers {
                            a: operand,
                            f: flags,
                            b: operand.rotate_left(1),
                            c: operand.rotate_right(1),
                            d: operand ^ 0x55,
                            e: operand ^ 0xAA,
                            h: 0x42,
                            l: 0x19,
                            pc: 0xFFFE,
                            sp: 0xD000,
                        };
                        let state = initial_state(registers, q);
                        let mut atomic = Z80::default();
                        atomic.import_architectural_state(state).unwrap();
                        let mut atomic_bus = TestBus::default();
                        atomic_bus.memory[0xFFFE] = 0xCB;
                        atomic_bus.memory[0xFFFF] = opcode;

                        let mut sequenced = Z80MicroSequencer::default();
                        sequenced.import_architectural_state(state).unwrap();
                        assert_eq!(atomic.step(&mut atomic_bus), Ok(8), "opcode {opcode:02X}");
                        let first_m1 = request(sequenced.begin_instruction().unwrap());
                        assert_eq!(first_m1.operation, Z80BusOperation::OpcodeFetch);
                        assert_eq!(first_m1.address, 0xFFFE);
                        let second = request(
                            sequenced
                                .complete_bus_cycle(first_m1.id, Some(0xCB))
                                .unwrap(),
                        );
                        assert_eq!(second.operation, Z80BusOperation::OpcodeFetch);
                        assert_eq!(second.address, 0xFFFF);
                        let retired = sequenced
                            .complete_bus_cycle(second.id, Some(opcode))
                            .unwrap();
                        assert_eq!(
                            retired,
                            Z80MicroStep::InstructionRetired { opcode, tstates: 8 },
                            "opcode {opcode:02X}"
                        );

                        assert_eq!(
                            sequenced.export_architectural_state(),
                            atomic.export_architectural_state(),
                            "architectural mismatch for CB {opcode:02X}"
                        );
                        assert_eq!(sequenced.pending_request(), None);
                        assert_eq!(atomic_bus.memory[0xFFFE], 0xCB);
                        assert_eq!(atomic_bus.memory[0xFFFF], opcode);
                    }
                }
            }
        }
    }

    #[test]
    fn cb_register_fetch_waits_extend_both_m1_cycles() {
        let mut cpu = Z80MicroSequencer::default();
        let first = request(cpu.begin_instruction().unwrap());
        cpu.insert_wait_tstates(1).unwrap();
        let second = request(cpu.complete_bus_cycle(first.id, Some(0xCB)).unwrap());
        cpu.insert_wait_tstates(2).unwrap();
        assert_eq!(
            cpu.complete_bus_cycle(second.id, Some(0x00)).unwrap(),
            Z80MicroStep::InstructionRetired {
                opcode: 0x00,
                tstates: 11
            }
        );
        assert_eq!(cpu.registers.pc, 2);
        assert_eq!(cpu.refresh & 0x7F, 2);
        assert_eq!(cpu.pending_request(), None);
    }

    #[test]
    fn cb_memory_encoding_issues_hl_read_after_two_m1_fetches() {
        let mut cpu = Z80MicroSequencer::default();
        let first = request(cpu.begin_instruction().unwrap());
        let second = request(cpu.complete_bus_cycle(first.id, Some(0xCB)).unwrap());
        assert_eq!(second.address, 1);
        let read = request(cpu.complete_bus_cycle(second.id, Some(0x06)).unwrap());
        assert_eq!(read.operation, Z80BusOperation::MemoryRead);
        assert_eq!(read.address, 0);
        assert_eq!(cpu.registers.pc, 2);
        assert_eq!(cpu.refresh & 0x7F, 2);
        assert!(cpu.instruction_active());
    }
}
