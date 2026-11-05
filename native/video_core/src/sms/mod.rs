//! Sega Master System VDP profiles and capability contracts; device migration is in progress.

mod profile;

pub use profile::{
    SmsVdpCapabilities, SmsVdpProfile, SmsVdpRevision, SpriteLimitMode, VideoStandard,
};
