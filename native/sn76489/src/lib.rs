//! Shared SN76489-family PSG core for Sega console hardware profiles.
//! Machine clocks are supplied by the owning machine/Scheduler.
mod psg;
pub use psg::{H2StreamingError, H2StreamingMetrics, H2StreamingRenderer, PsgAmplitudeEvent, PsgError, PsgEventCaptureError, PsgProfile, Sn76489Core};
