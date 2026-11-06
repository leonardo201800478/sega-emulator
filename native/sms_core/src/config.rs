//! Hardware profile primitives shared by the temporal SMS machine.

pub use sega_video::sms::{SmsVdpProfile, SmsVdpRevision, SpriteLimitMode, VideoStandard};

/// Console territory. This is independent from the selected video standard.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Territory {
    Japan,
    Export,
    Unknown,
}
