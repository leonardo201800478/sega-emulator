//! Standalone, Python-independent SG-1000 emulation core.
//!
//! This crate is the extraction target for the Rust runtime. Keep device state
//! and scheduling logic in plain Rust types; Python bindings belong in the
//! separate `sega-z80-core` PyO3 adapter crate until the frontend is migrated.
//!
//! The crate is intentionally built incrementally. The first slice establishes
//! deterministic cartridge/RAM mapping and its contract; CPU, VDP, PSG, I/O,
//! and machine scheduling are added as separately tested migrations.

/// Size of the Z80's logical address space.
pub const SG1000_CPU_ADDRESS_SPACE_SIZE: usize = 0x1_0000;

pub mod bus;
pub mod cpu;
pub mod memory;
pub mod machine;

pub use bus::CpuBus;
pub use cpu::{CpuError, Z80, Z80Registers};

pub use memory::{MemoryConfig, MemoryError, Sg1000Memory};
pub use machine::{MachineError, Sg1000Machine};
