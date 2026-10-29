//! Hardware-neutral memory and I/O contract for the Z80 CPU.
//!
//! A machine owns the bus and routes accesses to its own memory map and
//! devices. The contract contains no frontend dependencies and performs no
//! timing claims: a CPU implementation must model bus timing separately.

/// One logical M-cycle-level event emitted by an instrumented Z80 instruction.
///
/// This is an observational trace record, not a pin-level T-state sample and not
/// a scheduler callback. The initial implementation emits these events only for
/// unprefixed ED block-transfer and block-compare instructions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Z80BusEventKind {
    OpcodeFetch,
    MemoryRead,
    MemoryWrite,
    Internal,
}

/// A deterministic logical bus event with instruction-relative T-state timing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Z80BusEvent {
    pub instruction_pc: u16,
    pub tstate_offset: u32,
    pub kind: Z80BusEventKind,
    pub address: Option<u16>,
    pub data: Option<u8>,
    pub duration_tstates: u8,
}

impl Z80BusEvent {
    /// T-state offset immediately after this logical M-cycle event.
    #[inline]
    pub const fn end_tstate(&self) -> u32 {
        self.tstate_offset + self.duration_tstates as u32
    }
}

/// Phase label inferred for an instrumented Z80 T-state sample.
///
/// The pinned SingleStepTests corpus exposes address/data and RD/WR/MREQ/IORQ
/// samples; it does not expose every physical Z80 control pin. Refresh is a
/// phase label inferred from the documented M1 sequence, not a sampled RFSH pin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Z80TStatePhase {
    OpcodeFetch,
    Refresh,
    MemoryRead,
    MemoryWrite,
    Internal,
}

/// One deterministic address/data/control sample at an instruction-relative T-state.
///
/// Boolean control fields mean asserted, independent of the electrical active-low
/// representation. RD/WR/MREQ/IORQ follow the pinned corpus's simplified access profile, not a physical half-clock waveform. This initial model covers only selected ED block instructions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Z80TStateSample {
    pub instruction_pc: u16,
    pub tstate_offset: u32,
    pub phase: Z80TStatePhase,
    pub address: Option<u16>,
    pub data: Option<u8>,
    pub rd: bool,
    pub wr: bool,
    pub mreq: bool,
    pub iorq: bool,
}

/// Compact, complete architectural snapshot emitted around one CPU step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Z80CpuSnapshot {
    pub af: u16,
    pub bc: u16,
    pub de: u16,
    pub hl: u16,
    pub af_alt: u16,
    pub bc_alt: u16,
    pub de_alt: u16,
    pub hl_alt: u16,
    pub ix: u16,
    pub iy: u16,
    pub pc: u16,
    pub sp: u16,
    pub i: u8,
    pub r: u8,
    pub iff1: bool,
    pub iff2: bool,
    pub interrupt_mode: u8,
    pub wz: u16,
    pub ei_delay: u8,
    pub nmi_pending: bool,
    pub int_pending: Option<u8>,
    pub q: u8,
    pub p: bool,
    pub halted: bool,
    pub total_tstates: u64,
}

/// Outcome captured by the instruction diagnostic stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Z80InstructionOutcome {
    Completed,
    UnsupportedOpcode { opcode: u8, address: u16 },
}

/// Deterministic per-step record. It describes architectural state, not pin-level
/// timing; detailed bus timing remains available only for explicitly instrumented
/// instruction families.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Z80InstructionTrace {
    /// One-based step sequence since reset, including HALT and interrupt entry.
    pub instruction_index: u64,
    /// Opcode/prefix bytes fetched by the decoder, excluding immediate operands.
    pub opcode_bytes: [u8; 8],
    pub opcode_len: u8,
    pub opcode_bytes_truncated: bool,
    /// Instruction-stream bytes, including immediate operands and displacements.
    pub instruction_bytes: [u8; 8],
    pub instruction_len: u8,
    pub instruction_bytes_truncated: bool,
    /// Aggregate nominal T-states, or None when execution returned an error.
    pub tstates: Option<u32>,
    pub outcome: Z80InstructionOutcome,
    pub before: Z80CpuSnapshot,
    pub after: Z80CpuSnapshot,
}

impl Z80InstructionTrace {
    /// Write one deterministic JSON Lines record without adding a runtime dependency.
    pub fn write_json_line(&self, out: &mut impl core::fmt::Write) -> core::fmt::Result {
        write!(
            out,
            "{{\"instruction_index\":{},\"opcode_bytes\":[",
            self.instruction_index
        )?;
        for (index, byte) in self.opcode_bytes[..usize::from(self.opcode_len)]
            .iter()
            .enumerate()
        {
            if index != 0 {
                out.write_str(",")?;
            }
            write!(out, "\"{byte:02X}\"")?;
        }
        let outcome = match self.outcome {
            Z80InstructionOutcome::Completed => "\"completed\"".to_owned(),
            Z80InstructionOutcome::UnsupportedOpcode { opcode, address } => {
                format!(
                    "{{\"kind\":\"unsupported_opcode\",\"opcode\":\"{opcode:02X}\",\"address\":{address}}}"
                )
            }
        };
        write!(
            out,
            "],\"opcode_bytes_truncated\":{},\"instruction_bytes\":[",
            self.opcode_bytes_truncated
        )?;
        for (index, byte) in self.instruction_bytes[..usize::from(self.instruction_len)]
            .iter()
            .enumerate()
        {
            if index != 0 {
                out.write_str(",")?;
            }
            write!(out, "\"{byte:02X}\"")?;
        }
        write!(
            out,
            "],\"instruction_bytes_truncated\":{},\"tstates\":{},\"outcome\":{},\"before\":",
            self.instruction_bytes_truncated,
            self.tstates
                .map_or_else(|| "null".to_owned(), |v| v.to_string()),
            outcome
        )?;
        write_snapshot_json(out, self.before)?;
        out.write_str(",\"after\":")?;
        write_snapshot_json(out, self.after)?;
        out.write_str("}")
    }
}

fn write_snapshot_json(
    out: &mut impl core::fmt::Write,
    state: Z80CpuSnapshot,
) -> core::fmt::Result {
    write!(
        out,
        "{{\"af\":{},\"bc\":{},\"de\":{},\"hl\":{},\"af_alt\":{},\"bc_alt\":{},\"de_alt\":{},\"hl_alt\":{},\"ix\":{},\"iy\":{},\"pc\":{},\"sp\":{},\"i\":{},\"r\":{},\"iff1\":{},\"iff2\":{},\"interrupt_mode\":{},\"wz\":{},\"ei_delay\":{},\"nmi_pending\":{},\"int_pending\":{},\"q\":{},\"p\":{},\"halted\":{},\"total_tstates\":{}}}",
        state.af,
        state.bc,
        state.de,
        state.hl,
        state.af_alt,
        state.bc_alt,
        state.de_alt,
        state.hl_alt,
        state.ix,
        state.iy,
        state.pc,
        state.sp,
        state.i,
        state.r,
        state.iff1,
        state.iff2,
        state.interrupt_mode,
        state.wz,
        state.ei_delay,
        state.nmi_pending,
        state
            .int_pending
            .map_or_else(|| "null".to_owned(), |v| v.to_string()),
        state.q,
        state.p,
        state.halted,
        state.total_tstates
    )
}

/// Memory and port access available to a Z80 implementation.
///
/// Default port behavior represents an unimplemented/open bus placeholder,
/// not a claim that every target machine reads 0xFF from every unmapped port.
/// Each machine must override port handling when its hardware contract is known.
pub trait CpuBus {
    fn read_memory(&mut self, address: u16) -> u8;
    fn write_memory(&mut self, address: u16, value: u8);

    fn read_port(&mut self, _port: u16) -> u8 {
        0xFF
    }

    fn write_port(&mut self, _port: u16, _value: u8) {}

    /// Opt in to timing trace generation. False keeps the normal path allocation-free.
    fn timing_trace_enabled(&self) -> bool {
        false
    }

    /// Opt in to deterministic per-instruction architectural diagnostics.
    /// Disabled by default; the CPU does not allocate or format logs itself.
    fn instruction_trace_enabled(&self) -> bool {
        false
    }

    /// Receive one record after each outer CPU step when diagnostics are enabled.
    fn observe_instruction_trace(&mut self, _trace: Z80InstructionTrace) {}

    /// Observe an instrumented timing event. The default is allocation-free and
    /// has no effect on emulation; this hook is not yet emitted by every opcode.
    fn observe_timing_event(&mut self, _event: Z80BusEvent) {}

    /// Observe a T-state sample. The default is a no-op and does not advance the machine scheduler.
    fn observe_tstate_sample(&mut self, _sample: Z80TStateSample) {}
}

/// SG-1000 memory adapter for the local Z80 bus contract.
impl CpuBus for crate::Sg1000Memory {
    #[inline]
    fn read_memory(&mut self, address: u16) -> u8 {
        self.read(address)
    }

    #[inline]
    fn write_memory(&mut self, address: u16, value: u8) {
        self.write(address, value);
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CpuBus, Z80BusEvent, Z80BusEventKind, Z80CpuSnapshot, Z80InstructionOutcome,
        Z80InstructionTrace,
    };

    struct PlainBus;

    impl CpuBus for PlainBus {
        fn read_memory(&mut self, _address: u16) -> u8 {
            0
        }

        fn write_memory(&mut self, _address: u16, _value: u8) {}
    }

    #[test]
    fn instruction_trace_json_is_single_line_and_keeps_missing_values_null() {
        let state = Z80CpuSnapshot {
            af: 0x1234,
            bc: 0,
            de: 0,
            hl: 0,
            af_alt: 0,
            bc_alt: 0,
            de_alt: 0,
            hl_alt: 0,
            ix: 0,
            iy: 0,
            pc: 0xC002,
            sp: 0xFFFF,
            i: 0,
            r: 1,
            iff1: false,
            iff2: false,
            interrupt_mode: 1,
            wz: 0,
            ei_delay: 0,
            nmi_pending: false,
            int_pending: None,
            q: 0,
            p: false,
            halted: false,
            total_tstates: 7,
        };
        let trace = Z80InstructionTrace {
            instruction_index: 1,
            opcode_bytes: [0x3E, 0, 0, 0, 0, 0, 0, 0],
            opcode_len: 1,
            opcode_bytes_truncated: false,
            instruction_bytes: [0x3E, 0x42, 0, 0, 0, 0, 0, 0],
            instruction_len: 2,
            instruction_bytes_truncated: false,
            tstates: Some(7),
            outcome: Z80InstructionOutcome::Completed,
            before: Z80CpuSnapshot {
                pc: 0xC000,
                total_tstates: 0,
                ..state
            },
            after: state,
        };
        let mut line = String::new();
        trace.write_json_line(&mut line).expect("format JSONL");
        assert!(line.starts_with("{\"instruction_index\":1,"));
        assert!(line.contains("\"opcode_bytes\":[\"3E\"]"));
        assert!(line.contains("\"int_pending\":null"));
        assert!(!line.contains('\n'));
    }

    #[test]
    fn timing_trace_is_disabled_by_default() {
        assert!(!PlainBus.timing_trace_enabled());
    }

    #[test]
    fn event_end_tstate_matches_the_next_mcycle_boundary() {
        let fetch = Z80BusEvent {
            instruction_pc: 0xC000,
            tstate_offset: 0,
            kind: Z80BusEventKind::OpcodeFetch,
            address: Some(0xC000),
            data: Some(0xED),
            duration_tstates: 4,
        };
        let next_fetch = Z80BusEvent {
            instruction_pc: 0xC000,
            tstate_offset: 4,
            kind: Z80BusEventKind::OpcodeFetch,
            address: Some(0xC001),
            data: Some(0xA0),
            duration_tstates: 4,
        };

        assert_eq!(fetch.end_tstate(), next_fetch.tstate_offset);
        assert_eq!(next_fetch.end_tstate(), 8);
    }
}

/// SG-1000 memory adapter for the local Z80 bus contract.
impl CpuBus for crate::Sg1000Memory {
    #[inline]
    fn read_memory(&mut self, address: u16) -> u8 {
        self.read(address)
    }

    #[inline]
    fn write_memory(&mut self, address: u16, value: u8) {
        self.write(address, value);
    }
}
