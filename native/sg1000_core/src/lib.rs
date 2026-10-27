//! Standalone, Python-independent SG-1000 emulation core.
//!
//! Device state and scheduling logic belong in plain Rust types; Python
//! bindings belong in the separate PyO3 adapter until the frontend migrates.
//! This crate is being built incrementally; its public machine is not yet a
//! complete standalone emulator.
//!
//! Host controller APIs are normalized at the frontend boundary; the core
//! accepts deterministic active-low snapshots without polling SDL or Windows.

/// Size of the Z80's logical address space.
pub const SG1000_CPU_ADDRESS_SPACE_SIZE: usize = 0x1_0000;

pub mod alu;
pub mod bus;
pub mod cpu;
pub mod input;
pub mod io;
pub mod machine;
pub mod memory;
pub mod psg;
pub mod vdp;

pub use bus::CpuBus;
pub use cpu::{CpuError, Z80};
pub use input::{
    ControllerBackend, ControllerBackendPreference, ControllerButtons, ControllerFrame,
};
pub use io::{IoError, PsgWriteEvent, Sg1000Io};
pub use machine::{MachineError, Sg1000Machine};
pub use memory::{MemoryConfig, MemoryError, Sg1000Memory};
pub use psg::{PsgError, Sn76489};
pub use vdp::Tms9918A;
