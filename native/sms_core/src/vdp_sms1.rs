//! New SMS1/315-5124 VDP foundation, hosted by the SMS core crate.
//!
//! This module is deliberately independent from the legacy scanline renderer.
//! It currently owns only revision/profile identity and raw device state.
//! Port semantics, raster timing, fetch, sprites, pixels and IRQ are added
//! only when their contracts are supported by the SMS1 evidence matrix.

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
        vdp.write_control(0xBF, address as u8);
        vdp.write_control(0xBF, ((code & 3) << 6) | ((address >> 8) as u8 & 0x3F));
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
        assert_eq!(vdp.address(), 0x0001);
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
        vdp.write_control(0xBF, 0xA8);
        vdp.write_control(0xBF, 0x82);
        assert_eq!(vdp.state.registers.get(2), Some(0xA8));
        assert_eq!(vdp.address(), 0x02A8);
    }

    #[test]
    fn data_access_cancels_partial_control_word() {
        let mut vdp = Sms1Vdp::new(VideoStandard::Ntsc);
        vdp.write_control(0xBF, 0x34);
        vdp.write_data(0x77);
        vdp.write_control(0xBF, 0x12);
        assert_eq!(vdp.address(), 0);
        assert_eq!(vdp.state.memory.vram_byte(0), Some(0x77));
    }

    #[test]
    fn status_read_clears_flags_and_control_latch() {
        let mut vdp = Sms1Vdp::new(VideoStandard::Ntsc);
        vdp.set_status_bits(0xE0);
        vdp.write_control(0xBF, 0x12);
        assert_eq!(vdp.read_status(), 0xE0);
        assert_eq!(vdp.read_status(), 0);
        vdp.write_control(0xBF, 0x34);
        vdp.write_control(0xBF, 0x41);
        assert_eq!(vdp.address(), 0x0134);
    }
}
