//! New SMS1/315-5124 VDP foundation.
//!
//! This crate is deliberately independent from the legacy scanline renderer.
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
