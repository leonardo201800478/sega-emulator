//! Compatibility facade for the shared Z80 CPU implementation.
//!
//! The canonical instruction engine lives in `sega-z80-cpu`. SG-1000-specific
//! memory mapping and I/O remain implemented by the machine-owned bus.
pub use sega_z80_cpu::cpu::*;
