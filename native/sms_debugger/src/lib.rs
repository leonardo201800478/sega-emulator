//! Standalone Sega Master System diagnostic debugger.
#![forbid(unsafe_code)]

#[cfg(windows)]
mod app;

#[cfg(windows)]
pub use app::DebuggerApp;
