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
    bios: Option<Box<[u8]>>,
    control_port: u8,
}

impl SmsMemory {
    pub fn new(rom: CartridgeImage) -> Result<Self, SmsCoreError> {
        Self::with_bios(rom, None)
    }

    /// Construct memory with an optional user-supplied 8 KiB SMS BIOS.
    /// Only the first 1 KiB is overlaid at $0000-$03FF while BIOS is enabled.
    pub fn with_bios(rom: CartridgeImage, bios: Option<Vec<u8>>) -> Result<Self, SmsCoreError> {
        if rom.len() > MAX_ROM_SIZE {
            return Err(SmsCoreError::RomTooLargeForSegaMapper {
                size: rom.len(),
                max_size: MAX_ROM_SIZE,
            });
        }

        if bios.as_ref().is_some_and(|image| image.len() != 0x2000) {
            return Err(SmsCoreError::InvalidBiosSize {
                size: bios.as_ref().map_or(0, Vec::len),
                expected: 0x2000,
            });
        }

        // Without BIOS, preserve direct-cartridge boot. With BIOS, expose the
        // system-ROM overlay and leave the cartridge slot enabled for hand-off.
        let control_port = if bios.is_some() { 0x00 } else { 0x08 };
        Ok(Self {
            rom,
            mapper_control: 0,
            bank_registers: [0, 1, 2],
            work_ram: [0; RAM_SIZE],
            bios: bios.map(Vec::into_boxed_slice),
            control_port,
        })
    }

    #[inline]
    pub fn read(&self, address: u16) -> u8 {
        if address <= 0x03FF && self.bios.is_some() && self.control_port & 0x08 == 0 {
            return self.bios.as_ref().map_or(0xFF, |bios| bios[usize::from(address)]);
        }
        if address <= 0xBFFF && self.control_port & 0x40 != 0 {
            return 0xFF;
        }
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

    /// Apply the SMS memory-control latch written through I/O port $3E.
    #[inline]
    pub fn write_control_port(&mut self, value: u8) {
        self.control_port = value;
    }

    pub fn reset_control_port(&mut self) {
        self.control_port = if self.bios.is_some() { 0x00 } else { 0x08 };
    }

    pub fn control_port(&self) -> u8 { self.control_port }

    pub fn bios_enabled(&self) -> bool {
        self.bios.is_some() && self.control_port & 0x08 == 0
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
    fn bios_overlays_only_the_first_kib_and_can_be_disabled() {
        let rom = patterned_rom(2);
        let mut bios = vec![0xA5; 0x2000];
        bios[0x03FF] = 0x5A;
        let mut memory = SmsMemory::with_bios(rom, Some(bios)).unwrap();
        assert!(memory.bios_enabled());
        assert_eq!(memory.read(0x0000), 0xA5);
        assert_eq!(memory.read(0x03FF), 0x5A);
        assert_eq!(memory.read(0x0400), 0x00, "cartridge remains visible after overlay");
        memory.write_control_port(0x08);
        assert!(!memory.bios_enabled());
        assert_eq!(memory.read(0x0000), 0x00);
    }

    #[test]
    fn cartridge_slot_disable_returns_open_bus_without_disabling_work_ram() {
        let mut memory = SmsMemory::with_bios(patterned_rom(1), Some(vec![0xA5; 0x2000])).unwrap();
        memory.write_control_port(0x40);
        assert_eq!(memory.read(0x0400), 0xFF);
        assert_eq!(memory.read(0xC000), 0x00);
        assert_eq!(memory.read(0x0000), 0xA5);
    }

    #[test]
    fn rejects_bios_with_wrong_size() {
        let error = SmsMemory::with_bios(patterned_rom(1), Some(vec![0x00; 0x400])).unwrap_err();
        assert_eq!(error, SmsCoreError::InvalidBiosSize { size: 0x400, expected: 0x2000 });
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

impl sega_z80_bus::CpuBus for SmsMemory {
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
mod bus_contract_tests {
    use super::SmsMemory;
    use crate::CartridgeImage;
    use sega_z80_bus::CpuBus;

    #[test]
    fn shared_bus_reads_cartridge_and_writes_mirrored_work_ram() {
        let rom = CartridgeImage::new(vec![0xA5; 0x4000]).unwrap();
        let mut memory = SmsMemory::new(rom).unwrap();

        assert_eq!(memory.read_memory(0x0123), 0xA5);
        memory.write_memory(0xC123, 0x5A);
        assert_eq!(memory.read_memory(0xE123), 0x5A);
    }

    #[test]
    fn shared_bus_defaults_to_open_bus_for_unimplemented_ports() {
        let rom = CartridgeImage::new(vec![0x00; 0x4000]).unwrap();
        let mut memory = SmsMemory::new(rom).unwrap();

        assert_eq!(memory.read_port(0x00), 0xFF);
        memory.write_port(0x00, 0x42);
        assert_eq!(memory.read_memory(0x0000), 0x00);
    }
}
