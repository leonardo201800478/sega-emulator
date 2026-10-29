//! Standalone, Python-independent Sega Master System core.
//!
//! The SMS-owned Z80 instruction engine and standard Sega mapper/work RAM
//! are present. SMS I/O decoding, VDP, audio devices and machine scheduler
//! remain separate implementation milestones; this crate does not yet
//! represent a complete runnable console.

pub mod alu;
pub mod bus;
pub mod cartridge;
pub mod config;
pub mod cpu;
pub mod error;
pub mod memory;

pub use bus::{CpuBus, Z80BusEvent, Z80BusEventKind, Z80CpuSnapshot, Z80InstructionOutcome, Z80InstructionTrace, Z80TStatePhase, Z80TStateSample};
pub use cartridge::CartridgeImage;
pub use config::{FmHardware, SmsMachineConfig, SpriteLimitMode, Territory, VideoStandard};
pub use cpu::{CpuError, Z80};
pub use error::SmsCoreError;
pub use memory::SmsMemory;
