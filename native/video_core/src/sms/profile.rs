//! SMS VDP ASIC identity, independent video-standard selection and capabilities.

/// VDP ASIC revision installed in the modeled console.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SmsVdpRevision {
    /// SMS1 VDP ASIC.
    Sega3155124,
    /// SMS2 VDP ASIC; retained as the legacy configuration default.
    #[default]
    Sega3155246,
}

impl SmsVdpRevision {
    /// Return the numeric Sega ASIC identifier.
    pub const fn asic_number(self) -> u16 {
        match self {
            Self::Sega3155124 => 315_5124,
            Self::Sega3155246 => 315_5246,
        }
    }
}

/// Video timing family, independent from console territory and VDP revision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VideoStandard {
    /// NTSC-family timing.
    #[default]
    Ntsc,
    /// Brazil PAL-M: NTSC-like 525/60 timing with a distinct regional profile.
    PalM,
    /// PAL-family timing.
    Pal,
    /// Timing family has not been identified.
    Unknown,
}

/// Sprite overflow policy. Unlocked mode is a compatibility enhancement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SpriteLimitMode {
    /// Preserve the hardware line-buffer limit and overflow behavior.
    #[default]
    HardwareLimit,
    /// Draw beyond the hardware line-buffer limit for compatibility experiments.
    UnlockedCompatibility,
}

/// Hardware capabilities supported by a selected SMS VDP ASIC profile.
///
/// Unknown raster/overscan timing is deliberately represented as unavailable;
/// consumers must not synthesize values from another console or emulator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmsVdpCapabilities {
    /// Installed VDP ASIC.
    pub revision: SmsVdpRevision,
    /// Supported active image heights for this ASIC.
    pub active_height_modes: &'static [u16],
    /// VRAM capacity in bytes.
    pub vram_bytes: u16,
    /// Number of CRAM entries.
    pub cram_entries: u8,
    /// Number of digital color bits per CRAM entry.
    pub cram_bits: u8,
    /// Number of sprite attribute entries.
    pub sprite_entries: u8,
    /// Hardware sprite line-buffer capacity.
    pub sprites_per_line: u8,
    /// Width of the R0 bit-5 left-column mask in active pixels.
    pub left_column_mask_pixels: u8,
    /// Horizontal sprite displacement when R0 bit 3 is set.
    pub sprite_shift_pixels: i8,
    /// True only after the ASIC's overscan rectangles are validated.
    pub overscan_geometry_validated: bool,
    /// True only after the profile's sub-line raster timing is validated.
    pub subline_timing_validated: bool,
}

impl SmsVdpCapabilities {
    const SMS1_HEIGHTS: &'static [u16] = &[192];
    const SMS2_HEIGHTS: &'static [u16] = &[192, 224, 240];

    /// Return the documented baseline capabilities for an ASIC revision.
    pub const fn for_revision(revision: SmsVdpRevision) -> Self {
        let active_height_modes = match revision {
            SmsVdpRevision::Sega3155124 => Self::SMS1_HEIGHTS,
            SmsVdpRevision::Sega3155246 => Self::SMS2_HEIGHTS,
        };

        Self {
            revision,
            active_height_modes,
            vram_bytes: 16 * 1024,
            cram_entries: 32,
            cram_bits: 6,
            sprite_entries: 64,
            sprites_per_line: 8,
            left_column_mask_pixels: 8,
            sprite_shift_pixels: -8,
            overscan_geometry_validated: false,
            subline_timing_validated: false,
        }
    }
}

/// Immutable SMS VDP profile used to construct a device.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmsVdpProfile {
    /// Installed VDP ASIC revision.
    pub revision: SmsVdpRevision,
    /// Regional video standard; not inferred from the ASIC revision.
    pub video_standard: VideoStandard,
    /// Sprite line-limit policy.
    pub sprite_limit: SpriteLimitMode,
}

impl SmsVdpProfile {
    /// Construct an explicit VDP profile.
    pub const fn new(
        revision: SmsVdpRevision,
        video_standard: VideoStandard,
        sprite_limit: SpriteLimitMode,
    ) -> Self {
        Self {
            revision,
            video_standard,
            sprite_limit,
        }
    }

    /// Initial evidence-backed target: SMS1 315-5124, NTSC, hardware sprite limit.
    pub const fn sms1_t0_ntsc() -> Self {
        Self::new(
            SmsVdpRevision::Sega3155124,
            VideoStandard::Ntsc,
            SpriteLimitMode::HardwareLimit,
        )
    }

    /// Return capabilities without inventing unvalidated raster geometry.
    pub const fn capabilities(self) -> SmsVdpCapabilities {
        SmsVdpCapabilities::for_revision(self.revision)
    }

    /// Return true when this ASIC profile documents the requested active height.
    pub fn supports_active_height(self, height: u16) -> bool {
        self.capabilities().active_height_modes.contains(&height)
    }
}

#[cfg(test)]
mod tests {
    use super::{SmsVdpCapabilities, SmsVdpProfile, SmsVdpRevision, SpriteLimitMode, VideoStandard};

    #[test]
    fn initial_sms1_target_is_315_5124_ntsc_with_hardware_sprite_limit() {
        let profile = SmsVdpProfile::sms1_t0_ntsc();

        assert_eq!(profile.revision, SmsVdpRevision::Sega3155124);
        assert_eq!(profile.video_standard, VideoStandard::Ntsc);
        assert_eq!(profile.sprite_limit, SpriteLimitMode::HardwareLimit);
        assert_eq!(profile.capabilities().active_height_modes, &[192]);
    }

    #[test]
    fn sms2_height_modes_are_profile_specific() {
        let sms1 = SmsVdpProfile::new(
            SmsVdpRevision::Sega3155124,
            VideoStandard::Ntsc,
            SpriteLimitMode::HardwareLimit,
        );
        let sms2 = SmsVdpProfile::new(
            SmsVdpRevision::Sega3155246,
            VideoStandard::Ntsc,
            SpriteLimitMode::HardwareLimit,
        );

        assert!(!sms1.supports_active_height(224));
        assert!(!sms1.supports_active_height(240));
        assert!(sms2.supports_active_height(224));
        assert!(sms2.supports_active_height(240));
    }

    #[test]
    fn unverified_overscan_and_subline_timing_are_not_advertised() {
        for revision in [SmsVdpRevision::Sega3155124, SmsVdpRevision::Sega3155246] {
            let capabilities = SmsVdpCapabilities::for_revision(revision);
            assert!(!capabilities.overscan_geometry_validated);
            assert!(!capabilities.subline_timing_validated);
        }
    }

    #[test]
    fn unlocked_sprite_policy_does_not_change_hardware_capabilities() {
        let hardware = SmsVdpProfile::new(
            SmsVdpRevision::Sega3155124,
            VideoStandard::Ntsc,
            SpriteLimitMode::HardwareLimit,
        );
        let unlocked = SmsVdpProfile::new(
            SmsVdpRevision::Sega3155124,
            VideoStandard::Ntsc,
            SpriteLimitMode::UnlockedCompatibility,
        );

        assert_eq!(hardware.capabilities(), unlocked.capabilities());
        assert_eq!(hardware.capabilities().sprites_per_line, 8);
    }

    #[test]
    fn standard_and_asic_revision_are_independent() {
        let profile = SmsVdpProfile::new(
            SmsVdpRevision::Sega3155124,
            VideoStandard::Pal,
            SpriteLimitMode::HardwareLimit,
        );

        assert_eq!(profile.revision.asic_number(), 315_5124);
        assert_eq!(profile.video_standard, VideoStandard::Pal);
    }
}
