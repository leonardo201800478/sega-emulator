//! Standard Sega mapper and 8 KiB mirrored work RAM for SMS cartridges.
//!
//! The standard mapper exposes three 16 KiB slots. The first 1 KiB at
//! 0000-03FF is fixed to ROM bank 0; writes at FFFD-FFFF select the bank
//! visible in the remaining slots. Writes at FFFC configure cartridge RAM
//! mapping. Mapper register writes also reach mirrored work RAM, and reads
//! from those addresses return the RAM copy.
//!
//! Reference: https://www.smspower.org/Development/Mappers
//! Reference: https://www.smspower.org/Development/MemoryMap
//! This initial slice does not model cartridge SRAM or mapper variants. ROMs
//! beyond the standard mapper's 256-bank range are rejected, never truncated.

use crate::{CartridgeImage, SmsCoreError};

const BANK_SIZE: usize = 0x4000;
// SMS Power documents up to six significant bank bits (1 MiB) among known
// Sega mapper revisions; smaller revisions exist. Do not infer 4 MiB support
// merely from the width of the software-visible register.
const MAX_BANKS: usize = 64;
const MAX_ROM_SIZE: usize = BANK_SIZE * MAX_BANKS;
const RAM_SIZE: usize = 0x2000;
const RAM_START: u16 = 0xC000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmsMemory {
    rom: CartridgeImage,
    mapper_control: u8,
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

        // Deterministic startup profile matching the documented 315-5235
        // power-up values. Other Sega mapper revisions may leave registers
        // undefined or reset them differently; model selection is not yet exposed.
        Ok(Self {
            rom,
            mapper_control: 0,
            bank_registers: [0, 1, 2],
            work_ram: [0; RAM_SIZE],
        })
    }

    #[inline]
    pub fn read(&self, address: u16) -> u8 {
        match address {
            0x0000..=0x03FF => self.read_rom_offset(usize::from(address)),
            0x0400..=0x3FFF => self.read_bank(self.bank_registers[0], usize::from(address)),
            0x4000..=0x7FFF => {
                self.read_bank(self.bank_registers[1], usize::from(address) - 0x4000)
            }
            0x8000..=0xBFFF => {
                self.read_bank(self.bank_registers[2], usize::from(address) - 0x8000)
            }
            0xC000..=0xFFFF => self.work_ram[usize::from(address - RAM_START) & (RAM_SIZE - 1)],
        }
    }

    #[inline]
    pub fn write(&mut self, address: u16, value: u8) {
        // Work RAM is mirrored through FFFF. Mapper writes update both the
        // mapper register and the RAM mirror at the same logical address.
        if address >= RAM_START {
            self.work_ram[usize::from(address - RAM_START) & (RAM_SIZE - 1)] = value;
        }

        match address {
            0xFFFC => self.mapper_control = value,
            0xFFFD => self.bank_registers[0] = value,
            0xFFFE => self.bank_registers[1] = value,
            0xFFFF => self.bank_registers[2] = value,
            _ => {}
        }
    }

    #[inline]
    fn read_bank(&self, bank: u8, offset: usize) -> u8 {
        let bank_count = self.rom.len() / BANK_SIZE;
        let bank = if self.rom.len().is_multiple_of(BANK_SIZE) && bank_count.is_power_of_two() {
            // For power-of-two ROM sizes, unconnected high bank-address lines
            // mirror the physical ROM. SMS Power documents this aliasing.
            usize::from(bank) & (bank_count - 1)
        } else {
            // Non-power-of-two images need mapper-specific evidence; keep this
            // slice deterministic rather than claiming hardware-accurate aliasing.
            usize::from(bank)
        };
        self.read_rom_offset(bank * BANK_SIZE + offset)
    }

    #[inline]
    fn read_rom_offset(&self, offset: usize) -> u8 {
        let rom_size = self.rom.len();
        // Small power-of-two ROMs (for example 8 KiB images) leave address
        // lines unconnected and mirror through the 16 KiB CPU window.
        // For non-power-of-two images, do not invent aliasing behavior.
        let physical_offset = if rom_size.is_power_of_two() {
            offset & (rom_size - 1)
        } else {
            offset
        };
        self.rom
            .bytes()
            .get(physical_offset)
            .copied()
            .unwrap_or(0xFF)
    }

    pub fn work_ram(&self) -> &[u8] {
        &self.work_ram
    }

    pub fn selected_banks(&self) -> [u8; 3] {
        self.bank_registers
    }

    pub fn mapper_control(&self) -> u8 {
        self.mapper_control
    }
}

#[cfg(test)]
mod tests {
    use super::{BANK_SIZE, MAX_ROM_SIZE, RAM_SIZE, SmsMemory};
    use crate::{CartridgeImage, SmsCoreError};

    fn patterned_rom(bank_count: usize) -> CartridgeImage {
        let mut bytes = vec![0; bank_count * BANK_SIZE];
        for bank in 0..bank_count {
            let start = bank * BANK_SIZE;
            bytes[start..start + BANK_SIZE].fill(bank as u8);
        }
        CartridgeImage::new(bytes).unwrap()
    }

    #[test]
    fn mirrors_small_power_of_two_roms_across_the_16k_window() {
        let mut bytes = vec![0x12; 0x2000];
        bytes[0x1000..].fill(0x34);
        let memory = SmsMemory::new(CartridgeImage::new(bytes).unwrap()).unwrap();

        assert_eq!(memory.read(0x0000), 0x12);
        assert_eq!(memory.read(0x1000), 0x34);
        assert_eq!(memory.read(0x2000), 0x12);
        assert_eq!(memory.read(0x3000), 0x34);
        assert_eq!(memory.read(0x4000), 0x12);
        assert_eq!(memory.read(0x5000), 0x34);
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
    fn slot_zero_preserves_bank_offset_after_fixed_vector_area() {
        let mut bytes = vec![0; 4 * BANK_SIZE];
        bytes[0x03FF] = 0x3F;
        bytes[0x0400] = 0x40;
        bytes[0x3FFF] = 0x4F;
        bytes[3 * BANK_SIZE + 0x0400] = 0xD0;
        bytes[3 * BANK_SIZE + 0x3FFF] = 0xDF;
        let rom = CartridgeImage::new(bytes).unwrap();
        let mut memory = SmsMemory::new(rom).unwrap();

        assert_eq!(memory.read(0x03FF), 0x3F);
        assert_eq!(memory.read(0x0400), 0x40);
        assert_eq!(memory.read(0x3FFF), 0x4F);

        memory.write(0xFFFD, 3);
        assert_eq!(memory.read(0x03FF), 0x3F);
        assert_eq!(memory.read(0x0400), 0xD0);
        assert_eq!(memory.read(0x3FFF), 0xDF);
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
    fn mapper_register_writes_also_update_the_ram_mirror() {
        let mut memory = SmsMemory::new(patterned_rom(4)).unwrap();

        memory.write(0xFFFC, 0x08);
        assert_eq!(memory.mapper_control(), 0x08);
        assert_eq!(memory.read(0xDFFC), 0x08);
        assert_eq!(memory.read(0xFFFC), 0x08);

        memory.write(0xFFFD, 3);
        assert_eq!(memory.selected_banks()[0], 3);
        assert_eq!(memory.read(0xDFFD), 3);
        assert_eq!(memory.read(0xFFFD), 3);
    }

    #[test]
    fn power_of_two_rom_banks_mirror_when_bank_register_exceeds_size() {
        let mut memory = SmsMemory::new(patterned_rom(2)).unwrap();
        memory.write(0xFFFE, 0xFF);
        assert_eq!(memory.read(0x4000), 1);
    }

    #[test]
    fn non_power_of_two_rom_does_not_assume_bank_aliasing() {
        let mut memory = SmsMemory::new(patterned_rom(3)).unwrap();
        memory.write(0xFFFE, 3);
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
    fn ignores_rom_writes_and_only_mapper_registers_change_mapper_state() {
        let mut memory = SmsMemory::new(patterned_rom(3)).unwrap();
        memory.write(0x0000, 0x99);
        assert_eq!(memory.read(0x0000), 0);
        memory.write(0xFFFC, 0x08);
        assert_eq!(memory.selected_banks(), [0, 1, 2]);
        assert_eq!(memory.mapper_control(), 0x08);
    }
}
