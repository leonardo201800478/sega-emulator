//! Compatibility facade for the shared Z80 CPU implementation.
//!
//! The canonical implementation lives in `sega-z80-cpu`; this module keeps
//! existing SMS call sites stable while the SG-1000 adapter is migrated.
pub use sega_z80_cpu::cpu::*;
