//! Compatibility facade for the shared Z80 CPU implementation.
//!
//! The canonical implementation lives in `sega-z80-cpu`; this module keeps
//! existing SMS call sites stable without introducing a console-local CPU.
pub use sega_z80_cpu::cpu::*;
