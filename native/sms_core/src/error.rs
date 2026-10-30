//! Errors surfaced by the initial SMS core slice.

use std::fmt;

use crate::psg::PsgError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SmsCoreError {
    EmptyCartridge,
    Audio(PsgError),
    RomTooLargeForSegaMapper { size: usize, max_size: usize },
    InvalidBiosSize { size: usize, expected: usize },
}

impl fmt::Display for SmsCoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyCartridge => write!(f, "cartridge image must not be empty"),
            Self::Audio(error) => write!(f, "SMS PSG initialization failed: {error}"),
            Self::InvalidBiosSize { size, expected } => write!(
                f,
                "SMS BIOS size {size} bytes is invalid; expected {expected} bytes"
            ),
            Self::RomTooLargeForSegaMapper { size, max_size } => write!(
                f,
                "ROM size {size} exceeds the standard Sega mapper range of {max_size} bytes; select a supported mapper"
            ),
        }
    }
}

impl std::error::Error for SmsCoreError {}

impl From<PsgError> for SmsCoreError {
    fn from(error: PsgError) -> Self {
        Self::Audio(error)
    }
}
