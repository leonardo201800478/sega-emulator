//! Shared Sega video hardware profiles and device cores.
//!
//! The crate owns the unified VDP family boundary. Hardware engines are
//! migrated incrementally while preserving their existing semantics. This
//! crate remains separate from host presentation/runtime transports.

#![forbid(unsafe_code)]

pub mod profile;
pub mod sms;

pub use profile::{VideoEngineKind, VideoSystem};
