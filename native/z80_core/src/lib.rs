//! PyO3 adapter for the integrated, Python-independent SG-1000 Rust machine.
//!
//! CPU execution and hardware state live in `sega_sg1000_core`; this crate
//! exposes the machine and immutable snapshots to the Qt/Python application.

use pyo3::prelude::*;

mod audio;
mod machine;
use audio::NativePcmResampler;
use machine::{NativeCpuSnapshot, NativePsgSnapshot, NativeSG1000Machine, NativeVdpSnapshot};

#[pymodule]
fn sega_z80_core(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<NativePcmResampler>()?;
    module.add_class::<NativeSG1000Machine>()?;
    module.add_class::<NativeCpuSnapshot>()?;
    module.add_class::<NativeVdpSnapshot>()?;
    module.add_class::<NativePsgSnapshot>()?;
    Ok(())
}
