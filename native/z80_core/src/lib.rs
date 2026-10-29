//! PyO3 adapter for the integrated, Python-independent SG-1000 Rust machine.
//!
//! CPU execution and hardware state live in `sega_sg1000_core`; this crate
//! exposes the machine and immutable snapshots to the Qt/Python application.

use pyo3::prelude::*;

mod audio;
mod machine;
mod sms_machine;
use audio::{NativePcmResampler, NativeWasapiOutput, NativeXAudio2Output};
use machine::{NativeCpuSnapshot, NativePsgSnapshot, NativeSG1000Machine, NativeVdpSnapshot};
use sms_machine::NativeSMSMachine;

#[pymodule]
fn sega_z80_core(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<NativePcmResampler>()?;
    module.add_class::<NativeWasapiOutput>()?;
    module.add_class::<NativeXAudio2Output>()?;
    module.add_class::<NativeSG1000Machine>()?;
    module.add_class::<NativeSMSMachine>()?;
    module.add_class::<NativeCpuSnapshot>()?;
    module.add_class::<NativeVdpSnapshot>()?;
    module.add_class::<NativePsgSnapshot>()?;
    Ok(())
}
