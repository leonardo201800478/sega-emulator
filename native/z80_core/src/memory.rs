//! PyO3 adapter for the Python-independent SG-1000 memory map.

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use sega_sg1000_core::{MemoryConfig, MemoryError, Sg1000Memory};

#[pyclass(module = "sega_z80_core")]
pub(crate) struct NativeMemoryBus {
    memory: Sg1000Memory,
}

#[pymethods]
impl NativeMemoryBus {
    #[new]
    #[pyo3(signature = (cartridge_rom, internal_ram_size=1024, cartridge_ram_size=0, cartridge_ram_start=0, cartridge_ram_end=-1))]
    fn new(
        cartridge_rom: Vec<u8>,
        internal_ram_size: usize,
        cartridge_ram_size: usize,
        cartridge_ram_start: u16,
        cartridge_ram_end: i32,
    ) -> PyResult<Self> {
        Self::new_native(
            cartridge_rom,
            internal_ram_size,
            cartridge_ram_size,
            cartridge_ram_start,
            cartridge_ram_end,
        )
    }

    fn read(&self, address: u16) -> u8 {
        self.read_byte(address)
    }

    fn write(&mut self, address: u16, value: u8) {
        self.write_byte(address, value);
    }

    #[getter]
    fn internal_ram_size(&self) -> usize {
        self.memory.internal_ram().len()
    }

    #[getter]
    fn cartridge_ram_size(&self) -> usize {
        self.memory.cartridge_ram().len()
    }
}

impl NativeMemoryBus {
    pub(crate) fn new_native(
        cartridge_rom: Vec<u8>,
        internal_ram_size: usize,
        cartridge_ram_size: usize,
        cartridge_ram_start: u16,
        cartridge_ram_end: i32,
    ) -> PyResult<Self> {
        // Keep the Python adapter's existing -1 sentinel for "no cartridge RAM".
        let (cartridge_ram_start, cartridge_ram_end) = if cartridge_ram_size == 0 {
            if cartridge_ram_start != 0 || cartridge_ram_end != -1 {
                return Err(PyValueError::new_err(
                    "Cartridge RAM address bounds require cartridge RAM.",
                ));
            }
            (0, 0)
        } else {
            if cartridge_ram_end < 0 || cartridge_ram_end > i32::from(u16::MAX) {
                return Err(PyValueError::new_err(
                    "Cartridge RAM must be mapped below C000h.",
                ));
            }
            (cartridge_ram_start, cartridge_ram_end as u16)
        };

        let config = MemoryConfig {
            internal_ram_size,
            cartridge_ram_size,
            cartridge_ram_start,
            cartridge_ram_end,
        };
        let memory = Sg1000Memory::new(&cartridge_rom, config).map_err(|error| {
            let message = match error {
                MemoryError::InvalidRomSize(_) => {
                    "A ROM must contain between 1 byte and 48 KiB."
                }
                MemoryError::InvalidInternalRamSize(_) => {
                    "Internal RAM must be exactly 1 KiB or 8 KiB."
                }
                MemoryError::InvalidCartridgeRamSize(_) => {
                    "Cartridge RAM must be empty, 1 KiB, or 8 KiB."
                }
                MemoryError::InvalidCartridgeRamRange => {
                    "Cartridge RAM must be mapped below C000h."
                }
            };
            PyValueError::new_err(message)
        })?;
        Ok(Self { memory })
    }

    #[inline]
    pub(crate) fn read_byte(&self, address: u16) -> u8 {
        self.memory.read(address)
    }

    #[inline]
    pub(crate) fn write_byte(&mut self, address: u16, value: u8) {
        self.memory.write(address, value);
    }
}

#[cfg(test)]
mod tests {
    use super::NativeMemoryBus;

    #[test]
    fn mirrors_small_rom_across_cartridge_window() {
        let bus = NativeMemoryBus::new(vec![0x3E, 0x12], 0x0400, 0, 0, -1).unwrap();
        assert_eq!(bus.read_byte(0x0000), 0x3E);
        assert_eq!(bus.read_byte(0x0001), 0x12);
        assert_eq!(bus.read_byte(0x0002), 0x3E);
        assert_eq!(bus.read_byte(0xBFFF), 0x12);
    }

    #[test]
    fn mirrors_internal_ram_and_ignores_rom_writes() {
        let mut bus = NativeMemoryBus::new(vec![0x00], 0x0400, 0, 0, -1).unwrap();
        bus.write_byte(0xC000, 0xA5);
        assert_eq!(bus.read_byte(0xC000), 0xA5);
        assert_eq!(bus.read_byte(0xC400), 0xA5);
        bus.write_byte(0x0000, 0xFF);
        assert_eq!(bus.read_byte(0x0000), 0x00);
    }

    #[test]
    fn rejects_cartridge_ram_mapped_over_internal_ram() {
        assert!(NativeMemoryBus::new(vec![0x00], 0x0400, 0x2000, 0xC000, 0xFFFF).is_err());
    }

    #[test]
    fn maps_cartridge_ram_without_overwriting_rom_window() {
        let mut bus = NativeMemoryBus::new(vec![0x55], 0x0400, 0x2000, 0x8000, 0xBFFF).unwrap();
        bus.write_byte(0x8000, 0xA7);
        assert_eq!(bus.read_byte(0x8000), 0xA7);
        assert_eq!(bus.read_byte(0xA000), 0xA7);
        assert_eq!(bus.read_byte(0x0000), 0x55);
        assert_eq!(bus.read_byte(0xC000), 0x00);
    }
}
