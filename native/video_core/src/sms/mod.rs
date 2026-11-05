//! Sega Master System VDP profiles and capability contracts; device migration is in progress.

mod profile;
mod vdp;

pub use profile::{
    SmsVdpCapabilities, SmsVdpProfile, SmsVdpRevision, SpriteLimitMode, VideoStandard,
};
pub use vdp::{
    SMS_VDP_MAX_HEIGHT, SMS_VDP_WIDTH, SmsRasterPosition, SmsVdp, SmsVdpEye,
    StereoPresentation,
};
