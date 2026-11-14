//! Shared console identity and renderer selection for the Sega VDP family.
//!
//! This is the first migration boundary only. Device construction and
//! profile-specific hardware state are added in later migration steps.

/// Console/ASIC target supported by the unified video subsystem.
///
/// SMS1 and SMS2 are deliberately separate identities even though they share
/// the Mode 4 engine. Game Gear also uses Mode 4 but has distinct color-RAM and
/// output-aperture semantics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VideoSystem {
    /// Sega Master System 1, VDP ASIC 315-5124.
    Sms1,
    /// Sega Master System 2, VDP ASIC 315-5246.
    Sms2,
    /// Sega SG-1000 with a TMS9918A-compatible VDP.
    Sg1000,
    /// Sega Game Gear handheld, using Mode 4 with handheld-specific output.
    GameGear,
}

/// Specialized hardware engine selected for a console profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VideoEngineKind {
    /// TMS9918A-compatible legacy video engine.
    Tms9918A,
    /// Sega Mode 4 video engine shared by SMS1, SMS2 and Game Gear.
    Mode4,
}

impl VideoSystem {
    /// Return the specialized engine required by this system.
    pub const fn engine(self) -> VideoEngineKind {
        match self {
            Self::Sg1000 => VideoEngineKind::Tms9918A,
            Self::Sms1 | Self::Sms2 | Self::GameGear => VideoEngineKind::Mode4,
        }
    }

    /// Return the implementation priority for the staged VDP migration.
    /// Lower values are scheduled first; this is project sequencing, not
    /// hardware timing or runtime scheduling priority.
    pub const fn migration_priority(self) -> u8 {
        match self {
            Self::Sms1 => 1,
            Self::Sms2 => 2,
            Self::Sg1000 => 3,
            Self::GameGear => 4,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{VideoEngineKind, VideoSystem};

    #[test]
    fn system_profiles_select_the_correct_specialized_engine() {
        assert_eq!(VideoSystem::Sms1.engine(), VideoEngineKind::Mode4);
        assert_eq!(VideoSystem::Sms2.engine(), VideoEngineKind::Mode4);
        assert_eq!(VideoSystem::Sg1000.engine(), VideoEngineKind::Tms9918A);
        assert_eq!(VideoSystem::GameGear.engine(), VideoEngineKind::Mode4);
    }

    #[test]
    fn migration_order_is_sms1_sms2_sg1000_then_game_gear() {
        assert_eq!(VideoSystem::Sms1.migration_priority(), 1);
        assert_eq!(VideoSystem::Sms2.migration_priority(), 2);
        assert_eq!(VideoSystem::Sg1000.migration_priority(), 3);
        assert_eq!(VideoSystem::GameGear.migration_priority(), 4);
    }
}
