//! Shared Sega video hardware profiles and device cores.
//!
//! This crate owns hardware video devices. It is intentionally separate from
//! sega-video-runtime, which transports completed frames to host presentation.

#![forbid(unsafe_code)]

pub mod sms;
