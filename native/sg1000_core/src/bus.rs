//! Shared Z80 bus contract with the SG-1000 memory adapter.
//!
//! The trait and diagnostic records are defined by `sega-z80-cpu` so every
//! console uses the same CPU-facing contract. This adapter remains machine
//! specific and deliberately does not advance emulated time.

pub use sega_z80_cpu::bus::*;

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
