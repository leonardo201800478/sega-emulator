//! Native SG-1000 cartridge and RAM mapping for the Rust Z80 runtime.

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

const ROM_WINDOW_SIZE: usize = 0xC000;
const RAM_START: u16 = 0xC000;

#[pyclass(module = "sega_z80_core")]
pub(crate) struct NativeMemoryBus {
    rom_window: Vec<u8>,
    ram: Vec<u8>,
    cartridge_ram: Vec<u8>,
    cartridge_ram_start: u16,
    cartridge_ram_end: u16,
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
        if cartridge_rom.is_empty() || cartridge_rom.len() > ROM_WINDOW_SIZE {
            return Err(PyValueError::new_err(
                "A ROM must contain between 1 byte and 48 KiB.",
            ));
        }
        if !matches!(internal_ram_size, 0x0400 | 0x2000) {
            return Err(PyValueError::new_err(
                "Internal RAM must be exactly 1 KiB or 8 KiB.",
            ));
        }
        if cartridge_ram_size != 0 && !matches!(cartridge_ram_size, 0x0400 | 0x2000) {
            return Err(PyValueError::new_err(
                "Cartridge RAM must be empty, 1 KiB, or 8 KiB.",
            ));
        }
        if cartridge_ram_size == 0 {
            if cartridge_ram_end != -1 {
                return Err(PyValueError::new_err(
                    "A cartridge RAM end address requires cartridge RAM.",
                ));
            }
        } else if cartridge_ram_end < i32::from(cartridge_ram_start)
            || cartridge_ram_end > i32::from(u16::MAX)
        {
            return Err(PyValueError::new_err(
                "Invalid cartridge RAM address range.",
            ));
        }

        let mut rom_window = Vec::with_capacity(ROM_WINDOW_SIZE);
        while rom_window.len() < ROM_WINDOW_SIZE {
            let remaining = ROM_WINDOW_SIZE - rom_window.len();
            let take = remaining.min(cartridge_rom.len());
            rom_window.extend_from_slice(&cartridge_rom[..take]);
        }

        Ok(Self {
            rom_window,
            ram: vec![0; internal_ram_size],
            cartridge_ram: vec![0; cartridge_ram_size],
            cartridge_ram_start,
            cartridge_ram_end: if cartridge_ram_end < 0 {
                0
            } else {
                cartridge_ram_end as u16
            },
        })
    }

    fn read(&self, address: u16) -> u8 {
        self.read_byte(address)
    }

    fn write(&mut self, address: u16, value: u8) {
        self.write_byte(address, value);
    }

    #[getter]
    fn internal_ram_size(&self) -> usize {
        self.ram.len()
    }

    #[getter]
    fn cartridge_ram_size(&self) -> usize {
        self.cartridge_ram.len()
    }
}

impl NativeMemoryBus {
    pub(crate) fn read_byte(&self, address: u16) -> u8 {
        if !self.cartridge_ram.is_empty()
            && address >= self.cartridge_ram_start
            && address <= self.cartridge_ram_end
        {
            let offset =
                usize::from(address - self.cartridge_ram_start) & (self.cartridge_ram.len() - 1);
            return self.cartridge_ram[offset];
        }
        if address < RAM_START {
            return self.rom_window[usize::from(address)];
        }
        let offset = (usize::from(address) - usize::from(RAM_START)) & (self.ram.len() - 1);
        self.ram[offset]
    }

    pub(crate) fn write_byte(&mut self, address: u16, value: u8) {
        if !self.cartridge_ram.is_empty()
            && address >= self.cartridge_ram_start
            && address <= self.cartridge_ram_end
        {
            let offset =
                usize::from(address - self.cartridge_ram_start) & (self.cartridge_ram.len() - 1);
            self.cartridge_ram[offset] = value;
            return;
        }
        if address < RAM_START {
            // Cartridge ROM writes are ignored by this baseline mapper.
            return;
        }
        let offset = (usize::from(address) - usize::from(RAM_START)) & (self.ram.len() - 1);
        self.ram[offset] = value;
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
    fn maps_cartridge_ram_without_overwriting_rom_window() {
        let mut bus = NativeMemoryBus::new(vec![0x55], 0x0400, 0x2000, 0x8000, 0xBFFF).unwrap();
        bus.write_byte(0x8000, 0xA7);
        assert_eq!(bus.read_byte(0x8000), 0xA7);
        assert_eq!(bus.read_byte(0xA000), 0xA7);
        assert_eq!(bus.read_byte(0x0000), 0x55);
        assert_eq!(bus.read_byte(0xC000), 0x00);
    }
}
