//! Initial standalone, Python-independent Sega Master System core.
//!
//! This first slice implements configuration, cartridge image ownership, and
//! standard Sega mapper/work RAM only. CPU, I/O, VDP, audio, and scheduling
//! remain unimplemented and must be added with fixed regression tests.

pub mod cartridge;
pub mod config;
pub mod error;
pub mod memory;

pub use cartridge::CartridgeImage;
pub use config::{FmHardware, SmsMachineConfig, SpriteLimitMode, Territory, VideoStandard};
pub use error::SmsCoreError;
pub use memory::SmsMemory;
