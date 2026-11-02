//! Shared Z80 CPU implementation, initially extracted from the SMS core.
//!
//! The CPU owns architectural state and instruction semantics. Machine-specific
//! memory maps, ports, interrupt wiring, and clock scheduling remain outside this
//! crate. The current executor preserves the SMS instruction-step behavior and
//! reports nominal T-states; it is not yet a micro-operation sequencer.

pub mod alu {
    pub use sega_z80_alu::*;
}

pub mod bus {
    pub use sega_z80_bus::*;
}

pub mod cpu;

pub use cpu::{CpuError, Z80, Z80AlternateRegisters, Z80Registers};
