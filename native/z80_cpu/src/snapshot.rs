//! Versioned deterministic codec for the resumable Z80 microsequencer state.
//!
//! The wire format is explicit and independent of Serde. It stores only the
//! state represented by the current microsequencer slice; it is not a full Z80
//! architectural save state.

use crate::{
    Z80_MICRO_SEQUENCER_STATE_VERSION, Z80AlternateRegisters, Z80BusHandshakeState,
    Z80BusOperation, Z80BusRequest, Z80IndexRegister, Z80MicroSequencer, Z80MicroSequencerPhase,
    Z80MicroSequencerState, Z80PrefixState, Z80Registers,
};
use core::fmt;

pub const Z80_MICRO_SEQUENCER_STATE_MAGIC: [u8; 8] = *b"Z80SEQ\0\0";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Z80MicroSequencerStateCodecError {
    InvalidMagic,
    UnsupportedVersion { expected: u16, actual: u16 },
    Truncated,
    TrailingData,
    InvalidTag(u8),
    ChecksumMismatch,
    InvalidState,
}

impl fmt::Display for Z80MicroSequencerStateCodecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidMagic => f.write_str("Z80 sequencer state has invalid magic"),
            Self::UnsupportedVersion { expected, actual } => write!(
                f,
                "Z80 sequencer state version {actual} is unsupported (expected {expected})"
            ),
            Self::Truncated => f.write_str("Z80 sequencer state is truncated"),
            Self::TrailingData => f.write_str("Z80 sequencer state has trailing bytes"),
            Self::InvalidTag(tag) => write!(f, "Z80 sequencer state has invalid tag {tag}"),
            Self::ChecksumMismatch => f.write_str("Z80 sequencer state checksum mismatch"),
            Self::InvalidState => {
                f.write_str("Z80 sequencer state violates CPU/handshake invariants")
            }
        }
    }
}

impl std::error::Error for Z80MicroSequencerStateCodecError {}

impl Z80MicroSequencerState {
    /// Encode a validated snapshot into the versioned canonical binary format.
    pub fn encode(&self) -> Result<Vec<u8>, Z80MicroSequencerStateCodecError> {
        Z80MicroSequencer::restore_state(self.clone())
            .map_err(|_| Z80MicroSequencerStateCodecError::InvalidState)?;

        let mut body = Writer::default();
        for value in [
            self.registers.a,
            self.registers.f,
            self.registers.b,
            self.registers.c,
            self.registers.d,
            self.registers.e,
            self.registers.h,
            self.registers.l,
        ] {
            body.u8(value);
        }
        body.u16(self.registers.sp);
        body.u16(self.registers.pc);
        body.u8(self.refresh);
        body.u8(self.q);
        body.u8(self.q_previous);
        body.u32(self.instruction_tstates);
        body.u32(self.mcycle_index);
        body.u16(self.tstate_index);
        match self.opcode {
            None => body.u8(0),
            Some(value) => {
                body.u8(1);
                body.u8(value);
            }
        }
        match self.prefix_state {
            Z80PrefixState::None => body.u8(0),
            Z80PrefixState::Cb => body.u8(1),
            Z80PrefixState::Ed => body.u8(2),
            Z80PrefixState::Index(Z80IndexRegister::Ix) => body.u8(3),
            Z80PrefixState::Index(Z80IndexRegister::Iy) => body.u8(4),
        }
        write_phase(&mut body, self.phase);
        body.u64(self.handshake.next_request_id);
        match self.handshake.pending_request {
            None => body.u8(0),
            Some(request) => {
                body.u8(1);
                write_request(&mut body, request);
            }
        }
        body.u32(self.handshake.wait_tstates);
        for value in [
            self.alternate.a,
            self.alternate.f,
            self.alternate.b,
            self.alternate.c,
            self.alternate.d,
            self.alternate.e,
            self.alternate.h,
            self.alternate.l,
        ] {
            body.u8(value);
        }
        body.u16(self.ix);
        body.u16(self.iy);
        body.u8(self.i);
        body.u8(u8::from(self.iff1));
        body.u8(u8::from(self.iff2));
        body.u8(self.interrupt_mode);
        body.u16(self.wz);
        body.u8(self.ei_delay);
        body.u8(u8::from(self.nmi_pending));
        match self.int_pending {
            None => body.u8(0),
            Some(value) => {
                body.u8(1);
                body.u8(value);
            }
        }
        body.u8(u8::from(self.p));
        body.u8(u8::from(self.halted));

        let body_len = u32::try_from(body.bytes.len())
            .map_err(|_| Z80MicroSequencerStateCodecError::InvalidState)?;
        let mut frame = Writer::default();
        frame
            .bytes
            .extend_from_slice(&Z80_MICRO_SEQUENCER_STATE_MAGIC);
        frame.u16(Z80_MICRO_SEQUENCER_STATE_VERSION);
        frame.u32(body_len);
        frame.bytes.extend_from_slice(&body.bytes);
        frame.u32(crc32(&body.bytes));
        Ok(frame.bytes)
    }

    /// Decode exactly one frame and validate all CPU/handshake cross-invariants.
    pub fn decode(bytes: &[u8]) -> Result<Self, Z80MicroSequencerStateCodecError> {
        let mut frame = Reader::new(bytes);
        if frame.array::<8>()? != Z80_MICRO_SEQUENCER_STATE_MAGIC {
            return Err(Z80MicroSequencerStateCodecError::InvalidMagic);
        }
        let version = frame.u16()?;
        if version != Z80_MICRO_SEQUENCER_STATE_VERSION {
            return Err(Z80MicroSequencerStateCodecError::UnsupportedVersion {
                expected: Z80_MICRO_SEQUENCER_STATE_VERSION,
                actual: version,
            });
        }
        let body_len = usize::try_from(frame.u32()?)
            .map_err(|_| Z80MicroSequencerStateCodecError::Truncated)?;
        let body = frame.take(body_len)?;
        let checksum = frame.u32()?;
        if !frame.finished() {
            return Err(Z80MicroSequencerStateCodecError::TrailingData);
        }
        if crc32(body) != checksum {
            return Err(Z80MicroSequencerStateCodecError::ChecksumMismatch);
        }

        let mut reader = Reader::new(body);
        let registers = Z80Registers {
            a: reader.u8()?,
            f: reader.u8()?,
            b: reader.u8()?,
            c: reader.u8()?,
            d: reader.u8()?,
            e: reader.u8()?,
            h: reader.u8()?,
            l: reader.u8()?,
            sp: reader.u16()?,
            pc: reader.u16()?,
        };
        let refresh = reader.u8()?;
        let q = reader.u8()?;
        let q_previous = reader.u8()?;
        let instruction_tstates = reader.u32()?;
        let mcycle_index = reader.u32()?;
        let tstate_index = reader.u16()?;
        let opcode = match reader.u8()? {
            0 => None,
            1 => Some(reader.u8()?),
            tag => return Err(Z80MicroSequencerStateCodecError::InvalidTag(tag)),
        };
        let prefix_state = match reader.u8()? {
            0 => Z80PrefixState::None,
            1 => Z80PrefixState::Cb,
            2 => Z80PrefixState::Ed,
            3 => Z80PrefixState::Index(Z80IndexRegister::Ix),
            4 => Z80PrefixState::Index(Z80IndexRegister::Iy),
            tag => return Err(Z80MicroSequencerStateCodecError::InvalidTag(tag)),
        };
        let phase = read_phase(&mut reader)?;
        let next_request_id = reader.u64()?;
        let pending_request = match reader.u8()? {
            0 => None,
            1 => Some(read_request(&mut reader)?),
            tag => return Err(Z80MicroSequencerStateCodecError::InvalidTag(tag)),
        };
        let wait_tstates = reader.u32()?;
        let alternate = Z80AlternateRegisters {
            a: reader.u8()?,
            f: reader.u8()?,
            b: reader.u8()?,
            c: reader.u8()?,
            d: reader.u8()?,
            e: reader.u8()?,
            h: reader.u8()?,
            l: reader.u8()?,
        };
        let ix = reader.u16()?;
        let iy = reader.u16()?;
        let i = reader.u8()?;
        let iff1 = read_bool(&mut reader)?;
        let iff2 = read_bool(&mut reader)?;
        let interrupt_mode = reader.u8()?;
        let wz = reader.u16()?;
        let ei_delay = reader.u8()?;
        let nmi_pending = read_bool(&mut reader)?;
        let int_pending = match reader.u8()? {
            0 => None,
            1 => Some(reader.u8()?),
            tag => return Err(Z80MicroSequencerStateCodecError::InvalidTag(tag)),
        };
        let p = read_bool(&mut reader)?;
        let halted = read_bool(&mut reader)?;
        if !reader.finished() {
            return Err(Z80MicroSequencerStateCodecError::TrailingData);
        }

        let state = Self {
            format_version: version,
            registers,
            refresh,
            q,
            q_previous,
            handshake: Z80BusHandshakeState {
                next_request_id,
                pending_request,
                wait_tstates,
            },
            phase,
            opcode,
            prefix_state,
            instruction_tstates,
            mcycle_index,
            tstate_index,
            alternate,
            ix,
            iy,
            i,
            iff1,
            iff2,
            interrupt_mode,
            wz,
            ei_delay,
            nmi_pending,
            int_pending,
            p,
            halted,
        };
        Z80MicroSequencer::restore_state(state.clone())
            .map_err(|_| Z80MicroSequencerStateCodecError::InvalidState)?;
        Ok(state)
    }
}

fn write_phase(writer: &mut Writer, phase: Z80MicroSequencerPhase) {
    match phase {
        Z80MicroSequencerPhase::Ready => writer.u8(0),
        Z80MicroSequencerPhase::OpcodeFetch => writer.u8(1),
        Z80MicroSequencerPhase::InterruptAcknowledge => writer.u8(64),
        Z80MicroSequencerPhase::InterruptAcknowledgeComplete { vector, mode } => {
            writer.u8(65);
            writer.u8(vector);
            writer.u8(mode);
        }
        Z80MicroSequencerPhase::InterruptModeDispatchInternal {
            mode,
            vector,
            return_address,
            remaining_tstates,
        } => {
            writer.u8(66);
            writer.u8(mode);
            writer.u8(vector);
            writer.u16(return_address);
            writer.u8(remaining_tstates);
        }
        Z80MicroSequencerPhase::InterruptStackPushWrite {
            mode,
            vector,
            stage,
            return_address,
        } => {
            writer.u8(67);
            writer.u8(mode);
            writer.u8(vector);
            writer.u8(stage);
            writer.u16(return_address);
        }
        Z80MicroSequencerPhase::InterruptMode2VectorReadLow { vector_address } => {
            writer.u8(68);
            writer.u16(vector_address);
        }
        Z80MicroSequencerPhase::InterruptMode2VectorReadHigh {
            vector_address,
            low,
        } => {
            writer.u8(69);
            writer.u16(vector_address);
            writer.u8(low);
        }
        Z80MicroSequencerPhase::NmiResponse => writer.u8(61),
        Z80MicroSequencerPhase::NmiStackPushInternal { return_address } => {
            writer.u8(62);
            writer.u16(return_address);
        }
        Z80MicroSequencerPhase::NmiStackPushWrite {
            stage,
            return_address,
        } => {
            writer.u8(63);
            writer.u8(stage);
            writer.u16(return_address);
        }
        Z80MicroSequencerPhase::ImmediateReadRegister { destination } => {
            writer.u8(2);
            writer.u8(destination);
        }
        Z80MicroSequencerPhase::ImmediateReadRegisterPairLow { pair } => {
            writer.u8(10);
            writer.u8(pair);
        }
        Z80MicroSequencerPhase::ImmediateReadRegisterPairHigh { pair, low } => {
            writer.u8(11);
            writer.u8(pair);
            writer.u8(low);
        }
        Z80MicroSequencerPhase::ImmediateReadMemoryWrite => writer.u8(3),
        Z80MicroSequencerPhase::ImmediateReadAlu { operation } => {
            writer.u8(8);
            writer.u8(operation);
        }
        Z80MicroSequencerPhase::MemoryReadRegister { destination } => {
            writer.u8(4);
            writer.u8(destination);
        }
        Z80MicroSequencerPhase::MemoryReadAlu { operation, address } => {
            writer.u8(9);
            writer.u8(operation);
            writer.u16(address);
        }
        Z80MicroSequencerPhase::MemoryReadIncDec { decrement, address } => {
            writer.u8(5);
            writer.u8(u8::from(decrement));
            writer.u16(address);
        }
        Z80MicroSequencerPhase::MemoryIncDecInternal {
            decrement,
            old,
            address,
        } => {
            writer.u8(6);
            writer.u8(u8::from(decrement));
            writer.u8(old);
            writer.u16(address);
        }
        Z80MicroSequencerPhase::BaseInternal {
            opcode,
            remaining_tstates,
        } => {
            writer.u8(35);
            writer.u8(opcode);
            writer.u8(remaining_tstates);
        }
        Z80MicroSequencerPhase::BaseAbsoluteTransfer {
            stage,
            address,
            low,
        } => {
            writer.u8(36);
            writer.u8(stage);
            writer.u16(address);
            writer.u8(low);
        }
        Z80MicroSequencerPhase::BaseRelativeBranchImmediate => writer.u8(37),
        Z80MicroSequencerPhase::BaseRelativeBranchInternal {
            displacement,
            branch_taken,
            remaining_tstates,
        } => {
            writer.u8(38);
            writer.u8(displacement as u8);
            writer.u8(u8::from(branch_taken));
            writer.u8(remaining_tstates);
        }
        Z80MicroSequencerPhase::BaseAbsoluteJump { stage, low } => {
            writer.u8(39);
            writer.u8(stage);
            writer.u8(low);
        }
        Z80MicroSequencerPhase::BaseStackPushInternal { high, low } => {
            writer.u8(40);
            writer.u8(high);
            writer.u8(low);
        }
        Z80MicroSequencerPhase::BaseStackPushWrite { stage, low } => {
            writer.u8(41);
            writer.u8(stage);
            writer.u8(low);
        }
        Z80MicroSequencerPhase::BaseStackPop { stage, low } => {
            writer.u8(42);
            writer.u8(stage);
            writer.u8(low);
        }
        Z80MicroSequencerPhase::BaseCallImmediate { stage, low } => {
            writer.u8(43);
            writer.u8(stage);
            writer.u8(low);
        }
        Z80MicroSequencerPhase::BaseControlPushInternal { target, high, low } => {
            writer.u8(44);
            writer.u16(target);
            writer.u8(high);
            writer.u8(low);
        }
        Z80MicroSequencerPhase::BaseControlPushWrite { stage, target, low } => {
            writer.u8(45);
            writer.u8(stage);
            writer.u16(target);
            writer.u8(low);
        }
        Z80MicroSequencerPhase::BaseReturnConditionInternal { branch_taken } => {
            writer.u8(46);
            writer.u8(u8::from(branch_taken));
        }
        Z80MicroSequencerPhase::BaseReturnPop { stage, low } => {
            writer.u8(47);
            writer.u8(stage);
            writer.u8(low);
        }
        Z80MicroSequencerPhase::BaseExchangeStack {
            stage,
            low,
            high,
            old_hl,
        } => {
            writer.u8(48);
            writer.u8(stage);
            writer.u8(low);
            writer.u8(high);
            writer.u16(old_hl);
        }
        Z80MicroSequencerPhase::BaseExchangeStackInternal {
            stage,
            low,
            high,
            old_hl,
            remaining_tstates,
        } => {
            writer.u8(49);
            writer.u8(stage);
            writer.u8(low);
            writer.u8(high);
            writer.u16(old_hl);
            writer.u8(remaining_tstates);
        }
        Z80MicroSequencerPhase::BaseImmediatePortAddress { opcode } => {
            writer.u8(50);
            writer.u8(opcode);
        }
        Z80MicroSequencerPhase::BaseImmediatePortRead { port_address } => {
            writer.u8(51);
            writer.u16(port_address);
        }
        Z80MicroSequencerPhase::BaseImmediatePortWrite {
            port_address,
            value,
        } => {
            writer.u8(52);
            writer.u16(port_address);
            writer.u8(value);
        }
        Z80MicroSequencerPhase::MemoryWrite { address, value } => {
            writer.u8(7);
            writer.u16(address);
            writer.u8(value);
        }
        Z80MicroSequencerPhase::IndexDisplacementRead => writer.u8(53),
        Z80MicroSequencerPhase::IndexedCbDisplacementRead => writer.u8(57),
        Z80MicroSequencerPhase::IndexedCbOpcodeRead { address } => {
            writer.u8(58);
            writer.u16(address);
        }
        Z80MicroSequencerPhase::IndexedCbMemoryRead { address } => {
            writer.u8(59);
            writer.u16(address);
        }
        Z80MicroSequencerPhase::IndexedCbMemoryWrite {
            address,
            value,
            destination,
        } => {
            writer.u8(60);
            writer.u16(address);
            writer.u8(value);
            writer.u8(u8::from(destination.is_some()));
            if let Some(destination) = destination {
                writer.u8(destination);
            }
        }
        Z80MicroSequencerPhase::IndexDisplacementInternal {
            address,
            remaining_tstates,
        } => {
            writer.u8(54);
            writer.u16(address);
            writer.u8(remaining_tstates);
        }
        Z80MicroSequencerPhase::IndexedImmediateReadMemoryWrite { address } => {
            writer.u8(55);
            writer.u16(address);
        }
        Z80MicroSequencerPhase::IndexedImmediateStoreInternal {
            address,
            value,
            remaining_tstates,
        } => {
            writer.u8(56);
            writer.u16(address);
            writer.u8(value);
            writer.u8(remaining_tstates);
        }
        Z80MicroSequencerPhase::CbMemoryRead { address } => {
            writer.u8(12);
            writer.u16(address);
        }
        Z80MicroSequencerPhase::CbMemoryInternal { address, result } => {
            writer.u8(13);
            writer.u16(address);
            match result {
                None => writer.u8(0),
                Some(value) => {
                    writer.u8(1);
                    writer.u8(value);
                }
            }
        }
        Z80MicroSequencerPhase::CbMemoryWrite { address, value } => {
            writer.u8(14);
            writer.u16(address);
            writer.u8(value);
        }
        Z80MicroSequencerPhase::EdRegisterInternal { opcode } => {
            writer.u8(15);
            writer.u8(opcode);
        }
        Z80MicroSequencerPhase::EdPortRead { register, address } => {
            writer.u8(16);
            writer.u8(register);
            writer.u16(address);
        }
        Z80MicroSequencerPhase::EdPortWrite { address, value } => {
            writer.u8(17);
            writer.u16(address);
            writer.u8(value);
        }
        Z80MicroSequencerPhase::Ed16ArithmeticInternal {
            opcode,
            remaining_tstates,
        } => {
            writer.u8(18);
            writer.u8(opcode);
            writer.u8(remaining_tstates);
        }
        Z80MicroSequencerPhase::EdAbsoluteTransfer {
            stage,
            pair,
            store,
            address,
            low,
            pair_value,
        } => {
            writer.u8(19);
            writer.u8(stage);
            writer.u8(pair);
            writer.u8(u8::from(store));
            writer.u16(address);
            writer.u8(low);
            writer.u16(pair_value);
        }
        Z80MicroSequencerPhase::EdReturn { stage, low } => {
            writer.u8(20);
            writer.u8(stage);
            writer.u8(low);
        }
        Z80MicroSequencerPhase::EdNibbleRotateRead { opcode, address } => {
            writer.u8(21);
            writer.u8(opcode);
            writer.u16(address);
        }
        Z80MicroSequencerPhase::EdNibbleRotateInternal {
            opcode,
            address,
            result_a,
            result_memory,
            remaining_tstates,
        } => {
            writer.u8(22);
            writer.u8(opcode);
            writer.u16(address);
            writer.u8(result_a);
            writer.u8(result_memory);
            writer.u8(remaining_tstates);
        }
        Z80MicroSequencerPhase::EdBlockTransferRead {
            opcode,
            source,
            destination,
        } => {
            writer.u8(23);
            writer.u8(opcode);
            writer.u16(source);
            writer.u16(destination);
        }
        Z80MicroSequencerPhase::EdBlockTransferWrite {
            opcode,
            source,
            destination,
            value,
        } => {
            writer.u8(24);
            writer.u8(opcode);
            writer.u16(source);
            writer.u16(destination);
            writer.u8(value);
        }
        Z80MicroSequencerPhase::EdBlockRepeatInternal {
            opcode,
            remaining_tstates,
        } => {
            writer.u8(25);
            writer.u8(opcode);
            writer.u8(remaining_tstates);
        }
        Z80MicroSequencerPhase::EdBlockCompareRead { opcode, address } => {
            writer.u8(26);
            writer.u8(opcode);
            writer.u16(address);
        }
        Z80MicroSequencerPhase::EdBlockCompareInternal {
            opcode,
            remaining_tstates,
            repeat,
        } => {
            writer.u8(27);
            writer.u8(opcode);
            writer.u8(remaining_tstates);
            writer.u8(u8::from(repeat));
        }
        Z80MicroSequencerPhase::EdBlockCompareRepeatInternal {
            opcode,
            remaining_tstates,
        } => {
            writer.u8(28);
            writer.u8(opcode);
            writer.u8(remaining_tstates);
        }
        Z80MicroSequencerPhase::EdBlockIoInitialInternal { opcode } => {
            writer.u8(29);
            writer.u8(opcode);
        }
        Z80MicroSequencerPhase::EdBlockIoInputPortRead {
            opcode,
            port_address,
            memory_address,
        } => {
            writer.u8(30);
            writer.u8(opcode);
            writer.u16(port_address);
            writer.u16(memory_address);
        }
        Z80MicroSequencerPhase::EdBlockIoInputMemoryWrite {
            opcode,
            port_address,
            memory_address,
            value,
        } => {
            writer.u8(31);
            writer.u8(opcode);
            writer.u16(port_address);
            writer.u16(memory_address);
            writer.u8(value);
        }
        Z80MicroSequencerPhase::EdBlockIoOutputMemoryRead {
            opcode,
            memory_address,
        } => {
            writer.u8(32);
            writer.u8(opcode);
            writer.u16(memory_address);
        }
        Z80MicroSequencerPhase::EdBlockIoOutputPortWrite {
            opcode,
            port_address,
            memory_address,
            value,
        } => {
            writer.u8(33);
            writer.u8(opcode);
            writer.u16(port_address);
            writer.u16(memory_address);
            writer.u8(value);
        }
        Z80MicroSequencerPhase::EdBlockIoRepeatInternal {
            opcode,
            remaining_tstates,
        } => {
            writer.u8(34);
            writer.u8(opcode);
            writer.u8(remaining_tstates);
        }
    }
}

fn read_phase(
    reader: &mut Reader<'_>,
) -> Result<Z80MicroSequencerPhase, Z80MicroSequencerStateCodecError> {
    Ok(match reader.u8()? {
        0 => Z80MicroSequencerPhase::Ready,
        1 => Z80MicroSequencerPhase::OpcodeFetch,
        64 => Z80MicroSequencerPhase::InterruptAcknowledge,
        65 => Z80MicroSequencerPhase::InterruptAcknowledgeComplete {
            vector: reader.u8()?,
            mode: reader.u8()?,
        },
        66 => Z80MicroSequencerPhase::InterruptModeDispatchInternal {
            mode: reader.u8()?,
            vector: reader.u8()?,
            return_address: reader.u16()?,
            remaining_tstates: reader.u8()?,
        },
        67 => Z80MicroSequencerPhase::InterruptStackPushWrite {
            mode: reader.u8()?,
            vector: reader.u8()?,
            stage: reader.u8()?,
            return_address: reader.u16()?,
        },
        68 => Z80MicroSequencerPhase::InterruptMode2VectorReadLow {
            vector_address: reader.u16()?,
        },
        69 => Z80MicroSequencerPhase::InterruptMode2VectorReadHigh {
            vector_address: reader.u16()?,
            low: reader.u8()?,
        },
        61 => Z80MicroSequencerPhase::NmiResponse,
        62 => Z80MicroSequencerPhase::NmiStackPushInternal {
            return_address: reader.u16()?,
        },
        63 => Z80MicroSequencerPhase::NmiStackPushWrite {
            stage: reader.u8()?,
            return_address: reader.u16()?,
        },
        2 => Z80MicroSequencerPhase::ImmediateReadRegister {
            destination: reader.u8()?,
        },
        10 => Z80MicroSequencerPhase::ImmediateReadRegisterPairLow { pair: reader.u8()? },
        11 => Z80MicroSequencerPhase::ImmediateReadRegisterPairHigh {
            pair: reader.u8()?,
            low: reader.u8()?,
        },
        3 => Z80MicroSequencerPhase::ImmediateReadMemoryWrite,
        8 => Z80MicroSequencerPhase::ImmediateReadAlu {
            operation: reader.u8()?,
        },
        4 => Z80MicroSequencerPhase::MemoryReadRegister {
            destination: reader.u8()?,
        },
        9 => Z80MicroSequencerPhase::MemoryReadAlu {
            operation: reader.u8()?,
            address: reader.u16()?,
        },
        5 => Z80MicroSequencerPhase::MemoryReadIncDec {
            decrement: read_bool(reader)?,
            address: reader.u16()?,
        },
        6 => Z80MicroSequencerPhase::MemoryIncDecInternal {
            decrement: read_bool(reader)?,
            old: reader.u8()?,
            address: reader.u16()?,
        },
        35 => Z80MicroSequencerPhase::BaseInternal {
            opcode: reader.u8()?,
            remaining_tstates: reader.u8()?,
        },
        36 => Z80MicroSequencerPhase::BaseAbsoluteTransfer {
            stage: reader.u8()?,
            address: reader.u16()?,
            low: reader.u8()?,
        },
        37 => Z80MicroSequencerPhase::BaseRelativeBranchImmediate,
        38 => Z80MicroSequencerPhase::BaseRelativeBranchInternal {
            displacement: reader.u8()? as i8,
            branch_taken: read_bool(reader)?,
            remaining_tstates: reader.u8()?,
        },
        39 => Z80MicroSequencerPhase::BaseAbsoluteJump {
            stage: reader.u8()?,
            low: reader.u8()?,
        },
        40 => Z80MicroSequencerPhase::BaseStackPushInternal {
            high: reader.u8()?,
            low: reader.u8()?,
        },
        41 => Z80MicroSequencerPhase::BaseStackPushWrite {
            stage: reader.u8()?,
            low: reader.u8()?,
        },
        42 => Z80MicroSequencerPhase::BaseStackPop {
            stage: reader.u8()?,
            low: reader.u8()?,
        },
        43 => Z80MicroSequencerPhase::BaseCallImmediate {
            stage: reader.u8()?,
            low: reader.u8()?,
        },
        44 => Z80MicroSequencerPhase::BaseControlPushInternal {
            target: reader.u16()?,
            high: reader.u8()?,
            low: reader.u8()?,
        },
        45 => Z80MicroSequencerPhase::BaseControlPushWrite {
            stage: reader.u8()?,
            target: reader.u16()?,
            low: reader.u8()?,
        },
        46 => Z80MicroSequencerPhase::BaseReturnConditionInternal {
            branch_taken: read_bool(reader)?,
        },
        47 => Z80MicroSequencerPhase::BaseReturnPop {
            stage: reader.u8()?,
            low: reader.u8()?,
        },
        48 => Z80MicroSequencerPhase::BaseExchangeStack {
            stage: reader.u8()?,
            low: reader.u8()?,
            high: reader.u8()?,
            old_hl: reader.u16()?,
        },
        49 => Z80MicroSequencerPhase::BaseExchangeStackInternal {
            stage: reader.u8()?,
            low: reader.u8()?,
            high: reader.u8()?,
            old_hl: reader.u16()?,
            remaining_tstates: reader.u8()?,
        },
        50 => Z80MicroSequencerPhase::BaseImmediatePortAddress {
            opcode: reader.u8()?,
        },
        51 => Z80MicroSequencerPhase::BaseImmediatePortRead {
            port_address: reader.u16()?,
        },
        52 => Z80MicroSequencerPhase::BaseImmediatePortWrite {
            port_address: reader.u16()?,
            value: reader.u8()?,
        },
        53 => Z80MicroSequencerPhase::IndexDisplacementRead,
        57 => Z80MicroSequencerPhase::IndexedCbDisplacementRead,
        58 => Z80MicroSequencerPhase::IndexedCbOpcodeRead {
            address: reader.u16()?,
        },
        59 => Z80MicroSequencerPhase::IndexedCbMemoryRead {
            address: reader.u16()?,
        },
        60 => {
            let address = reader.u16()?;
            let value = reader.u8()?;
            let destination = if read_bool(reader)? {
                Some(reader.u8()?)
            } else {
                None
            };
            Z80MicroSequencerPhase::IndexedCbMemoryWrite {
                address,
                value,
                destination,
            }
        }
        54 => Z80MicroSequencerPhase::IndexDisplacementInternal {
            address: reader.u16()?,
            remaining_tstates: reader.u8()?,
        },
        55 => Z80MicroSequencerPhase::IndexedImmediateReadMemoryWrite {
            address: reader.u16()?,
        },
        56 => Z80MicroSequencerPhase::IndexedImmediateStoreInternal {
            address: reader.u16()?,
            value: reader.u8()?,
            remaining_tstates: reader.u8()?,
        },
        7 => Z80MicroSequencerPhase::MemoryWrite {
            address: reader.u16()?,
            value: reader.u8()?,
        },
        12 => Z80MicroSequencerPhase::CbMemoryRead {
            address: reader.u16()?,
        },
        13 => {
            let address = reader.u16()?;
            let result = if read_bool(reader)? {
                Some(reader.u8()?)
            } else {
                None
            };
            Z80MicroSequencerPhase::CbMemoryInternal { address, result }
        }
        14 => Z80MicroSequencerPhase::CbMemoryWrite {
            address: reader.u16()?,
            value: reader.u8()?,
        },
        15 => Z80MicroSequencerPhase::EdRegisterInternal {
            opcode: reader.u8()?,
        },
        16 => Z80MicroSequencerPhase::EdPortRead {
            register: reader.u8()?,
            address: reader.u16()?,
        },
        17 => Z80MicroSequencerPhase::EdPortWrite {
            address: reader.u16()?,
            value: reader.u8()?,
        },
        18 => Z80MicroSequencerPhase::Ed16ArithmeticInternal {
            opcode: reader.u8()?,
            remaining_tstates: reader.u8()?,
        },
        19 => Z80MicroSequencerPhase::EdAbsoluteTransfer {
            stage: reader.u8()?,
            pair: reader.u8()?,
            store: read_bool(reader)?,
            address: reader.u16()?,
            low: reader.u8()?,
            pair_value: reader.u16()?,
        },
        20 => Z80MicroSequencerPhase::EdReturn {
            stage: reader.u8()?,
            low: reader.u8()?,
        },
        21 => Z80MicroSequencerPhase::EdNibbleRotateRead {
            opcode: reader.u8()?,
            address: reader.u16()?,
        },
        22 => Z80MicroSequencerPhase::EdNibbleRotateInternal {
            opcode: reader.u8()?,
            address: reader.u16()?,
            result_a: reader.u8()?,
            result_memory: reader.u8()?,
            remaining_tstates: reader.u8()?,
        },
        23 => Z80MicroSequencerPhase::EdBlockTransferRead {
            opcode: reader.u8()?,
            source: reader.u16()?,
            destination: reader.u16()?,
        },
        24 => Z80MicroSequencerPhase::EdBlockTransferWrite {
            opcode: reader.u8()?,
            source: reader.u16()?,
            destination: reader.u16()?,
            value: reader.u8()?,
        },
        25 => Z80MicroSequencerPhase::EdBlockRepeatInternal {
            opcode: reader.u8()?,
            remaining_tstates: reader.u8()?,
        },
        26 => Z80MicroSequencerPhase::EdBlockCompareRead {
            opcode: reader.u8()?,
            address: reader.u16()?,
        },
        27 => Z80MicroSequencerPhase::EdBlockCompareInternal {
            opcode: reader.u8()?,
            remaining_tstates: reader.u8()?,
            repeat: read_bool(reader)?,
        },
        28 => Z80MicroSequencerPhase::EdBlockCompareRepeatInternal {
            opcode: reader.u8()?,
            remaining_tstates: reader.u8()?,
        },
        29 => Z80MicroSequencerPhase::EdBlockIoInitialInternal {
            opcode: reader.u8()?,
        },
        30 => Z80MicroSequencerPhase::EdBlockIoInputPortRead {
            opcode: reader.u8()?,
            port_address: reader.u16()?,
            memory_address: reader.u16()?,
        },
        31 => Z80MicroSequencerPhase::EdBlockIoInputMemoryWrite {
            opcode: reader.u8()?,
            port_address: reader.u16()?,
            memory_address: reader.u16()?,
            value: reader.u8()?,
        },
        32 => Z80MicroSequencerPhase::EdBlockIoOutputMemoryRead {
            opcode: reader.u8()?,
            memory_address: reader.u16()?,
        },
        33 => Z80MicroSequencerPhase::EdBlockIoOutputPortWrite {
            opcode: reader.u8()?,
            port_address: reader.u16()?,
            memory_address: reader.u16()?,
            value: reader.u8()?,
        },
        34 => Z80MicroSequencerPhase::EdBlockIoRepeatInternal {
            opcode: reader.u8()?,
            remaining_tstates: reader.u8()?,
        },
        tag => return Err(Z80MicroSequencerStateCodecError::InvalidTag(tag)),
    })
}

fn write_request(writer: &mut Writer, request: Z80BusRequest) {
    writer.u64(request.id);
    match request.operation {
        Z80BusOperation::OpcodeFetch => writer.u8(0),
        Z80BusOperation::MemoryRead => writer.u8(1),
        Z80BusOperation::InterruptAcknowledge => writer.u8(5),
        Z80BusOperation::MemoryWrite { value } => {
            writer.u8(2);
            writer.u8(value);
        }
        Z80BusOperation::PortRead => writer.u8(3),
        Z80BusOperation::PortWrite { value } => {
            writer.u8(4);
            writer.u8(value);
        }
    }
    writer.u16(request.address);
    writer.u8(request.base_tstates);
}

fn read_request(
    reader: &mut Reader<'_>,
) -> Result<Z80BusRequest, Z80MicroSequencerStateCodecError> {
    let id = reader.u64()?;
    let operation = match reader.u8()? {
        0 => Z80BusOperation::OpcodeFetch,
        1 => Z80BusOperation::MemoryRead,
        5 => Z80BusOperation::InterruptAcknowledge,
        2 => Z80BusOperation::MemoryWrite {
            value: reader.u8()?,
        },
        3 => Z80BusOperation::PortRead,
        4 => Z80BusOperation::PortWrite {
            value: reader.u8()?,
        },
        tag => return Err(Z80MicroSequencerStateCodecError::InvalidTag(tag)),
    };
    Ok(Z80BusRequest {
        id,
        operation,
        address: reader.u16()?,
        base_tstates: reader.u8()?,
    })
}

fn read_bool(reader: &mut Reader<'_>) -> Result<bool, Z80MicroSequencerStateCodecError> {
    match reader.u8()? {
        0 => Ok(false),
        1 => Ok(true),
        tag => Err(Z80MicroSequencerStateCodecError::InvalidTag(tag)),
    }
}

#[derive(Default)]
struct Writer {
    bytes: Vec<u8>,
}

impl Writer {
    fn u8(&mut self, value: u8) {
        self.bytes.push(value);
    }

    fn u16(&mut self, value: u16) {
        self.bytes.extend_from_slice(&value.to_be_bytes());
    }

    fn u32(&mut self, value: u32) {
        self.bytes.extend_from_slice(&value.to_be_bytes());
    }

    fn u64(&mut self, value: u64) {
        self.bytes.extend_from_slice(&value.to_be_bytes());
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn take(&mut self, len: usize) -> Result<&'a [u8], Z80MicroSequencerStateCodecError> {
        let end = self
            .offset
            .checked_add(len)
            .ok_or(Z80MicroSequencerStateCodecError::Truncated)?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or(Z80MicroSequencerStateCodecError::Truncated)?;
        self.offset = end;
        Ok(value)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], Z80MicroSequencerStateCodecError> {
        self.take(N)?
            .try_into()
            .map_err(|_| Z80MicroSequencerStateCodecError::Truncated)
    }

    fn u8(&mut self) -> Result<u8, Z80MicroSequencerStateCodecError> {
        Ok(self.array::<1>()?[0])
    }

    fn u16(&mut self) -> Result<u16, Z80MicroSequencerStateCodecError> {
        Ok(u16::from_be_bytes(self.array()?))
    }

    fn u32(&mut self) -> Result<u32, Z80MicroSequencerStateCodecError> {
        Ok(u32::from_be_bytes(self.array()?))
    }

    fn u64(&mut self) -> Result<u64, Z80MicroSequencerStateCodecError> {
        Ok(u64::from_be_bytes(self.array()?))
    }

    fn finished(&self) -> bool {
        self.offset == self.bytes.len()
    }
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = !0_u32;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            let mask = 0_u32.wrapping_sub(crc & 1);
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

#[cfg(test)]
mod tests {
    use super::{Reader, Z80_MICRO_SEQUENCER_STATE_MAGIC, Z80MicroSequencerStateCodecError};
    use crate::{Z80MicroSequencer, Z80MicroSequencerState};

    #[test]
    fn complete_architectural_dto_round_trips_through_versioned_codec() {
        let mut cpu = Z80MicroSequencer::default();
        let mut state = cpu.export_architectural_state();
        state.alternate = crate::Z80AlternateRegisters {
            a: 0x21,
            f: 0x5A,
            b: 0x43,
            c: 0x65,
            d: 0x87,
            e: 0xA9,
            h: 0xCB,
            l: 0xED,
        };
        state.ix = 0x1357;
        state.iy = 0x2468;
        state.i = 0x81;
        state.r = 0xFE;
        state.iff1 = true;
        state.iff2 = false;
        state.interrupt_mode = 2;
        state.wz = 0xABCD;
        state.ei_delay = 1;
        state.nmi_pending = true;
        state.int_pending = Some(0xCF);
        state.q = 0x28;
        state.q_previous = 0xD7;
        state.p = true;
        state.halted = true;
        cpu.import_architectural_state(state).unwrap();

        let encoded = cpu.export_state().encode().unwrap();
        let decoded = Z80MicroSequencerState::decode(&encoded).unwrap();
        let restored = Z80MicroSequencer::restore_state(decoded).unwrap();
        assert_eq!(restored.export_architectural_state(), state);
    }

    #[test]
    fn pending_fetch_snapshot_round_trips_with_wait_and_allocator() {
        let mut cpu = Z80MicroSequencer::default();
        let request = match cpu.begin_instruction().unwrap() {
            crate::Z80MicroStep::BusRequest(request) => request,
            other => panic!("expected request, got {other:?}"),
        };
        cpu.insert_wait_tstate().unwrap();
        let state = cpu.export_state();

        let encoded = state.encode().unwrap();
        let decoded = Z80MicroSequencerState::decode(&encoded).unwrap();
        assert_eq!(decoded, state);
        let mut restored = Z80MicroSequencer::restore_state(decoded).unwrap();
        assert_eq!(
            restored.complete_bus_cycle(request.id, Some(0)).unwrap(),
            crate::Z80MicroStep::InstructionRetired {
                opcode: 0,
                tstates: 5
            }
        );
    }

    #[test]
    fn internal_cycle_safe_point_round_trips_through_binary_codec() {
        let mut cpu = Z80MicroSequencer::default();
        let fetch = match cpu.begin_instruction().unwrap() {
            crate::Z80MicroStep::BusRequest(request) => request,
            other => panic!("expected opcode fetch, got {other:?}"),
        };
        let read = match cpu.complete_bus_cycle(fetch.id, Some(0x34)).unwrap() {
            crate::Z80MicroStep::BusRequest(request) => request,
            other => panic!("expected memory read, got {other:?}"),
        };
        assert_eq!(
            cpu.complete_bus_cycle(read.id, Some(0x7F)).unwrap(),
            crate::Z80MicroStep::InternalCycle { tstates: 1 }
        );

        let state = cpu.export_state();
        assert_eq!((state.mcycle_index, state.tstate_index), (1, 3));
        let decoded = Z80MicroSequencerState::decode(&state.encode().unwrap()).unwrap();
        assert_eq!(decoded, state);
        let mut restored = Z80MicroSequencer::restore_state(decoded).unwrap();
        let write = match restored.complete_internal_cycle().unwrap() {
            crate::Z80MicroStep::BusRequest(request) => request,
            other => panic!("expected memory write, got {other:?}"),
        };
        assert_eq!(
            write.operation,
            crate::Z80BusOperation::MemoryWrite { value: 0x80 }
        );
        assert_eq!(
            (
                restored.export_state().mcycle_index,
                restored.export_state().tstate_index
            ),
            (2, 0)
        );
    }

    #[test]
    fn pending_memory_write_snapshot_round_trips_captured_address_and_payload() {
        let mut cpu = Z80MicroSequencer::default();
        let fetch = match cpu.begin_instruction().unwrap() {
            crate::Z80MicroStep::BusRequest(request) => request,
            other => panic!("expected opcode fetch, got {other:?}"),
        };
        let write = match cpu.complete_bus_cycle(fetch.id, Some(0x77)).unwrap() {
            crate::Z80MicroStep::BusRequest(request) => request,
            other => panic!("expected memory write, got {other:?}"),
        };
        let state = cpu.export_state();
        let encoded = state.encode().unwrap();
        let decoded = Z80MicroSequencerState::decode(&encoded).unwrap();
        assert_eq!(decoded, state);

        let restored = Z80MicroSequencer::restore_state(decoded).unwrap();
        assert_eq!(restored.pending_request(), Some(write));
    }

    #[test]
    fn codec_rejects_pending_write_payload_mismatch_with_valid_checksum() {
        let mut cpu = Z80MicroSequencer::default();
        let fetch = match cpu.begin_instruction().unwrap() {
            crate::Z80MicroStep::BusRequest(request) => request,
            other => panic!("expected opcode fetch, got {other:?}"),
        };
        let operand = match cpu.complete_bus_cycle(fetch.id, Some(0x36)).unwrap() {
            crate::Z80MicroStep::BusRequest(request) => request,
            other => panic!("expected immediate read, got {other:?}"),
        };
        let write = match cpu.complete_bus_cycle(operand.id, Some(0xA5)).unwrap() {
            crate::Z80MicroStep::BusRequest(request) => request,
            other => panic!("expected memory write, got {other:?}"),
        };
        assert_eq!(
            write.operation,
            crate::Z80BusOperation::MemoryWrite { value: 0xA5 }
        );

        let mut encoded = cpu.export_state().encode().unwrap();
        const FRAME_HEADER_LEN: usize = 14;
        let body_len = u32::from_be_bytes(
            encoded[10..FRAME_HEADER_LEN]
                .try_into()
                .expect("encoded frame contains body length"),
        ) as usize;
        let checksum_offset = FRAME_HEADER_LEN + body_len;

        // Walk the canonical body to the pending operation payload instead of
        // relying on a magic absolute offset in the serialized frame.
        let payload_offset = {
            let mut reader = Reader::new(&encoded[FRAME_HEADER_LEN..checksum_offset]);
            reader.take(25).unwrap(); // registers, refresh and M/T counters
            reader.take(2).unwrap(); // opcode option tag and opcode
            reader.take(1).unwrap(); // prefix-state tag
            super::read_phase(&mut reader).unwrap();
            reader.u64().unwrap(); // next request ID
            assert_eq!(reader.u8().unwrap(), 1); // pending request present
            reader.u64().unwrap(); // pending request ID
            assert_eq!(reader.u8().unwrap(), 2); // MemoryWrite operation tag
            reader.offset
        };
        encoded[FRAME_HEADER_LEN + payload_offset] = 0x5A;

        let checksum = super::crc32(&encoded[FRAME_HEADER_LEN..checksum_offset]);
        encoded[checksum_offset..checksum_offset + 4].copy_from_slice(&checksum.to_be_bytes());

        assert_eq!(
            Z80MicroSequencerState::decode(&encoded),
            Err(Z80MicroSequencerStateCodecError::InvalidState)
        );
    }

    #[test]
    fn codec_rejects_corruption_truncation_and_trailing_data() {
        let state = Z80MicroSequencer::default().export_state();
        let encoded = state.encode().unwrap();

        let mut corrupted = encoded.clone();
        let last = corrupted.len() - 1;
        corrupted[last] ^= 1;
        assert_eq!(
            Z80MicroSequencerState::decode(&corrupted),
            Err(Z80MicroSequencerStateCodecError::ChecksumMismatch)
        );
        assert_eq!(
            Z80MicroSequencerState::decode(&encoded[..encoded.len() - 1]),
            Err(Z80MicroSequencerStateCodecError::Truncated)
        );

        let mut trailing = encoded;
        trailing.push(0);
        assert_eq!(
            Z80MicroSequencerState::decode(&trailing),
            Err(Z80MicroSequencerStateCodecError::TrailingData)
        );
        assert_eq!(Z80_MICRO_SEQUENCER_STATE_MAGIC, *b"Z80SEQ\0\0");
    }
    #[test]
    fn codec_rejects_hl_write_address_mismatch_with_valid_checksum() {
        let mut cpu = Z80MicroSequencer::default();
        cpu.registers.set_hl(0xC010);
        let fetch = match cpu.begin_instruction().unwrap() {
            crate::Z80MicroStep::BusRequest(request) => request,
            other => panic!("expected opcode fetch, got {other:?}"),
        };
        let operand = match cpu.complete_bus_cycle(fetch.id, Some(0x36)).unwrap() {
            crate::Z80MicroStep::BusRequest(request) => request,
            other => panic!("expected immediate read, got {other:?}"),
        };
        cpu.complete_bus_cycle(operand.id, Some(0xA5)).unwrap();

        let mut encoded = cpu.export_state().encode().unwrap();
        const FRAME_HEADER_LEN: usize = 14;
        let body_len = u32::from_be_bytes(
            encoded[10..FRAME_HEADER_LEN]
                .try_into()
                .expect("encoded frame contains body length"),
        ) as usize;
        let checksum_offset = FRAME_HEADER_LEN + body_len;
        let (phase_address_offset, request_address_offset) = {
            let mut reader = Reader::new(&encoded[FRAME_HEADER_LEN..checksum_offset]);
            reader.take(25).unwrap();
            reader.take(2).unwrap();
            reader.take(1).unwrap(); // prefix-state tag
            let phase_address_offset = reader.offset + 1;
            let phase = super::read_phase(&mut reader).unwrap();
            assert!(matches!(
                phase,
                crate::Z80MicroSequencerPhase::MemoryWrite { .. }
            ));
            reader.u64().unwrap();
            assert_eq!(reader.u8().unwrap(), 1);
            reader.u64().unwrap();
            assert_eq!(reader.u8().unwrap(), 2);
            reader.u8().unwrap();
            let request_address_offset = reader.offset;
            (phase_address_offset, request_address_offset)
        };
        let tampered_address = 0xC011_u16.to_be_bytes();
        encoded
            [FRAME_HEADER_LEN + phase_address_offset..FRAME_HEADER_LEN + phase_address_offset + 2]
            .copy_from_slice(&tampered_address);
        encoded[FRAME_HEADER_LEN + request_address_offset
            ..FRAME_HEADER_LEN + request_address_offset + 2]
            .copy_from_slice(&tampered_address);

        let checksum = super::crc32(&encoded[FRAME_HEADER_LEN..checksum_offset]);
        encoded[checksum_offset..checksum_offset + 4].copy_from_slice(&checksum.to_be_bytes());
        assert_eq!(
            Z80MicroSequencerState::decode(&encoded),
            Err(Z80MicroSequencerStateCodecError::InvalidState)
        );
    }

    #[test]
    fn pending_ld_register_pair_operand_phases_round_trip_through_snapshot_codec() {
        for opcode in [0x01, 0x11, 0x21, 0x31] {
            let mut cpu = Z80MicroSequencer::default();
            let fetch = match cpu.begin_instruction().unwrap() {
                crate::Z80MicroStep::BusRequest(request) => request,
                other => panic!("expected fetch request, got {other:?}"),
            };
            let low_read = match cpu.complete_bus_cycle(fetch.id, Some(opcode)).unwrap() {
                crate::Z80MicroStep::BusRequest(request) => request,
                other => panic!("expected low-byte read, got {other:?}"),
            };
            cpu.insert_wait_tstate().unwrap();
            let low_state = cpu.export_state();
            let low_encoded = low_state.encode().unwrap();
            let low_decoded = crate::Z80MicroSequencerState::decode(&low_encoded).unwrap();
            let mut restored = Z80MicroSequencer::restore_state(low_decoded).unwrap();
            assert_eq!(restored.pending_request(), Some(low_read));
            assert_eq!(restored.export_state(), low_state);

            let high_read = match restored
                .complete_bus_cycle(low_read.id, Some(0x34))
                .unwrap()
            {
                crate::Z80MicroStep::BusRequest(request) => request,
                other => panic!("expected high-byte read, got {other:?}"),
            };
            restored.insert_wait_tstate().unwrap();
            let high_state = restored.export_state();
            let high_encoded = high_state.encode().unwrap();
            let high_decoded = crate::Z80MicroSequencerState::decode(&high_encoded).unwrap();
            let mut restored = Z80MicroSequencer::restore_state(high_decoded).unwrap();
            assert_eq!(restored.pending_request(), Some(high_read));
            assert_eq!(restored.export_state(), high_state);
            assert_eq!(
                restored
                    .complete_bus_cycle(high_read.id, Some(0x12))
                    .unwrap(),
                crate::Z80MicroStep::InstructionRetired {
                    opcode,
                    tstates: 12,
                }
            );
        }
    }

    #[test]
    fn pending_alu_operand_phases_round_trip_through_snapshot_codec() {
        for opcode in [0xC6, 0x86] {
            let mut cpu = Z80MicroSequencer::default();
            cpu.registers.set_hl(0xC123);
            let fetch = match cpu.begin_instruction().unwrap() {
                crate::Z80MicroStep::BusRequest(request) => request,
                other => panic!("expected fetch request, got {other:?}"),
            };
            let read = match cpu.complete_bus_cycle(fetch.id, Some(opcode)).unwrap() {
                crate::Z80MicroStep::BusRequest(request) => request,
                other => panic!("expected operand request, got {other:?}"),
            };
            cpu.insert_wait_tstate().unwrap();
            let encoded = cpu.export_state().encode().unwrap();
            let decoded = crate::Z80MicroSequencerState::decode(&encoded).unwrap();
            let restored = Z80MicroSequencer::restore_state(decoded).unwrap();
            assert_eq!(restored.pending_request(), Some(read));
            assert_eq!(restored.export_state(), cpu.export_state());
        }
    }

    #[test]
    fn cb_memory_read_internal_and_write_phases_round_trip_through_binary_codec() {
        let mut cpu = Z80MicroSequencer::default();
        cpu.registers.set_hl(0xC123);
        cpu.registers.pc = 0x0100;
        let first = match cpu.begin_instruction().unwrap() {
            crate::Z80MicroStep::BusRequest(request) => request,
            other => panic!("expected first M1, got {other:?}"),
        };
        let second = match cpu.complete_bus_cycle(first.id, Some(0xCB)).unwrap() {
            crate::Z80MicroStep::BusRequest(request) => request,
            other => panic!("expected second M1, got {other:?}"),
        };
        let read = match cpu.complete_bus_cycle(second.id, Some(0x06)).unwrap() {
            crate::Z80MicroStep::BusRequest(request) => request,
            other => panic!("expected CB memory read, got {other:?}"),
        };
        let read_state = cpu.export_state();
        assert_eq!(
            Z80MicroSequencerState::decode(&read_state.encode().unwrap()).unwrap(),
            read_state
        );

        assert_eq!(
            cpu.complete_bus_cycle(read.id, Some(0x81)).unwrap(),
            crate::Z80MicroStep::InternalCycle { tstates: 1 }
        );
        let internal_state = cpu.export_state();
        assert_eq!(
            Z80MicroSequencerState::decode(&internal_state.encode().unwrap()).unwrap(),
            internal_state
        );

        let write = match cpu.complete_internal_cycle().unwrap() {
            crate::Z80MicroStep::BusRequest(request) => request,
            other => panic!("expected CB memory write, got {other:?}"),
        };
        let write_state = cpu.export_state();
        assert_eq!(
            Z80MicroSequencerState::decode(&write_state.encode().unwrap()).unwrap(),
            write_state
        );
        let restored = Z80MicroSequencer::restore_state(
            Z80MicroSequencerState::decode(&write_state.encode().unwrap()).unwrap(),
        )
        .unwrap();
        assert_eq!(restored.pending_request(), Some(write));
    }

    #[test]
    fn cb_bit_memory_internal_phase_round_trips_without_write_payload() {
        let mut cpu = Z80MicroSequencer::default();
        cpu.registers.set_hl(0xC123);
        cpu.registers.pc = 0x0100;
        let first = match cpu.begin_instruction().unwrap() {
            crate::Z80MicroStep::BusRequest(request) => request,
            other => panic!("expected first M1, got {other:?}"),
        };
        let second = match cpu.complete_bus_cycle(first.id, Some(0xCB)).unwrap() {
            crate::Z80MicroStep::BusRequest(request) => request,
            other => panic!("expected second M1, got {other:?}"),
        };
        let read = match cpu.complete_bus_cycle(second.id, Some(0x46)).unwrap() {
            crate::Z80MicroStep::BusRequest(request) => request,
            other => panic!("expected BIT memory read, got {other:?}"),
        };
        assert_eq!(
            cpu.complete_bus_cycle(read.id, Some(0x80)).unwrap(),
            crate::Z80MicroStep::InternalCycle { tstates: 1 }
        );
        let state = cpu.export_state();
        assert_eq!(
            Z80MicroSequencerState::decode(&state.encode().unwrap()).unwrap(),
            state
        );
        let mut restored = Z80MicroSequencer::restore_state(
            Z80MicroSequencerState::decode(&state.encode().unwrap()).unwrap(),
        )
        .unwrap();
        assert_eq!(
            restored.complete_internal_cycle().unwrap(),
            crate::Z80MicroStep::InstructionRetired {
                opcode: 0x46,
                tstates: 12
            }
        );
    }

    #[test]
    fn ed_register_internal_phase_round_trips_through_binary_codec() {
        let mut cpu = Z80MicroSequencer::default();
        cpu.registers.a = 0x5A;
        cpu.registers.pc = 0x0100;
        let first = match cpu.begin_instruction().unwrap() {
            crate::Z80MicroStep::BusRequest(request) => request,
            other => panic!("expected ED prefix M1, got {other:?}"),
        };
        let second = match cpu.complete_bus_cycle(first.id, Some(0xED)).unwrap() {
            crate::Z80MicroStep::BusRequest(request) => request,
            other => panic!("expected ED opcode M1, got {other:?}"),
        };
        assert_eq!(
            cpu.complete_bus_cycle(second.id, Some(0x47)).unwrap(),
            crate::Z80MicroStep::InternalCycle { tstates: 1 }
        );

        let state = cpu.export_state();
        assert_eq!(
            Z80MicroSequencerState::decode(&state.encode().unwrap()).unwrap(),
            state
        );
        let mut restored = Z80MicroSequencer::restore_state(
            Z80MicroSequencerState::decode(&state.encode().unwrap()).unwrap(),
        )
        .unwrap();
        assert_eq!(
            restored.complete_internal_cycle().unwrap(),
            crate::Z80MicroStep::InstructionRetired {
                opcode: 0x47,
                tstates: 9
            }
        );
        assert_eq!(restored.i, 0x5A);
    }
}

#[cfg(test)]
mod base_internal_snapshot_tests {
    use crate::{Z80MicroSequencer, Z80MicroSequencerState, Z80MicroStep};

    fn round_trip(cpu: &Z80MicroSequencer) -> Z80MicroSequencer {
        let state = cpu.export_state();
        let encoded = state.encode().unwrap();
        let decoded = Z80MicroSequencerState::decode(&encoded).unwrap();
        assert_eq!(decoded, state);
        Z80MicroSequencer::restore_state(decoded).unwrap()
    }

    #[test]
    fn interrupt_acknowledge_pending_and_vector_latch_round_trip_through_codec() {
        let mut cpu = Z80MicroSequencer::default();
        cpu.iff1 = true;
        cpu.iff2 = true;
        cpu.set_int_line(Some(0xCF));

        let request = match cpu.begin_instruction().unwrap() {
            crate::Z80MicroStep::BusRequest(request) => request,
            other => panic!("expected INT acknowledge request, got {other:?}"),
        };
        cpu.insert_wait_tstates(2).unwrap();
        cpu = round_trip(&cpu);
        assert_eq!(cpu.pending_request(), Some(request));

        assert_eq!(
            cpu.complete_bus_cycle(request.id, Some(0xCF)).unwrap(),
            crate::Z80MicroStep::InterruptAcknowledge {
                vector: 0xCF,
                tstates: 7,
            }
        );
        cpu = round_trip(&cpu);
        assert_eq!(
            cpu.export_state().phase,
            crate::Z80MicroSequencerPhase::InterruptAcknowledgeComplete { vector: 0xCF, mode: 0 }
        );
    }

    #[test]
    fn inc_pair_internal_phase_round_trips_at_each_safe_point() {
        let mut cpu = Z80MicroSequencer::default();
        let fetch = match cpu.begin_instruction().unwrap() {
            Z80MicroStep::BusRequest(request) => request,
            other => panic!("expected opcode fetch, got {other:?}"),
        };
        assert_eq!(
            cpu.complete_bus_cycle(fetch.id, Some(0x03)).unwrap(),
            Z80MicroStep::InternalCycle { tstates: 1 }
        );

        cpu = round_trip(&cpu);
        assert_eq!(
            cpu.complete_internal_cycle().unwrap(),
            Z80MicroStep::InternalCycle { tstates: 1 }
        );
        cpu = round_trip(&cpu);
        assert_eq!(
            cpu.complete_internal_cycle().unwrap(),
            Z80MicroStep::InstructionRetired {
                opcode: 0x03,
                tstates: 6
            }
        );
    }

    #[test]
    fn add_hl_pair_internal_mcycles_round_trip_at_each_safe_point() {
        let mut cpu = Z80MicroSequencer::default();
        let fetch = match cpu.begin_instruction().unwrap() {
            Z80MicroStep::BusRequest(request) => request,
            other => panic!("expected opcode fetch, got {other:?}"),
        };
        assert_eq!(
            cpu.complete_bus_cycle(fetch.id, Some(0x09)).unwrap(),
            Z80MicroStep::InternalCycle { tstates: 1 }
        );

        cpu = round_trip(&cpu);
        for tstate in 0..7 {
            let step = cpu.complete_internal_cycle().unwrap();
            if tstate == 6 {
                assert_eq!(
                    step,
                    Z80MicroStep::InstructionRetired {
                        opcode: 0x09,
                        tstates: 11
                    }
                );
            } else {
                assert_eq!(step, Z80MicroStep::InternalCycle { tstates: 1 });
                cpu = round_trip(&cpu);
            }
        }
    }
}

#[cfg(test)]
mod base_absolute_transfer_snapshot_tests {
    use crate::{Z80BusOperation, Z80MicroSequencer, Z80MicroSequencerState, Z80MicroStep};

    fn round_trip(cpu: &Z80MicroSequencer) -> Z80MicroSequencer {
        let state = cpu.export_state();
        let encoded = state.encode().unwrap();
        let decoded = Z80MicroSequencerState::decode(&encoded).unwrap();
        assert_eq!(decoded, state);
        Z80MicroSequencer::restore_state(decoded).unwrap()
    }

    #[test]
    fn absolute_transfer_pending_requests_round_trip_at_each_stage() {
        for opcode in [0x22, 0x2A, 0x32, 0x3A] {
            let mut cpu = Z80MicroSequencer::default();
            cpu.registers.pc = 0x1000;
            cpu.registers.set_hl(0xBEEF);
            cpu.registers.a = 0xA5;
            let mut step = cpu.begin_instruction().unwrap();
            let mut memory = [0_u8; 0x1_0000];
            memory[0x1000] = opcode;
            memory[0x1001] = 0x00;
            memory[0x1002] = 0x20;
            memory[0x2000] = 0x34;
            memory[0x2001] = 0x12;

            for _ in 0..8 {
                step = match step {
                    Z80MicroStep::BusRequest(request) => {
                        cpu = round_trip(&cpu);
                        let data = match request.operation {
                            Z80BusOperation::OpcodeFetch | Z80BusOperation::MemoryRead => {
                                Some(memory[usize::from(request.address)])
                            }
                            Z80BusOperation::MemoryWrite { value } => {
                                memory[usize::from(request.address)] = value;
                                None
                            }
                            operation => panic!("unexpected operation: {operation:?}"),
                        };
                        cpu.complete_bus_cycle(request.id, data).unwrap()
                    }
                    Z80MicroStep::InterruptAcknowledge { .. } => {
                        panic!("unexpected INT acknowledge in absolute transfer test")
                    }
                    Z80MicroStep::InstructionRetired { .. }
                    | Z80MicroStep::InterruptServiced { .. } => break,
                    Z80MicroStep::InternalCycle { .. } => {
                        panic!("absolute transfer has no internal cycle")
                    }
                };
            }
            assert!(matches!(step, Z80MicroStep::InstructionRetired { .. }));
        }
    }
}

#[cfg(test)]
mod base_relative_branch_snapshot_tests {
    use crate::{Z80MicroSequencer, Z80MicroSequencerPhase, Z80MicroSequencerState, Z80MicroStep};

    fn round_trip(cpu: &Z80MicroSequencer) -> Z80MicroSequencer {
        let state = cpu.export_state();
        let encoded = state.encode().unwrap();
        let decoded = Z80MicroSequencerState::decode(&encoded).unwrap();
        assert_eq!(decoded, state);
        Z80MicroSequencer::restore_state(decoded).unwrap()
    }

    fn start_branch(opcode: u8, b: u8, displacement: u8) -> (Z80MicroSequencer, Z80MicroStep) {
        let mut cpu = Z80MicroSequencer::default();
        cpu.registers.pc = 0x1000;
        cpu.registers.b = b;
        let fetch = match cpu.begin_instruction().unwrap() {
            Z80MicroStep::BusRequest(request) => request,
            other => panic!("expected opcode fetch, got {other:?}"),
        };
        let immediate = cpu.complete_bus_cycle(fetch.id, Some(opcode)).unwrap();
        let request = match immediate {
            Z80MicroStep::BusRequest(request) => request,
            other => panic!("expected displacement read, got {other:?}"),
        };
        let mut restored = round_trip(&cpu);
        let step = restored
            .complete_bus_cycle(request.id, Some(displacement))
            .unwrap();
        (restored, step)
    }

    #[test]
    fn relative_branch_internal_safe_points_round_trip_through_codec() {
        for (opcode, b, displacement, expected_tstates) in [
            (0x18, 0x00, 0x02, 12),
            (0x10, 0x02, 0xFE, 13),
            (0x10, 0x01, 0xFE, 8),
        ] {
            let (mut cpu, mut step) = start_branch(opcode, b, displacement);
            loop {
                match step {
                    Z80MicroStep::InternalCycle { .. } => {
                        cpu = round_trip(&cpu);
                        step = cpu.complete_internal_cycle().unwrap();
                    }
                    Z80MicroStep::InstructionRetired { tstates, .. } => {
                        assert_eq!(tstates, expected_tstates);
                        break;
                    }
                    other => panic!("unexpected relative branch continuation: {other:?}"),
                }
            }
        }
    }

    #[test]
    fn relative_branch_restore_rejects_inconsistent_continuations() {
        let (cpu, step) = start_branch(0x18, 0x00, 0x02);
        assert_eq!(step, Z80MicroStep::InternalCycle { tstates: 1 });
        let mut invalid = cpu.export_state();
        invalid.phase = Z80MicroSequencerPhase::BaseRelativeBranchInternal {
            displacement: 2,
            branch_taken: false,
            remaining_tstates: 5,
        };
        assert!(Z80MicroSequencer::restore_state(invalid).is_err());

        let mut invalid = cpu.export_state();
        invalid.phase = Z80MicroSequencerPhase::BaseRelativeBranchInternal {
            displacement: 2,
            branch_taken: true,
            remaining_tstates: 0,
        };
        assert!(Z80MicroSequencer::restore_state(invalid).is_err());
    }
}

#[cfg(test)]
mod base_absolute_jump_snapshot_tests {
    use crate::{Z80MicroSequencer, Z80MicroSequencerPhase, Z80MicroSequencerState, Z80MicroStep};

    fn round_trip(cpu: &Z80MicroSequencer) -> Z80MicroSequencer {
        let state = cpu.export_state();
        let encoded = state.encode().unwrap();
        let decoded = Z80MicroSequencerState::decode(&encoded).unwrap();
        assert_eq!(decoded, state);
        Z80MicroSequencer::restore_state(decoded).unwrap()
    }

    #[test]
    fn absolute_jump_immediate_fetch_stages_round_trip_through_codec() {
        let mut cpu = Z80MicroSequencer::default();
        cpu.registers.pc = 0x1000;
        let fetch = match cpu.begin_instruction().unwrap() {
            Z80MicroStep::BusRequest(request) => request,
            other => panic!("expected opcode fetch, got {other:?}"),
        };
        let low_request = match cpu.complete_bus_cycle(fetch.id, Some(0xC3)).unwrap() {
            Z80MicroStep::BusRequest(request) => request,
            other => panic!("expected low-byte read, got {other:?}"),
        };
        cpu = round_trip(&cpu);
        let high_request = match cpu.complete_bus_cycle(low_request.id, Some(0x34)).unwrap() {
            Z80MicroStep::BusRequest(request) => request,
            other => panic!("expected high-byte read, got {other:?}"),
        };
        cpu = round_trip(&cpu);
        assert_eq!(
            cpu.complete_bus_cycle(high_request.id, Some(0x12)).unwrap(),
            Z80MicroStep::InstructionRetired {
                opcode: 0xC3,
                tstates: 10,
            }
        );
        assert_eq!(cpu.registers.pc, 0x1234);
        assert_eq!(cpu.wz, 0x1234);
    }

    #[test]
    fn absolute_jump_restore_rejects_invalid_stage_and_opcode() {
        let mut cpu = Z80MicroSequencer::default();
        cpu.registers.pc = 0x1000;
        let fetch = match cpu.begin_instruction().unwrap() {
            Z80MicroStep::BusRequest(request) => request,
            other => panic!("expected opcode fetch, got {other:?}"),
        };
        let _ = cpu.complete_bus_cycle(fetch.id, Some(0xC3)).unwrap();
        let mut invalid = cpu.export_state();
        invalid.phase = Z80MicroSequencerPhase::BaseAbsoluteJump {
            stage: 2,
            low: 0x34,
        };
        assert!(Z80MicroSequencer::restore_state(invalid).is_err());

        let mut invalid = cpu.export_state();
        invalid.opcode = Some(0x00);
        assert!(Z80MicroSequencer::restore_state(invalid).is_err());
    }
}

#[cfg(test)]
mod base_stack_snapshot_tests {
    use crate::{Z80MicroSequencer, Z80MicroSequencerPhase, Z80MicroSequencerState, Z80MicroStep};

    fn round_trip(cpu: &Z80MicroSequencer) -> Z80MicroSequencer {
        let state = cpu.export_state();
        let encoded = state.encode().unwrap();
        let decoded = Z80MicroSequencerState::decode(&encoded).unwrap();
        assert_eq!(decoded, state);
        Z80MicroSequencer::restore_state(decoded).unwrap()
    }

    #[test]
    fn push_internal_and_both_write_boundaries_round_trip() {
        let mut cpu = Z80MicroSequencer::default();
        cpu.registers.pc = 0x1000;
        cpu.registers.sp = 0x2000;
        cpu.registers.set_bc(0x1234);
        let fetch = match cpu.begin_instruction().unwrap() {
            Z80MicroStep::BusRequest(request) => request,
            other => panic!("expected opcode fetch, got {other:?}"),
        };
        assert_eq!(
            cpu.complete_bus_cycle(fetch.id, Some(0xC5)).unwrap(),
            Z80MicroStep::InternalCycle { tstates: 1 }
        );
        cpu = round_trip(&cpu);
        let high_write = match cpu.complete_internal_cycle().unwrap() {
            Z80MicroStep::BusRequest(request) => request,
            other => panic!("expected high-byte write, got {other:?}"),
        };
        assert_eq!(cpu.registers.sp, 0x1FFF);
        cpu = round_trip(&cpu);
        let low_write = match cpu.complete_bus_cycle(high_write.id, None).unwrap() {
            Z80MicroStep::BusRequest(request) => request,
            other => panic!("expected low-byte write, got {other:?}"),
        };
        assert_eq!(cpu.registers.sp, 0x1FFE);
        cpu = round_trip(&cpu);
        assert_eq!(
            cpu.complete_bus_cycle(low_write.id, None).unwrap(),
            Z80MicroStep::InstructionRetired {
                opcode: 0xC5,
                tstates: 11,
            }
        );
    }

    #[test]
    fn pop_low_and_high_read_boundaries_round_trip() {
        let mut cpu = Z80MicroSequencer::default();
        cpu.registers.pc = 0x1000;
        cpu.registers.sp = 0x2000;
        let fetch = match cpu.begin_instruction().unwrap() {
            Z80MicroStep::BusRequest(request) => request,
            other => panic!("expected opcode fetch, got {other:?}"),
        };
        let low_read = match cpu.complete_bus_cycle(fetch.id, Some(0xF1)).unwrap() {
            Z80MicroStep::BusRequest(request) => request,
            other => panic!("expected low-byte read, got {other:?}"),
        };
        cpu = round_trip(&cpu);
        let high_read = match cpu.complete_bus_cycle(low_read.id, Some(0x34)).unwrap() {
            Z80MicroStep::BusRequest(request) => request,
            other => panic!("expected high-byte read, got {other:?}"),
        };
        assert_eq!(cpu.registers.sp, 0x2001);
        cpu = round_trip(&cpu);
        assert_eq!(
            cpu.complete_bus_cycle(high_read.id, Some(0x12)).unwrap(),
            Z80MicroStep::InstructionRetired {
                opcode: 0xF1,
                tstates: 10,
            }
        );
        assert_eq!(cpu.registers.sp, 0x2002);
        assert_eq!(cpu.registers.a, 0x12);
        assert_eq!(cpu.registers.f, 0x34);
    }

    #[test]
    fn stack_restore_rejects_invalid_stage_and_wrong_push_opcode() {
        let mut cpu = Z80MicroSequencer::default();
        cpu.registers.pc = 0x1000;
        cpu.registers.sp = 0x2000;
        let fetch = match cpu.begin_instruction().unwrap() {
            Z80MicroStep::BusRequest(request) => request,
            other => panic!("expected opcode fetch, got {other:?}"),
        };
        let _ = cpu.complete_bus_cycle(fetch.id, Some(0xC5)).unwrap();

        let mut invalid = cpu.export_state();
        invalid.phase = Z80MicroSequencerPhase::BaseStackPushInternal { high: 0, low: 0 };
        invalid.opcode = Some(0xC1);
        assert!(Z80MicroSequencer::restore_state(invalid).is_err());

        let mut invalid = cpu.export_state();
        invalid.phase = Z80MicroSequencerPhase::BaseStackPop { stage: 2, low: 0 };
        assert!(Z80MicroSequencer::restore_state(invalid).is_err());
    }
}

#[cfg(test)]
mod base_call_rst_snapshot_tests {
    use crate::{Z80MicroSequencer, Z80MicroSequencerPhase, Z80MicroSequencerState, Z80MicroStep};

    fn round_trip(cpu: &Z80MicroSequencer) -> Z80MicroSequencer {
        let state = cpu.export_state();
        let encoded = state.encode().unwrap();
        let decoded = Z80MicroSequencerState::decode(&encoded).unwrap();
        assert_eq!(decoded, state);
        Z80MicroSequencer::restore_state(decoded).unwrap()
    }

    #[test]
    fn call_immediate_internal_and_stack_write_boundaries_round_trip() {
        let mut cpu = Z80MicroSequencer::default();
        cpu.registers.pc = 0x1000;
        cpu.registers.sp = 0x2000;
        let fetch = match cpu.begin_instruction().unwrap() {
            Z80MicroStep::BusRequest(request) => request,
            other => panic!("expected opcode fetch, got {other:?}"),
        };
        let low_read = match cpu.complete_bus_cycle(fetch.id, Some(0xCD)).unwrap() {
            Z80MicroStep::BusRequest(request) => request,
            other => panic!("expected CALL low-byte read, got {other:?}"),
        };
        cpu = round_trip(&cpu);
        let high_read = match cpu.complete_bus_cycle(low_read.id, Some(0x00)).unwrap() {
            Z80MicroStep::BusRequest(request) => request,
            other => panic!("expected CALL high-byte read, got {other:?}"),
        };
        cpu = round_trip(&cpu);
        assert_eq!(
            cpu.complete_bus_cycle(high_read.id, Some(0x20)).unwrap(),
            Z80MicroStep::InternalCycle { tstates: 1 }
        );
        cpu = round_trip(&cpu);
        let high_write = match cpu.complete_internal_cycle().unwrap() {
            Z80MicroStep::BusRequest(request) => request,
            other => panic!("expected CALL high stack write, got {other:?}"),
        };
        cpu = round_trip(&cpu);
        let low_write = match cpu.complete_bus_cycle(high_write.id, None).unwrap() {
            Z80MicroStep::BusRequest(request) => request,
            other => panic!("expected CALL low stack write, got {other:?}"),
        };
        cpu = round_trip(&cpu);
        assert_eq!(
            cpu.complete_bus_cycle(low_write.id, None).unwrap(),
            Z80MicroStep::InstructionRetired {
                opcode: 0xCD,
                tstates: 17,
            }
        );
        assert_eq!(cpu.registers.pc, 0x2000);
        assert_eq!(cpu.wz, 0x2000);
        assert_eq!(cpu.registers.sp, 0x1FFE);
    }

    #[test]
    fn rst_internal_and_stack_write_boundaries_round_trip() {
        let mut cpu = Z80MicroSequencer::default();
        cpu.registers.pc = 0x1000;
        cpu.registers.sp = 0x2000;
        let fetch = match cpu.begin_instruction().unwrap() {
            Z80MicroStep::BusRequest(request) => request,
            other => panic!("expected opcode fetch, got {other:?}"),
        };
        assert_eq!(
            cpu.complete_bus_cycle(fetch.id, Some(0xFF)).unwrap(),
            Z80MicroStep::InternalCycle { tstates: 1 }
        );
        cpu = round_trip(&cpu);
        let high_write = match cpu.complete_internal_cycle().unwrap() {
            Z80MicroStep::BusRequest(request) => request,
            other => panic!("expected RST high stack write, got {other:?}"),
        };
        cpu = round_trip(&cpu);
        let low_write = match cpu.complete_bus_cycle(high_write.id, None).unwrap() {
            Z80MicroStep::BusRequest(request) => request,
            other => panic!("expected RST low stack write, got {other:?}"),
        };
        cpu = round_trip(&cpu);
        assert_eq!(
            cpu.complete_bus_cycle(low_write.id, None).unwrap(),
            Z80MicroStep::InstructionRetired {
                opcode: 0xFF,
                tstates: 11,
            }
        );
        assert_eq!(cpu.registers.pc, 0x0038);
        assert_eq!(cpu.wz, 0x0038);
    }

    #[test]
    fn control_push_restore_rejects_wrong_target() {
        let mut cpu = Z80MicroSequencer::default();
        cpu.registers.pc = 0x1000;
        let fetch = match cpu.begin_instruction().unwrap() {
            Z80MicroStep::BusRequest(request) => request,
            other => panic!("expected opcode fetch, got {other:?}"),
        };
        let low = match cpu.complete_bus_cycle(fetch.id, Some(0xCD)).unwrap() {
            Z80MicroStep::BusRequest(request) => request,
            other => panic!("expected CALL low-byte read, got {other:?}"),
        };
        let high = match cpu.complete_bus_cycle(low.id, Some(0x00)).unwrap() {
            Z80MicroStep::BusRequest(request) => request,
            other => panic!("expected CALL high-byte read, got {other:?}"),
        };
        let _ = cpu.complete_bus_cycle(high.id, Some(0x20)).unwrap();
        let mut invalid = cpu.export_state();
        invalid.phase = Z80MicroSequencerPhase::BaseControlPushInternal {
            target: 0x1234,
            high: 0x10,
            low: 0x03,
        };
        assert!(Z80MicroSequencer::restore_state(invalid).is_err());
    }
}

#[cfg(test)]
mod base_return_snapshot_tests {
    use crate::{Z80MicroSequencer, Z80MicroSequencerState, Z80MicroStep};

    fn round_trip(cpu: &Z80MicroSequencer) -> Z80MicroSequencer {
        let state = cpu.export_state();
        let encoded = state.encode().unwrap();
        let decoded = Z80MicroSequencerState::decode(&encoded).unwrap();
        assert_eq!(decoded, state);
        Z80MicroSequencer::restore_state(decoded).unwrap()
    }

    #[test]
    fn conditional_return_internal_and_stack_read_boundaries_round_trip() {
        let mut cpu = Z80MicroSequencer::default();
        cpu.registers.pc = 0x1000;
        cpu.registers.sp = 0x2000;
        cpu.registers.f = 0x00;
        let fetch = match cpu.begin_instruction().unwrap() {
            Z80MicroStep::BusRequest(request) => request,
            other => panic!("expected opcode fetch, got {other:?}"),
        };
        assert_eq!(
            cpu.complete_bus_cycle(fetch.id, Some(0xC0)).unwrap(),
            Z80MicroStep::InternalCycle { tstates: 1 }
        );
        cpu = round_trip(&cpu);
        let low_read = match cpu.complete_internal_cycle().unwrap() {
            Z80MicroStep::BusRequest(request) => request,
            other => panic!("expected return low-byte read, got {other:?}"),
        };
        cpu = round_trip(&cpu);
        let high_read = match cpu.complete_bus_cycle(low_read.id, Some(0x34)).unwrap() {
            Z80MicroStep::BusRequest(request) => request,
            other => panic!("expected return high-byte read, got {other:?}"),
        };
        cpu = round_trip(&cpu);
        assert_eq!(
            cpu.complete_bus_cycle(high_read.id, Some(0x12)).unwrap(),
            Z80MicroStep::InstructionRetired {
                opcode: 0xC0,
                tstates: 11,
            }
        );
        assert_eq!(cpu.registers.pc, 0x1234);
        assert_eq!(cpu.wz, 0x1234);
        assert_eq!(cpu.registers.sp, 0x2002);
    }

    #[test]
    fn not_taken_conditional_return_round_trips_and_retires_after_five_tstates() {
        let mut cpu = Z80MicroSequencer::default();
        cpu.registers.pc = 0x1000;
        cpu.registers.f = 0x40;
        let fetch = match cpu.begin_instruction().unwrap() {
            Z80MicroStep::BusRequest(request) => request,
            other => panic!("expected opcode fetch, got {other:?}"),
        };
        assert_eq!(
            cpu.complete_bus_cycle(fetch.id, Some(0xC0)).unwrap(),
            Z80MicroStep::InternalCycle { tstates: 1 }
        );
        cpu = round_trip(&cpu);
        assert_eq!(
            cpu.complete_internal_cycle().unwrap(),
            Z80MicroStep::InstructionRetired {
                opcode: 0xC0,
                tstates: 5,
            }
        );
        assert_eq!(cpu.registers.pc, 0x1001);
    }

    #[test]
    fn return_restore_rejects_condition_mismatch() {
        let mut cpu = Z80MicroSequencer::default();
        cpu.registers.pc = 0x1000;
        cpu.registers.sp = 0x2000;
        cpu.registers.f = 0x00;
        let fetch = match cpu.begin_instruction().unwrap() {
            Z80MicroStep::BusRequest(request) => request,
            other => panic!("expected opcode fetch, got {other:?}"),
        };
        let _ = cpu.complete_bus_cycle(fetch.id, Some(0xC0)).unwrap();
        let _ = cpu.complete_internal_cycle().unwrap();
        let mut invalid = cpu.export_state();
        invalid.registers.f = 0x40;
        assert!(Z80MicroSequencer::restore_state(invalid).is_err());
    }
}

#[cfg(test)]
mod base_exchange_stack_snapshot_tests {
    use crate::{Z80MicroSequencer, Z80MicroSequencerPhase, Z80MicroSequencerState, Z80MicroStep};

    fn round_trip(cpu: &Z80MicroSequencer) -> Z80MicroSequencer {
        let state = cpu.export_state();
        let encoded = state.encode().unwrap();
        let decoded = Z80MicroSequencerState::decode(&encoded).unwrap();
        assert_eq!(decoded, state);
        Z80MicroSequencer::restore_state(decoded).unwrap()
    }

    #[test]
    fn ex_sp_hl_all_bus_and_internal_boundaries_round_trip() {
        let mut cpu = Z80MicroSequencer::default();
        cpu.registers.pc = 0x1000;
        cpu.registers.sp = 0x2000;
        cpu.registers.set_hl(0xABCD);
        let fetch = match cpu.begin_instruction().unwrap() {
            Z80MicroStep::BusRequest(request) => request,
            other => panic!("expected opcode fetch, got {other:?}"),
        };
        let low_read = match cpu.complete_bus_cycle(fetch.id, Some(0xE3)).unwrap() {
            Z80MicroStep::BusRequest(request) => request,
            other => panic!("expected stack low-byte read, got {other:?}"),
        };
        cpu = round_trip(&cpu);
        let high_read = match cpu.complete_bus_cycle(low_read.id, Some(0x34)).unwrap() {
            Z80MicroStep::BusRequest(request) => request,
            other => panic!("expected stack high-byte read, got {other:?}"),
        };
        cpu = round_trip(&cpu);
        assert_eq!(
            cpu.complete_bus_cycle(high_read.id, Some(0x12)).unwrap(),
            Z80MicroStep::InternalCycle { tstates: 1 }
        );
        cpu = round_trip(&cpu);
        let low_write = match cpu.complete_internal_cycle().unwrap() {
            Z80MicroStep::BusRequest(request) => request,
            other => panic!("expected low-byte write, got {other:?}"),
        };
        cpu = round_trip(&cpu);
        let high_write = match cpu.complete_bus_cycle(low_write.id, None).unwrap() {
            Z80MicroStep::BusRequest(request) => request,
            other => panic!("expected high-byte write, got {other:?}"),
        };
        cpu = round_trip(&cpu);
        assert_eq!(
            cpu.complete_bus_cycle(high_write.id, None).unwrap(),
            Z80MicroStep::InternalCycle { tstates: 1 }
        );
        cpu = round_trip(&cpu);
        assert_eq!(
            cpu.complete_internal_cycle().unwrap(),
            Z80MicroStep::InternalCycle { tstates: 1 }
        );
        cpu = round_trip(&cpu);
        assert_eq!(
            cpu.complete_internal_cycle().unwrap(),
            Z80MicroStep::InstructionRetired {
                opcode: 0xE3,
                tstates: 19,
            }
        );
        assert_eq!(cpu.registers.hl(), 0x1234);
        assert_eq!(cpu.wz, 0x1234);
        assert_eq!(cpu.registers.sp, 0x2000);
    }

    #[test]
    fn ex_sp_hl_restore_rejects_invalid_internal_duration() {
        let mut cpu = Z80MicroSequencer::default();
        cpu.registers.pc = 0x1000;
        cpu.registers.sp = 0x2000;
        cpu.registers.set_hl(0xABCD);
        let fetch = match cpu.begin_instruction().unwrap() {
            Z80MicroStep::BusRequest(request) => request,
            other => panic!("expected opcode fetch, got {other:?}"),
        };
        let low = match cpu.complete_bus_cycle(fetch.id, Some(0xE3)).unwrap() {
            Z80MicroStep::BusRequest(request) => request,
            other => panic!("expected low-byte read, got {other:?}"),
        };
        let high = match cpu.complete_bus_cycle(low.id, Some(0x34)).unwrap() {
            Z80MicroStep::BusRequest(request) => request,
            other => panic!("expected high-byte read, got {other:?}"),
        };
        let _ = cpu.complete_bus_cycle(high.id, Some(0x12)).unwrap();
        let mut invalid = cpu.export_state();
        invalid.phase = Z80MicroSequencerPhase::BaseExchangeStackInternal {
            stage: 0,
            low: 0x34,
            high: 0x12,
            old_hl: 0xABCD,
            remaining_tstates: 2,
        };
        assert!(Z80MicroSequencer::restore_state(invalid).is_err());
    }
}

#[cfg(test)]
mod base_immediate_io_snapshot_tests {
    use crate::{Z80MicroSequencer, Z80MicroSequencerPhase, Z80MicroSequencerState, Z80MicroStep};

    fn round_trip(cpu: &Z80MicroSequencer) -> Z80MicroSequencer {
        let state = cpu.export_state();
        let encoded = state.encode().unwrap();
        let decoded = Z80MicroSequencerState::decode(&encoded).unwrap();
        assert_eq!(decoded, state);
        Z80MicroSequencer::restore_state(decoded).unwrap()
    }

    #[test]
    fn in_a_n_address_and_port_read_boundaries_round_trip() {
        let mut cpu = Z80MicroSequencer::default();
        cpu.registers.pc = 0x1000;
        cpu.registers.a = 0x12;
        let fetch = match cpu.begin_instruction().unwrap() {
            Z80MicroStep::BusRequest(request) => request,
            other => panic!("expected opcode fetch, got {other:?}"),
        };
        let immediate = match cpu.complete_bus_cycle(fetch.id, Some(0xDB)).unwrap() {
            Z80MicroStep::BusRequest(request) => request,
            other => panic!("expected IN immediate read, got {other:?}"),
        };
        cpu = round_trip(&cpu);
        let port = match cpu.complete_bus_cycle(immediate.id, Some(0xDC)).unwrap() {
            Z80MicroStep::BusRequest(request) => request,
            other => panic!("expected IN port read, got {other:?}"),
        };
        assert_eq!(port.address, 0x12DC);
        cpu = round_trip(&cpu);
        assert_eq!(
            cpu.complete_bus_cycle(port.id, Some(0xFE)).unwrap(),
            Z80MicroStep::InstructionRetired {
                opcode: 0xDB,
                tstates: 11,
            }
        );
        assert_eq!(cpu.registers.a, 0xFE);
    }

    #[test]
    fn out_n_a_address_and_port_write_boundaries_round_trip() {
        let mut cpu = Z80MicroSequencer::default();
        cpu.registers.pc = 0x1000;
        cpu.registers.a = 0x5A;
        let fetch = match cpu.begin_instruction().unwrap() {
            Z80MicroStep::BusRequest(request) => request,
            other => panic!("expected opcode fetch, got {other:?}"),
        };
        let immediate = match cpu.complete_bus_cycle(fetch.id, Some(0xD3)).unwrap() {
            Z80MicroStep::BusRequest(request) => request,
            other => panic!("expected OUT immediate read, got {other:?}"),
        };
        cpu = round_trip(&cpu);
        let port = match cpu.complete_bus_cycle(immediate.id, Some(0x3F)).unwrap() {
            Z80MicroStep::BusRequest(request) => request,
            other => panic!("expected OUT port write, got {other:?}"),
        };
        assert_eq!(port.address, 0x5A3F);
        assert_eq!(
            port.operation,
            crate::Z80BusOperation::PortWrite { value: 0x5A }
        );
        cpu = round_trip(&cpu);
        assert_eq!(
            cpu.complete_bus_cycle(port.id, None).unwrap(),
            Z80MicroStep::InstructionRetired {
                opcode: 0xD3,
                tstates: 11,
            }
        );
    }

    #[test]
    fn immediate_port_restore_rejects_coherently_mismatched_high_address_byte() {
        let mut input = Z80MicroSequencer::default();
        input.registers.pc = 0x1000;
        input.registers.a = 0x12;
        let fetch = match input.begin_instruction().unwrap() {
            Z80MicroStep::BusRequest(request) => request,
            other => panic!("expected opcode fetch, got {other:?}"),
        };
        let immediate = match input.complete_bus_cycle(fetch.id, Some(0xDB)).unwrap() {
            Z80MicroStep::BusRequest(request) => request,
            other => panic!("expected IN immediate read, got {other:?}"),
        };
        let _ = input.complete_bus_cycle(immediate.id, Some(0xDC)).unwrap();
        let mut invalid_input = input.export_state();
        invalid_input.phase = Z80MicroSequencerPhase::BaseImmediatePortRead {
            port_address: 0x13DC,
        };
        invalid_input
            .handshake
            .pending_request
            .as_mut()
            .unwrap()
            .address = 0x13DC;
        assert!(Z80MicroSequencer::restore_state(invalid_input).is_err());

        let mut output = Z80MicroSequencer::default();
        output.registers.pc = 0x1000;
        output.registers.a = 0x5A;
        let fetch = match output.begin_instruction().unwrap() {
            Z80MicroStep::BusRequest(request) => request,
            other => panic!("expected opcode fetch, got {other:?}"),
        };
        let immediate = match output.complete_bus_cycle(fetch.id, Some(0xD3)).unwrap() {
            Z80MicroStep::BusRequest(request) => request,
            other => panic!("expected OUT immediate read, got {other:?}"),
        };
        let _ = output.complete_bus_cycle(immediate.id, Some(0x3F)).unwrap();
        let mut invalid_output = output.export_state();
        invalid_output.phase = Z80MicroSequencerPhase::BaseImmediatePortWrite {
            port_address: 0x5B3F,
            value: 0x5A,
        };
        invalid_output
            .handshake
            .pending_request
            .as_mut()
            .unwrap()
            .address = 0x5B3F;
        assert!(Z80MicroSequencer::restore_state(invalid_output).is_err());
    }

    #[test]
    fn immediate_port_restore_rejects_mismatched_address() {
        let mut cpu = Z80MicroSequencer::default();
        cpu.registers.pc = 0x1000;
        cpu.registers.a = 0x12;
        let fetch = match cpu.begin_instruction().unwrap() {
            Z80MicroStep::BusRequest(request) => request,
            other => panic!("expected opcode fetch, got {other:?}"),
        };
        let immediate = match cpu.complete_bus_cycle(fetch.id, Some(0xDB)).unwrap() {
            Z80MicroStep::BusRequest(request) => request,
            other => panic!("expected IN immediate read, got {other:?}"),
        };
        let _ = cpu.complete_bus_cycle(immediate.id, Some(0xDC)).unwrap();
        let mut invalid = cpu.export_state();
        invalid.phase = Z80MicroSequencerPhase::BaseImmediatePortRead {
            port_address: 0x13DC,
        };
        assert!(Z80MicroSequencer::restore_state(invalid).is_err());
    }
}
