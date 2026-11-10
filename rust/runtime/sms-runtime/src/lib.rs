//! Python-independent runtime for the temporally scheduled Sega Master System.

#![forbid(unsafe_code)]

mod audio;
mod worker;

pub use audio::AudioSession;
pub use worker::{
    SmsDebugSnapshot, SmsTemporalWorker, SmsTemporalWorkerDebugHandle, sms_temporal_region_profile,
};
