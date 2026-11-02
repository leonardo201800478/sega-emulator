//! SG-1000 SN76489AN hardware profile.
//! Shared digital synthesis is implemented by the `sega-sn76489` crate.

use std::ops::{Deref, DerefMut};

pub use sega_sn76489::Sn76489Core;
pub use sega_sn76489::{PsgError, PsgProfile};

#[derive(Debug, Clone, PartialEq)]
pub struct Sn76489(Sn76489Core);

impl Sn76489 {
    /// Construct the PSG using this console's hardware profile.
    pub fn new(clock_hz: u64, sample_rate: u64) -> Result<Self, PsgError> {
        Sn76489Core::new_with_profile(clock_hz, sample_rate, PsgProfile::Sg1000).map(Self)
    }
}

impl Deref for Sn76489 {
    type Target = Sn76489Core;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Sn76489 {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
