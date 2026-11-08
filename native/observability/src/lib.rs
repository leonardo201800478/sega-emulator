//! Shared structured logging for the Sega Emulator.
//!
//! Hardware crates use the re-exported `tracing` API. The native application
//! enables the `runtime` feature to install the bounded, non-blocking writer.

pub use tracing;

#[cfg(feature = "runtime")]
mod runtime;

#[cfg(feature = "runtime")]
pub use runtime::{
    InitError, LogConfig, LoggingHandle, SessionMetadata, current, init,
};
