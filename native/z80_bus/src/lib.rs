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

    /// Observe an instrumented timing event. The default is allocation-free and
    /// has no effect on emulation; this hook is not yet emitted by every opcode.
    fn observe_timing_event(&mut self, _event: Z80BusEvent) {}
}
