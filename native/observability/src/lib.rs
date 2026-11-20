//! Shared structured logging for the Sega Emulator.
//!
//! Hardware crates use the re-exported `tracing` API. The native application
//! enables the `runtime` feature to install the bounded, non-blocking writer.

pub use tracing;

/// Capability proving that diagnostics activation followed an explicit user action.
///
/// Construct this value only in direct response to an affirmative diagnostics
/// opt-in in the UI. It is intentionally available without the runtime logging
/// feature so in-memory debugger capture uses the same consent boundary.
#[derive(Clone, Copy, Debug)]
#[must_use = "pass the consent capability to diagnostics activation APIs"]
pub struct DiagnosticsConsent {
    _private: (),
}

impl DiagnosticsConsent {
    /// Create a consent capability after the user explicitly enables diagnostics.
    ///
    /// Never call during startup, preference restoration, or background processing.
    pub fn from_explicit_user_action() -> Self {
        Self { _private: () }
    }
}

#[cfg(feature = "runtime")]
mod writer;

#[cfg(feature = "runtime")]
mod runtime;

#[cfg(feature = "runtime")]
pub use runtime::{
    InitError, LogConfig, LoggingHandle, SessionMetadata, current, init_after_explicit_consent,
};
