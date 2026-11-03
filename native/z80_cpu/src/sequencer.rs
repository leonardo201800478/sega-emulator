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

fn stack_pair_value(registers: &Z80Registers, opcode: u8) -> Option<u16> {
    match opcode {
        0xC5 | 0xC1 => Some(registers.bc()),
        0xD5 | 0xD1 => Some(registers.de()),
        0xE5 | 0xE1 => Some(registers.hl()),
        0xF5 | 0xF1 => Some(u16::from_be_bytes([registers.a, registers.f])),
        _ => None,
    }
}

fn is_call_opcode(opcode: u8) -> bool {
    matches!(
        opcode,
        0xCD | 0xC4 | 0xCC | 0xD4 | 0xDC | 0xE4 | 0xEC | 0xF4 | 0xFC
    )
}

fn is_rst_opcode(opcode: u8) -> bool {
    opcode & 0xC7 == 0xC7
}

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
    BaseAbsoluteTransfer {
        stage: u8,
        address: u16,
        low: u8,
    },
    BaseRelativeBranchImmediate,
    BaseRelativeBranchInternal {
        displacement: i8,
        branch_taken: bool,
        remaining_tstates: u8,
    },
    BaseAbsoluteJump {
        stage: u8,
        low: u8,
    },
    BaseStackPushInternal {
        high: u8,
        low: u8,
    },
    BaseStackPushWrite {
        stage: u8,
        low: u8,
    },
    BaseStackPop {
        stage: u8,
        low: u8,
    },
    BaseCallImmediate {
        stage: u8,
        low: u8,
    },
    BaseControlPushInternal {
        target: u16,
        high: u8,
        low: u8,
    },
    BaseControlPushWrite {
        stage: u8,
        target: u16,
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
    EdRegisterInternal {
        opcode: u8,
    },
    EdPortRead {
        register: u8,
        address: u16,
    },
    EdPortWrite {
        address: u16,
        value: u8,
    },
    Ed16ArithmeticInternal {
        opcode: u8,
        remaining_tstates: u8,
    },
    EdAbsoluteTransfer {
        stage: u8,
        pair: u8,
        store: bool,
        address: u16,
        low: u8,
        pair_value: u16,
    },
    EdReturn {
        stage: u8,
        low: u8,
    },
    EdNibbleRotateRead {
        opcode: u8,
        address: u16,
    },
    EdNibbleRotateInternal {
        opcode: u8,
        address: u16,
        result_a: u8,
        result_memory: u8,
        remaining_tstates: u8,
    },
    EdBlockTransferRead {
        opcode: u8,
        source: u16,
        destination: u16,
    },
    EdBlockTransferWrite {
        opcode: u8,
        source: u16,
        destination: u16,
        value: u8,
    },
    EdBlockRepeatInternal {
        opcode: u8,
        remaining_tstates: u8,
    },
    EdBlockCompareRead {
        opcode: u8,
        address: u16,
    },
    EdBlockCompareInternal {
        opcode: u8,
        remaining_tstates: u8,
        repeat: bool,
    },
    EdBlockCompareRepeatInternal {
        opcode: u8,
        remaining_tstates: u8,
    },
    EdBlockIoInitialInternal {
        opcode: u8,
    },
    EdBlockIoInputPortRead {
        opcode: u8,
        port_address: u16,
        memory_address: u16,
    },
    EdBlockIoInputMemoryWrite {
        opcode: u8,
        port_address: u16,
        memory_address: u16,
        value: u8,
    },
    EdBlockIoOutputMemoryRead {
        opcode: u8,
        memory_address: u16,
    },
    EdBlockIoOutputPortWrite {
        opcode: u8,
        port_address: u16,
        memory_address: u16,
        value: u8,
    },
    EdBlockIoRepeatInternal {
        opcode: u8,
        remaining_tstates: u8,
    },
    BaseInternal {
        opcode: u8,
        remaining_tstates: u8,
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

impl Z80MicroSequencerPhase {
    /// Whether this continuation is waiting for a CPU-internal T-state.
    ///
    /// Internal phases do not own a memory or I/O request. The machine scheduler
    /// must advance the requested T-state(s) before the sequencer can resume.
    pub const fn is_internal_cycle(self) -> bool {
        matches!(
            self,
            Self::CbMemoryInternal { .. }
                | Self::EdRegisterInternal { .. }
                | Self::Ed16ArithmeticInternal { .. }
                | Self::EdNibbleRotateInternal { .. }
                | Self::EdBlockRepeatInternal { .. }
                | Self::EdBlockCompareInternal { .. }
                | Self::EdBlockCompareRepeatInternal { .. }
                | Self::EdBlockIoInitialInternal { .. }
                | Self::EdBlockIoRepeatInternal { .. }
                | Self::BaseInternal { .. }
                | Self::BaseRelativeBranchInternal { .. }
                | Self::BaseStackPushInternal { .. }
                | Self::BaseControlPushInternal { .. }
                | Self::MemoryIncDecInternal { .. }
        )
    }
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
            Z80MicroSequencerPhase::BaseAbsoluteTransfer {
                stage,
                address,
                low,
            } => {
                let opcode = state.opcode;
                let opcode_matches = matches!(opcode, Some(0x22 | 0x2A | 0x32 | 0x3A));
                let request_matches = pending.is_some_and(|request| {
                    let (operation, expected_address) = match stage {
                        0 | 1 => (Z80BusOperation::MemoryRead, state.registers.pc),
                        2 if matches!(opcode, Some(0x22 | 0x32)) => {
                            let value = if opcode == Some(0x22) {
                                state.registers.l
                            } else {
                                state.registers.a
                            };
                            (Z80BusOperation::MemoryWrite { value }, address)
                        }
                        2 => (Z80BusOperation::MemoryRead, address),
                        3 if opcode == Some(0x22) => (
                            Z80BusOperation::MemoryWrite {
                                value: state.registers.h,
                            },
                            address.wrapping_add(1),
                        ),
                        3 => (Z80BusOperation::MemoryRead, address.wrapping_add(1)),
                        _ => return false,
                    };
                    request.operation == operation
                        && request.address == expected_address
                        && request.base_tstates == 3
                });
                opcode_matches
                    && stage <= 3
                    && request_matches
                    && match stage {
                        0 => address == 0 && low == 0 && state.instruction_tstates >= 4,
                        1 => address == 0 && state.instruction_tstates >= 7,
                        2 => state.instruction_tstates >= 10,
                        3 => state.instruction_tstates >= 13,
                        _ => false,
                    }
            }
            Z80MicroSequencerPhase::BaseAbsoluteJump { stage, low } => {
                let opcode_matches = state.opcode.is_some_and(|opcode| {
                    matches!(
                        opcode,
                        0xC2 | 0xC3 | 0xCA | 0xD2 | 0xDA | 0xE2 | 0xEA | 0xF2 | 0xFA
                    )
                });
                opcode_matches
                    && stage <= 1
                    && pending.is_some_and(|request| {
                        request.operation == Z80BusOperation::MemoryRead
                            && request.address == state.registers.pc
                            && request.base_tstates == 3
                    })
                    && match stage {
                        0 => low == 0 && state.instruction_tstates >= 4,
                        1 => state.instruction_tstates >= 7,
                        _ => false,
                    }
            }
            Z80MicroSequencerPhase::BaseCallImmediate { stage, low } => {
                state.opcode.is_some_and(is_call_opcode)
                    && stage <= 1
                    && pending.is_some_and(|request| {
                        request.operation == Z80BusOperation::MemoryRead
                            && request.address == state.registers.pc
                            && request.base_tstates == 3
                    })
                    && match stage {
                        0 => low == 0 && state.instruction_tstates >= 4,
                        1 => state.instruction_tstates >= 7,
                        _ => false,
                    }
            }
            Z80MicroSequencerPhase::BaseControlPushInternal { target, high, low } => {
                let opcode = state.opcode;
                let is_call = opcode.is_some_and(is_call_opcode);
                let is_rst = opcode.is_some_and(is_rst_opcode);
                let target_valid = match opcode {
                    Some(opcode) if is_call_opcode(opcode) => state.wz == target,
                    Some(opcode) if is_rst_opcode(opcode) => target == u16::from(opcode & 0x38),
                    _ => false,
                };
                let return_bytes = state.registers.pc.to_be_bytes();
                (is_call || is_rst)
                    && target_valid
                    && high == return_bytes[0]
                    && low == return_bytes[1]
                    && pending.is_none()
                    && if is_call {
                        state.mcycle_index == 2
                            && state.tstate_index == 3
                            && state.instruction_tstates >= 10
                    } else {
                        state.mcycle_index == 0
                            && state.tstate_index == 4
                            && state.instruction_tstates >= 4
                    }
            }
            Z80MicroSequencerPhase::BaseControlPushWrite { stage, target, low } => {
                let opcode = state.opcode;
                let is_call = opcode.is_some_and(is_call_opcode);
                let is_rst = opcode.is_some_and(is_rst_opcode);
                let target_valid = match opcode {
                    Some(opcode) if is_call_opcode(opcode) => state.wz == target,
                    Some(opcode) if is_rst_opcode(opcode) => target == u16::from(opcode & 0x38),
                    _ => false,
                };
                let return_bytes = state.registers.pc.to_be_bytes();
                let expected_value = if stage == 0 {
                    return_bytes[0]
                } else {
                    return_bytes[1]
                };
                (is_call || is_rst)
                    && stage <= 1
                    && target_valid
                    && low == return_bytes[1]
                    && pending.is_some_and(|request| {
                        matches!(
                            request.operation,
                            Z80BusOperation::MemoryWrite { value } if value == expected_value
                        ) && request.address == state.registers.sp
                            && request.base_tstates == 3
                    })
                    && if is_call {
                        state.mcycle_index == 3 + u32::from(stage)
                            && state.instruction_tstates >= 11 + u32::from(stage) * 3
                    } else {
                        state.mcycle_index == 1 + u32::from(stage)
                            && state.instruction_tstates >= 5 + u32::from(stage) * 3
                    }
            }
            Z80MicroSequencerPhase::BaseStackPushInternal { high, low } => {
                let opcode_matches = state
                    .opcode
                    .is_some_and(|opcode| matches!(opcode, 0xC5 | 0xD5 | 0xE5 | 0xF5));
                let expected = state
                    .opcode
                    .and_then(|opcode| stack_pair_value(&state.registers, opcode));
                opcode_matches
                    && expected
                        .is_some_and(|value| high == (value >> 8) as u8 && low == value as u8)
                    && pending.is_none()
                    && state.mcycle_index == 0
                    && state.tstate_index == 4
                    && state.instruction_tstates >= 4
            }
            Z80MicroSequencerPhase::BaseStackPushWrite { stage, low } => {
                let opcode_matches = state
                    .opcode
                    .is_some_and(|opcode| matches!(opcode, 0xC5 | 0xD5 | 0xE5 | 0xF5));
                let expected = state
                    .opcode
                    .and_then(|opcode| stack_pair_value(&state.registers, opcode));
                let request_matches = expected.is_some_and(|value| {
                    let expected_value = if stage == 0 {
                        (value >> 8) as u8
                    } else {
                        value as u8
                    };
                    pending.is_some_and(|request| {
                        matches!(
                            request.operation,
                            Z80BusOperation::MemoryWrite { value: request_value }
                                if request_value == expected_value
                        ) && request.address == state.registers.sp
                            && request.base_tstates == 3
                    })
                });
                opcode_matches
                    && stage <= 1
                    && expected.is_some_and(|value| low == value as u8)
                    && request_matches
                    && state.instruction_tstates >= 5 + u32::from(stage) * 3
            }
            Z80MicroSequencerPhase::BaseStackPop { stage, low } => {
                state
                    .opcode
                    .is_some_and(|opcode| matches!(opcode, 0xC1 | 0xD1 | 0xE1 | 0xF1))
                    && stage <= 1
                    && pending.is_some_and(|request| {
                        request.operation == Z80BusOperation::MemoryRead
                            && request.address == state.registers.sp
                            && request.base_tstates == 3
                    })
                    && match stage {
                        0 => low == 0 && state.instruction_tstates >= 4,
                        1 => state.instruction_tstates >= 7,
                        _ => false,
                    }
            }
            Z80MicroSequencerPhase::BaseRelativeBranchImmediate => {
                state
                    .opcode
                    .is_some_and(|opcode| matches!(opcode, 0x10 | 0x18 | 0x20 | 0x28 | 0x30 | 0x38))
                    && pending.is_some_and(|request| {
                        request.operation == Z80BusOperation::MemoryRead
                            && request.address == state.registers.pc
                            && request.base_tstates == 3
                    })
                    && state.instruction_tstates >= 4
            }
            Z80MicroSequencerPhase::BaseRelativeBranchInternal {
                branch_taken,
                remaining_tstates,
                ..
            } => {
                let opcode = state.opcode;
                let total_tstates: u8 = match (opcode, branch_taken) {
                    (Some(0x10), true) => 6,
                    (Some(0x10), false) => 1,
                    (Some(0x18 | 0x20 | 0x28 | 0x30 | 0x38), true) => 5,
                    _ => 0,
                };
                let completed = total_tstates.saturating_sub(remaining_tstates);
                let branch_outcome_valid = match opcode {
                    Some(0x10) => (state.registers.b != 0) == branch_taken,
                    Some(0x18) => branch_taken,
                    Some(0x20) => (state.registers.f & FLAG_Z == 0) == branch_taken,
                    Some(0x28) => (state.registers.f & FLAG_Z != 0) == branch_taken,
                    Some(0x30) => (state.registers.f & FLAG_C == 0) == branch_taken,
                    Some(0x38) => (state.registers.f & FLAG_C != 0) == branch_taken,
                    _ => false,
                };
                total_tstates != 0
                    && branch_outcome_valid
                    && (1..=total_tstates).contains(&remaining_tstates)
                    && pending.is_none()
                    && state.mcycle_index == 2
                    && state.tstate_index == u16::from(completed)
                    && state.instruction_tstates >= 7 + u32::from(completed)
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
            Z80MicroSequencerPhase::EdRegisterInternal { opcode } => {
                matches!(opcode, 0x47 | 0x4F | 0x57 | 0x5F)
                    && state.opcode == Some(opcode)
                    && pending.is_none()
                    && state.mcycle_index == 1
                    && state.tstate_index >= 4
                    && state.instruction_tstates >= 4 + u32::from(state.tstate_index)
            }
            Z80MicroSequencerPhase::EdPortRead { register, address } => {
                state.opcode.is_some_and(|opcode| {
                    opcode & 0xC7 == 0x40 && register == ((opcode >> 3) & 0x07)
                }) && pending.is_some_and(|request| {
                    request.operation == Z80BusOperation::PortRead
                        && request.address == address
                        && request.base_tstates == 4
                }) && address == state.registers.bc()
                    && state.instruction_tstates >= 8
            }
            Z80MicroSequencerPhase::EdPortWrite { address, value } => {
                let Some(request) = pending else {
                    return Err(Z80MicroSequencerError::InvalidState);
                };
                let expected_value = match state.opcode {
                    Some(opcode) if opcode & 0xC7 == 0x41 => match (opcode >> 3) & 0x07 {
                        0 => Some(state.registers.b),
                        1 => Some(state.registers.c),
                        2 => Some(state.registers.d),
                        3 => Some(state.registers.e),
                        4 => Some(state.registers.h),
                        5 => Some(state.registers.l),
                        6 => Some(0),
                        7 => Some(state.registers.a),
                        _ => None,
                    },
                    _ => None,
                };
                expected_value == Some(value)
                    && matches!(
                        request.operation,
                        Z80BusOperation::PortWrite {
                            value: request_value
                        } if request_value == value
                    )
                    && request.address == address
                    && request.base_tstates == 4
                    && address == state.registers.bc()
                    && state.instruction_tstates >= 8
            }
            Z80MicroSequencerPhase::Ed16ArithmeticInternal {
                opcode,
                remaining_tstates,
            } => {
                matches!(opcode & 0xCF, 0x4A | 0x42)
                    && state.opcode == Some(opcode)
                    && (1..=7).contains(&remaining_tstates)
                    && pending.is_none()
                    && if remaining_tstates >= 4 {
                        state.mcycle_index == 2
                            && state.tstate_index == u16::from(7 - remaining_tstates)
                    } else {
                        state.mcycle_index == 3
                            && state.tstate_index == u16::from(3 - remaining_tstates)
                    }
                    && state.instruction_tstates >= 8 + u32::from(7 - remaining_tstates)
            }
            Z80MicroSequencerPhase::EdReturn { stage, low } => {
                state.opcode.is_some_and(|opcode| opcode & 0xC7 == 0x45)
                    && stage <= 1
                    && (stage != 0 || low == 0)
                    && pending.is_some_and(|request| {
                        request.operation == Z80BusOperation::MemoryRead
                            && request.address == state.registers.sp
                            && request.base_tstates == 3
                    })
                    && state.instruction_tstates >= 8
            }
            Z80MicroSequencerPhase::EdNibbleRotateRead { opcode, address } => {
                matches!(opcode, 0x67 | 0x6F)
                    && state.opcode == Some(opcode)
                    && pending.is_some_and(|request| {
                        request.operation == Z80BusOperation::MemoryRead
                            && request.address == address
                            && request.base_tstates == 3
                    })
                    && address == state.registers.hl()
                    && state.instruction_tstates >= 8
            }
            Z80MicroSequencerPhase::EdNibbleRotateInternal {
                opcode,
                address,
                remaining_tstates,
                ..
            } => {
                matches!(opcode, 0x67 | 0x6F)
                    && state.opcode == Some(opcode)
                    && (1..=4).contains(&remaining_tstates)
                    && pending.is_none()
                    && address == state.registers.hl()
                    && state.instruction_tstates >= 11 + u32::from(4 - remaining_tstates)
            }
            Z80MicroSequencerPhase::EdBlockIoInitialInternal { opcode } => {
                matches!(
                    opcode,
                    0xA2 | 0xAA | 0xB2 | 0xBA | 0xA3 | 0xAB | 0xB3 | 0xBB
                ) && state.opcode == Some(opcode)
                    && pending.is_none()
                    && state.instruction_tstates >= 8
            }
            Z80MicroSequencerPhase::EdBlockIoInputPortRead {
                opcode,
                port_address,
                memory_address,
            } => {
                matches!(opcode, 0xA2 | 0xAA | 0xB2 | 0xBA)
                    && state.opcode == Some(opcode)
                    && port_address
                        == ((u16::from(state.registers.b.wrapping_add(1)) << 8)
                            | u16::from(state.registers.c))
                    && memory_address == state.registers.hl()
                    && pending.is_some_and(|request| {
                        request.operation == Z80BusOperation::PortRead
                            && request.address == port_address
                            && request.base_tstates == 4
                    })
                    && state.instruction_tstates >= 9
            }
            Z80MicroSequencerPhase::EdBlockIoInputMemoryWrite {
                opcode,
                port_address,
                memory_address,
                value,
            } => {
                matches!(opcode, 0xA2 | 0xAA | 0xB2 | 0xBA)
                    && state.opcode == Some(opcode)
                    && memory_address == state.registers.hl()
                    && port_address
                        == ((u16::from(state.registers.b.wrapping_add(1)) << 8)
                            | u16::from(state.registers.c))
                    && pending.is_some_and(|request| {
                        request.operation == Z80BusOperation::MemoryWrite { value }
                            && request.address == memory_address
                            && request.base_tstates == 3
                    })
                    && state.instruction_tstates >= 13
            }
            Z80MicroSequencerPhase::EdBlockIoOutputMemoryRead {
                opcode,
                memory_address,
            } => {
                matches!(opcode, 0xA3 | 0xAB | 0xB3 | 0xBB)
                    && state.opcode == Some(opcode)
                    && memory_address == state.registers.hl()
                    && pending.is_some_and(|request| {
                        request.operation == Z80BusOperation::MemoryRead
                            && request.address == memory_address
                            && request.base_tstates == 3
                    })
                    && state.instruction_tstates >= 9
            }
            Z80MicroSequencerPhase::EdBlockIoOutputPortWrite {
                opcode,
                port_address,
                memory_address,
                value,
            } => {
                let direction = if matches!(opcode, 0xAB | 0xBB) {
                    -1_i16
                } else {
                    1_i16
                };
                matches!(opcode, 0xA3 | 0xAB | 0xB3 | 0xBB)
                    && state.opcode == Some(opcode)
                    && port_address == state.registers.bc()
                    && memory_address.wrapping_add_signed(direction) == state.registers.hl()
                    && pending.is_some_and(|request| {
                        request.operation == Z80BusOperation::PortWrite { value }
                            && request.address == port_address
                            && request.base_tstates == 4
                    })
                    && state.instruction_tstates >= 12
            }
            Z80MicroSequencerPhase::EdBlockIoRepeatInternal {
                opcode,
                remaining_tstates,
            } => {
                matches!(opcode, 0xB2 | 0xBA | 0xB3 | 0xBB)
                    && state.opcode == Some(opcode)
                    && state.registers.b != 0
                    && state.wz == state.registers.pc.wrapping_add(1)
                    && (1..=5).contains(&remaining_tstates)
                    && pending.is_none()
                    && state.instruction_tstates >= 16 + u32::from(5 - remaining_tstates)
            }
            Z80MicroSequencerPhase::EdBlockTransferRead {
                opcode,
                source,
                destination,
            } => {
                matches!(opcode, 0xA0 | 0xA8 | 0xB0 | 0xB8)
                    && state.opcode == Some(opcode)
                    && source == state.registers.hl()
                    && destination == state.registers.de()
                    && pending.is_some_and(|request| {
                        request.operation == Z80BusOperation::MemoryRead
                            && request.address == source
                            && request.base_tstates == 3
                    })
                    && state.instruction_tstates >= 8
            }
            Z80MicroSequencerPhase::EdBlockTransferWrite {
                opcode,
                source,
                destination,
                value,
            } => {
                matches!(opcode, 0xA0 | 0xA8 | 0xB0 | 0xB8)
                    && state.opcode == Some(opcode)
                    && source == state.registers.hl()
                    && destination == state.registers.de()
                    && pending.is_some_and(|request| {
                        request.operation == Z80BusOperation::MemoryWrite { value }
                            && request.address == destination
                            && request.base_tstates == 5
                    })
                    && state.instruction_tstates >= 11
            }
            Z80MicroSequencerPhase::EdBlockRepeatInternal {
                opcode,
                remaining_tstates,
            } => {
                matches!(opcode, 0xB0 | 0xB8)
                    && state.opcode == Some(opcode)
                    && (1..=5).contains(&remaining_tstates)
                    && pending.is_none()
                    && state.instruction_tstates >= 16 + u32::from(5 - remaining_tstates)
            }
            Z80MicroSequencerPhase::EdBlockCompareRead { opcode, address } => {
                matches!(opcode, 0xA1 | 0xA9 | 0xB1 | 0xB9)
                    && state.opcode == Some(opcode)
                    && address == state.registers.hl()
                    && pending.is_some_and(|request| {
                        request.operation == Z80BusOperation::MemoryRead
                            && request.address == address
                            && request.base_tstates == 3
                    })
                    && state.instruction_tstates >= 8
            }
            Z80MicroSequencerPhase::EdBlockCompareInternal {
                opcode,
                remaining_tstates,
                repeat,
            } => {
                matches!(opcode, 0xA1 | 0xA9 | 0xB1 | 0xB9)
                    && state.opcode == Some(opcode)
                    && (!repeat || matches!(opcode, 0xB1 | 0xB9))
                    && (1..=5).contains(&remaining_tstates)
                    && pending.is_none()
                    && state.instruction_tstates >= 11 + u32::from(5 - remaining_tstates)
            }
            Z80MicroSequencerPhase::EdBlockCompareRepeatInternal {
                opcode,
                remaining_tstates,
            } => {
                matches!(opcode, 0xB1 | 0xB9)
                    && state.opcode == Some(opcode)
                    && (1..=5).contains(&remaining_tstates)
                    && pending.is_none()
                    && state.instruction_tstates >= 16 + u32::from(5 - remaining_tstates)
            }
            Z80MicroSequencerPhase::EdAbsoluteTransfer {
                stage,
                pair,
                store,
                address,
                low,
                pair_value,
            } => {
                let opcode_matches = state.opcode.is_some_and(|opcode| {
                    let expected = if store { 0x43 } else { 0x4B };
                    opcode & 0xCF == expected && pair == ((opcode >> 4) & 0x03)
                });
                let request_matches = pending.is_some_and(|request| {
                    let (operation_matches, expected_address) = match stage {
                        0 | 1 => (
                            request.operation == Z80BusOperation::MemoryRead,
                            state.registers.pc,
                        ),
                        2 if store => (
                            request.operation
                                == Z80BusOperation::MemoryWrite {
                                    value: pair_value as u8,
                                },
                            address,
                        ),
                        2 => (request.operation == Z80BusOperation::MemoryRead, address),
                        3 if store => (
                            request.operation
                                == Z80BusOperation::MemoryWrite {
                                    value: (pair_value >> 8) as u8,
                                },
                            address.wrapping_add(1),
                        ),
                        3 => (
                            request.operation == Z80BusOperation::MemoryRead,
                            address.wrapping_add(1),
                        ),
                        _ => return false,
                    };
                    operation_matches
                        && request.address == expected_address
                        && request.base_tstates == 3
                });
                opcode_matches
                    && pair <= 3
                    && stage <= 3
                    && request_matches
                    && match stage {
                        0 => address == 0 && low == 0,
                        1 => address <= 0x00FF && low == 0,
                        2 => low == 0,
                        3 => true,
                        _ => false,
                    }
                    && state.instruction_tstates >= 8 + u32::from(stage) * 3
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
            Z80MicroSequencerPhase::BaseInternal {
                opcode,
                remaining_tstates,
            } => {
                let total_internal_tstates: u8 = match opcode {
                    0x03 | 0x0B | 0x13 | 0x1B | 0x23 | 0x2B | 0x33 | 0x3B | 0xF9 => 2,
                    0x09 | 0x19 | 0x29 | 0x39 => 7,
                    _ => 0,
                };
                let completed_tstates = total_internal_tstates.saturating_sub(remaining_tstates);
                let position_valid = if total_internal_tstates == 7 {
                    if remaining_tstates >= 4 {
                        state.mcycle_index == 1
                            && state.tstate_index == u16::from(completed_tstates)
                    } else {
                        state.mcycle_index == 2
                            && state.tstate_index == u16::from(3 - remaining_tstates)
                    }
                } else {
                    state.mcycle_index == 0
                        && state.tstate_index == 4 + u16::from(completed_tstates)
                };
                total_internal_tstates != 0
                    && (1..=total_internal_tstates).contains(&remaining_tstates)
                    && state.opcode == Some(opcode)
                    && pending.is_none()
                    && position_valid
                    && state.instruction_tstates >= 4 + u32::from(completed_tstates)
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
                    Some(0x22) => (10, Some(address), None),
                    Some(0x32) => (10, Some(address), Some(state.registers.a)),
                    Some(0x34 | 0x35) => (8, Some(state.registers.hl()), None),
                    Some(0x67 | 0x6F) => (15, Some(state.registers.hl()), None),
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
            | Z80MicroSequencerPhase::MemoryReadIncDec { .. }
            | Z80MicroSequencerPhase::BaseRelativeBranchImmediate => {
                state.mcycle_index == 1 && state.tstate_index == 0
            }
            Z80MicroSequencerPhase::BaseRelativeBranchInternal {
                branch_taken,
                remaining_tstates,
                ..
            } => {
                let total_tstates: u8 = match (state.opcode, branch_taken) {
                    (Some(0x10), true) => 6,
                    (Some(0x10), false) => 1,
                    (Some(0x18 | 0x20 | 0x28 | 0x30 | 0x38), true) => 5,
                    _ => 0,
                };
                total_tstates != 0
                    && state.mcycle_index == 2
                    && state.tstate_index
                        == u16::from(total_tstates.saturating_sub(remaining_tstates))
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
            Z80MicroSequencerPhase::EdRegisterInternal { .. } => {
                state.mcycle_index == 1
                    && state.tstate_index >= 4
                    && state.instruction_tstates >= 4 + u32::from(state.tstate_index)
            }
            Z80MicroSequencerPhase::EdPortRead { .. }
            | Z80MicroSequencerPhase::EdPortWrite { .. } => {
                state.mcycle_index == 2 && state.tstate_index == 0
            }
            Z80MicroSequencerPhase::Ed16ArithmeticInternal {
                remaining_tstates, ..
            } => {
                if remaining_tstates >= 4 {
                    state.mcycle_index == 2
                        && state.tstate_index == u16::from(7 - remaining_tstates)
                } else {
                    state.mcycle_index == 3
                        && state.tstate_index == u16::from(3 - remaining_tstates)
                }
            }
            Z80MicroSequencerPhase::ImmediateReadRegisterPairHigh { .. } => {
                state.mcycle_index == 2 && state.tstate_index == 0
            }
            Z80MicroSequencerPhase::BaseAbsoluteTransfer { stage, .. } => {
                state.mcycle_index == 1 + u32::from(stage) && state.tstate_index == 0
            }
            Z80MicroSequencerPhase::BaseCallImmediate { stage, .. } => {
                stage <= 1 && state.mcycle_index == 1 + u32::from(stage) && state.tstate_index == 0
            }
            Z80MicroSequencerPhase::BaseControlPushInternal { .. } => match state.opcode {
                Some(opcode) if is_call_opcode(opcode) => {
                    state.mcycle_index == 2 && state.tstate_index == 3
                }
                Some(opcode) if is_rst_opcode(opcode) => {
                    state.mcycle_index == 0 && state.tstate_index == 4
                }
                _ => false,
            },
            Z80MicroSequencerPhase::BaseControlPushWrite { stage, .. } => {
                stage <= 1
                    && match state.opcode {
                        Some(opcode) if is_call_opcode(opcode) => {
                            state.mcycle_index == 3 + u32::from(stage) && state.tstate_index == 0
                        }
                        Some(opcode) if is_rst_opcode(opcode) => {
                            state.mcycle_index == 1 + u32::from(stage) && state.tstate_index == 0
                        }
                        _ => false,
                    }
            }
            Z80MicroSequencerPhase::BaseStackPushInternal { .. } => {
                state.mcycle_index == 0 && state.tstate_index == 4
            }
            Z80MicroSequencerPhase::BaseStackPushWrite { stage, .. } => {
                stage <= 1 && state.mcycle_index == 1 + u32::from(stage) && state.tstate_index == 0
            }
            Z80MicroSequencerPhase::BaseStackPop { stage, .. } => {
                stage <= 1 && state.mcycle_index == 1 + u32::from(stage) && state.tstate_index == 0
            }
            Z80MicroSequencerPhase::BaseAbsoluteJump { stage, .. } => {
                state.mcycle_index == 1 + u32::from(stage) && state.tstate_index == 0
            }
            Z80MicroSequencerPhase::EdAbsoluteTransfer { stage, .. } => {
                state.mcycle_index == 2 + u32::from(stage) && state.tstate_index == 0
            }
            Z80MicroSequencerPhase::EdReturn { stage, .. } => {
                state.mcycle_index == 2 + u32::from(stage) && state.tstate_index == 0
            }
            Z80MicroSequencerPhase::EdNibbleRotateRead { .. } => {
                state.mcycle_index == 2 && state.tstate_index == 0
            }
            Z80MicroSequencerPhase::EdNibbleRotateInternal { .. } => {
                state.mcycle_index == 3 && state.tstate_index <= 3
            }
            Z80MicroSequencerPhase::EdBlockIoInitialInternal { .. } => {
                state.mcycle_index == 1 && state.tstate_index == 4
            }
            Z80MicroSequencerPhase::EdBlockIoInputPortRead { .. }
            | Z80MicroSequencerPhase::EdBlockIoOutputMemoryRead { .. } => {
                state.mcycle_index == 2 && state.tstate_index == 0
            }
            Z80MicroSequencerPhase::EdBlockIoInputMemoryWrite { .. }
            | Z80MicroSequencerPhase::EdBlockIoOutputPortWrite { .. } => {
                state.mcycle_index == 3 && state.tstate_index == 0
            }
            Z80MicroSequencerPhase::EdBlockIoRepeatInternal {
                remaining_tstates, ..
            } => {
                state.mcycle_index == 4
                    && state.tstate_index == u16::from(5_u8.saturating_sub(remaining_tstates))
            }
            Z80MicroSequencerPhase::EdBlockTransferRead { .. } => {
                state.mcycle_index == 2 && state.tstate_index == 0
            }
            Z80MicroSequencerPhase::EdBlockTransferWrite { .. } => {
                state.mcycle_index == 3 && state.tstate_index == 0
            }
            Z80MicroSequencerPhase::EdBlockRepeatInternal {
                remaining_tstates, ..
            } => state.mcycle_index == 4 && state.tstate_index == u16::from(5 - remaining_tstates),
            Z80MicroSequencerPhase::EdBlockCompareRead { .. } => {
                state.mcycle_index == 2 && state.tstate_index == 0
            }
            Z80MicroSequencerPhase::EdBlockCompareInternal {
                remaining_tstates, ..
            } => state.mcycle_index == 3 && state.tstate_index == u16::from(5 - remaining_tstates),
            Z80MicroSequencerPhase::EdBlockCompareRepeatInternal {
                remaining_tstates, ..
            } => state.mcycle_index == 4 && state.tstate_index == u16::from(5 - remaining_tstates),
            Z80MicroSequencerPhase::BaseInternal {
                opcode,
                remaining_tstates,
            } => {
                let completed_tstates = match opcode {
                    0x03 | 0x0B | 0x13 | 0x1B | 0x23 | 0x2B | 0x33 | 0x3B | 0xF9 => {
                        2_u8.saturating_sub(remaining_tstates)
                    }
                    0x09 | 0x19 | 0x29 | 0x39 => 7_u8.saturating_sub(remaining_tstates),
                    _ => u8::MAX,
                };
                match opcode {
                    0x09 | 0x19 | 0x29 | 0x39 if remaining_tstates >= 4 => {
                        state.mcycle_index == 1
                            && state.tstate_index == u16::from(completed_tstates)
                    }
                    0x09 | 0x19 | 0x29 | 0x39 => {
                        state.mcycle_index == 2
                            && state.tstate_index == u16::from(3 - remaining_tstates)
                    }
                    _ => {
                        state.mcycle_index == 0
                            && state.tstate_index == 4 + u16::from(completed_tstates)
                    }
                }
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
                Some(0x22 | 0x32) => {
                    state.mcycle_index == 3
                        && state.tstate_index == 0
                        && state.instruction_tstates >= 10
                }
                Some(0x34 | 0x35) => {
                    state.mcycle_index == 2
                        && state.tstate_index == 0
                        && state.instruction_tstates >= 8
                }
                Some(0x67 | 0x6F) => {
                    state.mcycle_index == 4
                        && state.tstate_index == 0
                        && state.instruction_tstates >= 15
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
                if previous_prefix == Some(0xED) {
                    return self.execute_ed_register_control(opcode, waits);
                }
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
                    0x22 | 0x2A | 0x32 | 0x3A => self.issue_read(
                        Z80MicroSequencerPhase::BaseAbsoluteTransfer {
                            stage: 0,
                            address: 0,
                            low: 0,
                        },
                        self.registers.pc,
                    ),
                    0xC2 | 0xC3 | 0xCA | 0xD2 | 0xDA | 0xE2 | 0xEA | 0xF2 | 0xFA => self
                        .issue_read(
                            Z80MicroSequencerPhase::BaseAbsoluteJump { stage: 0, low: 0 },
                            self.registers.pc,
                        ),
                    0xCD | 0xC4 | 0xCC | 0xD4 | 0xDC | 0xE4 | 0xEC | 0xF4 | 0xFC => self
                        .issue_read(
                            Z80MicroSequencerPhase::BaseCallImmediate { stage: 0, low: 0 },
                            self.registers.pc,
                        ),
                    opcode if is_rst_opcode(opcode) => {
                        let target = u16::from(opcode & 0x38);
                        let [high, low] = self.registers.pc.to_be_bytes();
                        self.mcycle_index = 0;
                        self.tstate_index = 4;
                        self.phase =
                            Z80MicroSequencerPhase::BaseControlPushInternal { target, high, low };
                        Ok(Z80MicroStep::InternalCycle { tstates: 1 })
                    }
                    0xC5 | 0xD5 | 0xE5 | 0xF5 => {
                        let value = stack_pair_value(&self.registers, opcode)
                            .ok_or(Z80MicroSequencerError::UnexpectedCompletion)?;
                        let [high, low] = value.to_be_bytes();
                        self.mcycle_index = 0;
                        self.tstate_index = 4;
                        self.phase = Z80MicroSequencerPhase::BaseStackPushInternal { high, low };
                        Ok(Z80MicroStep::InternalCycle { tstates: 1 })
                    }
                    0xC1 | 0xD1 | 0xE1 | 0xF1 => self.issue_read(
                        Z80MicroSequencerPhase::BaseStackPop { stage: 0, low: 0 },
                        self.registers.sp,
                    ),
                    0x10 | 0x18 | 0x20 | 0x28 | 0x30 | 0x38 => self.issue_read(
                        Z80MicroSequencerPhase::BaseRelativeBranchImmediate,
                        self.registers.pc,
                    ),
                    0x03 | 0x0B | 0x13 | 0x1B | 0x23 | 0x2B | 0x33 | 0x3B | 0xF9 => {
                        self.mcycle_index = 0;
                        self.tstate_index = 4;
                        self.phase = Z80MicroSequencerPhase::BaseInternal {
                            opcode,
                            remaining_tstates: 2,
                        };
                        Ok(Z80MicroStep::InternalCycle { tstates: 1 })
                    }
                    0x09 | 0x19 | 0x29 | 0x39 => {
                        self.mcycle_index = 1;
                        self.tstate_index = 0;
                        self.phase = Z80MicroSequencerPhase::BaseInternal {
                            opcode,
                            remaining_tstates: 7,
                        };
                        Ok(Z80MicroStep::InternalCycle { tstates: 1 })
                    }
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
            Z80MicroSequencerPhase::BaseAbsoluteTransfer { stage: 0, .. } => {
                let low = response
                    .data
                    .ok_or(Z80MicroSequencerError::UnexpectedCompletion)?;
                self.registers.pc = self.registers.pc.wrapping_add(1);
                self.issue_read(
                    Z80MicroSequencerPhase::BaseAbsoluteTransfer {
                        stage: 1,
                        address: 0,
                        low,
                    },
                    self.registers.pc,
                )
            }
            Z80MicroSequencerPhase::BaseAbsoluteTransfer { stage: 1, low, .. } => {
                let high = response
                    .data
                    .ok_or(Z80MicroSequencerError::UnexpectedCompletion)?;
                self.registers.pc = self.registers.pc.wrapping_add(1);
                let address = u16::from_le_bytes([low, high]);
                let opcode = self
                    .opcode
                    .ok_or(Z80MicroSequencerError::UnexpectedCompletion)?;
                match opcode {
                    0x22 => self.issue_base_absolute_write(address, self.registers.l, 2, 0),
                    0x32 => self.issue_base_absolute_write(address, self.registers.a, 2, 0),
                    0x2A | 0x3A => self.issue_read(
                        Z80MicroSequencerPhase::BaseAbsoluteTransfer {
                            stage: 2,
                            address,
                            low: 0,
                        },
                        address,
                    ),
                    _ => Err(Z80MicroSequencerError::UnexpectedCompletion),
                }
            }
            Z80MicroSequencerPhase::BaseAbsoluteTransfer {
                stage: 2, address, ..
            } => {
                let opcode = self
                    .opcode
                    .ok_or(Z80MicroSequencerError::UnexpectedCompletion)?;
                match opcode {
                    0x22 => self.issue_base_absolute_write(
                        address.wrapping_add(1),
                        self.registers.h,
                        3,
                        0,
                    ),
                    0x32 => {
                        self.wz =
                            (u16::from(self.registers.a) << 8) | (address.wrapping_add(1) & 0x00FF);
                        self.retire(opcode)
                    }
                    0x2A => {
                        let low = response
                            .data
                            .ok_or(Z80MicroSequencerError::UnexpectedCompletion)?;
                        self.issue_read(
                            Z80MicroSequencerPhase::BaseAbsoluteTransfer {
                                stage: 3,
                                address,
                                low,
                            },
                            address.wrapping_add(1),
                        )
                    }
                    0x3A => {
                        self.registers.a = response
                            .data
                            .ok_or(Z80MicroSequencerError::UnexpectedCompletion)?;
                        self.wz = address.wrapping_add(1);
                        self.retire(opcode)
                    }
                    _ => Err(Z80MicroSequencerError::UnexpectedCompletion),
                }
            }
            Z80MicroSequencerPhase::BaseAbsoluteTransfer {
                stage: 3,
                address,
                low,
            } => {
                let opcode = self
                    .opcode
                    .ok_or(Z80MicroSequencerError::UnexpectedCompletion)?;
                match opcode {
                    0x22 => {
                        self.wz = address.wrapping_add(1);
                        self.retire(opcode)
                    }
                    0x2A => {
                        let high = response
                            .data
                            .ok_or(Z80MicroSequencerError::UnexpectedCompletion)?;
                        self.registers.set_hl(u16::from_le_bytes([low, high]));
                        self.wz = address.wrapping_add(1);
                        self.retire(opcode)
                    }
                    _ => Err(Z80MicroSequencerError::UnexpectedCompletion),
                }
            }
            Z80MicroSequencerPhase::BaseAbsoluteTransfer { .. } => {
                Err(Z80MicroSequencerError::UnexpectedCompletion)
            }
            Z80MicroSequencerPhase::BaseAbsoluteJump { stage: 0, .. } => {
                let low = response
                    .data
                    .ok_or(Z80MicroSequencerError::UnexpectedCompletion)?;
                self.registers.pc = self.registers.pc.wrapping_add(1);
                self.issue_read(
                    Z80MicroSequencerPhase::BaseAbsoluteJump { stage: 1, low },
                    self.registers.pc,
                )
            }
            Z80MicroSequencerPhase::BaseAbsoluteJump { stage: 1, low } => {
                let high = response
                    .data
                    .ok_or(Z80MicroSequencerError::UnexpectedCompletion)?;
                self.registers.pc = self.registers.pc.wrapping_add(1);
                let target = u16::from_le_bytes([low, high]);
                let opcode = self
                    .opcode
                    .ok_or(Z80MicroSequencerError::UnexpectedCompletion)?;
                self.wz = target;
                if opcode == 0xC3 || self.condition((opcode >> 3) & 0x07) {
                    self.registers.pc = target;
                }
                self.retire(opcode)
            }
            Z80MicroSequencerPhase::BaseAbsoluteJump { .. } => {
                Err(Z80MicroSequencerError::UnexpectedCompletion)
            }
            Z80MicroSequencerPhase::BaseCallImmediate { stage: 0, .. } => {
                let low = response
                    .data
                    .ok_or(Z80MicroSequencerError::UnexpectedCompletion)?;
                self.registers.pc = self.registers.pc.wrapping_add(1);
                self.issue_read(
                    Z80MicroSequencerPhase::BaseCallImmediate { stage: 1, low },
                    self.registers.pc,
                )
            }
            Z80MicroSequencerPhase::BaseCallImmediate { stage: 1, low } => {
                let high = response
                    .data
                    .ok_or(Z80MicroSequencerError::UnexpectedCompletion)?;
                self.registers.pc = self.registers.pc.wrapping_add(1);
                let target = u16::from_le_bytes([low, high]);
                let opcode = self
                    .opcode
                    .ok_or(Z80MicroSequencerError::UnexpectedCompletion)?;
                self.wz = target;
                if opcode != 0xCD && !self.condition((opcode >> 3) & 0x07) {
                    self.retire(opcode)
                } else {
                    let [high, low] = self.registers.pc.to_be_bytes();
                    self.mcycle_index = 2;
                    self.tstate_index = 3;
                    self.phase =
                        Z80MicroSequencerPhase::BaseControlPushInternal { target, high, low };
                    Ok(Z80MicroStep::InternalCycle { tstates: 1 })
                }
            }
            Z80MicroSequencerPhase::BaseCallImmediate { .. } => {
                Err(Z80MicroSequencerError::UnexpectedCompletion)
            }
            Z80MicroSequencerPhase::BaseControlPushWrite {
                stage: 0,
                target,
                low,
            } => {
                self.registers.sp = self.registers.sp.wrapping_sub(1);
                self.issue_base_control_push_write(1, target, low, self.registers.sp, low)
            }
            Z80MicroSequencerPhase::BaseControlPushWrite {
                stage: 1, target, ..
            } => {
                let opcode = self
                    .opcode
                    .ok_or(Z80MicroSequencerError::UnexpectedCompletion)?;
                self.registers.pc = target;
                self.wz = target;
                self.retire(opcode)
            }
            Z80MicroSequencerPhase::BaseControlPushWrite { .. } => {
                Err(Z80MicroSequencerError::UnexpectedCompletion)
            }
            Z80MicroSequencerPhase::BaseStackPushWrite { stage: 0, low } => {
                self.registers.sp = self.registers.sp.wrapping_sub(1);
                self.issue_base_stack_push_write(1, low, self.registers.sp, low)
            }
            Z80MicroSequencerPhase::BaseStackPushWrite { stage: 1, .. } => {
                let opcode = self
                    .opcode
                    .ok_or(Z80MicroSequencerError::UnexpectedCompletion)?;
                self.retire(opcode)
            }
            Z80MicroSequencerPhase::BaseStackPushWrite { .. } => {
                Err(Z80MicroSequencerError::UnexpectedCompletion)
            }
            Z80MicroSequencerPhase::BaseStackPop { stage: 0, .. } => {
                let low = response
                    .data
                    .ok_or(Z80MicroSequencerError::UnexpectedCompletion)?;
                self.registers.sp = self.registers.sp.wrapping_add(1);
                self.issue_read(
                    Z80MicroSequencerPhase::BaseStackPop { stage: 1, low },
                    self.registers.sp,
                )
            }
            Z80MicroSequencerPhase::BaseStackPop { stage: 1, low } => {
                let high = response
                    .data
                    .ok_or(Z80MicroSequencerError::UnexpectedCompletion)?;
                self.registers.sp = self.registers.sp.wrapping_add(1);
                let opcode = self
                    .opcode
                    .ok_or(Z80MicroSequencerError::UnexpectedCompletion)?;
                self.set_stack_pair_value(opcode, u16::from_le_bytes([low, high]))?;
                self.retire(opcode)
            }
            Z80MicroSequencerPhase::BaseStackPop { .. } => {
                Err(Z80MicroSequencerError::UnexpectedCompletion)
            }
            Z80MicroSequencerPhase::BaseRelativeBranchImmediate => {
                let displacement = response
                    .data
                    .ok_or(Z80MicroSequencerError::UnexpectedCompletion)?
                    as i8;
                self.registers.pc = self.registers.pc.wrapping_add(1);
                let opcode = self
                    .opcode
                    .ok_or(Z80MicroSequencerError::UnexpectedCompletion)?;
                let branch_taken = match opcode {
                    0x10 => {
                        self.registers.b = self.registers.b.wrapping_sub(1);
                        self.registers.b != 0
                    }
                    0x18 => true,
                    0x20 | 0x28 | 0x30 | 0x38 => self.condition((opcode >> 3) & 0x03),
                    _ => return Err(Z80MicroSequencerError::UnexpectedCompletion),
                };
                if opcode != 0x10 && !branch_taken {
                    self.retire(opcode)
                } else {
                    let remaining_tstates = if opcode == 0x10 {
                        if branch_taken { 6 } else { 1 }
                    } else {
                        5
                    };
                    self.mcycle_index = 2;
                    self.tstate_index = 0;
                    self.phase = Z80MicroSequencerPhase::BaseRelativeBranchInternal {
                        displacement,
                        branch_taken,
                        remaining_tstates,
                    };
                    Ok(Z80MicroStep::InternalCycle { tstates: 1 })
                }
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
            Z80MicroSequencerPhase::EdRegisterInternal { opcode } => {
                self.execute_ed_register_internal(opcode);
                self.retire(opcode)
            }
            Z80MicroSequencerPhase::EdPortRead { register, address } => {
                let value = response
                    .data
                    .ok_or(Z80MicroSequencerError::UnexpectedCompletion)?;
                if register != 6 {
                    self.write_register(register, value);
                }
                let mut flags = (self.registers.f & FLAG_C) | (value & (FLAG_S | FLAG_XY));
                if value == 0 {
                    flags |= FLAG_Z;
                }
                if value.count_ones().is_multiple_of(2) {
                    flags |= FLAG_PV;
                }
                self.set_flags(flags);
                self.wz = address.wrapping_add(1);
                self.retire(
                    self.opcode
                        .ok_or(Z80MicroSequencerError::UnexpectedCompletion)?,
                )
            }
            Z80MicroSequencerPhase::EdPortWrite { address, .. } => {
                self.wz = address.wrapping_add(1);
                self.retire(
                    self.opcode
                        .ok_or(Z80MicroSequencerError::UnexpectedCompletion)?,
                )
            }
            Z80MicroSequencerPhase::Ed16ArithmeticInternal { .. } => {
                Err(Z80MicroSequencerError::UnexpectedCompletion)
            }
            Z80MicroSequencerPhase::EdBlockIoInputPortRead {
                opcode,
                port_address,
                memory_address,
            } => {
                let value = response
                    .data
                    .ok_or(Z80MicroSequencerError::UnexpectedCompletion)?;
                self.issue_block_io_memory_write(opcode, port_address, memory_address, value)
            }
            Z80MicroSequencerPhase::EdBlockIoInputMemoryWrite {
                opcode,
                port_address,
                value,
                ..
            } => {
                let direction = if matches!(opcode, 0xAA | 0xBA) {
                    -1_i16
                } else {
                    1_i16
                };
                self.registers
                    .set_hl(self.registers.hl().wrapping_add_signed(direction));
                self.finish_block_io_iteration(opcode, port_address, value)
            }
            Z80MicroSequencerPhase::EdBlockIoOutputMemoryRead {
                opcode,
                memory_address,
            } => {
                let value = response
                    .data
                    .ok_or(Z80MicroSequencerError::UnexpectedCompletion)?;
                let direction = if matches!(opcode, 0xAB | 0xBB) {
                    -1_i16
                } else {
                    1_i16
                };
                self.registers
                    .set_hl(self.registers.hl().wrapping_add_signed(direction));
                self.issue_block_io_port_write(opcode, memory_address, value)
            }
            Z80MicroSequencerPhase::EdBlockIoOutputPortWrite {
                opcode,
                port_address,
                value,
                ..
            } => self.finish_block_io_iteration(opcode, port_address, value),
            Z80MicroSequencerPhase::EdBlockIoInitialInternal { .. }
            | Z80MicroSequencerPhase::EdBlockIoRepeatInternal { .. } => {
                Err(Z80MicroSequencerError::UnexpectedCompletion)
            }
            Z80MicroSequencerPhase::EdBlockTransferRead {
                opcode,
                source,
                destination,
            } => {
                let value = response
                    .data
                    .ok_or(Z80MicroSequencerError::UnexpectedCompletion)?;
                self.issue_ed_block_write(
                    destination,
                    value,
                    Z80MicroSequencerPhase::EdBlockTransferWrite {
                        opcode,
                        source,
                        destination,
                        value,
                    },
                )
            }
            Z80MicroSequencerPhase::EdBlockTransferWrite {
                opcode,
                source,
                destination,
                value,
            } => {
                let direction = if matches!(opcode, 0xA8 | 0xB8) {
                    -1_i16
                } else {
                    1_i16
                };
                self.registers.set_hl(source.wrapping_add_signed(direction));
                self.registers
                    .set_de(destination.wrapping_add_signed(direction));
                let count = self.registers.bc().wrapping_sub(1);
                self.registers.set_bc(count);
                let sum = self.registers.a.wrapping_add(value);
                let mut flags = self.registers.f & (FLAG_S | FLAG_Z | FLAG_C);
                if count != 0 {
                    flags |= FLAG_PV;
                }
                flags |= sum & 0x08;
                flags |= (sum & 0x02) << 4;
                if matches!(opcode, 0xB0 | 0xB8) && count != 0 {
                    self.registers.pc = self.registers.pc.wrapping_sub(2);
                    self.wz = self.registers.pc.wrapping_add(1);
                    flags = (flags & !FLAG_XY) | (((self.registers.pc >> 8) as u8) & FLAG_XY);
                    self.set_flags(flags);
                    self.mcycle_index = 4;
                    self.tstate_index = 0;
                    self.phase = Z80MicroSequencerPhase::EdBlockRepeatInternal {
                        opcode,
                        remaining_tstates: 5,
                    };
                    Ok(Z80MicroStep::InternalCycle { tstates: 1 })
                } else {
                    self.set_flags(flags);
                    self.retire(opcode)
                }
            }
            Z80MicroSequencerPhase::EdBlockRepeatInternal { .. } => {
                Err(Z80MicroSequencerError::UnexpectedCompletion)
            }
            Z80MicroSequencerPhase::EdBlockCompareRead { opcode, address } => {
                let value = response
                    .data
                    .ok_or(Z80MicroSequencerError::UnexpectedCompletion)?;
                let direction = if matches!(opcode, 0xA9 | 0xB9) {
                    -1_i16
                } else {
                    1_i16
                };
                let accumulator = self.registers.a;
                let result = accumulator.wrapping_sub(value);
                let half = (accumulator & 0x0F) < (value & 0x0F);
                self.registers
                    .set_hl(address.wrapping_add_signed(direction));
                let count = self.registers.bc().wrapping_sub(1);
                self.registers.set_bc(count);
                let adjusted = result.wrapping_sub(u8::from(half));
                let mut flags = (self.registers.f & FLAG_C) | FLAG_N | (result & FLAG_S);
                if result == 0 {
                    flags |= FLAG_Z;
                }
                if half {
                    flags |= FLAG_H;
                }
                if count != 0 {
                    flags |= FLAG_PV;
                }
                flags |= adjusted & 0x08;
                flags |= (adjusted & 0x02) << 4;
                let repeat = matches!(opcode, 0xB1 | 0xB9) && count != 0 && result != 0;
                if repeat {
                    self.registers.pc = self.registers.pc.wrapping_sub(2);
                    self.wz = self.registers.pc.wrapping_add(1);
                    flags = (flags & !FLAG_XY) | (((self.registers.pc >> 8) as u8) & FLAG_XY);
                } else {
                    self.wz = self.wz.wrapping_add_signed(direction);
                }
                self.set_flags(flags);
                self.mcycle_index = 3;
                self.tstate_index = 0;
                self.phase = Z80MicroSequencerPhase::EdBlockCompareInternal {
                    opcode,
                    remaining_tstates: 5,
                    repeat,
                };
                Ok(Z80MicroStep::InternalCycle { tstates: 1 })
            }
            Z80MicroSequencerPhase::EdBlockCompareInternal { .. }
            | Z80MicroSequencerPhase::EdBlockCompareRepeatInternal { .. } => {
                Err(Z80MicroSequencerError::UnexpectedCompletion)
            }
            Z80MicroSequencerPhase::EdReturn { stage, low } => match stage {
                0 => {
                    let low = response
                        .data
                        .ok_or(Z80MicroSequencerError::UnexpectedCompletion)?;
                    self.registers.sp = self.registers.sp.wrapping_add(1);
                    self.issue_read(
                        Z80MicroSequencerPhase::EdReturn { stage: 1, low },
                        self.registers.sp,
                    )
                }
                1 => {
                    let high = response
                        .data
                        .ok_or(Z80MicroSequencerError::UnexpectedCompletion)?;
                    self.registers.sp = self.registers.sp.wrapping_add(1);
                    self.registers.pc = u16::from_le_bytes([low, high]);
                    self.iff1 = self.iff2;
                    self.wz = self.registers.pc;
                    self.retire(
                        self.opcode
                            .ok_or(Z80MicroSequencerError::UnexpectedCompletion)?,
                    )
                }
                _ => Err(Z80MicroSequencerError::InvalidState),
            },
            Z80MicroSequencerPhase::EdNibbleRotateRead { opcode, address } => {
                let memory_value = response
                    .data
                    .ok_or(Z80MicroSequencerError::UnexpectedCompletion)?;
                let accumulator = self.registers.a;
                let (result_a, result_memory) = if opcode == 0x67 {
                    (
                        (accumulator & 0xF0) | (memory_value & 0x0F),
                        ((accumulator & 0x0F) << 4) | (memory_value >> 4),
                    )
                } else {
                    (
                        (accumulator & 0xF0) | (memory_value >> 4),
                        (memory_value << 4) | (accumulator & 0x0F),
                    )
                };
                self.phase = Z80MicroSequencerPhase::EdNibbleRotateInternal {
                    opcode,
                    address,
                    result_a,
                    result_memory,
                    remaining_tstates: 4,
                };
                self.mcycle_index = 3;
                self.tstate_index = 0;
                Ok(Z80MicroStep::InternalCycle { tstates: 1 })
            }
            Z80MicroSequencerPhase::EdNibbleRotateInternal { .. } => {
                Err(Z80MicroSequencerError::UnexpectedCompletion)
            }
            Z80MicroSequencerPhase::EdAbsoluteTransfer {
                stage,
                pair,
                store,
                address,
                low,
                pair_value,
            } => match stage {
                0 => {
                    let address_low = response
                        .data
                        .ok_or(Z80MicroSequencerError::UnexpectedCompletion)?;
                    self.registers.pc = self.registers.pc.wrapping_add(1);
                    self.issue_read(
                        Z80MicroSequencerPhase::EdAbsoluteTransfer {
                            stage: 1,
                            pair,
                            store,
                            address: u16::from(address_low),
                            low: 0,
                            pair_value,
                        },
                        self.registers.pc,
                    )
                }
                1 => {
                    let address_high = response
                        .data
                        .ok_or(Z80MicroSequencerError::UnexpectedCompletion)?;
                    self.registers.pc = self.registers.pc.wrapping_add(1);
                    let address = u16::from_le_bytes([address as u8, address_high]);
                    let next_phase = Z80MicroSequencerPhase::EdAbsoluteTransfer {
                        stage: 2,
                        pair,
                        store,
                        address,
                        low: 0,
                        pair_value,
                    };
                    if store {
                        self.issue_ed_absolute_write(address, pair_value as u8, next_phase)
                    } else {
                        self.issue_read(next_phase, address)
                    }
                }
                2 if store => self.issue_ed_absolute_write(
                    address.wrapping_add(1),
                    (pair_value >> 8) as u8,
                    Z80MicroSequencerPhase::EdAbsoluteTransfer {
                        stage: 3,
                        pair,
                        store,
                        address,
                        low: 0,
                        pair_value,
                    },
                ),
                2 => {
                    let data_low = response
                        .data
                        .ok_or(Z80MicroSequencerError::UnexpectedCompletion)?;
                    self.issue_read(
                        Z80MicroSequencerPhase::EdAbsoluteTransfer {
                            stage: 3,
                            pair,
                            store,
                            address,
                            low: data_low,
                            pair_value,
                        },
                        address.wrapping_add(1),
                    )
                }
                3 if store => {
                    self.wz = address.wrapping_add(1);
                    self.retire(
                        self.opcode
                            .ok_or(Z80MicroSequencerError::UnexpectedCompletion)?,
                    )
                }
                3 => {
                    let data_high = response
                        .data
                        .ok_or(Z80MicroSequencerError::UnexpectedCompletion)?;
                    self.write_register_pair(pair, u16::from_le_bytes([low, data_high]));
                    self.wz = address.wrapping_add(1);
                    self.retire(
                        self.opcode
                            .ok_or(Z80MicroSequencerError::UnexpectedCompletion)?,
                    )
                }
                _ => Err(Z80MicroSequencerError::InvalidState),
            },
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
            Z80MicroSequencerPhase::BaseInternal { .. }
            | Z80MicroSequencerPhase::BaseRelativeBranchInternal { .. }
            | Z80MicroSequencerPhase::BaseStackPushInternal { .. }
            | Z80MicroSequencerPhase::BaseControlPushInternal { .. }
            | Z80MicroSequencerPhase::MemoryIncDecInternal { .. } => {
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
            Z80MicroSequencerPhase::BaseInternal {
                opcode,
                remaining_tstates,
            } => {
                let remaining_tstates = remaining_tstates
                    .checked_sub(1)
                    .ok_or(Z80MicroSequencerError::InvalidState)?;
                next.instruction_tstates = next_instruction_tstates;
                if remaining_tstates == 0 {
                    next.execute_base_internal(opcode);
                    next.retire(opcode)?
                } else {
                    if matches!(opcode, 0x09 | 0x19 | 0x29 | 0x39) {
                        if remaining_tstates == 3 {
                            next.mcycle_index = 2;
                            next.tstate_index = 0;
                        } else {
                            next.tstate_index = next
                                .tstate_index
                                .checked_add(1)
                                .ok_or(Z80MicroSequencerError::InstructionTStateOverflow)?;
                        }
                    } else {
                        next.tstate_index = next
                            .tstate_index
                            .checked_add(1)
                            .ok_or(Z80MicroSequencerError::InstructionTStateOverflow)?;
                    }
                    next.phase = Z80MicroSequencerPhase::BaseInternal {
                        opcode,
                        remaining_tstates,
                    };
                    Z80MicroStep::InternalCycle { tstates: 1 }
                }
            }
            Z80MicroSequencerPhase::BaseStackPushInternal { high, low } => {
                next.instruction_tstates = next_instruction_tstates;
                next.registers.sp = next.registers.sp.wrapping_sub(1);
                next.issue_base_stack_push_write(0, low, next.registers.sp, high)?
            }
            Z80MicroSequencerPhase::BaseControlPushInternal { target, high, low } => {
                next.instruction_tstates = next_instruction_tstates;
                next.registers.sp = next.registers.sp.wrapping_sub(1);
                next.issue_base_control_push_write(0, target, low, next.registers.sp, high)?
            }
            Z80MicroSequencerPhase::BaseRelativeBranchInternal {
                displacement,
                branch_taken,
                remaining_tstates,
            } => {
                let opcode = next
                    .opcode
                    .ok_or(Z80MicroSequencerError::UnexpectedCompletion)?;
                let remaining_tstates = remaining_tstates
                    .checked_sub(1)
                    .ok_or(Z80MicroSequencerError::InvalidState)?;
                next.instruction_tstates = next_instruction_tstates;
                if remaining_tstates == 0 {
                    if branch_taken {
                        next.registers.pc = next
                            .registers
                            .pc
                            .wrapping_add_signed(i16::from(displacement));
                        next.wz = next.registers.pc;
                    }
                    next.retire(opcode)?
                } else {
                    next.tstate_index = next
                        .tstate_index
                        .checked_add(1)
                        .ok_or(Z80MicroSequencerError::InstructionTStateOverflow)?;
                    next.phase = Z80MicroSequencerPhase::BaseRelativeBranchInternal {
                        displacement,
                        branch_taken,
                        remaining_tstates,
                    };
                    Z80MicroStep::InternalCycle { tstates: 1 }
                }
            }
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
            Z80MicroSequencerPhase::EdBlockIoInitialInternal { opcode } => {
                next.instruction_tstates = next_instruction_tstates;
                // M3 T1 captures old BC for IN and decrements B; OUT decrements B
                // before its M3 memory read. These updates are visible at the bus boundary.
                if matches!(opcode, 0xA2 | 0xAA | 0xB2 | 0xBA) {
                    let port_address = next.registers.bc();
                    let memory_address = next.registers.hl();
                    next.registers.b = next.registers.b.wrapping_sub(1);
                    next.issue_block_io_port_read(opcode, port_address, memory_address)?
                } else {
                    let memory_address = next.registers.hl();
                    next.registers.b = next.registers.b.wrapping_sub(1);
                    next.issue_read(
                        Z80MicroSequencerPhase::EdBlockIoOutputMemoryRead {
                            opcode,
                            memory_address,
                        },
                        memory_address,
                    )?
                }
            }
            Z80MicroSequencerPhase::EdBlockIoRepeatInternal {
                opcode,
                remaining_tstates,
            } => {
                next.instruction_tstates = next_instruction_tstates;
                if remaining_tstates == 1 {
                    next.retire(opcode)?
                } else {
                    next.phase = Z80MicroSequencerPhase::EdBlockIoRepeatInternal {
                        opcode,
                        remaining_tstates: remaining_tstates - 1,
                    };
                    next.tstate_index = next
                        .tstate_index
                        .checked_add(1)
                        .ok_or(Z80MicroSequencerError::InstructionTStateOverflow)?;
                    Z80MicroStep::InternalCycle { tstates: 1 }
                }
            }
            Z80MicroSequencerPhase::EdBlockRepeatInternal {
                opcode,
                remaining_tstates,
            } => {
                next.instruction_tstates = next_instruction_tstates;
                if remaining_tstates == 1 {
                    next.retire(opcode)?
                } else {
                    next.phase = Z80MicroSequencerPhase::EdBlockRepeatInternal {
                        opcode,
                        remaining_tstates: remaining_tstates - 1,
                    };
                    next.tstate_index = next
                        .tstate_index
                        .checked_add(1)
                        .ok_or(Z80MicroSequencerError::InstructionTStateOverflow)?;
                    Z80MicroStep::InternalCycle { tstates: 1 }
                }
            }
            Z80MicroSequencerPhase::EdNibbleRotateInternal {
                opcode,
                address,
                result_a,
                result_memory,
                remaining_tstates,
            } => {
                next.instruction_tstates = next_instruction_tstates;
                if remaining_tstates == 1 {
                    next.registers.a = result_a;
                    let mut flags = (next.registers.f & FLAG_C) | (result_a & (FLAG_S | FLAG_XY));
                    if result_a == 0 {
                        flags |= FLAG_Z;
                    }
                    if result_a.count_ones().is_multiple_of(2) {
                        flags |= FLAG_PV;
                    }
                    next.set_flags(flags);
                    next.wz = address.wrapping_add(1);
                    next.issue_write(address, result_memory)?
                } else {
                    next.phase = Z80MicroSequencerPhase::EdNibbleRotateInternal {
                        opcode,
                        address,
                        result_a,
                        result_memory,
                        remaining_tstates: remaining_tstates - 1,
                    };
                    next.tstate_index = next
                        .tstate_index
                        .checked_add(1)
                        .ok_or(Z80MicroSequencerError::InstructionTStateOverflow)?;
                    Z80MicroStep::InternalCycle { tstates: 1 }
                }
            }
            Z80MicroSequencerPhase::EdBlockCompareInternal {
                opcode,
                remaining_tstates,
                repeat,
            } => {
                next.instruction_tstates = next_instruction_tstates;
                if remaining_tstates == 1 {
                    if repeat {
                        next.mcycle_index = 4;
                        next.tstate_index = 0;
                        next.phase = Z80MicroSequencerPhase::EdBlockCompareRepeatInternal {
                            opcode,
                            remaining_tstates: 5,
                        };
                        Z80MicroStep::InternalCycle { tstates: 1 }
                    } else {
                        next.retire(opcode)?
                    }
                } else {
                    next.phase = Z80MicroSequencerPhase::EdBlockCompareInternal {
                        opcode,
                        remaining_tstates: remaining_tstates - 1,
                        repeat,
                    };
                    next.tstate_index = next
                        .tstate_index
                        .checked_add(1)
                        .ok_or(Z80MicroSequencerError::InstructionTStateOverflow)?;
                    Z80MicroStep::InternalCycle { tstates: 1 }
                }
            }
            Z80MicroSequencerPhase::EdBlockCompareRepeatInternal {
                opcode,
                remaining_tstates,
            } => {
                next.instruction_tstates = next_instruction_tstates;
                if remaining_tstates == 1 {
                    next.retire(opcode)?
                } else {
                    next.phase = Z80MicroSequencerPhase::EdBlockCompareRepeatInternal {
                        opcode,
                        remaining_tstates: remaining_tstates - 1,
                    };
                    next.tstate_index = next
                        .tstate_index
                        .checked_add(1)
                        .ok_or(Z80MicroSequencerError::InstructionTStateOverflow)?;
                    Z80MicroStep::InternalCycle { tstates: 1 }
                }
            }
            Z80MicroSequencerPhase::EdRegisterInternal { opcode } => {
                next.instruction_tstates = next_instruction_tstates;
                next.execute_ed_register_internal(opcode);
                next.retire(opcode)?
            }
            Z80MicroSequencerPhase::Ed16ArithmeticInternal {
                opcode,
                remaining_tstates,
            } => {
                next.instruction_tstates = next_instruction_tstates;
                if remaining_tstates == 1 {
                    next.execute_ed_16bit_arithmetic(opcode);
                    next.retire(opcode)?
                } else {
                    let remaining_tstates = remaining_tstates - 1;
                    if remaining_tstates == 3 {
                        next.mcycle_index = 3;
                        next.tstate_index = 0;
                    } else {
                        next.tstate_index = next
                            .tstate_index
                            .checked_add(1)
                            .ok_or(Z80MicroSequencerError::InstructionTStateOverflow)?;
                    }
                    next.phase = Z80MicroSequencerPhase::Ed16ArithmeticInternal {
                        opcode,
                        remaining_tstates,
                    };
                    Z80MicroStep::InternalCycle { tstates: 1 }
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

    /// Execute the ED register/control subset that retires after two M1 fetches.
    /// The four LD I/R variants expose the documented fifth T-state of M2.
    fn execute_ed_register_control(
        &mut self,
        opcode: u8,
        waits: u32,
    ) -> Result<Z80MicroStep, Z80MicroSequencerError> {
        match opcode {
            op if op & 0xC7 == 0x44 => {
                let result = sub8(0, self.registers.a);
                self.registers.a = result.value;
                self.set_flags(result.flags);
                self.retire(opcode)
            }
            0x46 | 0x4E | 0x66 | 0x6E => {
                self.interrupt_mode = 0;
                self.retire(opcode)
            }
            0x56 | 0x76 => {
                self.interrupt_mode = 1;
                self.retire(opcode)
            }
            0x5E | 0x7E => {
                self.interrupt_mode = 2;
                self.retire(opcode)
            }
            0x47 | 0x4F | 0x57 | 0x5F => {
                let wait_offset = u16::try_from(waits)
                    .ok()
                    .and_then(|waits| 4_u16.checked_add(waits))
                    .ok_or(Z80MicroSequencerError::InstructionTStateOverflow)?;
                self.tstate_index = wait_offset;
                self.phase = Z80MicroSequencerPhase::EdRegisterInternal { opcode };
                Ok(Z80MicroStep::InternalCycle { tstates: 1 })
            }
            op if op & 0xC7 == 0x40 => self.issue_ed_port_read((opcode >> 3) & 0x07),
            op if op & 0xC7 == 0x41 => {
                let register = (opcode >> 3) & 0x07;
                let value = if register == 6 {
                    0
                } else {
                    self.read_register(register)
                };
                self.issue_ed_port_write(value)
            }
            op if op & 0xCF == 0x43 => {
                let pair = (opcode >> 4) & 0x03;
                let pair_value = self.read_register_pair(pair);
                self.issue_read(
                    Z80MicroSequencerPhase::EdAbsoluteTransfer {
                        stage: 0,
                        pair,
                        store: true,
                        address: 0,
                        low: 0,
                        pair_value,
                    },
                    self.registers.pc,
                )
            }
            op if op & 0xCF == 0x4B => {
                let pair = (opcode >> 4) & 0x03;
                self.issue_read(
                    Z80MicroSequencerPhase::EdAbsoluteTransfer {
                        stage: 0,
                        pair,
                        store: false,
                        address: 0,
                        low: 0,
                        pair_value: 0,
                    },
                    self.registers.pc,
                )
            }
            0xA2 | 0xAA | 0xB2 | 0xBA | 0xA3 | 0xAB | 0xB3 | 0xBB => {
                self.mcycle_index = 1;
                self.tstate_index = 4;
                self.phase = Z80MicroSequencerPhase::EdBlockIoInitialInternal { opcode };
                Ok(Z80MicroStep::InternalCycle { tstates: 1 })
            }
            0xA1 | 0xA9 | 0xB1 | 0xB9 => self.issue_read(
                Z80MicroSequencerPhase::EdBlockCompareRead {
                    opcode,
                    address: self.registers.hl(),
                },
                self.registers.hl(),
            ),
            0xA0 | 0xA8 | 0xB0 | 0xB8 => self.issue_read(
                Z80MicroSequencerPhase::EdBlockTransferRead {
                    opcode,
                    source: self.registers.hl(),
                    destination: self.registers.de(),
                },
                self.registers.hl(),
            ),
            op if op & 0xC7 == 0x45 => self.issue_read(
                Z80MicroSequencerPhase::EdReturn { stage: 0, low: 0 },
                self.registers.sp,
            ),
            0x67 | 0x6F => self.issue_read(
                Z80MicroSequencerPhase::EdNibbleRotateRead {
                    opcode,
                    address: self.registers.hl(),
                },
                self.registers.hl(),
            ),
            op if op & 0xCF == 0x4A || op & 0xCF == 0x42 => {
                self.mcycle_index = 2;
                self.tstate_index = 0;
                self.phase = Z80MicroSequencerPhase::Ed16ArithmeticInternal {
                    opcode,
                    remaining_tstates: 7,
                };
                Ok(Z80MicroStep::InternalCycle { tstates: 1 })
            }
            _ => self.unsupported(opcode),
        }
    }

    /// Execute ADC/SBC HL,ss after its two M1 fetches and seven internal T-states.
    fn execute_ed_16bit_arithmetic(&mut self, opcode: u8) {
        let pair = (opcode >> 4) & 0x03;
        let operand = match pair {
            0 => self.registers.bc(),
            1 => self.registers.de(),
            2 => self.registers.hl(),
            3 => self.registers.sp,
            _ => unreachable!("two-bit register-pair field"),
        };
        let lhs = self.registers.hl();
        let carry = u32::from(self.registers.f & FLAG_C != 0);
        let rhs = u32::from(operand);
        let lhs32 = u32::from(lhs);
        let subtract = opcode & 0x0F == 0x02;
        let result32 = if subtract {
            lhs32.wrapping_sub(rhs + carry)
        } else {
            lhs32 + rhs + carry
        };
        let result = result32 as u16;
        let mut flags = ((result >> 8) as u8) & (FLAG_S | FLAG_XY);
        if result == 0 {
            flags |= FLAG_Z;
        }
        if subtract {
            flags |= FLAG_N;
            if u32::from(lhs & 0x0FFF) < u32::from(operand & 0x0FFF) + carry {
                flags |= FLAG_H;
            }
            if ((lhs ^ operand) & (lhs ^ result) & 0x8000) != 0 {
                flags |= FLAG_PV;
            }
            if lhs32 < rhs + carry {
                flags |= FLAG_C;
            }
        } else {
            if u32::from(lhs & 0x0FFF) + u32::from(operand & 0x0FFF) + carry > 0x0FFF {
                flags |= FLAG_H;
            }
            if (!(lhs ^ operand) & (lhs ^ result) & 0x8000) != 0 {
                flags |= FLAG_PV;
            }
            if result32 > u32::from(u16::MAX) {
                flags |= FLAG_C;
            }
        }
        self.registers.set_hl(result);
        self.wz = lhs.wrapping_add(1);
        self.set_flags(flags);
    }

    /// Commit the semantic effect at the final T-state of an ED register load.
    fn execute_ed_register_internal(&mut self, opcode: u8) {
        match opcode {
            0x47 => self.i = self.registers.a,
            0x4F => self.refresh = self.registers.a,
            0x57 | 0x5F => {
                let value = if opcode == 0x57 { self.i } else { self.refresh };
                self.registers.a = value;
                let mut flags = (self.registers.f & FLAG_C) | (value & (FLAG_S | FLAG_XY));
                if value == 0 {
                    flags |= FLAG_Z;
                }
                if self.iff2 {
                    flags |= FLAG_PV;
                }
                self.set_flags(flags);
                self.p = true;
            }
            _ => unreachable!("ED register-load opcode validation"),
        }
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

    /// Commit a BASE 16-bit operation after its documented internal T-states.
    fn set_stack_pair_value(
        &mut self,
        opcode: u8,
        value: u16,
    ) -> Result<(), Z80MicroSequencerError> {
        let [high, low] = value.to_be_bytes();
        match opcode {
            0xC1 => {
                self.registers.set_bc(value);
            }
            0xD1 => {
                self.registers.set_de(value);
            }
            0xE1 => {
                self.registers.set_hl(value);
            }
            0xF1 => {
                self.registers.a = high;
                self.registers.f = low;
            }
            _ => return Err(Z80MicroSequencerError::UnexpectedCompletion),
        }
        Ok(())
    }

    fn issue_base_control_push_write(
        &mut self,
        stage: u8,
        target: u16,
        low: u8,
        address: u16,
        value: u8,
    ) -> Result<Z80MicroStep, Z80MicroSequencerError> {
        let step = self.issue_write(address, value)?;
        self.phase = Z80MicroSequencerPhase::BaseControlPushWrite { stage, target, low };
        Ok(step)
    }

    fn issue_base_stack_push_write(
        &mut self,
        stage: u8,
        low: u8,
        address: u16,
        value: u8,
    ) -> Result<Z80MicroStep, Z80MicroSequencerError> {
        let step = self.issue_write(address, value)?;
        self.phase = Z80MicroSequencerPhase::BaseStackPushWrite { stage, low };
        Ok(step)
    }

    fn condition(&self, code: u8) -> bool {
        let flags = self.registers.f;
        match code & 0x07 {
            0 => flags & FLAG_Z == 0,
            1 => flags & FLAG_Z != 0,
            2 => flags & FLAG_C == 0,
            3 => flags & FLAG_C != 0,
            4 => flags & FLAG_PV == 0,
            5 => flags & FLAG_PV != 0,
            6 => flags & FLAG_S == 0,
            _ => flags & FLAG_S != 0,
        }
    }

    fn execute_base_internal(&mut self, opcode: u8) {
        match opcode {
            0x03 | 0x0B | 0x13 | 0x1B | 0x23 | 0x2B | 0x33 | 0x3B => {
                let pair = (opcode >> 4) & 0x03;
                let value = self.read_register_pair(pair);
                self.write_register_pair(
                    pair,
                    if opcode & 0x08 != 0 {
                        value.wrapping_sub(1)
                    } else {
                        value.wrapping_add(1)
                    },
                );
            }
            0x09 | 0x19 | 0x29 | 0x39 => {
                let operand = self.read_register_pair((opcode >> 4) & 0x03);
                let hl = self.registers.hl();
                let result = u32::from(hl) + u32::from(operand);
                let result16 = result as u16;
                self.wz = hl.wrapping_add(1);

                let mut flags = self.registers.f & (FLAG_S | FLAG_Z | FLAG_PV);
                flags |= (result16 >> 8) as u8 & FLAG_XY;
                if (hl & 0x0FFF) + (operand & 0x0FFF) > 0x0FFF {
                    flags |= FLAG_H;
                }
                if result > u32::from(u16::MAX) {
                    flags |= FLAG_C;
                }
                self.registers.set_hl(result16);
                self.set_flags(flags);
            }
            0xF9 => self.registers.sp = self.registers.hl(),
            _ => unreachable!("BASE internal opcode validated before execution"),
        }
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

    fn read_register_pair(&self, pair: u8) -> u16 {
        match pair {
            0 => self.registers.bc(),
            1 => self.registers.de(),
            2 => self.registers.hl(),
            3 => self.registers.sp,
            _ => unreachable!("two-bit register-pair field"),
        }
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

    fn issue_block_io_port_read(
        &mut self,
        opcode: u8,
        port_address: u16,
        memory_address: u16,
    ) -> Result<Z80MicroStep, Z80MicroSequencerError> {
        let next_mcycle_index = self
            .mcycle_index
            .checked_add(1)
            .ok_or(Z80MicroSequencerError::InstructionTStateOverflow)?;
        let request = self
            .handshake
            .begin(Z80BusOperation::PortRead, port_address, 4)?;
        self.mcycle_index = next_mcycle_index;
        self.tstate_index = 0;
        self.phase = Z80MicroSequencerPhase::EdBlockIoInputPortRead {
            opcode,
            port_address,
            memory_address,
        };
        Ok(Z80MicroStep::BusRequest(request))
    }

    fn issue_block_io_memory_write(
        &mut self,
        opcode: u8,
        port_address: u16,
        memory_address: u16,
        value: u8,
    ) -> Result<Z80MicroStep, Z80MicroSequencerError> {
        let next_mcycle_index = self
            .mcycle_index
            .checked_add(1)
            .ok_or(Z80MicroSequencerError::InstructionTStateOverflow)?;
        let request =
            self.handshake
                .begin(Z80BusOperation::MemoryWrite { value }, memory_address, 3)?;
        self.mcycle_index = next_mcycle_index;
        self.tstate_index = 0;
        self.phase = Z80MicroSequencerPhase::EdBlockIoInputMemoryWrite {
            opcode,
            port_address,
            memory_address,
            value,
        };
        Ok(Z80MicroStep::BusRequest(request))
    }

    fn issue_block_io_port_write(
        &mut self,
        opcode: u8,
        memory_address: u16,
        value: u8,
    ) -> Result<Z80MicroStep, Z80MicroSequencerError> {
        let port_address = self.registers.bc();
        let next_mcycle_index = self
            .mcycle_index
            .checked_add(1)
            .ok_or(Z80MicroSequencerError::InstructionTStateOverflow)?;
        let request =
            self.handshake
                .begin(Z80BusOperation::PortWrite { value }, port_address, 4)?;
        self.mcycle_index = next_mcycle_index;
        self.tstate_index = 0;
        self.phase = Z80MicroSequencerPhase::EdBlockIoOutputPortWrite {
            opcode,
            port_address,
            memory_address,
            value,
        };
        Ok(Z80MicroStep::BusRequest(request))
    }

    fn finish_block_io_iteration(
        &mut self,
        opcode: u8,
        port_address: u16,
        value: u8,
    ) -> Result<Z80MicroStep, Z80MicroSequencerError> {
        let input = matches!(opcode, 0xA2 | 0xAA | 0xB2 | 0xBA);
        let repeat = matches!(opcode, 0xB2 | 0xBA | 0xB3 | 0xBB);
        let direction = if matches!(opcode, 0xAA | 0xBA | 0xAB | 0xBB) {
            -1_i16
        } else {
            1_i16
        };
        let low_term = if input {
            self.registers.c.wrapping_add_signed(direction as i8)
        } else {
            self.registers.l
        };
        let sum = u16::from(value) + u16::from(low_term);
        self.wz = port_address.wrapping_add_signed(direction);
        let mut flags = self.registers.b & (FLAG_S | FLAG_XY);
        if self.registers.b == 0 {
            flags |= FLAG_Z;
        }
        if value & 0x80 != 0 {
            flags |= FLAG_N;
        }
        if sum > 0xFF {
            flags |= FLAG_H | FLAG_C;
        }
        let parity_arg = (sum as u8 & 0x07) ^ self.registers.b;
        if parity_arg.count_ones().is_multiple_of(2) {
            flags |= FLAG_PV;
        }

        if repeat && self.registers.b != 0 {
            self.registers.pc = self.registers.pc.wrapping_sub(2);
            self.wz = self.registers.pc.wrapping_add(1);
            flags = (flags & !FLAG_XY) | (((self.registers.pc >> 8) as u8) & FLAG_XY);
            let carry = flags & FLAG_C != 0;
            let parity_value = if carry {
                if value & 0x80 != 0 {
                    self.registers.b.wrapping_sub(1) & 0x07
                } else {
                    self.registers.b.wrapping_add(1) & 0x07
                }
            } else {
                self.registers.b & 0x07
            };
            let parity = parity_value.count_ones().is_multiple_of(2);
            if (flags & FLAG_PV != 0) ^ parity ^ true {
                flags |= FLAG_PV;
            } else {
                flags &= !FLAG_PV;
            }
            if carry {
                let half = if value & 0x80 != 0 {
                    self.registers.b & 0x0F == 0
                } else {
                    self.registers.b & 0x0F == 0x0F
                };
                if half {
                    flags |= FLAG_H;
                } else {
                    flags &= !FLAG_H;
                }
            }
            self.set_flags(flags);
            self.mcycle_index = 4;
            self.tstate_index = 0;
            self.phase = Z80MicroSequencerPhase::EdBlockIoRepeatInternal {
                opcode,
                remaining_tstates: 5,
            };
            Ok(Z80MicroStep::InternalCycle { tstates: 1 })
        } else {
            self.set_flags(flags);
            self.retire(opcode)
        }
    }

    fn issue_ed_port_read(&mut self, register: u8) -> Result<Z80MicroStep, Z80MicroSequencerError> {
        let address = self.registers.bc();
        let next_mcycle_index = self
            .mcycle_index
            .checked_add(1)
            .ok_or(Z80MicroSequencerError::InstructionTStateOverflow)?;
        let request = self
            .handshake
            .begin(Z80BusOperation::PortRead, address, 4)?;
        self.mcycle_index = next_mcycle_index;
        self.tstate_index = 0;
        self.phase = Z80MicroSequencerPhase::EdPortRead { register, address };
        Ok(Z80MicroStep::BusRequest(request))
    }

    fn issue_ed_port_write(&mut self, value: u8) -> Result<Z80MicroStep, Z80MicroSequencerError> {
        let address = self.registers.bc();
        let next_mcycle_index = self
            .mcycle_index
            .checked_add(1)
            .ok_or(Z80MicroSequencerError::InstructionTStateOverflow)?;
        let request = self
            .handshake
            .begin(Z80BusOperation::PortWrite { value }, address, 4)?;
        self.mcycle_index = next_mcycle_index;
        self.tstate_index = 0;
        self.phase = Z80MicroSequencerPhase::EdPortWrite { address, value };
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

    fn issue_ed_block_write(
        &mut self,
        address: u16,
        value: u8,
        phase: Z80MicroSequencerPhase,
    ) -> Result<Z80MicroStep, Z80MicroSequencerError> {
        let next_mcycle_index = self
            .mcycle_index
            .checked_add(1)
            .ok_or(Z80MicroSequencerError::InstructionTStateOverflow)?;
        let request = self
            .handshake
            .begin(Z80BusOperation::MemoryWrite { value }, address, 5)?;
        self.mcycle_index = next_mcycle_index;
        self.tstate_index = 0;
        self.phase = phase;
        Ok(Z80MicroStep::BusRequest(request))
    }

    fn issue_ed_absolute_write(
        &mut self,
        address: u16,
        value: u8,
        phase: Z80MicroSequencerPhase,
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
        self.phase = phase;
        Ok(Z80MicroStep::BusRequest(request))
    }

    fn issue_base_absolute_write(
        &mut self,
        address: u16,
        value: u8,
        stage: u8,
        low: u8,
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
        self.phase = Z80MicroSequencerPhase::BaseAbsoluteTransfer {
            stage,
            address: if stage == 2 {
                address
            } else {
                address.wrapping_sub(1)
            },
            low,
        };
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
        if self.ei_delay > 0 && opcode != 0xFB {
            self.ei_delay -= 1;
        }
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
            cpu.complete_bus_cycle(fetch.id, Some(0x76)),
            Err(Z80MicroSequencerError::UnsupportedOpcode { opcode: 0x76, .. })
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
mod ed_16bit_arithmetic_differential_tests {
    use super::{Z80MicroSequencer, Z80MicroStep};
    use crate::bus::CpuBus;
    use crate::{
        Z80, Z80ArchitecturalState, Z80BusOperation, Z80MicroSequencerState, Z80Registers,
    };

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

    fn read_opcode(
        cpu: &mut Z80MicroSequencer,
        bus: &mut TestBus,
        step: Z80MicroStep,
    ) -> Z80MicroStep {
        let request = request(step);
        assert_eq!(request.operation, Z80BusOperation::OpcodeFetch);
        let value = bus.read_memory(request.address);
        cpu.complete_bus_cycle(request.id, Some(value)).unwrap()
    }

    fn state(hl: u16, bc: u16, de: u16, sp: u16, carry: bool) -> Z80ArchitecturalState {
        Z80ArchitecturalState {
            registers: Z80Registers {
                a: 0xA5,
                f: if carry { 0xFF } else { 0xFE },
                b: (bc >> 8) as u8,
                c: bc as u8,
                d: (de >> 8) as u8,
                e: de as u8,
                h: (hl >> 8) as u8,
                l: hl as u8,
                pc: 0xFFFE,
                sp,
            },
            alternate: crate::Z80AlternateRegisters::default(),
            ix: 0x1357,
            iy: 0x2468,
            i: 0xA5,
            r: 0xFE,
            iff1: false,
            iff2: true,
            interrupt_mode: 1,
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
        let encoded = cpu.export_state().encode().unwrap();
        let decoded = Z80MicroSequencerState::decode(&encoded).unwrap();
        Z80MicroSequencer::restore_state(decoded).unwrap()
    }

    fn execute_pair(opcode: u8, architectural: Z80ArchitecturalState) {
        let mut atomic = Z80::default();
        atomic.import_architectural_state(architectural).unwrap();
        let mut atomic_bus = TestBus::default();
        atomic_bus.memory[0xFFFE] = 0xED;
        atomic_bus.memory[0xFFFF] = opcode;

        let mut sequenced = Z80MicroSequencer::default();
        sequenced.import_architectural_state(architectural).unwrap();
        let mut sequenced_bus = atomic_bus.clone();

        assert_eq!(
            atomic.step(&mut atomic_bus),
            Ok(15),
            "atomic ED {opcode:02X}"
        );

        let first = request(sequenced.begin_instruction().unwrap());
        let second = read_opcode(
            &mut sequenced,
            &mut sequenced_bus,
            Z80MicroStep::BusRequest(first),
        );
        let internal = read_opcode(&mut sequenced, &mut sequenced_bus, second);
        assert_eq!(internal, Z80MicroStep::InternalCycle { tstates: 1 });

        sequenced = roundtrip(sequenced);
        let mut retired = internal;
        for index in 0..7 {
            retired = sequenced.complete_internal_cycle().unwrap();
            if index < 6 {
                assert_eq!(
                    retired,
                    Z80MicroStep::InternalCycle { tstates: 1 },
                    "internal T-state {index} for ED {opcode:02X}"
                );
            }
        }
        assert_eq!(
            retired,
            Z80MicroStep::InstructionRetired {
                opcode,
                tstates: 15
            }
        );
        assert_eq!(
            sequenced.export_architectural_state(),
            atomic.export_architectural_state(),
            "architectural mismatch for ED {opcode:02X}"
        );
        assert_eq!(sequenced_bus.reads, atomic_bus.reads);
        assert_eq!(sequenced_bus.writes, atomic_bus.writes);
        assert_eq!(sequenced.pending_request(), None);
    }

    #[test]
    fn all_adc_sbc_hl_register_pairs_match_atomic_executor() {
        let opcodes = [
            0x4A, 0x5A, 0x6A, 0x7A, // ADC HL,BC/DE/HL/SP
            0x42, 0x52, 0x62, 0x72, // SBC HL,BC/DE/HL/SP
        ];
        let values = [0x0000, 0x0001, 0x0FFF, 0x1000, 0x7FFF, 0x8000, 0xFFFF];

        for opcode in opcodes {
            for &hl in &values {
                for &operand in &values {
                    for carry in [false, true] {
                        let pair = (opcode >> 4) & 0x03;
                        let (bc, de, sp) = match pair {
                            0 => (operand, 0x1357, 0x2468),
                            1 => (0x1357, operand, 0x2468),
                            2 => (0x1357, 0x2468, 0x369C),
                            3 => (0x1357, 0x2468, operand),
                            _ => unreachable!(),
                        };
                        execute_pair(opcode, state(hl, bc, de, sp, carry));
                    }
                }
            }
        }
    }

    #[test]
    fn adc_sbc_internal_mcycles_waits_and_snapshots_resume_exactly() {
        let architectural = state(0x7FFF, 0x0001, 0xFFFF, 0x8000, true);
        let mut cpu = Z80MicroSequencer::default();
        cpu.import_architectural_state(architectural).unwrap();
        let mut bus = TestBus::default();
        bus.memory[0xFFFE] = 0xED;
        bus.memory[0xFFFF] = 0x4A; // ADC HL,BC

        let first = request(cpu.begin_instruction().unwrap());
        let second = read_opcode(&mut cpu, &mut bus, Z80MicroStep::BusRequest(first));
        cpu.insert_wait_tstates(3).unwrap();
        assert_eq!(
            read_opcode(&mut cpu, &mut bus, second),
            Z80MicroStep::InternalCycle { tstates: 1 }
        );
        assert_eq!(cpu.export_state().instruction_tstates, 11);

        for _ in 0..4 {
            assert_eq!(
                cpu.complete_internal_cycle().unwrap(),
                Z80MicroStep::InternalCycle { tstates: 1 }
            );
        }
        cpu = roundtrip(cpu);
        assert_eq!(cpu.export_state().mcycle_index, 3);
        assert_eq!(cpu.export_state().tstate_index, 0);

        assert_eq!(
            cpu.complete_internal_cycle().unwrap(),
            Z80MicroStep::InternalCycle { tstates: 1 }
        );
        assert_eq!(
            cpu.complete_internal_cycle().unwrap(),
            Z80MicroStep::InternalCycle { tstates: 1 }
        );
        assert_eq!(
            cpu.complete_internal_cycle().unwrap(),
            Z80MicroStep::InstructionRetired {
                opcode: 0x4A,
                tstates: 18
            }
        );
    }
}

#[cfg(test)]
mod ed_port_io_differential_tests {
    use super::{Z80MicroSequencer, Z80MicroStep};
    use crate::bus::CpuBus;
    use crate::{Z80, Z80ArchitecturalState, Z80BusOperation, Z80Registers};

    #[derive(Clone)]
    struct TestBus {
        memory: Box<[u8; 0x10000]>,
        reads: Vec<u16>,
        writes: Vec<(u16, u8)>,
        port_reads: Vec<u16>,
        port_writes: Vec<(u16, u8)>,
    }

    impl Default for TestBus {
        fn default() -> Self {
            Self {
                memory: Box::new([0; 0x10000]),
                reads: Vec::new(),
                writes: Vec::new(),
                port_reads: Vec::new(),
                port_writes: Vec::new(),
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

        fn read_port(&mut self, port: u16) -> u8 {
            self.port_reads.push(port);
            port as u8 ^ (port >> 8) as u8
        }

        fn write_port(&mut self, port: u16, value: u8) {
            self.port_writes.push((port, value));
        }
    }

    fn request(step: Z80MicroStep) -> crate::Z80BusRequest {
        match step {
            Z80MicroStep::BusRequest(request) => request,
            other => panic!("expected bus request, got {other:?}"),
        }
    }

    fn read_opcode(
        cpu: &mut Z80MicroSequencer,
        bus: &mut TestBus,
        step: Z80MicroStep,
    ) -> Z80MicroStep {
        let request = request(step);
        assert_eq!(request.operation, Z80BusOperation::OpcodeFetch);
        let value = bus.read_memory(request.address);
        cpu.complete_bus_cycle(request.id, Some(value)).unwrap()
    }

    fn state() -> Z80ArchitecturalState {
        Z80ArchitecturalState {
            registers: Z80Registers {
                a: 0xA5,
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
            alternate: crate::Z80AlternateRegisters::default(),
            ix: 0x1357,
            iy: 0x2468,
            i: 0xA5,
            r: 0xFE,
            iff1: false,
            iff2: true,
            interrupt_mode: 1,
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
    fn all_ed_in_and_out_c_encodings_match_atomic_state_and_port_trace() {
        let opcodes = [
            0x40, 0x48, 0x50, 0x58, 0x60, 0x68, 0x70, 0x78, // IN r,(C)
            0x41, 0x49, 0x51, 0x59, 0x61, 0x69, 0x71, 0x79, // OUT (C),r
        ];

        for opcode in opcodes {
            let architectural = state();
            let mut atomic = Z80::default();
            atomic.import_architectural_state(architectural).unwrap();
            let mut atomic_bus = TestBus::default();
            atomic_bus.memory[0xFFFE] = 0xED;
            atomic_bus.memory[0xFFFF] = opcode;

            let mut sequenced = Z80MicroSequencer::default();
            sequenced.import_architectural_state(architectural).unwrap();
            let mut sequenced_bus = atomic_bus.clone();

            assert_eq!(
                atomic.step(&mut atomic_bus),
                Ok(12),
                "atomic ED {opcode:02X}"
            );

            let first = request(sequenced.begin_instruction().unwrap());
            let second = read_opcode(
                &mut sequenced,
                &mut sequenced_bus,
                Z80MicroStep::BusRequest(first),
            );
            sequenced = roundtrip(sequenced);
            let io_request = request(read_opcode(&mut sequenced, &mut sequenced_bus, second));
            assert_eq!(io_request.address, 0x1234);
            assert_eq!(io_request.base_tstates, 4);

            let retired = match io_request.operation {
                Z80BusOperation::PortRead => {
                    let value = sequenced_bus.read_port(io_request.address);
                    sequenced
                        .complete_bus_cycle(io_request.id, Some(value))
                        .unwrap()
                }
                Z80BusOperation::PortWrite { value } => {
                    sequenced_bus.write_port(io_request.address, value);
                    sequenced.complete_bus_cycle(io_request.id, None).unwrap()
                }
                other => panic!("unexpected ED port operation: {other:?}"),
            };
            assert_eq!(
                retired,
                Z80MicroStep::InstructionRetired {
                    opcode,
                    tstates: 12
                }
            );
            assert_eq!(
                sequenced.export_architectural_state(),
                atomic.export_architectural_state(),
                "architectural mismatch for ED {opcode:02X}"
            );
            assert_eq!(sequenced_bus.reads, atomic_bus.reads);
            assert_eq!(sequenced_bus.writes, atomic_bus.writes);
            assert_eq!(sequenced_bus.port_reads, atomic_bus.port_reads);
            assert_eq!(sequenced_bus.port_writes, atomic_bus.port_writes);
            assert_eq!(sequenced.pending_request(), None);
        }
    }

    #[test]
    fn ed_port_wait_states_and_pending_read_write_snapshots_are_preserved() {
        let mut read_cpu = Z80MicroSequencer::default();
        read_cpu.import_architectural_state(state()).unwrap();
        let mut read_bus = TestBus::default();
        read_bus.memory[0xFFFE] = 0xED;
        read_bus.memory[0xFFFF] = 0x40; // IN B,(C)

        let first = request(read_cpu.begin_instruction().unwrap());
        let second = read_opcode(
            &mut read_cpu,
            &mut read_bus,
            Z80MicroStep::BusRequest(first),
        );
        read_cpu.insert_wait_tstates(2).unwrap();
        let read = request(read_opcode(&mut read_cpu, &mut read_bus, second));
        assert_eq!(read.operation, Z80BusOperation::PortRead);
        read_cpu.insert_wait_tstates(3).unwrap();
        let encoded = read_cpu.export_state().encode().unwrap();
        let decoded = crate::Z80MicroSequencerState::decode(&encoded).unwrap();
        let mut restored_read = Z80MicroSequencer::restore_state(decoded).unwrap();
        let value = read_bus.read_port(read.address);
        assert_eq!(
            restored_read
                .complete_bus_cycle(read.id, Some(value))
                .unwrap(),
            Z80MicroStep::InstructionRetired {
                opcode: 0x40,
                tstates: 17
            }
        );
        assert_eq!(restored_read.registers.b, value);

        let mut write_cpu = Z80MicroSequencer::default();
        write_cpu.import_architectural_state(state()).unwrap();
        let mut write_bus = TestBus::default();
        write_bus.memory[0xFFFE] = 0xED;
        write_bus.memory[0xFFFF] = 0x79; // OUT (C),A

        let first = request(write_cpu.begin_instruction().unwrap());
        let second = read_opcode(
            &mut write_cpu,
            &mut write_bus,
            Z80MicroStep::BusRequest(first),
        );
        let write = request(read_opcode(&mut write_cpu, &mut write_bus, second));
        assert_eq!(write.operation, Z80BusOperation::PortWrite { value: 0xA5 });
        let encoded = write_cpu.export_state().encode().unwrap();
        let decoded = crate::Z80MicroSequencerState::decode(&encoded).unwrap();
        let mut restored_write = Z80MicroSequencer::restore_state(decoded).unwrap();
        write_bus.write_port(write.address, 0xA5);
        assert_eq!(
            restored_write.complete_bus_cycle(write.id, None).unwrap(),
            Z80MicroStep::InstructionRetired {
                opcode: 0x79,
                tstates: 12
            }
        );
    }
}

#[cfg(test)]
mod ed_register_control_differential_tests {
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
        assert_eq!(request.operation, Z80BusOperation::OpcodeFetch);
        let value = bus.read_memory(request.address);
        cpu.complete_bus_cycle(request.id, Some(value)).unwrap()
    }

    fn state(accumulator: u8, iff2: bool) -> Z80ArchitecturalState {
        Z80ArchitecturalState {
            registers: Z80Registers {
                a: accumulator,
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
            alternate: crate::Z80AlternateRegisters::default(),
            ix: 0x1357,
            iy: 0x2468,
            i: 0xA5,
            r: 0xFE,
            iff1: false,
            iff2,
            interrupt_mode: 1,
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
    fn ed_neg_im_and_i_r_register_operations_match_atomic_executor() {
        let opcodes = [
            0x44, 0x4C, 0x54, 0x5C, 0x64, 0x6C, 0x74, 0x7C, // NEG aliases
            0x46, 0x4E, 0x66, 0x6E, // IM 0 aliases
            0x56, 0x76, // IM 1 aliases
            0x5E, 0x7E, // IM 2 aliases
            0x47, 0x4F, 0x57, 0x5F, // LD I/R,A and LD A,I/R
        ];

        for opcode in opcodes {
            for accumulator in [0x00, 0x01, 0x7F, 0x80, 0xFF] {
                for iff2 in [false, true] {
                    let architectural = state(accumulator, iff2);
                    let mut atomic = Z80::default();
                    atomic.import_architectural_state(architectural).unwrap();
                    let mut atomic_bus = TestBus::default();
                    atomic_bus.memory[0xFFFE] = 0xED;
                    atomic_bus.memory[0xFFFF] = opcode;

                    let mut sequenced = Z80MicroSequencer::default();
                    sequenced.import_architectural_state(architectural).unwrap();
                    let mut sequenced_bus = atomic_bus.clone();

                    let expected_tstates = if matches!(opcode, 0x47 | 0x4F | 0x57 | 0x5F) {
                        9
                    } else {
                        8
                    };
                    assert_eq!(
                        atomic.step(&mut atomic_bus),
                        Ok(expected_tstates),
                        "atomic ED {opcode:02X}, A={accumulator:02X}, IFF2={iff2}"
                    );

                    let first = request(sequenced.begin_instruction().unwrap());
                    assert_eq!(first.address, 0xFFFE);
                    let second = read_request(
                        &mut sequenced,
                        &mut sequenced_bus,
                        Z80MicroStep::BusRequest(first),
                    );
                    sequenced = roundtrip(sequenced);
                    let result = read_request(&mut sequenced, &mut sequenced_bus, second);
                    let retired = if expected_tstates == 9 {
                        assert_eq!(result, Z80MicroStep::InternalCycle { tstates: 1 });
                        sequenced = roundtrip(sequenced);
                        sequenced.complete_internal_cycle().unwrap()
                    } else {
                        result
                    };

                    assert_eq!(
                        retired,
                        Z80MicroStep::InstructionRetired {
                            opcode,
                            tstates: expected_tstates
                        },
                        "sequenced ED {opcode:02X}, A={accumulator:02X}, IFF2={iff2}"
                    );
                    assert_eq!(
                        sequenced.export_architectural_state(),
                        atomic.export_architectural_state(),
                        "architectural mismatch for ED {opcode:02X}, A={accumulator:02X}, IFF2={iff2}"
                    );
                    assert_eq!(
                        sequenced_bus.reads, atomic_bus.reads,
                        "read trace mismatch for ED {opcode:02X}"
                    );
                    assert_eq!(
                        sequenced_bus.writes, atomic_bus.writes,
                        "write trace mismatch for ED {opcode:02X}"
                    );
                    assert_eq!(sequenced.pending_request(), None);
                }
            }
        }
    }

    #[test]
    fn ed_ld_i_r_wait_states_are_counted_and_internal_phase_survives_snapshot() {
        let architectural = state(0x5A, true);
        let mut cpu = Z80MicroSequencer::default();
        cpu.import_architectural_state(architectural).unwrap();
        let mut bus = TestBus::default();
        bus.memory[0xFFFE] = 0xED;
        bus.memory[0xFFFF] = 0x47; // LD I,A

        let first = request(cpu.begin_instruction().unwrap());
        cpu.insert_wait_tstates(2).unwrap();
        let second = request(read_request(
            &mut cpu,
            &mut bus,
            Z80MicroStep::BusRequest(first),
        ));
        cpu.insert_wait_tstates(3).unwrap();
        assert_eq!(
            read_request(&mut cpu, &mut bus, Z80MicroStep::BusRequest(second),),
            Z80MicroStep::InternalCycle { tstates: 1 }
        );
        assert_eq!(cpu.export_state().instruction_tstates, 13);
        cpu = roundtrip(cpu);
        assert_eq!(
            cpu.complete_internal_cycle().unwrap(),
            Z80MicroStep::InstructionRetired {
                opcode: 0x47,
                tstates: 14
            }
        );
        assert_eq!(cpu.i, 0x5A);
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

#[cfg(test)]
mod ed_absolute_register_pair_differential_tests {
    use super::{Z80MicroSequencer, Z80MicroStep};
    use crate::bus::CpuBus;
    use crate::{Z80, Z80AlternateRegisters, Z80ArchitecturalState, Z80BusOperation, Z80Registers};

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

    fn architectural_state() -> Z80ArchitecturalState {
        Z80ArchitecturalState {
            registers: Z80Registers {
                a: 0xA5,
                f: 0xD7,
                b: 0x12,
                c: 0x34,
                d: 0x56,
                e: 0x78,
                h: 0x9A,
                l: 0xBC,
                pc: 0xFFFE,
                sp: 0xDEF0,
            },
            alternate: Z80AlternateRegisters::default(),
            ix: 0x1357,
            iy: 0x2468,
            i: 0xA5,
            r: 0xFE,
            iff1: false,
            iff2: true,
            interrupt_mode: 1,
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
        let encoded = cpu.export_state().encode().unwrap();
        let decoded = crate::Z80MicroSequencerState::decode(&encoded).unwrap();
        Z80MicroSequencer::restore_state(decoded).unwrap()
    }

    fn run_sequenced(
        mut cpu: Z80MicroSequencer,
        mut bus: TestBus,
        add_waits: bool,
    ) -> (Z80MicroSequencer, TestBus, u32) {
        let mut step = cpu.begin_instruction().unwrap();
        loop {
            match step {
                Z80MicroStep::BusRequest(request) => {
                    if add_waits {
                        cpu.insert_wait_tstate().unwrap();
                    }
                    let data = match request.operation {
                        Z80BusOperation::OpcodeFetch | Z80BusOperation::MemoryRead => {
                            Some(bus.read_memory(request.address))
                        }
                        Z80BusOperation::MemoryWrite { value } => {
                            bus.write_memory(request.address, value);
                            None
                        }
                        other => panic!("unexpected ED absolute transfer operation: {other:?}"),
                    };
                    step = cpu.complete_bus_cycle(request.id, data).unwrap();
                    cpu = roundtrip(cpu);
                }
                Z80MicroStep::InstructionRetired { opcode: _, tstates } => {
                    return (cpu, bus, tstates);
                }
                Z80MicroStep::InternalCycle { .. } => {
                    panic!("ED absolute register-pair transfers have no internal T-states")
                }
            }
        }
    }

    fn run_case(opcode: u8, address: u16, add_waits: bool) {
        let initial = architectural_state();
        let mut atomic = Z80::default();
        atomic.import_architectural_state(initial).unwrap();
        let mut atomic_bus = TestBus::default();
        atomic_bus.memory[0xFFFE] = 0xED;
        atomic_bus.memory[0xFFFF] = opcode;
        atomic_bus.memory[0x0000] = address as u8;
        atomic_bus.memory[0x0001] = (address >> 8) as u8;
        if !matches!(address, 0x0000 | 0x0001 | 0xFFFF) {
            atomic_bus.memory[usize::from(address)] = 0xC3;
        }
        let next_address = address.wrapping_add(1);
        if !matches!(next_address, 0x0000 | 0x0001 | 0xFFFF) {
            atomic_bus.memory[usize::from(next_address)] = 0x5A;
        }
        let sequenced_bus = atomic_bus.clone();

        assert_eq!(atomic.step(&mut atomic_bus), Ok(20), "ED {opcode:02X}");
        let mut sequenced = Z80MicroSequencer::default();
        sequenced.import_architectural_state(initial).unwrap();
        let (sequenced, sequenced_bus, tstates) =
            run_sequenced(sequenced, sequenced_bus, add_waits);

        assert_eq!(tstates, 20 + if add_waits { 6 } else { 0 });
        assert_eq!(
            sequenced.export_architectural_state(),
            atomic.export_architectural_state(),
            "architectural mismatch for ED {opcode:02X} at {address:04X}"
        );
        assert_eq!(sequenced_bus.reads, atomic_bus.reads);
        assert_eq!(sequenced_bus.writes, atomic_bus.writes);
        assert_eq!(sequenced_bus.memory, atomic_bus.memory);
        assert_eq!(sequenced.pending_request(), None);
    }

    #[test]
    fn all_eight_ed_absolute_pair_encodings_match_atomic_state_and_bus_order() {
        for opcode in [0x43, 0x53, 0x63, 0x73, 0x4B, 0x5B, 0x6B, 0x7B] {
            run_case(opcode, 0x4000, false);
        }
    }

    #[test]
    fn absolute_pair_transfer_wraps_data_address_at_ffff() {
        for opcode in [0x43, 0x4B, 0x73, 0x7B] {
            run_case(opcode, 0xFFFF, false);
        }
    }

    #[test]
    fn waits_and_snapshot_restore_preserve_all_six_machine_cycles() {
        for opcode in [0x43, 0x53, 0x63, 0x73, 0x4B, 0x5B, 0x6B, 0x7B] {
            run_case(opcode, 0x7FFE, true);
        }
    }
}

#[cfg(test)]
mod ed_return_nibble_rotate_differential_tests {
    use super::{Z80MicroSequencer, Z80MicroStep};
    use crate::bus::CpuBus;
    use crate::{
        Z80, Z80AlternateRegisters, Z80ArchitecturalState, Z80BusOperation, Z80MicroSequencerState,
        Z80Registers,
    };

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

    fn initial_state() -> Z80ArchitecturalState {
        Z80ArchitecturalState {
            registers: Z80Registers {
                a: 0x12,
                f: 0xD7,
                b: 0x43,
                c: 0x65,
                d: 0x87,
                e: 0xA9,
                h: 0xC0,
                l: 0x00,
                pc: 0x1000,
                sp: 0xD000,
            },
            alternate: Z80AlternateRegisters::default(),
            ix: 0x1357,
            iy: 0x2468,
            i: 0x81,
            r: 0xFE,
            iff1: false,
            iff2: true,
            interrupt_mode: 1,
            wz: 0xABCD,
            ei_delay: 0,
            nmi_pending: false,
            int_pending: None,
            q: 0,
            q_previous: 0,
            p: false,
            halted: false,
        }
    }

    fn roundtrip(cpu: Z80MicroSequencer) -> Z80MicroSequencer {
        let encoded = cpu.export_state().encode().unwrap();
        let decoded = Z80MicroSequencerState::decode(&encoded).unwrap();
        Z80MicroSequencer::restore_state(decoded).unwrap()
    }

    fn run_sequenced(
        mut cpu: Z80MicroSequencer,
        mut bus: TestBus,
        add_waits: bool,
    ) -> (Z80MicroSequencer, TestBus, u32) {
        let mut step = cpu.begin_instruction().unwrap();
        loop {
            match step {
                Z80MicroStep::BusRequest(request) => {
                    if add_waits {
                        cpu.insert_wait_tstate().unwrap();
                    }
                    let data = match request.operation {
                        Z80BusOperation::OpcodeFetch | Z80BusOperation::MemoryRead => {
                            Some(bus.read_memory(request.address))
                        }
                        Z80BusOperation::MemoryWrite { value } => {
                            bus.write_memory(request.address, value);
                            None
                        }
                        other => panic!("unexpected ED-5 bus operation: {other:?}"),
                    };
                    step = cpu.complete_bus_cycle(request.id, data).unwrap();
                    cpu = roundtrip(cpu);
                }
                Z80MicroStep::InternalCycle { tstates } => {
                    assert_eq!(
                        tstates, 1,
                        "internal cycles are advanced one T-state at a time"
                    );
                    step = cpu.complete_internal_cycle().unwrap();
                    cpu = roundtrip(cpu);
                }
                Z80MicroStep::InstructionRetired { tstates, .. } => {
                    return (cpu, bus, tstates);
                }
            }
        }
    }

    fn run_case(opcode: u8, add_waits: bool) {
        let initial = initial_state();
        let mut atomic = Z80::default();
        atomic.import_architectural_state(initial).unwrap();
        let mut atomic_bus = TestBus::default();
        atomic_bus.memory[0x1000] = 0xED;
        atomic_bus.memory[0x1001] = opcode;
        atomic_bus.memory[0xD000] = 0x34;
        atomic_bus.memory[0xD001] = 0x12;
        atomic_bus.memory[0xC000] = 0x34;
        let sequenced_bus = atomic_bus.clone();

        let expected = if matches!(opcode, 0x67 | 0x6F) {
            18
        } else {
            14
        };
        assert_eq!(
            atomic.step(&mut atomic_bus),
            Ok(expected),
            "ED {opcode:02X}"
        );
        let mut sequenced = Z80MicroSequencer::default();
        sequenced.import_architectural_state(initial).unwrap();
        let (sequenced, sequenced_bus, tstates) =
            run_sequenced(sequenced, sequenced_bus, add_waits);

        assert_eq!(tstates, expected + if add_waits { 4 } else { 0 });
        assert_eq!(
            sequenced.export_architectural_state(),
            atomic.export_architectural_state(),
            "architectural mismatch for ED {opcode:02X}"
        );
        assert_eq!(sequenced_bus.reads, atomic_bus.reads);
        assert_eq!(sequenced_bus.writes, atomic_bus.writes);
        assert_eq!(sequenced_bus.memory, atomic_bus.memory);
        assert_eq!(sequenced.pending_request(), None);
    }

    #[test]
    fn all_retn_aliases_and_reti_match_atomic_return_and_stack_order() {
        for opcode in [0x45, 0x4D, 0x55, 0x5D, 0x65, 0x6D, 0x75, 0x7D] {
            run_case(opcode, false);
        }
    }

    #[test]
    fn rrd_and_rld_match_atomic_nibble_rotation_and_flags() {
        run_case(0x67, false);
        run_case(0x6F, false);
    }

    #[test]
    fn ed5_waits_and_snapshot_restore_preserve_return_and_rmw_cycles() {
        for opcode in [0x45, 0x4D, 0x55, 0x5D, 0x65, 0x6D, 0x75, 0x7D, 0x67, 0x6F] {
            run_case(opcode, true);
        }
    }

    #[test]
    fn retn_and_reti_restore_iff1_from_iff2() {
        let mut retn = Z80MicroSequencer::default();
        let mut state = initial_state();
        state.registers.pc = 0x1000;
        state.registers.sp = 0xD000;
        retn.import_architectural_state(state).unwrap();
        let mut bus = TestBus::default();
        bus.memory[0x1000] = 0xED;
        bus.memory[0x1001] = 0x45;
        bus.memory[0xD000] = 0x78;
        bus.memory[0xD001] = 0x56;
        let (retn, _, _) = run_sequenced(retn, bus, false);
        assert!(retn.iff1);

        let mut reti = Z80MicroSequencer::default();
        reti.import_architectural_state(state).unwrap();
        let mut bus = TestBus::default();
        bus.memory[0x1000] = 0xED;
        bus.memory[0x1001] = 0x4D;
        bus.memory[0xD000] = 0x78;
        bus.memory[0xD001] = 0x56;
        let (reti, _, _) = run_sequenced(reti, bus, false);
        assert!(reti.iff1);
        assert!(reti.iff2);
    }
}

#[cfg(test)]
mod ed_block_transfer_differential_tests {
    use super::{Z80MicroSequencer, Z80MicroStep};
    use crate::bus::CpuBus;
    use crate::{
        Z80, Z80AlternateRegisters, Z80ArchitecturalState, Z80BusOperation, Z80MicroSequencerState,
        Z80Registers,
    };

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

    fn initial_state() -> Z80ArchitecturalState {
        Z80ArchitecturalState {
            registers: Z80Registers {
                a: 0x12,
                f: 0xD7,
                b: 0x00,
                c: 0x02,
                d: 0xC1,
                e: 0x00,
                h: 0xC0,
                l: 0x00,
                pc: 0x1000,
                sp: 0xD000,
            },
            alternate: Z80AlternateRegisters::default(),
            ix: 0x1357,
            iy: 0x2468,
            i: 0x81,
            r: 0xFE,
            iff1: false,
            iff2: true,
            interrupt_mode: 1,
            wz: 0xABCD,
            ei_delay: 0,
            nmi_pending: false,
            int_pending: None,
            q: 0,
            q_previous: 0,
            p: false,
            halted: false,
        }
    }

    fn roundtrip(cpu: Z80MicroSequencer) -> Z80MicroSequencer {
        let encoded = cpu.export_state().encode().unwrap();
        let decoded = Z80MicroSequencerState::decode(&encoded).unwrap();
        Z80MicroSequencer::restore_state(decoded).unwrap()
    }

    fn run_sequenced(
        mut cpu: Z80MicroSequencer,
        mut bus: TestBus,
        add_waits: bool,
    ) -> (
        Z80MicroSequencer,
        TestBus,
        u32,
        Vec<(Z80BusOperation, u16, u8)>,
    ) {
        let mut step = cpu.begin_instruction().unwrap();
        let mut cycles = Vec::new();
        loop {
            match step {
                Z80MicroStep::BusRequest(request) => {
                    if add_waits {
                        cpu.insert_wait_tstate().unwrap();
                    }
                    cycles.push((request.operation, request.address, request.base_tstates));
                    let data = match request.operation {
                        Z80BusOperation::OpcodeFetch | Z80BusOperation::MemoryRead => {
                            Some(bus.read_memory(request.address))
                        }
                        Z80BusOperation::MemoryWrite { value } => {
                            bus.write_memory(request.address, value);
                            None
                        }
                        other => panic!("unexpected ED block transfer operation: {other:?}"),
                    };
                    step = cpu.complete_bus_cycle(request.id, data).unwrap();
                    cpu = roundtrip(cpu);
                }
                Z80MicroStep::InternalCycle { tstates } => {
                    assert_eq!(tstates, 1);
                    step = cpu.complete_internal_cycle().unwrap();
                    cpu = roundtrip(cpu);
                }
                Z80MicroStep::InstructionRetired { tstates, .. } => {
                    return (cpu, bus, tstates, cycles);
                }
            }
        }
    }

    fn run_case(opcode: u8, initial_count: u16, add_waits: bool) {
        let mut initial = initial_state();
        initial.registers.set_bc(initial_count);
        let mut atomic = Z80::default();
        atomic.import_architectural_state(initial).unwrap();
        let mut atomic_bus = TestBus::default();
        atomic_bus.memory[0x1000] = 0xED;
        atomic_bus.memory[0x1001] = opcode;
        atomic_bus.memory[0xC000] = 0x5A;
        atomic_bus.memory[0xC001] = 0xA5;
        let sequenced_bus = atomic_bus.clone();

        let repeats = matches!(opcode, 0xB0 | 0xB8) && initial_count.wrapping_sub(1) != 0;
        let expected_tstates = if repeats { 21 } else { 16 };
        assert_eq!(
            atomic.step(&mut atomic_bus),
            Ok(expected_tstates),
            "ED {opcode:02X}, BC={initial_count:04X}"
        );

        let mut sequenced = Z80MicroSequencer::default();
        sequenced.import_architectural_state(initial).unwrap();
        let (sequenced, sequenced_bus, tstates, cycles) =
            run_sequenced(sequenced, sequenced_bus, add_waits);

        assert_eq!(
            tstates,
            expected_tstates + if add_waits { 4 } else { 0 },
            "ED {opcode:02X}, BC={initial_count:04X}"
        );
        assert_eq!(
            cycles,
            vec![
                (Z80BusOperation::OpcodeFetch, 0x1000, 4),
                (Z80BusOperation::OpcodeFetch, 0x1001, 4),
                (Z80BusOperation::MemoryRead, 0xC000, 3),
                (Z80BusOperation::MemoryWrite { value: 0x5A }, 0xC100, 5),
            ],
            "ED {opcode:02X} M-cycle order"
        );
        assert_eq!(
            sequenced.export_architectural_state(),
            atomic.export_architectural_state(),
            "architectural mismatch for ED {opcode:02X}, BC={initial_count:04X}"
        );
        assert_eq!(sequenced_bus.reads, atomic_bus.reads);
        assert_eq!(sequenced_bus.writes, atomic_bus.writes);
        assert_eq!(sequenced_bus.memory, atomic_bus.memory);
        assert_eq!(sequenced.pending_request(), None);
    }

    #[test]
    fn ldi_ldd_and_repeat_forms_match_atomic_state_and_bus_order() {
        for opcode in [0xA0, 0xA8, 0xB0, 0xB8] {
            run_case(opcode, 1, false);
            run_case(opcode, 2, false);
        }
    }

    #[test]
    fn ldir_and_lddr_repeat_when_count_wraps_from_zero() {
        run_case(0xB0, 0, false);
        run_case(0xB8, 0, false);
    }

    #[test]
    fn waits_and_snapshot_restore_preserve_all_transfer_phases() {
        for opcode in [0xA0, 0xA8, 0xB0, 0xB8] {
            run_case(opcode, 1, true);
            run_case(opcode, 2, true);
        }
    }

    #[test]
    fn transfer_source_and_destination_addresses_wrap_at_ffff() {
        let mut initial = initial_state();
        initial.registers.set_hl(0xFFFF);
        initial.registers.set_de(0x0000);
        initial.registers.set_bc(1);
        let mut atomic = Z80::default();
        atomic.import_architectural_state(initial).unwrap();
        let mut atomic_bus = TestBus::default();
        atomic_bus.memory[0x1000] = 0xED;
        atomic_bus.memory[0x1001] = 0xA0;
        atomic_bus.memory[0xFFFF] = 0xC3;
        let sequenced_bus = atomic_bus.clone();

        assert_eq!(atomic.step(&mut atomic_bus), Ok(16));
        let mut sequenced = Z80MicroSequencer::default();
        sequenced.import_architectural_state(initial).unwrap();
        let (sequenced, sequenced_bus, tstates, _) = run_sequenced(sequenced, sequenced_bus, false);
        assert_eq!(tstates, 16);
        assert_eq!(
            sequenced.export_architectural_state(),
            atomic.export_architectural_state()
        );
        assert_eq!(sequenced_bus.memory, atomic_bus.memory);
        assert_eq!(sequenced_bus.reads, atomic_bus.reads);
        assert_eq!(sequenced_bus.writes, atomic_bus.writes);
        assert_eq!(sequenced.registers.hl(), 0x0000);
        assert_eq!(sequenced.registers.de(), 0x0001);
    }
}

#[cfg(test)]
mod ed_block_compare_differential_tests {
    use super::{Z80MicroSequencer, Z80MicroStep};
    use crate::bus::CpuBus;
    use crate::{
        Z80, Z80AlternateRegisters, Z80ArchitecturalState, Z80BusOperation, Z80MicroSequencerState,
        Z80Registers,
    };

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

    fn initial_state() -> Z80ArchitecturalState {
        Z80ArchitecturalState {
            registers: Z80Registers {
                a: 0x12,
                f: 0xD7,
                b: 0x00,
                c: 0x02,
                d: 0xC1,
                e: 0x00,
                h: 0xC0,
                l: 0x00,
                pc: 0x1000,
                sp: 0xD000,
            },
            alternate: Z80AlternateRegisters::default(),
            ix: 0x1357,
            iy: 0x2468,
            i: 0x81,
            r: 0xFE,
            iff1: false,
            iff2: true,
            interrupt_mode: 1,
            wz: 0xABCD,
            ei_delay: 0,
            nmi_pending: false,
            int_pending: None,
            q: 0,
            q_previous: 0,
            p: false,
            halted: false,
        }
    }

    fn roundtrip(cpu: Z80MicroSequencer) -> Z80MicroSequencer {
        let encoded = cpu.export_state().encode().unwrap();
        let decoded = Z80MicroSequencerState::decode(&encoded).unwrap();
        Z80MicroSequencer::restore_state(decoded).unwrap()
    }

    fn run_sequenced(
        mut cpu: Z80MicroSequencer,
        mut bus: TestBus,
        add_waits: bool,
    ) -> (
        Z80MicroSequencer,
        TestBus,
        u32,
        Vec<(Z80BusOperation, u16, u8)>,
    ) {
        let mut step = cpu.begin_instruction().unwrap();
        let mut cycles = Vec::new();
        loop {
            match step {
                Z80MicroStep::BusRequest(request) => {
                    if add_waits {
                        cpu.insert_wait_tstate().unwrap();
                    }
                    cycles.push((request.operation, request.address, request.base_tstates));
                    let data = match request.operation {
                        Z80BusOperation::OpcodeFetch | Z80BusOperation::MemoryRead => {
                            Some(bus.read_memory(request.address))
                        }
                        Z80BusOperation::MemoryWrite { value } => {
                            bus.write_memory(request.address, value);
                            None
                        }
                        other => panic!("unexpected ED block compare operation: {other:?}"),
                    };
                    step = cpu.complete_bus_cycle(request.id, data).unwrap();
                    cpu = roundtrip(cpu);
                }
                Z80MicroStep::InternalCycle { tstates } => {
                    assert_eq!(tstates, 1);
                    step = cpu.complete_internal_cycle().unwrap();
                    cpu = roundtrip(cpu);
                }
                Z80MicroStep::InstructionRetired { tstates, .. } => {
                    return (cpu, bus, tstates, cycles);
                }
            }
        }
    }

    fn run_case(
        opcode: u8,
        initial_count: u16,
        accumulator: u8,
        memory_value: u8,
        add_waits: bool,
    ) {
        let mut initial = initial_state();
        initial.registers.a = accumulator;
        initial.registers.set_bc(initial_count);
        let mut atomic = Z80::default();
        atomic.import_architectural_state(initial).unwrap();
        let mut atomic_bus = TestBus::default();
        atomic_bus.memory[0x1000] = 0xED;
        atomic_bus.memory[0x1001] = opcode;
        atomic_bus.memory[0xC000] = memory_value;
        let sequenced_bus = atomic_bus.clone();

        let result = accumulator.wrapping_sub(memory_value);
        let repeats =
            matches!(opcode, 0xB1 | 0xB9) && initial_count.wrapping_sub(1) != 0 && result != 0;
        let expected_tstates = if repeats { 21 } else { 16 };
        assert_eq!(
            atomic.step(&mut atomic_bus),
            Ok(expected_tstates),
            "ED {opcode:02X}, BC={initial_count:04X}, A={accumulator:02X}, value={memory_value:02X}"
        );

        let mut sequenced = Z80MicroSequencer::default();
        sequenced.import_architectural_state(initial).unwrap();
        let (sequenced, sequenced_bus, tstates, cycles) =
            run_sequenced(sequenced, sequenced_bus, add_waits);

        assert_eq!(
            tstates,
            expected_tstates + if add_waits { 3 } else { 0 },
            "ED {opcode:02X}, BC={initial_count:04X}"
        );
        assert_eq!(
            cycles,
            vec![
                (Z80BusOperation::OpcodeFetch, 0x1000, 4),
                (Z80BusOperation::OpcodeFetch, 0x1001, 4),
                (Z80BusOperation::MemoryRead, 0xC000, 3),
            ],
            "ED {opcode:02X} M-cycle order"
        );
        assert_eq!(
            sequenced.export_architectural_state(),
            atomic.export_architectural_state(),
            "architectural mismatch for ED {opcode:02X}, BC={initial_count:04X}"
        );
        assert_eq!(sequenced_bus.reads, atomic_bus.reads);
        assert_eq!(sequenced_bus.writes, atomic_bus.writes);
        assert_eq!(sequenced_bus.memory, atomic_bus.memory);
        assert_eq!(sequenced.pending_request(), None);
    }

    #[test]
    fn cpi_cpd_and_repeat_forms_match_atomic_flags_and_state() {
        for opcode in [0xA1, 0xA9, 0xB1, 0xB9] {
            run_case(opcode, 1, 0x12, 0x34, false);
            run_case(opcode, 2, 0x12, 0x34, false);
            run_case(opcode, 2, 0x34, 0x34, false);
        }
    }

    #[test]
    fn cpir_and_cpdr_repeat_when_count_wraps_from_zero_and_no_match() {
        run_case(0xB1, 0, 0x12, 0x34, false);
        run_case(0xB9, 0, 0x12, 0x34, false);
    }

    #[test]
    fn waits_and_snapshot_restore_preserve_compare_internal_cycles() {
        for opcode in [0xA1, 0xA9, 0xB1, 0xB9] {
            run_case(opcode, 1, 0x12, 0x34, true);
            run_case(opcode, 2, 0x12, 0x34, true);
            run_case(opcode, 2, 0x34, 0x34, true);
        }
    }

    #[test]
    fn compare_address_wraps_at_ffff_in_both_directions() {
        for (opcode, expected_hl) in [(0xA1, 0x0000), (0xA9, 0xFFFE)] {
            let mut initial = initial_state();
            initial.registers.set_hl(0xFFFF);
            initial.registers.set_bc(1);
            let mut atomic = Z80::default();
            atomic.import_architectural_state(initial).unwrap();
            let mut atomic_bus = TestBus::default();
            atomic_bus.memory[0x1000] = 0xED;
            atomic_bus.memory[0x1001] = opcode;
            atomic_bus.memory[0xFFFF] = 0x34;
            let sequenced_bus = atomic_bus.clone();

            assert_eq!(atomic.step(&mut atomic_bus), Ok(16));
            let mut sequenced = Z80MicroSequencer::default();
            sequenced.import_architectural_state(initial).unwrap();
            let (sequenced, sequenced_bus, tstates, _) =
                run_sequenced(sequenced, sequenced_bus, false);
            assert_eq!(tstates, 16);
            assert_eq!(
                sequenced.export_architectural_state(),
                atomic.export_architectural_state()
            );
            assert_eq!(sequenced_bus.reads, atomic_bus.reads);
            assert_eq!(sequenced.registers.hl(), expected_hl);
        }
    }
}

#[cfg(test)]
mod ed_block_io_differential_tests {
    use crate::bus::CpuBus;
    use crate::{
        Z80, Z80AlternateRegisters, Z80ArchitecturalState, Z80BusOperation, Z80MicroSequencer,
        Z80MicroSequencerState, Z80MicroStep, Z80Registers,
    };

    struct NormalFlagsVector {
        opcode: u8,
        b: u8,
        c: u8,
        hl: u16,
        value: u8,
        port: u16,
        next_hl: u16,
        wz: u16,
        flags: u8,
    }

    struct RepeatFlagsVector {
        opcode: u8,
        c: u8,
        hl: u16,
        port: u16,
        input: bool,
        value: u8,
        flags: u8,
    }

    #[derive(Clone)]
    struct TestBus {
        memory: Box<[u8; 0x1_0000]>,
        ports: Box<[u8; 0x1_0000]>,
        reads: Vec<u16>,
        writes: Vec<(u16, u8)>,
        port_reads: Vec<u16>,
        port_writes: Vec<(u16, u8)>,
    }

    impl Default for TestBus {
        fn default() -> Self {
            let mut bus = Self {
                memory: Box::new([0; 0x1_0000]),
                ports: Box::new([0; 0x1_0000]),
                reads: Vec::new(),
                writes: Vec::new(),
                port_reads: Vec::new(),
                port_writes: Vec::new(),
            };
            for (address, (memory, port)) in
                bus.memory.iter_mut().zip(bus.ports.iter_mut()).enumerate()
            {
                *memory = (address as u8).wrapping_mul(13).wrapping_add(0xA5);
                *port = (address as u8)
                    .wrapping_mul(37)
                    .wrapping_add((address >> 8) as u8)
                    .wrapping_add(0x81);
            }
            bus
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

        fn read_port(&mut self, address: u16) -> u8 {
            self.port_reads.push(address);
            self.ports[usize::from(address)]
        }

        fn write_port(&mut self, address: u16, value: u8) {
            self.port_writes.push((address, value));
            self.ports[usize::from(address)] = value;
        }
    }

    fn architectural_state(opcode: u8, initial_b: u8) -> Z80ArchitecturalState {
        let decrement = matches!(opcode, 0xAA | 0xBA | 0xAB | 0xBB);
        Z80ArchitecturalState {
            registers: Z80Registers {
                a: 0x5A,
                f: 0xD7,
                b: initial_b,
                c: if matches!(opcode, 0xA2 | 0xB2) {
                    0xFF
                } else {
                    0x00
                },
                d: 0x12,
                e: 0x34,
                h: if decrement { 0x00 } else { 0xFF },
                l: if decrement { 0x00 } else { 0x80 },
                pc: 0x2000,
                sp: 0xD000,
            },
            alternate: Z80AlternateRegisters::default(),
            ix: 0x1357,
            iy: 0x2468,
            i: 0xA5,
            r: 0xFE,
            iff1: false,
            iff2: true,
            interrupt_mode: 1,
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

    fn state_with_registers(registers: Z80Registers, wz: u16) -> Z80ArchitecturalState {
        Z80ArchitecturalState {
            registers,
            alternate: Z80AlternateRegisters::default(),
            ix: 0x1357,
            iy: 0x2468,
            i: 0xA5,
            r: 0xFE,
            iff1: false,
            iff2: true,
            interrupt_mode: 1,
            wz,
            ei_delay: 0,
            nmi_pending: false,
            int_pending: None,
            q: 0x28,
            q_previous: 0x5A,
            p: false,
            halted: false,
        }
    }

    fn round_trip(cpu: &mut Z80MicroSequencer) {
        let state = cpu.export_state();
        let encoded = state.encode().expect("valid ED block-I/O snapshot");
        let decoded =
            Z80MicroSequencerState::decode(&encoded).expect("ED block-I/O snapshot decodes");
        assert_eq!(decoded, state);
        *cpu = Z80MicroSequencer::restore_state(decoded).expect("ED block-I/O snapshot restores");
    }

    fn run_sequenced_iteration(
        cpu: &mut Z80MicroSequencer,
        bus: &mut TestBus,
        add_waits: bool,
    ) -> u32 {
        let mut step = cpu.begin_instruction().unwrap();
        round_trip(cpu);
        loop {
            step = match step {
                Z80MicroStep::BusRequest(request) => {
                    if add_waits {
                        cpu.insert_wait_tstates(2).unwrap();
                        round_trip(cpu);
                    }
                    let data = match request.operation {
                        Z80BusOperation::OpcodeFetch | Z80BusOperation::MemoryRead => {
                            Some(bus.read_memory(request.address))
                        }
                        Z80BusOperation::MemoryWrite { value } => {
                            bus.write_memory(request.address, value);
                            None
                        }
                        Z80BusOperation::PortRead => Some(bus.read_port(request.address)),
                        Z80BusOperation::PortWrite { value } => {
                            bus.write_port(request.address, value);
                            None
                        }
                    };
                    let next = cpu.complete_bus_cycle(request.id, data).unwrap();
                    round_trip(cpu);
                    next
                }
                Z80MicroStep::InternalCycle { tstates } => {
                    let state = cpu.export_state();
                    if state.opcode.is_some_and(|opcode| {
                        matches!(
                            opcode,
                            0xA2 | 0xAA | 0xB2 | 0xBA | 0xA3 | 0xAB | 0xB3 | 0xBB
                        )
                    }) {
                        assert_eq!(tstates, 1);
                        match state.phase {
                            super::Z80MicroSequencerPhase::EdBlockIoInitialInternal { .. } => {
                                assert_eq!((state.mcycle_index, state.tstate_index), (1, 4));
                            }
                            super::Z80MicroSequencerPhase::EdBlockIoRepeatInternal { .. } => {
                                assert_eq!(state.mcycle_index, 4);
                            }
                            other => panic!("unexpected ED block-I/O internal phase: {other:?}"),
                        }
                    }
                    round_trip(cpu);
                    let next = cpu.complete_internal_cycle().unwrap();
                    round_trip(cpu);
                    next
                }
                Z80MicroStep::InstructionRetired { tstates, .. } => return tstates,
            };
        }
    }

    fn assert_same_iteration(
        opcode: u8,
        initial_b: u8,
        add_waits: bool,
        expected_iterations: usize,
    ) {
        let state = architectural_state(opcode, initial_b);
        let mut atomic = Z80::default();
        atomic.import_architectural_state(state).unwrap();
        let mut atomic_bus = TestBus::default();
        atomic_bus.memory[0x2000] = 0xED;
        atomic_bus.memory[0x2001] = opcode;

        let mut sequenced = Z80MicroSequencer::default();
        sequenced.import_architectural_state(state).unwrap();
        let mut sequenced_bus = atomic_bus.clone();

        for iteration in 0..expected_iterations {
            let atomic_tstates = atomic.step(&mut atomic_bus).unwrap();
            let sequenced_tstates =
                run_sequenced_iteration(&mut sequenced, &mut sequenced_bus, add_waits);
            let expected_waits = if add_waits { 8 } else { 0 };
            assert_eq!(
                sequenced_tstates,
                atomic_tstates + expected_waits,
                "timing mismatch ED {opcode:02X}, B={initial_b:02X}, iteration={iteration}"
            );
            assert_eq!(
                sequenced.export_architectural_state(),
                atomic.export_architectural_state(),
                "architectural mismatch ED {opcode:02X}, B={initial_b:02X}, iteration={iteration}"
            );
            assert_eq!(sequenced_bus.memory.as_ref(), atomic_bus.memory.as_ref());
            assert_eq!(sequenced_bus.ports.as_ref(), atomic_bus.ports.as_ref());
            assert_eq!(sequenced_bus.reads, atomic_bus.reads);
            assert_eq!(sequenced_bus.writes, atomic_bus.writes);
            assert_eq!(sequenced_bus.port_reads, atomic_bus.port_reads);
            assert_eq!(sequenced_bus.port_writes, atomic_bus.port_writes);
            assert_eq!(sequenced.pending_request(), None);
        }
    }

    #[test]
    fn all_eight_block_io_opcodes_match_atomic_executor_across_counter_wraparound() {
        for opcode in [0xA2, 0xAA, 0xB2, 0xBA, 0xA3, 0xAB, 0xB3, 0xBB] {
            for initial_b in [0, 1, 2] {
                let repeat = matches!(opcode, 0xB2 | 0xBA | 0xB3 | 0xBB);
                let iterations = if repeat {
                    if initial_b == 0 {
                        256
                    } else {
                        usize::from(initial_b)
                    }
                } else {
                    1
                };
                assert_same_iteration(opcode, initial_b, false, iterations);
            }
        }
    }

    #[test]
    fn every_block_io_bus_phase_preserves_waits_and_snapshot_restore() {
        for opcode in [0xA2, 0xAA, 0xB2, 0xBA, 0xA3, 0xAB, 0xB3, 0xBB] {
            assert_same_iteration(opcode, 1, true, 1);
        }
        for opcode in [0xB2, 0xBA, 0xB3, 0xBB] {
            assert_same_iteration(opcode, 2, true, 2);
        }
    }

    #[test]
    fn block_io_normal_flags_ports_and_wz_match_documented_edge_vectors() {
        let cases = [
            NormalFlagsVector {
                opcode: 0xA2,
                b: 0x01,
                c: 0xFF,
                hl: 0x12FF,
                value: 0x80,
                port: 0x01FF,
                next_hl: 0x1300,
                wz: 0x0200,
                flags: 0x46,
            },
            NormalFlagsVector {
                opcode: 0xAA,
                b: 0x01,
                c: 0x00,
                hl: 0x1200,
                value: 0x01,
                port: 0x0100,
                next_hl: 0x11FF,
                wz: 0x00FF,
                flags: 0x55,
            },
            NormalFlagsVector {
                opcode: 0xA3,
                b: 0x01,
                c: 0xFE,
                hl: 0x12FF,
                value: 0x80,
                port: 0x00FE,
                next_hl: 0x1300,
                wz: 0x00FF,
                flags: 0x46,
            },
            NormalFlagsVector {
                opcode: 0xAB,
                b: 0x01,
                c: 0x01,
                hl: 0x1200,
                value: 0xFF,
                port: 0x0001,
                next_hl: 0x11FF,
                wz: 0x0000,
                flags: 0x57,
            },
        ];

        for NormalFlagsVector {
            opcode,
            b,
            c,
            hl,
            value,
            port,
            next_hl,
            wz,
            flags,
        } in cases
        {
            let state = state_with_registers(
                Z80Registers {
                    a: 0x5A,
                    f: 0xD7,
                    b,
                    c,
                    h: (hl >> 8) as u8,
                    l: hl as u8,
                    pc: 0x2000,
                    ..Z80Registers::default()
                },
                0xA328,
            );
            let mut atomic = Z80::default();
            atomic.import_architectural_state(state).unwrap();
            let mut atomic_bus = TestBus::default();
            atomic_bus.memory[0x2000] = 0xED;
            atomic_bus.memory[0x2001] = opcode;
            if matches!(opcode, 0xA2 | 0xAA) {
                atomic_bus.ports[usize::from(port)] = value;
            } else {
                atomic_bus.memory[usize::from(hl)] = value;
            }

            let mut sequenced = Z80MicroSequencer::default();
            sequenced.import_architectural_state(state).unwrap();
            let mut sequenced_bus = atomic_bus.clone();

            assert_eq!(atomic.step(&mut atomic_bus), Ok(16));
            assert_eq!(
                run_sequenced_iteration(&mut sequenced, &mut sequenced_bus, false),
                16
            );
            assert_eq!(sequenced.registers.hl(), next_hl);
            assert_eq!(sequenced.registers.b, 0);
            assert_eq!(sequenced.wz, wz);
            assert_eq!(sequenced.registers.f, flags);
            assert_eq!(
                sequenced.export_architectural_state(),
                atomic.export_architectural_state()
            );
            if matches!(opcode, 0xA2 | 0xAA) {
                assert_eq!(sequenced_bus.port_reads, vec![port]);
                assert_eq!(sequenced_bus.memory[usize::from(hl)], value);
            } else {
                assert_eq!(sequenced_bus.port_writes, vec![(port, value)]);
            }
        }
    }

    #[test]
    fn block_io_repeat_flags_cover_half_carry_and_parity_edges() {
        let cases = [
            RepeatFlagsVector {
                opcode: 0xB2,
                c: 0x01,
                hl: 0x4000,
                port: 0x1101,
                input: true,
                value: 0xFF,
                flags: 0x33,
            },
            RepeatFlagsVector {
                opcode: 0xB3,
                c: 0x01,
                hl: 0x40FE,
                port: 0x1001,
                input: false,
                value: 0xFF,
                flags: 0x37,
            },
            RepeatFlagsVector {
                opcode: 0xBA,
                c: 0x02,
                hl: 0x4000,
                port: 0x1102,
                input: true,
                value: 0xFF,
                flags: 0x37,
            },
            RepeatFlagsVector {
                opcode: 0xBB,
                c: 0x02,
                hl: 0x4101,
                port: 0x1002,
                input: false,
                value: 0xFF,
                flags: 0x26,
            },
            RepeatFlagsVector {
                opcode: 0xB2,
                c: 0xFE,
                hl: 0x4000,
                port: 0x11FE,
                input: true,
                value: 0x01,
                flags: 0x25,
            },
            RepeatFlagsVector {
                opcode: 0xB3,
                c: 0x01,
                hl: 0x40FE,
                port: 0x1001,
                input: false,
                value: 0x01,
                flags: 0x25,
            },
            RepeatFlagsVector {
                opcode: 0xBA,
                c: 0x00,
                hl: 0x4000,
                port: 0x1100,
                input: true,
                value: 0x01,
                flags: 0x25,
            },
            RepeatFlagsVector {
                opcode: 0xBB,
                c: 0x02,
                hl: 0x4100,
                port: 0x1002,
                input: false,
                value: 0x01,
                flags: 0x25,
            },
        ];

        for RepeatFlagsVector {
            opcode,
            c,
            hl,
            port,
            input,
            value,
            flags: expected_flags,
        } in cases
        {
            let state = state_with_registers(
                Z80Registers {
                    b: 0x11,
                    c,
                    h: (hl >> 8) as u8,
                    l: hl as u8,
                    pc: 0x2000,
                    ..Z80Registers::default()
                },
                0,
            );
            let mut atomic = Z80::default();
            atomic.import_architectural_state(state).unwrap();
            let mut atomic_bus = TestBus::default();
            atomic_bus.memory[0x2000] = 0xED;
            atomic_bus.memory[0x2001] = opcode;
            if input {
                atomic_bus.ports[usize::from(port)] = value;
            } else {
                atomic_bus.memory[usize::from(hl)] = value;
            }

            let mut sequenced = Z80MicroSequencer::default();
            sequenced.import_architectural_state(state).unwrap();
            let mut sequenced_bus = atomic_bus.clone();

            assert_eq!(atomic.step(&mut atomic_bus), Ok(21));
            assert_eq!(
                run_sequenced_iteration(&mut sequenced, &mut sequenced_bus, false),
                21
            );
            assert_eq!(sequenced.registers.b, 0x10);
            assert_eq!(sequenced.registers.f, expected_flags);
            assert_eq!(sequenced.wz, 0x2001);
            assert_eq!(
                sequenced.export_architectural_state(),
                atomic.export_architectural_state()
            );
            assert_eq!(sequenced_bus.memory.as_ref(), atomic_bus.memory.as_ref());
            assert_eq!(sequenced_bus.ports.as_ref(), atomic_bus.ports.as_ref());
            assert_eq!(sequenced_bus.port_reads, atomic_bus.port_reads);
            assert_eq!(sequenced_bus.port_writes, atomic_bus.port_writes);
            if input {
                assert_eq!(sequenced_bus.port_reads, vec![port]);
                assert_eq!(sequenced_bus.memory[usize::from(hl)], value);
            } else {
                assert_eq!(sequenced_bus.port_writes, vec![(port, value)]);
            }
        }
    }

    #[test]
    fn block_io_repeat_uses_pc_plus_one_memptr_and_five_extra_tstates() {
        let state = state_with_registers(
            Z80Registers {
                b: 2,
                c: 1,
                h: 0x40,
                l: 0x00,
                pc: 0x2000,
                ..Z80Registers::default()
            },
            0,
        );
        let mut cpu = Z80MicroSequencer::default();
        cpu.import_architectural_state(state).unwrap();
        let mut bus = TestBus::default();
        bus.memory[0x2000] = 0xED;
        bus.memory[0x2001] = 0xB2;
        bus.ports[0x0201] = 0x80;

        assert_eq!(run_sequenced_iteration(&mut cpu, &mut bus, false), 21);
        assert_eq!(cpu.registers.b, 1);
        assert_eq!(cpu.registers.pc, 0x2000);
        assert_eq!(cpu.wz, 0x2001);
        assert_eq!(cpu.registers.f, 0x22);
        assert!(matches!(
            cpu.export_state().phase,
            super::Z80MicroSequencerPhase::Ready
        ));
    }
}

#[cfg(test)]
mod base_16bit_register_differential_tests {
    use super::{Z80MicroSequencer, Z80MicroStep};
    use crate::bus::CpuBus;
    use crate::{Z80, Z80AlternateRegisters, Z80ArchitecturalState, Z80BusOperation, Z80Registers};

    #[derive(Clone)]
    struct TestBus {
        memory: Box<[u8; 0x1_0000]>,
    }

    impl Default for TestBus {
        fn default() -> Self {
            Self {
                memory: Box::new([0; 0x1_0000]),
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

    fn initial_state() -> Z80ArchitecturalState {
        let mut registers = Z80Registers {
            a: 0xA5,
            f: 0xD7,
            pc: 0x1000,
            sp: 0x8001,
            ..Z80Registers::default()
        };
        registers.set_bc(0xFFFF);
        registers.set_de(0x0001);
        registers.set_hl(0x7FFF);
        Z80ArchitecturalState {
            registers,
            alternate: Z80AlternateRegisters::default(),
            ix: 0x1357,
            iy: 0x2468,
            i: 0x81,
            r: 0xFE,
            iff1: true,
            iff2: false,
            interrupt_mode: 1,
            wz: 0xA328,
            ei_delay: 0,
            nmi_pending: false,
            int_pending: None,
            q: 0xA5,
            q_previous: 0x5A,
            p: false,
            halted: false,
        }
    }

    fn sequencer_state(cpu: &Z80MicroSequencer) -> Z80ArchitecturalState {
        Z80ArchitecturalState {
            registers: cpu.registers,
            alternate: cpu.alternate,
            ix: cpu.ix,
            iy: cpu.iy,
            i: cpu.i,
            r: cpu.refresh,
            iff1: cpu.iff1,
            iff2: cpu.iff2,
            interrupt_mode: cpu.interrupt_mode,
            wz: cpu.wz,
            ei_delay: cpu.ei_delay,
            nmi_pending: cpu.nmi_pending,
            int_pending: cpu.int_pending,
            q: cpu.q,
            q_previous: cpu.q_previous,
            p: cpu.p,
            halted: cpu.halted,
        }
    }

    fn run_sequencer(
        mut cpu: Z80MicroSequencer,
        bus: &mut TestBus,
    ) -> (Z80ArchitecturalState, u32) {
        let mut step = cpu.begin_instruction().unwrap();
        for _ in 0..32 {
            step = match step {
                Z80MicroStep::BusRequest(request) => {
                    let data = match request.operation {
                        Z80BusOperation::OpcodeFetch | Z80BusOperation::MemoryRead => {
                            Some(bus.read_memory(request.address))
                        }
                        Z80BusOperation::MemoryWrite { value } => {
                            bus.write_memory(request.address, value);
                            None
                        }
                        operation => panic!("unexpected BASE bus operation: {operation:?}"),
                    };
                    cpu.complete_bus_cycle(request.id, data).unwrap()
                }
                Z80MicroStep::InternalCycle { .. } => {
                    // Every internal safe point must survive snapshot/restore.
                    cpu = Z80MicroSequencer::restore_state(cpu.export_state()).unwrap();
                    cpu.complete_internal_cycle().unwrap()
                }
                Z80MicroStep::InstructionRetired { opcode: _, tstates } => {
                    return (sequencer_state(&cpu), tstates);
                }
            };
        }
        panic!("BASE 16-bit opcode did not retire within 32 micro-operations");
    }

    #[test]
    fn inc_dec_pairs_add_hl_pairs_and_ld_sp_hl_match_atomic_executor() {
        for opcode in [
            0x03, 0x0B, 0x13, 0x1B, 0x23, 0x2B, 0x33, 0x3B, 0x09, 0x19, 0x29, 0x39, 0xF9,
        ] {
            let initial = initial_state();
            let mut atomic_bus = TestBus::default();
            atomic_bus.memory[usize::from(initial.registers.pc)] = opcode;
            let mut atomic = Z80::default();
            atomic.import_architectural_state(initial).unwrap();
            let mut micro_bus = atomic_bus.clone();
            let atomic_tstates = atomic.step(&mut atomic_bus).unwrap();
            let mut micro = Z80MicroSequencer::default();
            micro.import_architectural_state(initial).unwrap();
            let (micro_state, micro_tstates) = run_sequencer(micro, &mut micro_bus);

            assert_eq!(micro_tstates, atomic_tstates, "opcode {opcode:02X}");
            assert_eq!(
                micro_state,
                atomic.export_architectural_state(),
                "architectural mismatch for BASE opcode {opcode:02X}"
            );
            assert_eq!(
                micro_bus.memory, atomic_bus.memory,
                "memory mismatch for BASE opcode {opcode:02X}"
            );
            assert_eq!(
                atomic_tstates,
                if matches!(opcode, 0x09 | 0x19 | 0x29 | 0x39) {
                    11
                } else {
                    6
                },
                "Zilog UM008011-0816 timing mismatch for opcode {opcode:02X}"
            );
        }
    }
}

#[cfg(test)]
mod base_absolute_transfer_differential_tests {
    use super::{Z80MicroSequencer, Z80MicroStep};
    use crate::bus::CpuBus;
    use crate::{Z80, Z80AlternateRegisters, Z80ArchitecturalState, Z80BusOperation, Z80Registers};

    #[derive(Clone)]
    struct TestBus {
        memory: Box<[u8; 0x1_0000]>,
    }

    impl Default for TestBus {
        fn default() -> Self {
            Self {
                memory: Box::new([0; 0x1_0000]),
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

    fn initial_state() -> Z80ArchitecturalState {
        let mut registers = Z80Registers {
            a: 0xA5,
            f: 0xD7,
            pc: 0x1000,
            sp: 0x8001,
            ..Z80Registers::default()
        };
        registers.set_hl(0xBEEF);
        Z80ArchitecturalState {
            registers,
            alternate: Z80AlternateRegisters::default(),
            ix: 0x1357,
            iy: 0x2468,
            i: 0x81,
            r: 0xFE,
            iff1: true,
            iff2: false,
            interrupt_mode: 1,
            wz: 0xA328,
            ei_delay: 0,
            nmi_pending: false,
            int_pending: None,
            q: 0xA5,
            q_previous: 0x5A,
            p: false,
            halted: false,
        }
    }

    fn sequencer_state(cpu: &Z80MicroSequencer) -> Z80ArchitecturalState {
        Z80ArchitecturalState {
            registers: cpu.registers,
            alternate: cpu.alternate,
            ix: cpu.ix,
            iy: cpu.iy,
            i: cpu.i,
            r: cpu.refresh,
            iff1: cpu.iff1,
            iff2: cpu.iff2,
            interrupt_mode: cpu.interrupt_mode,
            wz: cpu.wz,
            ei_delay: cpu.ei_delay,
            nmi_pending: cpu.nmi_pending,
            int_pending: cpu.int_pending,
            q: cpu.q,
            q_previous: cpu.q_previous,
            p: cpu.p,
            halted: cpu.halted,
        }
    }

    fn run_sequencer(
        mut cpu: Z80MicroSequencer,
        bus: &mut TestBus,
    ) -> (Z80ArchitecturalState, u32) {
        let mut step = cpu.begin_instruction().unwrap();
        for _ in 0..16 {
            step = match step {
                Z80MicroStep::BusRequest(request) => {
                    cpu = Z80MicroSequencer::restore_state(cpu.export_state()).unwrap();
                    let data = match request.operation {
                        Z80BusOperation::OpcodeFetch | Z80BusOperation::MemoryRead => {
                            Some(bus.read_memory(request.address))
                        }
                        Z80BusOperation::MemoryWrite { value } => {
                            bus.write_memory(request.address, value);
                            None
                        }
                        operation => panic!("unexpected BASE bus operation: {operation:?}"),
                    };
                    cpu.complete_bus_cycle(request.id, data).unwrap()
                }
                Z80MicroStep::InternalCycle { .. } => {
                    cpu = Z80MicroSequencer::restore_state(cpu.export_state()).unwrap();
                    cpu.complete_internal_cycle().unwrap()
                }
                Z80MicroStep::InstructionRetired { tstates, .. } => {
                    return (sequencer_state(&cpu), tstates);
                }
            };
        }
        panic!("BASE absolute transfer did not retire");
    }

    #[test]
    fn absolute_memory_transfers_match_atomic_executor_and_restore_every_bus_boundary() {
        for address in [0x2000_u16, 0xFFFF] {
            for opcode in [0x22, 0x2A, 0x32, 0x3A] {
                let initial = initial_state();
                let mut atomic_bus = TestBus::default();
                let address_bytes = address.to_le_bytes();
                atomic_bus.memory[0x1000] = opcode;
                atomic_bus.memory[0x1001] = address_bytes[0];
                atomic_bus.memory[0x1002] = address_bytes[1];
                atomic_bus.memory[usize::from(address)] = 0x34;
                atomic_bus.memory[usize::from(address.wrapping_add(1))] = 0x12;

                let mut micro_bus = atomic_bus.clone();
                let mut atomic = Z80::default();
                atomic.import_architectural_state(initial).unwrap();
                let atomic_tstates = atomic.step(&mut atomic_bus).unwrap();

                let mut micro = Z80MicroSequencer::default();
                micro.import_architectural_state(initial).unwrap();
                let (micro_state, micro_tstates) = run_sequencer(micro, &mut micro_bus);

                assert_eq!(
                    micro_tstates, atomic_tstates,
                    "T-state mismatch for opcode {opcode:02X} at {address:04X}"
                );
                assert_eq!(
                    micro_state,
                    atomic.export_architectural_state(),
                    "architectural mismatch for opcode {opcode:02X} at {address:04X}"
                );
                assert_eq!(
                    micro_bus.memory, atomic_bus.memory,
                    "memory mismatch for opcode {opcode:02X} at {address:04X}"
                );
                assert_eq!(
                    atomic_tstates,
                    if matches!(opcode, 0x22 | 0x2A) {
                        16
                    } else {
                        13
                    }
                );
            }
        }
    }
}

#[cfg(test)]
mod base_relative_branch_differential_tests {
    use crate::bus::CpuBus;
    use crate::{Z80, Z80ArchitecturalState, Z80BusOperation, Z80MicroSequencer, Z80MicroStep};

    const FLAG_C: u8 = 0x01;
    const FLAG_Z: u8 = 0x40;

    #[derive(Clone)]
    struct TestBus {
        memory: Box<[u8; 0x1_0000]>,
    }

    impl Default for TestBus {
        fn default() -> Self {
            Self {
                memory: Box::new([0; 0x1_0000]),
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

    fn run_sequencer(
        mut cpu: Z80MicroSequencer,
        bus: &mut TestBus,
    ) -> (Z80ArchitecturalState, u32) {
        let mut step = cpu.begin_instruction().unwrap();
        for _ in 0..32 {
            step = match step {
                Z80MicroStep::BusRequest(request) => {
                    cpu = Z80MicroSequencer::restore_state(cpu.export_state()).unwrap();
                    let data = match request.operation {
                        Z80BusOperation::OpcodeFetch | Z80BusOperation::MemoryRead => {
                            Some(bus.read_memory(request.address))
                        }
                        Z80BusOperation::MemoryWrite { value } => {
                            bus.write_memory(request.address, value);
                            None
                        }
                        operation => {
                            panic!("unexpected relative-branch bus operation: {operation:?}")
                        }
                    };
                    cpu.complete_bus_cycle(request.id, data).unwrap()
                }
                Z80MicroStep::InternalCycle { .. } => {
                    cpu = Z80MicroSequencer::restore_state(cpu.export_state()).unwrap();
                    cpu.complete_internal_cycle().unwrap()
                }
                Z80MicroStep::InstructionRetired { tstates, .. } => {
                    return (cpu.export_architectural_state(), tstates);
                }
            };
        }
        panic!("relative branch did not retire within 32 micro-operations");
    }

    fn compare_case(opcode: u8, b: u8, flags: u8, displacement: u8, expected_tstates: u32) {
        let mut initial = Z80::default().export_architectural_state();
        initial.registers.pc = 0x1000;
        initial.registers.b = b;
        initial.registers.f = flags;
        initial.ei_delay = 1;
        initial.wz = 0xA328;

        let mut atomic_bus = TestBus::default();
        atomic_bus.memory[0x1000] = opcode;
        atomic_bus.memory[0x1001] = displacement;
        let mut micro_bus = atomic_bus.clone();

        let mut atomic = Z80::default();
        atomic.import_architectural_state(initial).unwrap();
        let atomic_tstates = atomic.step(&mut atomic_bus).unwrap();

        let mut micro = Z80MicroSequencer::default();
        micro.import_architectural_state(initial).unwrap();
        let (micro_state, micro_tstates) = run_sequencer(micro, &mut micro_bus);

        assert_eq!(micro_tstates, atomic_tstates, "opcode {opcode:02X}");
        assert_eq!(micro_tstates, expected_tstates, "opcode {opcode:02X}");
        assert_eq!(
            micro_state,
            atomic.export_architectural_state(),
            "architectural mismatch for opcode {opcode:02X}"
        );
        assert_eq!(
            micro_bus.memory, atomic_bus.memory,
            "memory mismatch for opcode {opcode:02X}"
        );
    }

    #[test]
    fn djnz_taken_and_not_taken_match_atomic_executor() {
        compare_case(0x10, 0x02, 0x00, 0xFE, 13);
        compare_case(0x10, 0x01, 0x00, 0xFE, 8);
    }

    #[test]
    fn jr_and_all_conditional_relative_branches_match_atomic_executor() {
        compare_case(0x18, 0x00, 0x00, 0x7F, 12);
        compare_case(0x20, 0x00, 0x00, 0x02, 12);
        compare_case(0x20, 0x00, FLAG_Z, 0x02, 7);
        compare_case(0x28, 0x00, FLAG_Z, 0x02, 12);
        compare_case(0x28, 0x00, 0x00, 0x02, 7);
        compare_case(0x30, 0x00, 0x00, 0x02, 12);
        compare_case(0x30, 0x00, FLAG_C, 0x02, 7);
        compare_case(0x38, 0x00, FLAG_C, 0x02, 12);
        compare_case(0x38, 0x00, 0x00, 0x02, 7);
    }

    #[test]
    fn jr_displacement_wraps_pc_at_the_16_bit_boundary() {
        let mut initial = Z80::default().export_architectural_state();
        initial.registers.pc = 0xFFFE;
        initial.wz = 0xA328;

        let mut atomic_bus = TestBus::default();
        atomic_bus.memory[0xFFFE] = 0x18;
        atomic_bus.memory[0xFFFF] = 0xFF;
        let mut micro_bus = atomic_bus.clone();

        let mut atomic = Z80::default();
        atomic.import_architectural_state(initial).unwrap();
        let atomic_tstates = atomic.step(&mut atomic_bus).unwrap();

        let mut micro = Z80MicroSequencer::default();
        micro.import_architectural_state(initial).unwrap();
        let (micro_state, micro_tstates) = run_sequencer(micro, &mut micro_bus);

        assert_eq!(micro_tstates, 12);
        assert_eq!(micro_tstates, atomic_tstates);
        assert_eq!(micro_state.registers.pc, 0xFFFF);
        assert_eq!(micro_state.wz, 0xFFFF);
        assert_eq!(micro_state, atomic.export_architectural_state());
        assert_eq!(micro_bus.memory, atomic_bus.memory);
    }
}

#[cfg(test)]
mod base_absolute_jump_differential_tests {
    use crate::bus::CpuBus;
    use crate::{Z80, Z80ArchitecturalState, Z80BusOperation, Z80MicroSequencer, Z80MicroStep};

    const FLAG_C: u8 = 0x01;
    const FLAG_PV: u8 = 0x04;
    const FLAG_Z: u8 = 0x40;
    const FLAG_S: u8 = 0x80;

    #[derive(Clone)]
    struct TestBus {
        memory: Box<[u8; 0x1_0000]>,
    }

    impl Default for TestBus {
        fn default() -> Self {
            Self {
                memory: Box::new([0; 0x1_0000]),
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

    fn run_sequencer(
        mut cpu: Z80MicroSequencer,
        bus: &mut TestBus,
    ) -> (Z80ArchitecturalState, u32) {
        let mut step = cpu.begin_instruction().unwrap();
        for _ in 0..16 {
            step = match step {
                Z80MicroStep::BusRequest(request) => {
                    cpu = Z80MicroSequencer::restore_state(cpu.export_state()).unwrap();
                    let data = match request.operation {
                        Z80BusOperation::OpcodeFetch | Z80BusOperation::MemoryRead => {
                            Some(bus.read_memory(request.address))
                        }
                        Z80BusOperation::MemoryWrite { value } => {
                            bus.write_memory(request.address, value);
                            None
                        }
                        operation => {
                            panic!("unexpected absolute-jump bus operation: {operation:?}")
                        }
                    };
                    cpu.complete_bus_cycle(request.id, data).unwrap()
                }
                Z80MicroStep::InternalCycle { .. } => {
                    cpu = Z80MicroSequencer::restore_state(cpu.export_state()).unwrap();
                    cpu.complete_internal_cycle().unwrap()
                }
                Z80MicroStep::InstructionRetired { tstates, .. } => {
                    return (cpu.export_architectural_state(), tstates);
                }
            };
        }
        panic!("absolute jump did not retire within 16 micro-operations");
    }

    fn compare_case(opcode: u8, flags: u8, expected_pc: u16) {
        let mut initial = Z80::default().export_architectural_state();
        initial.registers.pc = 0x1000;
        initial.registers.f = flags;
        initial.ei_delay = 1;
        initial.wz = 0xA328;

        let mut atomic_bus = TestBus::default();
        atomic_bus.memory[0x1000] = opcode;
        atomic_bus.memory[0x1001] = 0x00;
        atomic_bus.memory[0x1002] = 0x20;
        let mut micro_bus = atomic_bus.clone();

        let mut atomic = Z80::default();
        atomic.import_architectural_state(initial).unwrap();
        let atomic_tstates = atomic.step(&mut atomic_bus).unwrap();

        let mut micro = Z80MicroSequencer::default();
        micro.import_architectural_state(initial).unwrap();
        let (micro_state, micro_tstates) = run_sequencer(micro, &mut micro_bus);

        assert_eq!(micro_tstates, 10, "opcode {opcode:02X}");
        assert_eq!(micro_tstates, atomic_tstates, "opcode {opcode:02X}");
        assert_eq!(micro_state.registers.pc, expected_pc, "opcode {opcode:02X}");
        assert_eq!(micro_state.wz, 0x2000, "opcode {opcode:02X}");
        assert_eq!(
            micro_state,
            atomic.export_architectural_state(),
            "architectural mismatch for opcode {opcode:02X}"
        );
        assert_eq!(
            micro_bus.memory, atomic_bus.memory,
            "memory mismatch for opcode {opcode:02X}"
        );
    }

    #[test]
    fn unconditional_and_all_conditional_absolute_jumps_match_atomic_executor() {
        compare_case(0xC3, 0x00, 0x2000);
        compare_case(0xC2, 0x00, 0x2000);
        compare_case(0xC2, FLAG_Z, 0x1003);
        compare_case(0xCA, FLAG_Z, 0x2000);
        compare_case(0xCA, 0x00, 0x1003);
        compare_case(0xD2, 0x00, 0x2000);
        compare_case(0xD2, FLAG_C, 0x1003);
        compare_case(0xDA, FLAG_C, 0x2000);
        compare_case(0xDA, 0x00, 0x1003);
        compare_case(0xE2, 0x00, 0x2000);
        compare_case(0xE2, FLAG_PV, 0x1003);
        compare_case(0xEA, FLAG_PV, 0x2000);
        compare_case(0xEA, 0x00, 0x1003);
        compare_case(0xF2, 0x00, 0x2000);
        compare_case(0xF2, FLAG_S, 0x1003);
        compare_case(0xFA, FLAG_S, 0x2000);
        compare_case(0xFA, 0x00, 0x1003);
    }

    #[test]
    fn absolute_jump_immediate_fetch_wraps_pc() {
        let mut initial = Z80::default().export_architectural_state();
        initial.registers.pc = 0xFFFE;
        initial.wz = 0xA328;

        let mut atomic_bus = TestBus::default();
        atomic_bus.memory[0xFFFE] = 0xC3;
        atomic_bus.memory[0xFFFF] = 0x34;
        atomic_bus.memory[0x0000] = 0x12;
        let mut micro_bus = atomic_bus.clone();

        let mut atomic = Z80::default();
        atomic.import_architectural_state(initial).unwrap();
        let atomic_tstates = atomic.step(&mut atomic_bus).unwrap();

        let mut micro = Z80MicroSequencer::default();
        micro.import_architectural_state(initial).unwrap();
        let (micro_state, micro_tstates) = run_sequencer(micro, &mut micro_bus);

        assert_eq!(micro_tstates, 10);
        assert_eq!(micro_tstates, atomic_tstates);
        assert_eq!(micro_state.registers.pc, 0x1234);
        assert_eq!(micro_state.wz, 0x1234);
        assert_eq!(micro_state, atomic.export_architectural_state());
        assert_eq!(micro_bus.memory, atomic_bus.memory);
    }
}

#[cfg(test)]
mod base_stack_push_pop_differential_tests {
    use crate::bus::CpuBus;
    use crate::{Z80, Z80ArchitecturalState, Z80BusOperation, Z80MicroSequencer, Z80MicroStep};

    #[derive(Clone)]
    struct TestBus {
        memory: Box<[u8; 0x1_0000]>,
    }

    impl Default for TestBus {
        fn default() -> Self {
            Self {
                memory: Box::new([0; 0x1_0000]),
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

    fn run_sequencer(
        mut cpu: Z80MicroSequencer,
        bus: &mut TestBus,
    ) -> (Z80ArchitecturalState, u32) {
        let mut step = cpu.begin_instruction().unwrap();
        for _ in 0..16 {
            step = match step {
                Z80MicroStep::BusRequest(request) => {
                    cpu = Z80MicroSequencer::restore_state(cpu.export_state()).unwrap();
                    let data = match request.operation {
                        Z80BusOperation::OpcodeFetch | Z80BusOperation::MemoryRead => {
                            Some(bus.read_memory(request.address))
                        }
                        Z80BusOperation::MemoryWrite { value } => {
                            bus.write_memory(request.address, value);
                            None
                        }
                        operation => panic!("unexpected stack bus operation: {operation:?}"),
                    };
                    cpu.complete_bus_cycle(request.id, data).unwrap()
                }
                Z80MicroStep::InternalCycle { .. } => {
                    cpu = Z80MicroSequencer::restore_state(cpu.export_state()).unwrap();
                    cpu.complete_internal_cycle().unwrap()
                }
                Z80MicroStep::InstructionRetired { tstates, .. } => {
                    return (cpu.export_architectural_state(), tstates);
                }
            };
        }
        panic!("stack instruction did not retire within 16 micro-operations");
    }

    fn push_initial_state(opcode: u8, sp: u16) -> (Z80ArchitecturalState, u16) {
        let mut initial = Z80::default().export_architectural_state();
        initial.registers.pc = 0x1000;
        initial.registers.sp = sp;
        initial.ei_delay = 1;
        let value = match opcode {
            0xC5 => {
                initial.registers.b = 0x12;
                initial.registers.c = 0x34;
                0x1234
            }
            0xD5 => {
                initial.registers.d = 0x56;
                initial.registers.e = 0x78;
                0x5678
            }
            0xE5 => {
                initial.registers.h = 0x9A;
                initial.registers.l = 0xBC;
                0x9ABC
            }
            0xF5 => {
                initial.registers.a = 0xDE;
                initial.registers.f = 0xF0;
                0xDEF0
            }
            _ => panic!("not a PUSH opcode: {opcode:02X}"),
        };
        (initial, value)
    }

    fn compare_push(opcode: u8, sp: u16) {
        let (initial, value) = push_initial_state(opcode, sp);
        let mut atomic_bus = TestBus::default();
        atomic_bus.memory[0x1000] = opcode;
        let mut micro_bus = atomic_bus.clone();

        let mut atomic = Z80::default();
        atomic.import_architectural_state(initial).unwrap();
        let atomic_tstates = atomic.step(&mut atomic_bus).unwrap();

        let mut micro = Z80MicroSequencer::default();
        micro.import_architectural_state(initial).unwrap();
        let (micro_state, micro_tstates) = run_sequencer(micro, &mut micro_bus);

        let [high, low] = value.to_be_bytes();
        let high_address = sp.wrapping_sub(1);
        let low_address = sp.wrapping_sub(2);
        assert_eq!(micro_tstates, 11, "PUSH opcode {opcode:02X}");
        assert_eq!(micro_tstates, atomic_tstates, "PUSH opcode {opcode:02X}");
        assert_eq!(micro_state.registers.sp, low_address);
        assert_eq!(micro_bus.memory[usize::from(high_address)], high);
        assert_eq!(micro_bus.memory[usize::from(low_address)], low);
        assert_eq!(micro_state, atomic.export_architectural_state());
        assert_eq!(micro_bus.memory, atomic_bus.memory);
    }

    fn compare_pop(opcode: u8, sp: u16) {
        let mut initial = Z80::default().export_architectural_state();
        initial.registers.pc = 0x1000;
        initial.registers.sp = sp;
        initial.registers.a = 0xA5;
        initial.registers.f = 0xD7;
        initial.ei_delay = 1;

        let mut atomic_bus = TestBus::default();
        atomic_bus.memory[0x1000] = opcode;
        atomic_bus.memory[usize::from(sp)] = 0x34;
        atomic_bus.memory[usize::from(sp.wrapping_add(1))] = 0x12;
        let mut micro_bus = atomic_bus.clone();

        let mut atomic = Z80::default();
        atomic.import_architectural_state(initial).unwrap();
        let atomic_tstates = atomic.step(&mut atomic_bus).unwrap();

        let mut micro = Z80MicroSequencer::default();
        micro.import_architectural_state(initial).unwrap();
        let (micro_state, micro_tstates) = run_sequencer(micro, &mut micro_bus);

        assert_eq!(micro_tstates, 10, "POP opcode {opcode:02X}");
        assert_eq!(micro_tstates, atomic_tstates, "POP opcode {opcode:02X}");
        assert_eq!(micro_state.registers.sp, sp.wrapping_add(2));
        assert_eq!(micro_state, atomic.export_architectural_state());
        assert_eq!(micro_bus.memory, atomic_bus.memory);
    }

    #[test]
    fn all_push_pairs_match_atomic_executor_at_normal_and_wrapping_stack_addresses() {
        for opcode in [0xC5, 0xD5, 0xE5, 0xF5] {
            compare_push(opcode, 0x2000);
            compare_push(opcode, 0x0000);
        }
    }

    #[test]
    fn all_pop_pairs_match_atomic_executor_at_normal_and_wrapping_stack_addresses() {
        for opcode in [0xC1, 0xD1, 0xE1, 0xF1] {
            compare_pop(opcode, 0x2000);
            compare_pop(opcode, 0xFFFF);
        }
    }
}

#[cfg(test)]
mod base_call_rst_differential_tests {
    use crate::bus::CpuBus;
    use crate::{Z80, Z80ArchitecturalState, Z80BusOperation, Z80MicroSequencer, Z80MicroStep};

    const FLAG_C: u8 = 0x01;
    const FLAG_PV: u8 = 0x04;
    const FLAG_Z: u8 = 0x40;
    const FLAG_S: u8 = 0x80;

    #[derive(Clone)]
    struct TestBus {
        memory: Box<[u8; 0x1_0000]>,
    }

    impl Default for TestBus {
        fn default() -> Self {
            Self {
                memory: Box::new([0; 0x1_0000]),
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

    fn run_sequencer(
        mut cpu: Z80MicroSequencer,
        bus: &mut TestBus,
    ) -> (Z80ArchitecturalState, u32) {
        let mut step = cpu.begin_instruction().unwrap();
        for _ in 0..24 {
            step = match step {
                Z80MicroStep::BusRequest(request) => {
                    cpu = Z80MicroSequencer::restore_state(cpu.export_state()).unwrap();
                    let data = match request.operation {
                        Z80BusOperation::OpcodeFetch | Z80BusOperation::MemoryRead => {
                            Some(bus.read_memory(request.address))
                        }
                        Z80BusOperation::MemoryWrite { value } => {
                            bus.write_memory(request.address, value);
                            None
                        }
                        operation => panic!("unexpected CALL/RST bus operation: {operation:?}"),
                    };
                    cpu.complete_bus_cycle(request.id, data).unwrap()
                }
                Z80MicroStep::InternalCycle { .. } => {
                    cpu = Z80MicroSequencer::restore_state(cpu.export_state()).unwrap();
                    cpu.complete_internal_cycle().unwrap()
                }
                Z80MicroStep::InstructionRetired { tstates, .. } => {
                    return (cpu.export_architectural_state(), tstates);
                }
            };
        }
        panic!("CALL/RST did not retire within 24 micro-operations");
    }

    fn compare_call(opcode: u8, flags: u8, taken: bool) {
        let mut initial = Z80::default().export_architectural_state();
        initial.registers.pc = 0x1000;
        initial.registers.sp = 0x2000;
        initial.registers.f = flags;
        initial.ei_delay = 1;

        let mut atomic_bus = TestBus::default();
        atomic_bus.memory[0x1000] = opcode;
        atomic_bus.memory[0x1001] = 0x00;
        atomic_bus.memory[0x1002] = 0x20;
        let mut micro_bus = atomic_bus.clone();

        let mut atomic = Z80::default();
        atomic.import_architectural_state(initial).unwrap();
        let atomic_tstates = atomic.step(&mut atomic_bus).unwrap();

        let mut micro = Z80MicroSequencer::default();
        micro.import_architectural_state(initial).unwrap();
        let (micro_state, micro_tstates) = run_sequencer(micro, &mut micro_bus);

        assert_eq!(
            micro_tstates,
            if taken { 17 } else { 10 },
            "CALL {opcode:02X}"
        );
        assert_eq!(micro_tstates, atomic_tstates, "CALL {opcode:02X}");
        assert_eq!(
            micro_state.registers.pc,
            if taken { 0x2000 } else { 0x1003 },
            "CALL {opcode:02X}"
        );
        assert_eq!(micro_state.wz, 0x2000, "CALL {opcode:02X}");
        assert_eq!(
            micro_state.registers.sp,
            if taken { 0x1FFE } else { 0x2000 },
            "CALL {opcode:02X}"
        );
        if taken {
            assert_eq!(micro_bus.memory[0x1FFF], 0x10);
            assert_eq!(micro_bus.memory[0x1FFE], 0x03);
        }
        assert_eq!(
            micro_state,
            atomic.export_architectural_state(),
            "architectural mismatch for CALL {opcode:02X}"
        );
        assert_eq!(
            micro_bus.memory, atomic_bus.memory,
            "memory mismatch for CALL {opcode:02X}"
        );
    }

    fn compare_rst(opcode: u8, sp: u16) {
        let mut initial = Z80::default().export_architectural_state();
        initial.registers.pc = 0x1000;
        initial.registers.sp = sp;
        initial.ei_delay = 1;

        let mut atomic_bus = TestBus::default();
        atomic_bus.memory[0x1000] = opcode;
        let mut micro_bus = atomic_bus.clone();

        let mut atomic = Z80::default();
        atomic.import_architectural_state(initial).unwrap();
        let atomic_tstates = atomic.step(&mut atomic_bus).unwrap();

        let mut micro = Z80MicroSequencer::default();
        micro.import_architectural_state(initial).unwrap();
        let (micro_state, micro_tstates) = run_sequencer(micro, &mut micro_bus);

        let vector = u16::from(opcode & 0x38);
        assert_eq!(micro_tstates, 11, "RST {opcode:02X}");
        assert_eq!(micro_tstates, atomic_tstates, "RST {opcode:02X}");
        assert_eq!(micro_state.registers.pc, vector);
        assert_eq!(micro_state.wz, vector);
        assert_eq!(micro_state.registers.sp, sp.wrapping_sub(2));
        assert_eq!(micro_bus.memory[usize::from(sp.wrapping_sub(1))], 0x10);
        assert_eq!(micro_bus.memory[usize::from(sp.wrapping_sub(2))], 0x01);
        assert_eq!(micro_state, atomic.export_architectural_state());
        assert_eq!(micro_bus.memory, atomic_bus.memory);
    }

    #[test]
    fn unconditional_and_conditional_calls_match_atomic_executor() {
        compare_call(0xCD, 0x00, true);
        compare_call(0xC4, 0x00, true);
        compare_call(0xC4, FLAG_Z, false);
        compare_call(0xCC, FLAG_Z, true);
        compare_call(0xCC, 0x00, false);
        compare_call(0xD4, 0x00, true);
        compare_call(0xD4, FLAG_C, false);
        compare_call(0xDC, FLAG_C, true);
        compare_call(0xDC, 0x00, false);
        compare_call(0xE4, 0x00, true);
        compare_call(0xE4, FLAG_PV, false);
        compare_call(0xEC, FLAG_PV, true);
        compare_call(0xEC, 0x00, false);
        compare_call(0xF4, 0x00, true);
        compare_call(0xF4, FLAG_S, false);
        compare_call(0xFC, FLAG_S, true);
        compare_call(0xFC, 0x00, false);
    }

    #[test]
    fn all_rst_vectors_match_atomic_executor_with_stack_wraparound() {
        for opcode in [0xC7, 0xCF, 0xD7, 0xDF, 0xE7, 0xEF, 0xF7, 0xFF] {
            compare_rst(opcode, 0x2000);
        }
        compare_rst(0xFF, 0x0000);
    }
}
