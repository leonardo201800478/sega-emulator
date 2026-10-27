//! Shared Sega emulator components and integration boundary.
//!
//! Console-specific maps, devices, and schedulers belong in their own crates.
//! The optional PyO3 adapter remains separate until the Qt frontend migrates.

pub use sega_z80_alu as z80_alu;
pub use sega_z80_bus as z80_bus;
