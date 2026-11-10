//! Standalone Sega Master System diagnostic debugger.
#![forbid(unsafe_code)]

#[cfg(windows)]
mod app;

#[cfg(windows)]
/// Exposes the SMS debugger application to the Windows executable.
pub use app::DebuggerApp;
