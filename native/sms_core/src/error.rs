//! Errors surfaced by the temporal SMS core and cartridge memory.

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SmsCoreError {
    EmptyCartridge,
    RomTooLargeForSegaMapper { size: usize, max_size: usize },
    InvalidBiosSize { size: usize, expected: usize },
    InvalidCartridgeRamSize { size: usize, expected: usize },
}

impl fmt::Display for SmsCoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyCartridge => write!(f, "cartridge image must not be empty"),
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
