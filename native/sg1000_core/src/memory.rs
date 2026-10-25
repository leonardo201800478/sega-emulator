//! Python-independent SG-1000 cartridge and RAM mapping.

use std::fmt;

const ROM_WINDOW_SIZE: usize = 0xC000;
const RAM_START: u16 = 0xC000;
const INTERNAL_RAM_1K: usize = 0x0400;
const RAM_8K: usize = 0x2000;

/// Memory layout selected by the cartridge profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryConfig {
    pub internal_ram_size: usize,
    pub cartridge_ram_size: usize,
    pub cartridge_ram_start: u16,
    pub cartridge_ram_end: u16,
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            internal_ram_size: INTERNAL_RAM_1K,
            cartridge_ram_size: 0,
            cartridge_ram_start: 0,
            cartridge_ram_end: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemoryError {
    InvalidRomSize(usize),
    InvalidInternalRamSize(usize),
    InvalidCartridgeRamSize(usize),
    InvalidCartridgeRamRange,
}

impl fmt::Display for MemoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRomSize(size) => write!(
                f,
                "ROM size must be between 1 and {ROM_WINDOW_SIZE} bytes, got {size}"
            ),
            Self::InvalidInternalRamSize(size) => {
                write!(f, "internal RAM must be 1 KiB or 8 KiB, got {size}")
            }
            Self::InvalidCartridgeRamSize(size) => write!(
                f,
                "cartridge RAM must be empty, 1 KiB, or 8 KiB, got {size}"
            ),
            Self::InvalidCartridgeRamRange => {
                write!(f, "cartridge RAM range must be valid and below 0xC000")
            }
        }
    }
}

impl std::error::Error for MemoryError {}

/// SG-1000 memory map with ROM mirroring and explicitly configured RAM windows.
///
/// Writes to the ROM window are ignored for the baseline mapper. Banking and
/// cartridge-specific mapper ASICs are intentionally outside this type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sg1000Memory {
    rom_window: Vec<u8>,
    internal_ram: Vec<u8>,
    cartridge_ram: Vec<u8>,
    cartridge_ram_start: u16,
    cartridge_ram_end: u16,
}

impl Sg1000Memory {
    pub fn new(rom: &[u8], config: MemoryConfig) -> Result<Self, MemoryError> {
        if rom.is_empty() || rom.len() > ROM_WINDOW_SIZE {
            return Err(MemoryError::InvalidRomSize(rom.len()));
        }
        if !matches!(config.internal_ram_size, INTERNAL_RAM_1K | RAM_8K) {
            return Err(MemoryError::InvalidInternalRamSize(
                config.internal_ram_size,
            ));
        }
        if !matches!(config.cartridge_ram_size, 0 | INTERNAL_RAM_1K | RAM_8K) {
            return Err(MemoryError::InvalidCartridgeRamSize(
                config.cartridge_ram_size,
            ));
        }
        if config.cartridge_ram_size == 0 {
            if config.cartridge_ram_start != 0 || config.cartridge_ram_end != 0 {
                return Err(MemoryError::InvalidCartridgeRamRange);
            }
        } else if config.cartridge_ram_start >= RAM_START
            || config.cartridge_ram_end < config.cartridge_ram_start
            || config.cartridge_ram_end >= RAM_START
        {
            return Err(MemoryError::InvalidCartridgeRamRange);
        }

        let mut rom_window = Vec::with_capacity(ROM_WINDOW_SIZE);
        while rom_window.len() < ROM_WINDOW_SIZE {
            let remaining = ROM_WINDOW_SIZE - rom_window.len();
            let count = remaining.min(rom.len());
            rom_window.extend_from_slice(&rom[..count]);
        }

        Ok(Self {
            rom_window,
            internal_ram: vec![0; config.internal_ram_size],
            cartridge_ram: vec![0; config.cartridge_ram_size],
            cartridge_ram_start: config.cartridge_ram_start,
            cartridge_ram_end: config.cartridge_ram_end,
        })
    }

    #[inline]
    pub fn read(&self, address: u16) -> u8 {
        if !self.cartridge_ram.is_empty()
            && (self.cartridge_ram_start..=self.cartridge_ram_end).contains(&address)
        {
            let offset =
                usize::from(address - self.cartridge_ram_start) & (self.cartridge_ram.len() - 1);
            return self.cartridge_ram[offset];
        }
        if address < RAM_START {
            return self.rom_window[usize::from(address)];
        }
        let offset = (usize::from(address - RAM_START)) & (self.internal_ram.len() - 1);
        self.internal_ram[offset]
    }

    #[inline]
    pub fn write(&mut self, address: u16, value: u8) {
        if !self.cartridge_ram.is_empty()
            && (self.cartridge_ram_start..=self.cartridge_ram_end).contains(&address)
        {
            let offset =
                usize::from(address - self.cartridge_ram_start) & (self.cartridge_ram.len() - 1);
            self.cartridge_ram[offset] = value;
            return;
        }
        if address < RAM_START {
            return;
        }
        let offset = (usize::from(address - RAM_START)) & (self.internal_ram.len() - 1);
        self.internal_ram[offset] = value;
    }

    pub fn internal_ram(&self) -> &[u8] {
        &self.internal_ram
    }

    pub fn cartridge_ram(&self) -> &[u8] {
        &self.cartridge_ram
    }
}

#[cfg(test)]
mod tests {
    use super::{MemoryConfig, MemoryError, Sg1000Memory};

    #[test]
    fn mirrors_small_rom_across_cartridge_window() {
        let memory = Sg1000Memory::new(&[0x3E, 0x12], MemoryConfig::default()).unwrap();
        assert_eq!(memory.read(0x0000), 0x3E);
        assert_eq!(memory.read(0x0001), 0x12);
        assert_eq!(memory.read(0x0002), 0x3E);
        assert_eq!(memory.read(0xBFFF), 0x12);
    }

    #[test]
    fn mirrors_internal_ram_and_ignores_rom_writes() {
        let mut memory = Sg1000Memory::new(&[0x00], MemoryConfig::default()).unwrap();
        memory.write(0xC000, 0xA5);
        assert_eq!(memory.read(0xC000), 0xA5);
        assert_eq!(memory.read(0xC400), 0xA5);
        memory.write(0x0000, 0xFF);
        assert_eq!(memory.read(0x0000), 0x00);
    }

    #[test]
    fn maps_cartridge_ram_in_its_own_window() {
        let config = MemoryConfig {
            cartridge_ram_size: 0x2000,
            cartridge_ram_start: 0x8000,
            cartridge_ram_end: 0xBFFF,
            ..MemoryConfig::default()
        };
        let mut memory = Sg1000Memory::new(&[0x55], config).unwrap();
        memory.write(0x8000, 0xA7);
        assert_eq!(memory.read(0x8000), 0xA7);
        assert_eq!(memory.read(0xA000), 0xA7);
        assert_eq!(memory.read(0x0000), 0x55);
        assert_eq!(memory.read(0xC000), 0x00);
    }

    #[test]
    fn rejects_invalid_rom_and_ram_layouts() {
        assert_eq!(
            Sg1000Memory::new(&[], MemoryConfig::default()).unwrap_err(),
            MemoryError::InvalidRomSize(0)
        );
        let config = MemoryConfig {
            internal_ram_size: 2048,
            ..MemoryConfig::default()
        };
        assert_eq!(
            Sg1000Memory::new(&[0], config).unwrap_err(),
            MemoryError::InvalidInternalRamSize(2048)
        );
    }

    #[test]
    fn rejects_cartridge_ram_overlapping_internal_ram() {
        let config = MemoryConfig {
            cartridge_ram_size: 0x2000,
            cartridge_ram_start: 0xC000,
            cartridge_ram_end: 0xFFFF,
            ..MemoryConfig::default()
        };
        assert_eq!(
            Sg1000Memory::new(&[0], config).unwrap_err(),
            MemoryError::InvalidCartridgeRamRange
        );
    }
}
