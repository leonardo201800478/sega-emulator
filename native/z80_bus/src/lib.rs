//! Hardware-neutral memory and I/O contract for the Z80 CPU.
//!
//! A machine owns the bus and routes accesses to its own memory map and
//! devices. The contract contains no frontend dependencies and performs no
//! timing claims: a CPU implementation must model bus timing separately.

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
}
