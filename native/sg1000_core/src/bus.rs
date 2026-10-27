//! SG-1000 memory adapter for the shared Python-independent Z80 bus contract.

pub use sega_z80_bus::CpuBus;

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
