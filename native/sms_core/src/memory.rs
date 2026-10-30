//! Sega and Codemasters cartridge mappers plus 8 KiB mirrored SMS work RAM.
//!
//! The Sega mapper exposes three 16 KiB slots. The first 1 KiB at 0000-03FF
//! is fixed to ROM bank 0; writes at FFFD-FFFF select the remaining slots.
//! The Codemasters mapper pages all three slots through writes to 0000, 4000
//! and 8000. Sega mapper writes also reach mirrored work RAM, and reads from
//! those addresses return the RAM copy.
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
// SMS Plus GX initializes the BIOS slot with $E0: BIOS and work RAM enabled,
// while cartridge, card and expansion slots remain disabled. Port $3E bits
// are active-low (SMS Power / Charles MacDonald hardware notes).
const BIOS_POWER_ON_CONTROL: u8 = 0xE0;
const CARTRIDGE_BOOT_CONTROL: u8 = 0xAB;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MapperKind {
    Sega,
    Codemasters,
}

fn is_bcd(value: u8) -> bool {
    value >> 4 <= 9 && value & 0x0F <= 9
}

/// Detect the Codemasters header at $7FE0 without relying on the Sega checksum.
///
/// The bank count, BCD build timestamp, checksum/complement pair and zeroed
/// reserved bytes form a narrow signature. This avoids treating arbitrary
/// homebrew metadata (for example SDSC) as a Codemasters mapper.
fn has_codemasters_header(rom: &[u8]) -> bool {
    if rom.len() < 0x8000 || !rom.len().is_multiple_of(BANK_SIZE) {
        return false;
    }

    if &rom[0x7FF0..0x7FF8] != b"TMR SEGA" {
        return false;
    }
    let header = &rom[0x7FE0..0x7FF0];
    if usize::from(header[0]) != rom.len() / BANK_SIZE
        || !header[1..6].iter().copied().all(is_bcd)
        || header[10..].iter().any(|value| *value != 0)
    {
        return false;
    }

    let day = (header[1] >> 4) * 10 + (header[1] & 0x0F);
    let month = (header[2] >> 4) * 10 + (header[2] & 0x0F);
    let hour = (header[4] >> 4) * 10 + (header[4] & 0x0F);
    let minute = (header[5] >> 4) * 10 + (header[5] & 0x0F);
    if !(1..=31).contains(&day) || !(1..=12).contains(&month) || hour > 23 || minute > 59 {
        return false;
    }

    let checksum = u16::from_le_bytes([header[6], header[7]]);
    let complement = u16::from_le_bytes([header[8], header[9]]);
    checksum.wrapping_add(complement) == 0
}

fn detect_mapper(rom: &CartridgeImage) -> MapperKind {
    if has_codemasters_header(rom.bytes()) {
        MapperKind::Codemasters
    } else {
        MapperKind::Sega
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmsMemory {
    rom: CartridgeImage,
    mapper: MapperKind,
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
    /// The complete BIOS image is mapped at $0000-$1FFF while enabled.
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

        let mapper = detect_mapper(&rom);
        let bank_registers = match mapper {
            MapperKind::Sega => [0, 1, 2],
            MapperKind::Codemasters => [0, 1, 0],
        };

        // Power-on with BIOS: BIOS and work RAM enabled; cartridge, card and
        // expansion slots disabled. Without BIOS, emulate the normal post-BIOS
        // cartridge profile so legacy direct boot remains available.
        let control_port = if bios.is_some() {
            BIOS_POWER_ON_CONTROL
        } else {
            CARTRIDGE_BOOT_CONTROL
        };
        Ok(Self {
            rom,
            mapper,
            mapper_control: 0,
            bank_registers,
            work_ram: [0; RAM_SIZE],
            bios: bios.map(Vec::into_boxed_slice),
            control_port,
        })
    }

    #[inline]
    pub fn read(&self, address: u16) -> u8 {
        if address <= 0xBFFF && self.control_port & 0x40 != 0 {
            if address <= 0x1FFF && self.bios.is_some() && self.control_port & 0x08 == 0 {
                return self
                    .bios
                    .as_ref()
                    .map_or(0xFF, |bios| bios[usize::from(address)]);
            }
            return 0xFF;
        }
        // On the original SMS, the cartridge wins if both cartridge and BIOS
        // are enabled. SMS 2 has a distinct wired-AND behavior and is not the
        // hardware profile modeled by this core.
        match address {
            0x0000..=0x3FFF => {
                if self.mapper == MapperKind::Sega && address <= 0x03FF {
                    self.read_rom_offset(usize::from(address))
                } else {
                    self.read_bank(self.bank_registers[0], usize::from(address))
                }
            }
            0x4000..=0x7FFF => {
                self.read_bank(self.bank_registers[1], usize::from(address) - 0x4000)
            }
            0x8000..=0xBFFF => {
                self.read_bank(self.bank_registers[2], usize::from(address) - 0x8000)
            }
            0xC000..=0xFFFF => {
                if self.control_port & 0x10 != 0 {
                    0xFF
                } else {
                    self.work_ram[usize::from(address - RAM_START) & (RAM_SIZE - 1)]
                }
            }
        }
    }

    #[inline]
    pub fn write(&mut self, address: u16, value: u8) {
        // Work RAM is mirrored through FFFF. Mapper writes update both the
        // mapper register and the RAM mirror at the same logical address.
        if address >= RAM_START && self.control_port & 0x10 == 0 {
            self.work_ram[usize::from(address - RAM_START) & (RAM_SIZE - 1)] = value;
        }

        // The mapper registers belong to the cartridge slot. A BIOS-only
        // power-on profile must not mutate a disabled cartridge mapper.
        if self.control_port & 0x40 == 0 {
            match self.mapper {
                MapperKind::Sega => match address {
                    0xFFFC => self.mapper_control = value,
                    0xFFFD => self.bank_registers[0] = value,
                    0xFFFE => self.bank_registers[1] = value,
                    0xFFFF => self.bank_registers[2] = value,
                    _ => {}
                },
                MapperKind::Codemasters => match address {
                    0x0000..=0x3FFF => self.bank_registers[0] = value,
                    0x4000..=0x7FFF => self.bank_registers[1] = value,
                    0x8000..=0xBFFF => self.bank_registers[2] = value,
                    _ => {}
                },
            }
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
        self.control_port = if self.bios.is_some() {
            BIOS_POWER_ON_CONTROL
        } else {
            CARTRIDGE_BOOT_CONTROL
        };
    }

    pub fn control_port(&self) -> u8 {
        self.control_port
    }

    pub fn bios_enabled(&self) -> bool {
        self.bios.is_some() && self.control_port & 0x08 == 0
    }
}

#[cfg(test)]
mod tests {
    use super::{BANK_SIZE, BIOS_POWER_ON_CONTROL, MAX_ROM_SIZE, MapperKind, RAM_SIZE, SmsMemory};
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

    fn codemasters_rom(bank_count: usize) -> CartridgeImage {
        let mut bytes = vec![0; bank_count * BANK_SIZE];
        for bank in 0..bank_count {
            let start = bank * BANK_SIZE;
            bytes[start..start + BANK_SIZE].fill(bank as u8);
        }

        let header = [
            bank_count as u8,
            0x31,
            0x08,
            0x93,
            0x10,
            0x59,
            0xC5,
            0x1B,
            0x3B,
            0xE4,
            0,
            0,
            0,
            0,
            0,
            0,
        ];
        bytes[0x7FE0..0x7FF0].copy_from_slice(&header);
        bytes[0x7FF0..0x7FF8].copy_from_slice(b"TMR SEGA");
        CartridgeImage::new(bytes).unwrap()
    }

    #[test]
    fn codemasters_header_selects_its_three_slot_mapper() {
        let mut memory = SmsMemory::new(codemasters_rom(16)).unwrap();

        assert_eq!(memory.bank_registers, [0, 1, 0]);
        assert_eq!(memory.read(0x0000), 0);
        assert_eq!(memory.read(0x4000), 1);
        assert_eq!(memory.read(0x8000), 0);

        // Codemasters has no fixed first 1 KiB and decodes writes throughout
        // each 16 KiB slot, unlike the standard Sega mapper.
        memory.write(0x0000, 5);
        memory.write(0x4000, 6);
        memory.write(0x8000, 7);

        assert_eq!(memory.read(0x0000), 5);
        assert_eq!(memory.read(0x4000), 6);
        assert_eq!(memory.read(0x8000), 7);
    }

    #[test]
    fn codemasters_mapper_registers_stay_disabled_during_bios_power_on() {
        let mut memory =
            SmsMemory::with_bios(codemasters_rom(16), Some(vec![0xA5; 0x2000])).unwrap();

        memory.write(0x4000, 7);

        assert_eq!(memory.bank_registers, [0, 1, 0]);
        assert_eq!(memory.control_port, BIOS_POWER_ON_CONTROL);
    }

    #[test]
    fn invalid_codemasters_metadata_keeps_the_sega_mapper() {
        let mut rom = codemasters_rom(16);
        let mut bytes = rom.bytes().to_vec();
        bytes[0x7FE9] ^= 0x01;
        rom = CartridgeImage::new(bytes).unwrap();
        let memory = SmsMemory::new(rom).unwrap();

        assert_eq!(memory.mapper, MapperKind::Sega);
        assert_eq!(memory.selected_banks(), [0, 1, 2]);
    }

    #[test]
    fn bios_maps_all_eight_kib_and_can_be_disabled() {
        let rom = patterned_rom(2);
        let mut bios = vec![0xA5; 0x2000];
        bios[0x03FF] = 0x5A;
        bios[0x0400] = 0x6B;
        bios[0x1FFF] = 0x7C;
        let mut memory = SmsMemory::with_bios(rom, Some(bios)).unwrap();
        assert!(memory.bios_enabled());
        assert_eq!(memory.read(0x0000), 0xA5);
        assert_eq!(memory.read(0x03FF), 0x5A);
        assert_eq!(memory.read(0x0400), 0x6B);
        assert_eq!(memory.read(0x1FFF), 0x7C);
        assert_eq!(
            memory.read(0x2000),
            0xFF,
            "disabled cartridge slot remains unmapped outside the 8 KiB BIOS"
        );
        memory.write_control_port(0x08);
        assert!(!memory.bios_enabled());
        assert_eq!(memory.read(0x0000), 0x00);
        assert_eq!(memory.read(0x0400), 0x00);
    }

    #[test]
    fn bios_power_on_profile_matches_sms_slot_and_ram_latches() {
        let mut memory = SmsMemory::with_bios(patterned_rom(1), Some(vec![0xA5; 0x2000])).unwrap();
        assert_eq!(memory.control_port(), 0xE0);
        assert!(memory.bios_enabled());
        assert_eq!(memory.read(0x0000), 0xA5);
        assert_eq!(memory.read(0x0400), 0xA5);
        assert_eq!(memory.read(0xC000), 0x00);
        memory.write(0xC000, 0x5A);
        assert_eq!(memory.read(0xC000), 0x5A);
        assert!(
            memory.bios_enabled(),
            "RAM access must not disable the BIOS overlay"
        );
    }

    #[test]
    fn cartridge_has_priority_when_bios_and_cartridge_slots_are_both_enabled() {
        let mut memory = SmsMemory::with_bios(patterned_rom(1), Some(vec![0xA5; 0x2000])).unwrap();
        memory.write_control_port(0xA0); // BIOS + cartridge + work RAM enabled.
        assert_eq!(memory.read(0x0000), 0x00);
        assert_eq!(memory.read(0x2000), 0x00);
    }

    #[test]
    fn cartridge_boot_profile_matches_post_bios_slot_and_ram_latches() {
        let memory = SmsMemory::new(patterned_rom(1)).unwrap();
        assert_eq!(memory.control_port(), 0xAB);
        assert!(!memory.bios_enabled());
        assert_eq!(memory.read(0x0000), 0x00);
        assert_eq!(memory.read(0xC000), 0x00);
    }

    #[test]
    fn cartridge_slot_disable_returns_open_bus_without_disabling_work_ram() {
        let mut memory = SmsMemory::with_bios(patterned_rom(1), Some(vec![0xA5; 0x2000])).unwrap();
        memory.write_control_port(0x40);
        assert_eq!(
            memory.read(0x0400),
            0xA5,
            "BIOS remains mapped while its disable bit is clear"
        );
        assert_eq!(memory.read(0xC000), 0x00);
        assert_eq!(memory.read(0x0000), 0xA5);
    }

    #[test]
    fn disabled_cartridge_slot_does_not_receive_mapper_register_writes() {
        let mut memory = SmsMemory::with_bios(patterned_rom(3), Some(vec![0xA5; 0x2000])).unwrap();
        assert_eq!(memory.selected_banks(), [0, 1, 2]);
        memory.write(0xFFFD, 0x07);
        assert_eq!(memory.selected_banks(), [0, 1, 2]);

        memory.write_control_port(0xAB);
        memory.write(0xFFFD, 0x07);
        assert_eq!(memory.selected_banks(), [0x07, 1, 2]);
    }

    #[test]
    fn memory_control_port_can_disable_and_reenable_work_ram() {
        let mut memory = SmsMemory::new(patterned_rom(1)).unwrap();
        memory.write(0xC000, 0x5A);
        memory.write_control_port(0x18);
        assert_eq!(memory.read(0xC000), 0xFF);
        memory.write(0xC000, 0xA5);
        memory.write_control_port(0x08);
        assert_eq!(memory.read(0xC000), 0x5A);
    }

    #[test]
    fn rejects_bios_with_wrong_size() {
        let error = SmsMemory::with_bios(patterned_rom(1), Some(vec![0x00; 0x400])).unwrap_err();
        assert_eq!(
            error,
            SmsCoreError::InvalidBiosSize {
                size: 0x400,
                expected: 0x2000
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
