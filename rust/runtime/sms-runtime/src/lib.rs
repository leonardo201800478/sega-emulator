//! Python-independent runtime for the temporally scheduled Sega Master System.

#![forbid(unsafe_code)]

mod audio;
mod worker;

pub use audio::AudioSession;
pub use sega_audio_runtime::AnalogOutputConfig;
pub use sega_sms_core::SpriteLimitMode;
pub use worker::{
    AudioRendererMode, SmsDebugSnapshot, SmsFramePacingMetrics, SmsTemporalWorker,
    SmsTemporalWorkerDebugHandle, sms_temporal_region_profile,
};
