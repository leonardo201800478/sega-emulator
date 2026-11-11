//! Native Rust application shell for Sega Emulator.

#![forbid(unsafe_code)]

#[cfg(windows)]
mod app;

#[cfg(windows)]
mod debugger_window;

#[cfg(windows)]
pub use app::SegaApp;
