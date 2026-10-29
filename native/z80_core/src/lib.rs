//! Experimental native Z80 execution core.
//!
//! This first selectable increment intentionally supports a subset of the
//! Python reference core. Unsupported opcodes fail explicitly; they never
//! fall back silently to Python.

use pyo3::prelude::*;

mod machine;
use machine::{NativeCpuSnapshot, NativePsgSnapshot, NativeSG1000Machine, NativeVdpSnapshot};

const FLAG_C: u8 = 0x01;
const FLAG_N: u8 = 0x02;
const FLAG_PV: u8 = 0x04;
const FLAG_H: u8 = 0x10;
const FLAG_Z: u8 = 0x40;
const FLAG_S: u8 = 0x80;

#[pymodule]
fn sega_z80_core(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<NativeSG1000Machine>()?;
    module.add_class::<NativeCpuSnapshot>()?;
    module.add_class::<NativeVdpSnapshot>()?;
    module.add_class::<NativePsgSnapshot>()?;
    Ok(())
}
