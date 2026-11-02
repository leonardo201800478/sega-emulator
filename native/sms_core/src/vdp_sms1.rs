//! New SMS1/315-5124 VDP foundation, hosted by the SMS core crate.
//!
//! This module is deliberately independent from the legacy scanline renderer.
//! It owns SMS1 identity, raw memory/register state, CPU port protocol and a
//! deterministic functional Mode 4 background scanline renderer. Raster
//! scheduling, fetch arbitration, sprites and IRQ remain separate milestones.

/// SMS VDP ASIC revision. Video standard is modeled independently.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VdpRevision {
    /// Master System 1 ASIC 315-5124.
    Sms1_315_5124,
}

/// Regional video family; not an ASIC revision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoStandard {
    Ntsc,
    PalM,
    Pal,
    Unknown,
}

/// Immutable hardware identity for one VDP instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VdpProfile {
    pub revision: VdpRevision,
    pub video_standard: VideoStandard,
}

impl VdpProfile {
    /// Construct the currently supported SMS1 profile.
    pub const fn sms1(video_standard: VideoStandard) -> Self {
        Self {
            revision: VdpRevision::Sms1_315_5124,
            video_standard,
        }
    }
}

/// SMS1 raw memory capacities, independent of any display/timing assumptions.
pub const VRAM_SIZE: usize = 0x4000;
pub const CRAM_SIZE: usize = 0x20;
pub const REGISTER_COUNT: usize = 16;

/// Register storage. This type intentionally does not apply unverified
/// register masks, side effects, or raster latching rules.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisterFile {
    values: [u8; REGISTER_COUNT],
}

impl RegisterFile {
    /// Create all registers in their deterministic power-on representation.
    pub const fn new() -> Self {
        Self {
            values: [0; REGISTER_COUNT],
        }
    }

    /// Read the stored byte at a register index.
    pub fn get(&self, index: usize) -> Option<u8> {
        self.values.get(index).copied()
    }

    /// Store a raw register byte without inventing hardware side effects.
    pub fn set_raw(&mut self, index: usize, value: u8) -> bool {
        if let Some(register) = self.values.get_mut(index) {
            *register = value;
            true
        } else {
            false
        }
    }

    /// Borrow the complete register image for diagnostics/save-state codecs.
    pub const fn as_array(&self) -> &[u8; REGISTER_COUNT] {
        &self.values
    }
}

impl Default for RegisterFile {
    fn default() -> Self {
        Self::new()
    }
}

/// Explicit raw VRAM/CRAM storage owned by the new VDP.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VdpMemory {
    vram: [u8; VRAM_SIZE],
    cram: [u8; CRAM_SIZE],
}

impl VdpMemory {
    /// Allocate a zero-initialized SMS1 memory image.
    pub const fn new() -> Self {
        Self {
            vram: [0; VRAM_SIZE],
            cram: [0; CRAM_SIZE],
        }
    }

    /// Read one VRAM byte by physical array index; no address-port behavior.
    pub fn vram_byte(&self, index: usize) -> Option<u8> {
        self.vram.get(index).copied()
    }

    /// Write one VRAM byte by physical array index; no address-port behavior.
    pub fn set_vram_byte(&mut self, index: usize, value: u8) -> bool {
        if let Some(byte) = self.vram.get_mut(index) {
            *byte = value;
            true
        } else {
            false
        }
    }

    /// Read one CRAM byte by physical array index; no palette conversion.
    pub fn cram_byte(&self, index: usize) -> Option<u8> {
        self.cram.get(index).copied()
    }

    /// Write one raw CRAM byte; ASIC-specific masks belong to the port layer.
    pub fn set_cram_byte(&mut self, index: usize, value: u8) -> bool {
        if let Some(byte) = self.cram.get_mut(index) {
            *byte = value;
            true
        } else {
            false
        }
    }

    /// Borrow raw VRAM for deterministic state inspection.
    pub const fn vram(&self) -> &[u8; VRAM_SIZE] {
        &self.vram
    }

    /// Borrow raw CRAM for deterministic state inspection.
    pub const fn cram(&self) -> &[u8; CRAM_SIZE] {
        &self.cram
    }
}

impl Default for VdpMemory {
    fn default() -> Self {
        Self::new()
    }
}

/// Initial state container for the new SMS1 VDP.
///
/// This is not yet a running VDP: there is deliberately no scanline advance,
/// guessed raster constant, port protocol, renderer, or scheduler of its own.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sms1VdpState {
    pub profile: VdpProfile,
    pub registers: RegisterFile,
    pub memory: VdpMemory,
}

impl Sms1VdpState {
    /// Construct a deterministic SMS1 state for the selected regional profile.
    pub const fn new(video_standard: VideoStandard) -> Self {
        Self {
            profile: VdpProfile::sms1(video_standard),
            registers: RegisterFile::new(),
            memory: VdpMemory::new(),
        }
    }

    /// Reset raw registers and memories without changing the hardware profile.
    pub fn reset(&mut self, clear_vram: bool) {
        self.registers = RegisterFile::new();
        self.memory.cram.fill(0);
        if clear_vram {
            self.memory.vram.fill(0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{CRAM_SIZE, REGISTER_COUNT, Sms1VdpState, VRAM_SIZE, VideoStandard};

    #[test]
    fn sms1_profile_keeps_region_separate_from_asic_revision() {
        let ntsc = Sms1VdpState::new(VideoStandard::Ntsc);
        let pal = Sms1VdpState::new(VideoStandard::Pal);
        assert_eq!(ntsc.profile.revision, pal.profile.revision);
        assert_ne!(ntsc.profile.video_standard, pal.profile.video_standard);
    }

    #[test]
    fn raw_state_has_documented_sms1_capacities() {
        let state = Sms1VdpState::new(VideoStandard::Ntsc);
        assert_eq!(state.memory.vram().len(), VRAM_SIZE);
        assert_eq!(state.memory.cram().len(), CRAM_SIZE);
        assert_eq!(state.registers.as_array().len(), REGISTER_COUNT);
    }

    #[test]
    fn reset_preserves_vram_only_when_requested() {
        let mut state = Sms1VdpState::new(VideoStandard::Ntsc);
        assert!(state.memory.set_vram_byte(0x1234, 0xA5));
        assert!(state.memory.set_cram_byte(3, 0x2A));
        assert!(state.registers.set_raw(8, 0x55));

        state.reset(false);
        assert_eq!(state.memory.vram_byte(0x1234), Some(0xA5));
        assert_eq!(state.memory.cram_byte(3), Some(0));
        assert_eq!(state.registers.get(8), Some(0));

        state.reset(true);
        assert_eq!(state.memory.vram_byte(0x1234), Some(0));
    }

    #[test]
    fn raw_memory_access_rejects_out_of_range_indices() {
        let mut state = Sms1VdpState::new(VideoStandard::Ntsc);
        assert!(!state.memory.set_vram_byte(VRAM_SIZE, 1));
        assert!(!state.memory.set_cram_byte(CRAM_SIZE, 1));
        assert!(!state.registers.set_raw(REGISTER_COUNT, 1));
    }
}

/// Operation selected by the second byte written to the VDP control port.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PortCode {
    VramRead,
    VramWrite,
    RegisterWrite,
    CramWrite,
}

/// Stateful SMS1 VDP port interface.
///
/// This implements the documented two-byte control latch and buffered VRAM
/// reads. Raster generation, display fetch arbitration and IRQ timing are
/// intentionally not represented by this port-only milestone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sms1Vdp {
    pub state: Sms1VdpState,
    address: u16,
    read_buffer: u8,
    control_first: Option<u8>,
    code: PortCode,
    status: u8,
}

impl Sms1Vdp {
    /// Create an SMS1 VDP port device with deterministic zeroed state.
    pub const fn new(video_standard: VideoStandard) -> Self {
        Self {
            state: Sms1VdpState::new(video_standard),
            address: 0,
            read_buffer: 0,
            control_first: None,
            code: PortCode::VramRead,
            status: 0,
        }
    }

    /// Current 14-bit VDP address register.
    pub const fn address(&self) -> u16 {
        self.address
    }

    /// Current read-ahead buffer, exposed for diagnostics and tests.
    pub const fn read_buffer(&self) -> u8 {
        self.read_buffer
    }

    /// Read SMS data port ($BE): return the old buffer, prefetch VRAM,
    /// increment the address and cancel an incomplete control word.
    pub fn read_data(&mut self) -> u8 {
        let value = self.read_buffer;
        self.read_buffer = self.state.memory.vram[usize::from(self.address)];
        self.increment_address();
        self.control_first = None;
        value
    }

    /// Write SMS data port ($BE) using the selected control-port operation.
    pub fn write_data(&mut self, value: u8) {
        match self.code {
            PortCode::CramWrite => {
                let index = usize::from(self.address) & (CRAM_SIZE - 1);
                self.state.memory.cram[index] = value;
            }
            PortCode::VramRead | PortCode::VramWrite | PortCode::RegisterWrite => {
                self.state.memory.vram[usize::from(self.address)] = value;
            }
        }
        self.read_buffer = value;
        self.increment_address();
        self.control_first = None;
    }

    /// Write SMS control port ($BF). The first byte is latched; the second
    /// selects VRAM read/write, register write, or CRAM write.
    pub fn write_control(&mut self, value: u8) {
        let Some(first) = self.control_first.take() else {
            self.control_first = Some(value);
            return;
        };

        self.address = (u16::from(value & 0x3F) << 8) | u16::from(first);
        self.code = match value >> 6 {
            0 => PortCode::VramRead,
            1 => PortCode::VramWrite,
            2 => PortCode::RegisterWrite,
            _ => PortCode::CramWrite,
        };

        match self.code {
            PortCode::VramRead => {
                self.read_buffer = self.state.memory.vram[usize::from(self.address)];
                self.increment_address();
            }
            PortCode::RegisterWrite => {
                let register = usize::from(value & 0x0F);
                // SMS1 exposes registers R0-R10. Higher register numbers are
                // ignored rather than aliased to a valid register.
                if register <= 10 {
                    self.state.registers.values[register] = first;
                }
            }
            PortCode::VramWrite | PortCode::CramWrite => {}
        }
    }

    /// Read SMS control/status port ($BF). Reading status clears all status
    /// flags and abandons a partially written control word.
    pub fn read_status(&mut self) -> u8 {
        let status = self.status;
        self.status = 0;
        self.control_first = None;
        status
    }

    /// Set status bits from the raster/sprite subsystems when those are
    /// integrated. The low five bits are retained as the SMS status field.
    pub fn set_status_bits(&mut self, bits: u8) {
        self.status |= bits;
    }

    /// Read/write port decoder for mirrored SMS VDP port addresses.
    pub fn read_port(&mut self, port: u16) -> u8 {
        match (port as u8) & 0xC1 {
            0x80 => self.read_data(),
            0x81 => self.read_status(),
            _ => 0xFF,
        }
    }

    /// Write port decoder for mirrored SMS VDP port addresses.
    pub fn write_port(&mut self, port: u16, value: u8) {
        match (port as u8) & 0xC1 {
            0x80 => self.write_data(value),
            0x81 => self.write_control(value),
            _ => {}
        }
    }

    /// Reset port latches and registers; optionally preserve VRAM.
    pub fn reset(&mut self, clear_vram: bool) {
        self.state.reset(clear_vram);
        self.address = 0;
        self.read_buffer = 0;
        self.control_first = None;
        self.code = PortCode::VramRead;
        self.status = 0;
    }

    fn increment_address(&mut self) {
        self.address = self.address.wrapping_add(1) & 0x3FFF;
    }
}

#[cfg(test)]
mod port_tests {
    use super::{Sms1Vdp, VideoStandard};

    fn select(vdp: &mut Sms1Vdp, address: u16, code: u8) {
        vdp.write_control(address as u8);
        vdp.write_control(((code & 3) << 6) | ((address >> 8) as u8 & 0x3F));
    }

    #[test]
    fn vram_read_uses_prefetch_buffer_and_wraps_at_16_kib() {
        let mut vdp = Sms1Vdp::new(VideoStandard::Ntsc);
        vdp.state.memory.set_vram_byte(0x3FFF, 0xA5);
        vdp.state.memory.set_vram_byte(0x0000, 0x5A);
        select(&mut vdp, 0x3FFF, 0);

        assert_eq!(vdp.read_buffer(), 0xA5);
        assert_eq!(vdp.address(), 0x0000);
        assert_eq!(vdp.read_data(), 0xA5);
        assert_eq!(vdp.read_data(), 0x5A);
        assert_eq!(vdp.address(), 0x0002);
    }

    #[test]
    fn vram_write_starts_at_selected_address_and_updates_read_buffer() {
        let mut vdp = Sms1Vdp::new(VideoStandard::Ntsc);
        select(&mut vdp, 0x1234, 1);
        assert_eq!(vdp.address(), 0x1234);

        vdp.write_data(0xC7);
        assert_eq!(vdp.state.memory.vram_byte(0x1234), Some(0xC7));
        assert_eq!(vdp.read_buffer(), 0xC7);
        assert_eq!(vdp.address(), 0x1235);
    }

    #[test]
    fn cram_write_wraps_within_32_bytes() {
        let mut vdp = Sms1Vdp::new(VideoStandard::Ntsc);
        select(&mut vdp, 0x003F, 3);
        vdp.write_data(0x2D);
        assert_eq!(vdp.state.memory.cram_byte(0x1F), Some(0x2D));
        assert_eq!(vdp.address(), 0x0040);
    }

    #[test]
    fn register_write_decodes_register_number_from_second_byte() {
        let mut vdp = Sms1Vdp::new(VideoStandard::Ntsc);
        vdp.write_control(0xA8);
        vdp.write_control(0x82);
        assert_eq!(vdp.state.registers.get(2), Some(0xA8));
        assert_eq!(vdp.address(), 0x02A8);
    }

    #[test]
    fn data_access_cancels_partial_control_word() {
        let mut vdp = Sms1Vdp::new(VideoStandard::Ntsc);
        vdp.write_control(0x34);
        vdp.write_data(0x77);
        vdp.write_control(0x12);
        assert_eq!(vdp.address(), 1);
        assert_eq!(vdp.state.memory.vram_byte(0), Some(0x77));
    }

    #[test]
    fn status_read_clears_flags_and_control_latch() {
        let mut vdp = Sms1Vdp::new(VideoStandard::Ntsc);
        vdp.set_status_bits(0xE0);
        vdp.write_control(0x12);
        assert_eq!(vdp.read_status(), 0xE0);
        assert_eq!(vdp.read_status(), 0);
        vdp.write_control(0x34);
        vdp.write_control(0x41);
        assert_eq!(vdp.address(), 0x0134);
    }
}

/// One decoded Mode 4 background pixel. Priority is consumed by the sprite
/// compositor; palette_index retains the hardware CRAM entry for diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BackgroundPixel {
    pub color: [u8; 3],
    pub palette_index: u8,
    pub priority: bool,
}

/// Render one complete Mode 4 background scanline from the current register,
/// VRAM and CRAM image.
///
/// This is a deterministic functional renderer, not a dot-timed fetch model.
/// The caller owns raster scheduling and must provide the line's vertical
/// scroll latch when integrating mid-frame register writes.
pub fn render_mode4_background_line(
    state: &Sms1VdpState,
    screen_y: u16,
    latched_vertical_scroll: u8,
    output: &mut [BackgroundPixel; 256],
) {
    let registers = state.registers.as_array();
    let vram = &state.memory.vram;
    let cram = &state.memory.cram;

    let backdrop = registers[7] & 0x0F;
    let blank = registers[1] & 0x40 == 0 || registers[0] & 0x04 == 0;
    let name_table_base = usize::from(registers[2] & 0x0E) << 10;
    let horizontal_scroll = if registers[0] & 0x40 != 0 && screen_y < 16 {
        0
    } else {
        registers[8]
    };

    for (screen_x, pixel) in output.iter_mut().enumerate() {
        if blank {
            *pixel = decode_background_pixel(cram, backdrop, false);
            continue;
        }

        let fine_scroll = usize::from(horizontal_scroll & 7);
        let coarse_scroll = usize::from(horizontal_scroll & 0xF8);
        let source_x = (screen_x + 256 - coarse_scroll) & 0xFF;
        // SMS Mode 4 does not wrap the fine-scroll gap into the right edge:
        // the exposed left edge is filled with the backdrop color.
        if fine_scroll != 0 && screen_x < fine_scroll {
            *pixel = decode_background_pixel(cram, backdrop, false);
            continue;
        }

        let vertical = if registers[0] & 0x80 == 0 || screen_x < 192 {
            usize::from(latched_vertical_scroll)
        } else {
            0
        };
        let virtual_height = 224usize;
        let source_y = (usize::from(screen_y) + vertical) % virtual_height;

        let tile_x = source_x >> 3;
        let tile_y = source_y >> 3;
        let entry_address = (name_table_base + tile_y * 64 + tile_x * 2) & 0x3FFF;
        let entry =
            u16::from(vram[entry_address]) | (u16::from(vram[(entry_address + 1) & 0x3FFF]) << 8);
        let tile_index = usize::from(entry & 0x01FF);
        let mut pixel_x = source_x & 7;
        let mut pixel_y = source_y & 7;
        if entry & 0x0200 != 0 {
            pixel_x = 7 - pixel_x;
        }
        if entry & 0x0400 != 0 {
            pixel_y = 7 - pixel_y;
        }

        let pattern_address = (tile_index * 32 + pixel_y * 4) & 0x3FFF;
        let bit = 7 - pixel_x;
        let color_index = ((vram[pattern_address] >> bit) & 1)
            | (((vram[pattern_address + 1] >> bit) & 1) << 1)
            | (((vram[pattern_address + 2] >> bit) & 1) << 2)
            | (((vram[pattern_address + 3] >> bit) & 1) << 3);
        let palette_base = if entry & 0x0800 != 0 { 16 } else { 0 };
        let palette_index = palette_base + color_index;
        *pixel =
            decode_background_pixel(cram, palette_index, entry & 0x1000 != 0 && color_index != 0);
    }
}

fn decode_background_pixel(cram: &[u8; CRAM_SIZE], index: u8, priority: bool) -> BackgroundPixel {
    let value = cram[usize::from(index & 0x1F)];
    let red = (value >> 4) & 0x03;
    let green = (value >> 2) & 0x03;
    let blue = value & 0x03;
    BackgroundPixel {
        color: [red * 85, green * 85, blue * 85],
        palette_index: index & 0x1F,
        priority,
    }
}

#[cfg(test)]
mod background_tests {
    use super::{BackgroundPixel, Sms1VdpState, VideoStandard, render_mode4_background_line};

    fn put_tile(state: &mut Sms1VdpState, tile: usize, color: u8) {
        let base = tile * 32;
        for y in 0..8 {
            for plane in 0..4 {
                state.memory.vram[base + y * 4 + plane] = if color & (1 << plane) != 0 {
                    0xFF
                } else {
                    0x00
                };
            }
        }
    }

    #[test]
    fn decodes_mode4_planar_pixels_and_palette_channels() {
        let mut state = Sms1VdpState::new(VideoStandard::Ntsc);
        state.registers.set_raw(0, 0x04);
        state.registers.set_raw(1, 0x40);
        state.registers.set_raw(2, 0x0E);
        state.memory.vram[0x3800] = 1;
        state.memory.vram[0x3801] = 0;
        put_tile(&mut state, 1, 0b1011);
        state.memory.cram[11] = 0b11_01_10;

        let mut line = [BackgroundPixel {
            color: [0; 3],
            palette_index: 0,
            priority: false,
        }; 256];
        render_mode4_background_line(&state, 0, 0, &mut line);

        assert_eq!(line[0].palette_index, 11);
        assert_eq!(line[0].color, [255, 85, 170]);
    }

    #[test]
    fn decodes_horizontal_and_vertical_tile_flips_and_priority() {
        let mut state = Sms1VdpState::new(VideoStandard::Ntsc);
        state.registers.set_raw(0, 0x04);
        state.registers.set_raw(1, 0x40);
        state.registers.set_raw(2, 0x0E);
        // Tile 1, both flips and priority enabled.
        state.memory.vram[0x3800] = 1;
        state.memory.vram[0x3801] = 0x16;
        put_tile(&mut state, 1, 0b0010);
        // Change the first and last source columns so the flip is observable.
        state.memory.vram[32] = 0x80;
        state.memory.vram[32 + 7 * 4] = 0x00;
        state.memory.cram[2] = 0x0C;

        let mut line = [BackgroundPixel {
            color: [0; 3],
            palette_index: 0,
            priority: false,
        }; 256];
        render_mode4_background_line(&state, 0, 0, &mut line);

        assert!(line[0].priority);
        assert_eq!(line[0].palette_index, 2);
    }

    #[test]
    fn blanking_uses_sprite_palette_backdrop() {
        let mut state = Sms1VdpState::new(VideoStandard::Ntsc);
        state.registers.set_raw(7, 3);
        state.memory.cram[3] = 0x27;
        let mut line = [BackgroundPixel {
            color: [0; 3],
            palette_index: 0,
            priority: false,
        }; 256];
        render_mode4_background_line(&state, 0, 0, &mut line);
        assert_eq!(line[0].palette_index, 3);
        assert_eq!(line[0].color, [170, 85, 255]);
    }

    #[test]
    fn horizontal_scroll_inhibit_keeps_first_two_rows_fixed() {
        let mut state = Sms1VdpState::new(VideoStandard::Ntsc);
        state.registers.set_raw(0, 0x44);
        state.registers.set_raw(1, 0x40);
        state.registers.set_raw(2, 0x0E);
        state.registers.set_raw(8, 8);
        // First map tile is color 1, second is color 2.
        state.memory.vram[0x3800] = 1;
        state.memory.vram[0x3802] = 2;
        state.memory.vram[0x383E] = 2;
        put_tile(&mut state, 1, 1);
        put_tile(&mut state, 2, 2);
        put_tile(&mut state, 3, 3);

        let mut line = [BackgroundPixel {
            color: [0; 3],
            palette_index: 0,
            priority: false,
        }; 256];
        render_mode4_background_line(&state, 0, 0, &mut line);
        assert_eq!(line[0].palette_index, 1);
        render_mode4_background_line(&state, 16, 0, &mut line);
        assert_eq!(line[0].palette_index, 2);
    }

    #[test]
    fn name_table_base_uses_register_two_alignment() {
        let mut state = Sms1VdpState::new(VideoStandard::Ntsc);
        state.registers.set_raw(0, 0x04);
        state.registers.set_raw(1, 0x40);
        state.registers.set_raw(2, 0x0A);
        state.memory.vram[0x2800] = 1;
        put_tile(&mut state, 1, 4);
        let mut line = [BackgroundPixel {
            color: [0; 3],
            palette_index: 0,
            priority: false,
        }; 256];
        render_mode4_background_line(&state, 0, 0, &mut line);
        assert_eq!(line[0].palette_index, 4);
    }

    #[test]
    fn vertical_scroll_advances_source_rows_with_224_line_wrap() {
        let mut state = Sms1VdpState::new(VideoStandard::Ntsc);
        state.registers.set_raw(0, 0x04);
        state.registers.set_raw(1, 0x40);
        state.registers.set_raw(2, 0x0E);
        // Distinguish the first two name-table rows.
        state.memory.vram[0x3800] = 1;
        state.memory.vram[0x3840] = 2;
        put_tile(&mut state, 1, 1);
        put_tile(&mut state, 2, 2);
        let mut line = [BackgroundPixel {
            color: [0; 3],
            palette_index: 0,
            priority: false,
        }; 256];

        render_mode4_background_line(&state, 0, 8, &mut line);
        assert_eq!(line[0].palette_index, 2);
        render_mode4_background_line(&state, 223, 8, &mut line);
        assert_eq!(line[0].palette_index, 1);
    }

    #[test]
    fn fine_horizontal_scroll_exposes_backdrop_at_left_edge_without_wrapping() {
        let mut state = Sms1VdpState::new(VideoStandard::Ntsc);
        state.registers.set_raw(0, 0x04);
        state.registers.set_raw(1, 0x40);
        state.registers.set_raw(2, 0x0E);
        state.registers.set_raw(7, 3);
        state.registers.set_raw(8, 1);
        state.memory.vram[0x3800] = 1;
        state.memory.vram[0x3802] = 2;
        put_tile(&mut state, 1, 1);
        put_tile(&mut state, 2, 2);
        state.memory.cram[3] = 0x30;

        let mut line = [BackgroundPixel {
            color: [0; 3],
            palette_index: 0,
            priority: false,
        }; 256];
        render_mode4_background_line(&state, 0, 0, &mut line);

        assert_eq!(line[0].palette_index, 3);
        assert_eq!(line[1].palette_index, 1);
    }
}

/// Render the SMS1 Mode 4 sprite layer over a previously rendered background
/// scanline. Returns status bits raised by this line (overflow/collision).
///
/// Sprite evaluation follows SAT order and retains at most eight sprites.
/// This is a functional scanline model; it does not model per-dot SAT fetches,
/// the internal sprite shifter, or the exact HBlank evaluation phase.
/// Composite the SMS1 Mode 4 sprite plane over a rendered background line.
///
/// Returns the overflow and collision status bits generated by this line.
/// This is scanline-functional behavior, not dot-cycle timing.
pub fn render_mode4_sprites_line(
    state: &Sms1VdpState,
    screen_y: u16,
    background: &mut [BackgroundPixel; 256],
) -> u8 {
    const SPRITE_OVERFLOW: u8 = 0x40;
    const SPRITE_COLLISION: u8 = 0x20;
    const SPRITE_LIMIT: usize = 8;
    const SAT_TERMINATOR: u8 = 0xD0;

    if screen_y >= 192 || state.registers.as_array()[1] & 0x40 == 0 {
        return 0;
    }

    let registers = state.registers.as_array();
    let vram = &state.memory.vram;
    let sat_base = (usize::from(registers[5] & 0x7E) << 7) & 0x3F00;
    let sprite_pattern_base = if registers[6] & 0x04 != 0 { 0x2000 } else { 0 };
    let tall = registers[1] & 0x02 != 0;
    let magnified = registers[1] & 0x01 != 0;
    let sprite_height = if tall { 16usize } else { 8usize };
    let display_height = sprite_height * if magnified { 2 } else { 1 };
    let x_shift = if registers[0] & 0x08 != 0 { 8i16 } else { 0i16 };

    let mut selected = [(0usize, 0u8, 0u8, 0usize); SPRITE_LIMIT];
    let mut selected_count = 0usize;
    let mut overflow = false;

    for sprite in 0..64usize {
        let y = vram[(sat_base + sprite) & 0x3FFF];
        if y == SAT_TERMINATOR {
            break;
        }
        let top = i16::from(y.wrapping_add(1));
        let relative = i16::from(screen_y as u8) - top;
        if relative < 0 || relative >= display_height as i16 {
            continue;
        }
        if selected_count == SPRITE_LIMIT {
            overflow = true;
            break;
        }
        let x = vram[(sat_base + 0x80 + sprite * 2) & 0x3FFF];
        let tile = vram[(sat_base + 0x81 + sprite * 2) & 0x3FFF];
        let source_y = relative as usize / if magnified { 2 } else { 1 };
        selected[selected_count] = (sprite, x, tile, source_y);
        selected_count += 1;
    }

    let mut collision = false;
    let mut occupied = [false; 256];
    for (rank, &(_, x, tile, source_y)) in selected[..selected_count].iter().enumerate() {
        let horizontal_double = magnified && rank < selected_count.saturating_sub(4);
        let width_scale = if horizontal_double { 2usize } else { 1usize };
        let tile_number = if tall {
            usize::from(tile & 0xFE) + source_y / 8
        } else {
            usize::from(tile)
        };
        let pattern_row = source_y & 7;
        let pattern_address = (sprite_pattern_base + tile_number * 32 + pattern_row * 4) & 0x3FFF;
        let x_origin = i16::from(x) - x_shift;

        for source_x in 0..8usize {
            let bit = 7 - source_x;
            let color_index = ((vram[pattern_address] >> bit) & 1)
                | (((vram[pattern_address + 1] >> bit) & 1) << 1)
                | (((vram[pattern_address + 2] >> bit) & 1) << 2)
                | (((vram[pattern_address + 3] >> bit) & 1) << 3);
            if color_index == 0 {
                continue;
            }
            for duplicate in 0..width_scale {
                let x_screen = x_origin + (source_x * width_scale + duplicate) as i16;
                if !(0..256).contains(&x_screen) {
                    continue;
                }
                let x_screen = x_screen as usize;
                let was_occupied = occupied[x_screen];
                if was_occupied {
                    collision = true;
                } else {
                    occupied[x_screen] = true;
                }
                if !was_occupied {
                    let bg = background[x_screen];
                    if !(bg.priority && (bg.palette_index & 0x0F) != 0) {
                        let palette_index = 16 + color_index;
                        background[x_screen] =
                            decode_background_pixel(&state.memory.cram, palette_index, false);
                    }
                }
            }
        }
    }

    (if overflow { SPRITE_OVERFLOW } else { 0 }) | (if collision { SPRITE_COLLISION } else { 0 })
}

#[cfg(test)]
mod sprite_tests {
    use super::{BackgroundPixel, Sms1VdpState, VideoStandard, render_mode4_sprites_line};

    fn set_sprite(state: &mut Sms1VdpState, index: usize, y: u8, x: u8, tile: u8) {
        let sat = usize::from(state.registers.get(5).unwrap_or(0) & 0x7E) << 7;
        state.memory.vram[sat + index] = y;
        state.memory.vram[sat + 0x80 + index * 2] = x;
        state.memory.vram[sat + 0x81 + index * 2] = tile;
    }

    fn solid_tile(state: &mut Sms1VdpState, tile: usize, color: u8) {
        let base = tile * 32;
        for row in 0..8 {
            for plane in 0..4 {
                state.memory.vram[base + row * 4 + plane] =
                    if color & (1 << plane) != 0 { 0xFF } else { 0 };
            }
        }
    }

    fn background() -> [BackgroundPixel; 256] {
        [BackgroundPixel {
            color: [0; 3],
            palette_index: 0,
            priority: false,
        }; 256]
    }

    #[test]
    fn first_sprite_is_composited_from_sprite_palette() {
        let mut state = Sms1VdpState::new(VideoStandard::Ntsc);
        state.registers.set_raw(1, 0x40);
        state.registers.set_raw(5, 0x7F);
        set_sprite(&mut state, 0, 0xFF, 10, 1);
        solid_tile(&mut state, 1, 5);
        let mut line = background();
        let flags = render_mode4_sprites_line(&state, 0, &mut line);
        assert_eq!(flags, 0);
        assert_eq!(line[10].palette_index, 21);
        assert_eq!(line[17].palette_index, 21);
    }

    #[test]
    fn ninth_sprite_sets_overflow_even_when_offscreen() {
        let mut state = Sms1VdpState::new(VideoStandard::Ntsc);
        state.registers.set_raw(1, 0x40);
        state.registers.set_raw(5, 0x7F);
        for index in 0..9 {
            set_sprite(&mut state, index, 0xFF, 250, 0);
        }
        let mut line = background();
        let flags = render_mode4_sprites_line(&state, 0, &mut line);
        assert_eq!(flags & 0x40, 0x40);
    }

    #[test]
    fn overlapping_opaque_sprites_set_collision_and_keep_earlier_sprite() {
        let mut state = Sms1VdpState::new(VideoStandard::Ntsc);
        state.registers.set_raw(1, 0x40);
        state.registers.set_raw(5, 0x7F);
        set_sprite(&mut state, 0, 0xFF, 20, 1);
        set_sprite(&mut state, 1, 0xFF, 20, 2);
        solid_tile(&mut state, 1, 3);
        solid_tile(&mut state, 2, 6);
        let mut line = background();
        let flags = render_mode4_sprites_line(&state, 0, &mut line);
        assert_eq!(flags & 0x20, 0x20);
        assert_eq!(line[20].palette_index, 19);
    }

    #[test]
    fn sprite_priority_respects_opaque_high_priority_background() {
        let mut state = Sms1VdpState::new(VideoStandard::Ntsc);
        state.registers.set_raw(1, 0x40);
        state.registers.set_raw(5, 0x7F);
        set_sprite(&mut state, 0, 0xFF, 30, 1);
        solid_tile(&mut state, 1, 4);
        let mut line = background();
        line[30] = BackgroundPixel {
            color: [0; 3],
            palette_index: 2,
            priority: true,
        };
        render_mode4_sprites_line(&state, 0, &mut line);
        assert_eq!(line[30].palette_index, 2);
    }

    #[test]
    fn sms1_horizontal_zoom_skips_last_four_selected_sprites() {
        let mut state = Sms1VdpState::new(VideoStandard::Ntsc);
        state.registers.set_raw(1, 0x41);
        state.registers.set_raw(5, 0x7F);
        for index in 0..6 {
            set_sprite(&mut state, index, 0xFF, (index * 20) as u8, 1);
        }
        solid_tile(&mut state, 1, 1);
        let mut line = background();
        render_mode4_sprites_line(&state, 0, &mut line);
        assert_eq!(line[0].palette_index, 17);
        assert_eq!(line[15].palette_index, 17);
        assert_eq!(line[16].palette_index, 0);
        assert_eq!(line[40].palette_index, 17);
        assert_eq!(line[47].palette_index, 17);
        assert_eq!(line[48].palette_index, 0);
    }

    #[test]
    fn tall_magnified_sprite_uses_even_tile_pair_and_vertical_duplication() {
        let mut state = Sms1VdpState::new(VideoStandard::Ntsc);
        state.registers.set_raw(1, 0x43);
        state.registers.set_raw(5, 0x7F);
        set_sprite(&mut state, 0, 0xFF, 40, 3);
        solid_tile(&mut state, 2, 2);
        solid_tile(&mut state, 3, 7);
        let mut line = background();

        render_mode4_sprites_line(&state, 16, &mut line);

        assert_eq!(line[40].palette_index, 23);
        assert_eq!(line[41].palette_index, 23);
    }
}
