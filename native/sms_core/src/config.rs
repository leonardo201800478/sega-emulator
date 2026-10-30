//! Explicit hardware configuration for an SMS machine.

/// Console territory/model family. This is not inferred from video standard.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Territory {
    Japan,
    Export,
    Unknown,
}

/// Video timing family, kept separate from territory and console revision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoStandard {
    Ntsc,
    /// Brazil PAL-M: NTSC-like 525/60 timing with a distinct regional profile.
    /// The RGB framebuffer does not model composite PAL-M chroma encoding.
    PalM,
    Pal,
    Unknown,
}

/// Whether the modeled console has an installed YM2413-compatible FM unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FmHardware {
    Absent,
    Ym2413Present,
}

/// Unlocked mode is a compatibility enhancement, not a faithful hardware mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpriteLimitMode {
    HardwareLimit,
    UnlockedCompatibility,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmsMachineConfig {
    pub territory: Territory,
    pub video_standard: VideoStandard,
    pub fm_hardware: FmHardware,
    pub sprite_limit: SpriteLimitMode,
}

impl Default for SmsMachineConfig {
    fn default() -> Self {
        Self {
            territory: Territory::Export,
            video_standard: VideoStandard::Ntsc,
            fm_hardware: FmHardware::Absent,
            sprite_limit: SpriteLimitMode::HardwareLimit,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{FmHardware, SmsMachineConfig, SpriteLimitMode, Territory, VideoStandard};

    #[test]
    fn defaults_to_export_ntsc_without_fm_and_with_hardware_sprite_limit() {
        assert_eq!(
            SmsMachineConfig::default(),
            SmsMachineConfig {
                territory: Territory::Export,
                video_standard: VideoStandard::Ntsc,
                fm_hardware: FmHardware::Absent,
                sprite_limit: SpriteLimitMode::HardwareLimit,
            }
        );
    }

    #[test]
    fn territory_video_standard_and_fm_are_independent_fields() {
        let config = SmsMachineConfig {
            territory: Territory::Japan,
            video_standard: VideoStandard::Pal,
            fm_hardware: FmHardware::Absent,
            sprite_limit: SpriteLimitMode::UnlockedCompatibility,
        };
        assert_eq!(config.territory, Territory::Japan);
        assert_eq!(config.video_standard, VideoStandard::Pal);
        assert_eq!(config.fm_hardware, FmHardware::Absent);
    }
}
