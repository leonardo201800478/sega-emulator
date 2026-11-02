//! Shared SN76489-family PSG core for Sega console hardware profiles.
//! Machine clocks are supplied by the owning machine/Scheduler.
mod psg;
pub use psg::{PsgError, PsgProfile, Sn76489Core};
