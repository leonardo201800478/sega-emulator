//! Standard Sega mapper and 8 KiB mirrored work RAM for SMS cartridges.
//!
//! The standard mapper exposes three 16 KiB slots. The first 1 KiB at
//! 0000-03FF is fixed to ROM bank 0; writes at FFFD-FFFF select the bank
//! visible in the remaining slot 0, slot 1, and slot 2 respectively. This
//! initial slice does not model cartridge SRAM or mapper variants; it reports
//! ROMs beyond the 256-bank addressable range rather than truncating them.

use crate::{CartridgeImage, SmsCoreError};

const BANK_SIZE: usize = 0x4000;
const MAX_BANKS: usize = 256;
const MAX_ROM_SIZE: usize = BANK_SIZE * MAX_BANKS;
const RAM_SIZE: usize = 0x2000;
const RAM_START: u16 = 0xC000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmsMemory {
    rom: CartridgeImage,
    bank_registers: [u8; 3],
    work_ram: [u8; RAM_SIZE],
}

impl SmsMemory {
    pub fn new(rom: CartridgeImage) -> Result<Self, SmsCoreError> {
        if rom.len() > MAX_ROM_SIZE {
            return Err(SmsCoreError::RomTooLargeForSegaMapper {
                size: rom.len(),
                max_size: MAX_ROM_SIZE,
            });
        }

        Ok(Self {
            rom,
            bank_registers: [0, 1, 2],
            work_ram: [0; RAM_SIZE],
        })
    }

    #[inline]
    pub fn read(&self, address: u16) -> u8 {
        match address {
            0x0000..=0x03FF => self.read_rom_offset(usize::from(address)),
            0x0400..=0x3FFF => {
                self.read_bank(self.bank_registers[0], usize::from(address))
            }
            0x4000..=0x7FFF => {
                self.read_bank(self.bank_registers[1], usize::from(address) - 0x4000)
            }
            0x8000..=0xBFFF => {
                self.read_bank(self.bank_registers[2], usize::from(address) - 0x8000)
            }
            0xC000..=0xFFFF => {
                self.work_ram[usize::from(address - RAM_START) & (RAM_SIZE - 1)]
            }
        }
    }

    #[inline]
    pub fn write(&mut self, address: u16, value: u8) {
        match address {
            0xFFFD => self.bank_registers[0] = value,
            0xFFFE => self.bank_registers[1] = value,
            0xFFFF => self.bank_registers[2] = value,
            0xC000..=0xFFFF => {
                self.work_ram[usize::from(address - RAM_START) & (RAM_SIZE - 1)] = value;
            }
            _ => {}
        }
    }

    #[inline]
    fn read_bank(&self, bank: u8, offset: usize) -> u8 {
        // An absent bank reads as 0xFF; never wrap an invalid bank to bank 0.
        self.read_rom_offset(usize::from(bank) * BANK_SIZE + offset)
    }

    #[inline]
    fn read_rom_offset(&self, offset: usize) -> u8 {
        self.rom.bytes().get(offset).copied().unwrap_or(0xFF)
    }

    pub fn work_ram(&self) -> &[u8] {
        &self.work_ram
    }

    pub fn selected_banks(&self) -> [u8; 3] {
        self.bank_registers
    }
}

#[cfg(test)]
mod tests {
    use super::{BANK_SIZE, MAX_ROM_SIZE, RAM_SIZE, SmsMemory};
    use crate::{CartridgeImage, SmsCoreError};

    fn patterned_rom(bank_count: usize) -> CartridgeImage {
        let mut bytes = vec![0; bank_count * BANK_SIZE];
        for (bank, chunk) in bytes.chunks_exact_mut(BANK_SIZE).enumerate() {
            chunk.fill(bank as u8);
        }
        CartridgeImage::new(bytes).unwrap()
    }

    #[test]
    fn maps_fixed_first_kib_and_three_banked_slots() {
        let mut memory = SmsMemory::new(patterned_rom(4)).unwrap();
        assert_eq!(memory.read(0x0000), 0);
        assert_eq!(memory.read(0x03FF), 0);
        assert_eq!(memory.read(0x0400), 0);
        assert_eq!(memory.read(0x3FFF), 0);
        assert_eq!(memory.read(0x4000), 1);
        assert_eq!(memory.read(0x8000), 2);

        memory.write(0xFFFD, 3);
        assert_eq!(memory.read(0x0400), 3);
        assert_eq!(memory.read(0x03FF), 0, "first 1 KiB remains fixed");

        memory.write(0xFFFE, 2);
        memory.write(0xFFFF, 1);
        assert_eq!(memory.selected_banks(), [3, 2, 1]);
        assert_eq!(memory.read(0x4000), 2);
        assert_eq!(memory.read(0x8000), 1);
    }

    #[test]
    fn mirrors_work_ram_across_the_upper_address_window() {
        let mut memory = SmsMemory::new(patterned_rom(1)).unwrap();
        memory.write(0xC000, 0xA5);
        assert_eq!(memory.read(0xC000), 0xA5);
        assert_eq!(memory.read(0xE000), 0xA5);
        memory.write(0xDFFF, 0x5A);
        assert_eq!(memory.read(0xFFFF), 0x5A);
        assert_eq!(memory.work_ram().len(), RAM_SIZE);
    }

    #[test]
    fn absent_rom_banks_read_as_ff_without_wrapping() {
        let mut memory = SmsMemory::new(patterned_rom(2)).unwrap();
        memory.write(0xFFFE, 0xFF);
        assert_eq!(memory.read(0x4000), 0xFF);
    }

    #[test]
    fn rejects_rom_beyond_standard_mapper_range_without_truncation() {
        let image = CartridgeImage::new(vec![0xAA; MAX_ROM_SIZE + 1]).unwrap();
        assert_eq!(
            SmsMemory::new(image).unwrap_err(),
            SmsCoreError::RomTooLargeForSegaMapper {
                size: MAX_ROM_SIZE + 1,
                max_size: MAX_ROM_SIZE,
            }
        );
    }

    #[test]
    fn ignores_writes_to_rom_and_mapper_registers_are_at_fixed_addresses() {
        let mut memory = SmsMemory::new(patterned_rom(3)).unwrap();
        memory.write(0x0000, 0x99);
        assert_eq!(memory.read(0x0000), 0);
        memory.write(0xFFFC, 0x08);
        assert_eq!(memory.selected_banks(), [0, 1, 2]);
    }
}
