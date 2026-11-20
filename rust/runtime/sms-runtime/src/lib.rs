//! Python-independent runtime for the temporally scheduled Sega Master System.

#![forbid(unsafe_code)]

mod audio;
mod worker;

pub use audio::AudioSession;
pub use sega_audio_runtime::AnalogOutputConfig;
pub use sega_observability::DiagnosticsConsent;
pub use sega_sms_core::{
    SmsOverscanMode, SmsTemporalMachineState, SmsVdpRevision, SpriteLimitMode,
};
pub use worker::{
    AudioRendererMode, SMS_WORKER_SAVE_STATE_VERSION, SmsDebugSnapshot, SmsFramePacingMetrics,
    SmsTemporalWorker, SmsTemporalWorkerDebugHandle, SmsWorkerSaveState,
    sms_temporal_hardware_profile, sms_temporal_region_profile,
};
