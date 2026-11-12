//! Shared SN76489-family PSG core for Sega console hardware profiles.
//! Machine clocks are supplied by the owning machine/Scheduler.
mod psg;
pub use psg::{
    PsgError, PsgProfile, PsgStateError, SN76489_STATE_FORMAT_VERSION, Sn76489Core,
    Sn76489StateV1,
};
