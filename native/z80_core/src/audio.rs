use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyBytes;
use sega_audio_runtime::PcmResampler;

#[pyclass]
pub struct NativePcmResampler {
    inner: PcmResampler,
}

#[pymethods]
impl NativePcmResampler {
    #[new]
    #[pyo3(signature = (output_rate, volume=100))]
    fn new(output_rate: u32, volume: u8) -> PyResult<Self> {
        let inner = PcmResampler::new(output_rate, volume)
            .map_err(|err| PyValueError::new_err(err.to_string()))?;
        Ok(Self { inner })
    }

    #[getter]
    fn output_rate(&self) -> u32 {
        self.inner.output_rate()
    }

    #[getter]
    fn volume(&self) -> u8 {
        self.inner.volume()
    }

    fn set_volume(&mut self, volume: u8) -> PyResult<()> {
        self.inner.set_volume(volume)
            .map_err(|err| PyValueError::new_err(err.to_string()))
    }

    fn reset(&mut self) {
        self.inner.reset();
    }

    fn process<'py>(&mut self, py: Python<'py>, samples: Vec<f64>) -> Bound<'py, PyBytes> {
        PyBytes::new(py, &self.inner.process(&samples))
    }

    fn flush<'py>(&mut self, py: Python<'py>) -> Bound<'py, PyBytes> {
        PyBytes::new(py, &self.inner.flush())
    }
}
