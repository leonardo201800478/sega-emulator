//! Native Rust application shell for Sega Emulator.

#![forbid(unsafe_code)]

#[cfg(windows)]
mod diagnostics;

#[cfg(windows)]
mod app;

#[cfg(windows)]
pub use app::SegaApp;
