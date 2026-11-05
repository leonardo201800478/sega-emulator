//! Explicit hardware configuration for an SMS machine.

/// Physical console family selected for compatibility and machine-level defaults.
///
/// This field identifies the console family only. It does not imply a particular
/// VDP ASIC revision; that hardware identity is configured independently.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SmsConsoleModel {
    Sms1,
    /// Default console family for new configurations.
    #[default]
    Sms2,
}

pub use sega_video::sms::{
    SmsVdpProfile, SmsVdpRevision, SpriteLimitMode, VideoStandard,
};

/// Console territory/model family. This is not inferred from video standard.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Territory {
    Japan,
    Export,
    Unknown,
}

/// Whether the modeled console has an installed YM2413-compatible FM unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FmHardware {
    Absent,
    Ym2413Present,
}

/// Immutable hardware profile supplied when constructing an SMS machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmsMachineConfig {
    /// Console family; independent from the installed VDP ASIC.
    pub console_model: SmsConsoleModel,
    /// Exact VDP ASIC revision selected for this machine instance.
    pub vdp_revision: SmsVdpRevision,
    pub territory: Territory,
    pub video_standard: VideoStandard,
    pub fm_hardware: FmHardware,
    pub sprite_limit: SpriteLimitMode,
}

impl Default for SmsMachineConfig {
    fn default() -> Self {
        Self {
            console_model: SmsConsoleModel::Sms2,
            vdp_revision: SmsVdpRevision::Sega3155246,
            territory: Territory::Export,
            video_standard: VideoStandard::Ntsc,
            fm_hardware: FmHardware::Absent,
            sprite_limit: SpriteLimitMode::HardwareLimit,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        FmHardware, SmsConsoleModel, SmsMachineConfig, SmsVdpRevision, SpriteLimitMode, Territory,
        VideoStandard,
    };

    #[test]
    fn defaults_to_sms2_with_315_5246_and_export_ntsc_profile() {
        assert_eq!(
            SmsMachineConfig::default(),
            SmsMachineConfig {
                console_model: SmsConsoleModel::Sms2,
                vdp_revision: SmsVdpRevision::Sega3155246,
                territory: Territory::Export,
                video_standard: VideoStandard::Ntsc,
                fm_hardware: FmHardware::Absent,
                sprite_limit: SpriteLimitMode::HardwareLimit,
            }
        );
    }

    #[test]
    fn console_model_and_vdp_revision_are_independent_fields() {
        let config = SmsMachineConfig {
            console_model: SmsConsoleModel::Sms1,
            vdp_revision: SmsVdpRevision::Sega3155246,
            territory: Territory::Japan,
            video_standard: VideoStandard::Pal,
            fm_hardware: FmHardware::Absent,
            sprite_limit: SpriteLimitMode::UnlockedCompatibility,
        };
        assert_eq!(config.console_model, SmsConsoleModel::Sms1);
        assert_eq!(config.vdp_revision, SmsVdpRevision::Sega3155246);
        assert_eq!(config.territory, Territory::Japan);
        assert_eq!(config.video_standard, VideoStandard::Pal);
    }
}
