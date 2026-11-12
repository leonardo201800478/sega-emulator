//! Python-independent runtime for the temporally scheduled Sega Master System.

#![forbid(unsafe_code)]

mod audio;
mod worker;

pub use audio::AudioSession;
pub use sega_sms_core::SpriteLimitMode;
pub use worker::{
    SmsDebugSnapshot, SmsFramePacingMetrics, SmsTemporalWorker, SmsTemporalWorkerDebugHandle,
    sms_temporal_region_profile,
};
