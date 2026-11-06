//! Shared Sega video hardware profiles and device cores.
//!
//! The initial crate boundary hosts profile/capability types; hardware-device
//! migrations are incremental. This crate is intentionally separate from
//! sega-video-runtime, which transports completed frames to host presentation.

#![forbid(unsafe_code)]

pub mod sms;
