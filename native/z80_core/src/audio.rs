use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyBytes;
use sega_audio_runtime::PcmResampler;
use sega_audio_output::NativeWasapiOutput as RustWasapiOutput;

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
        self.inner
            .set_volume(volume)
            .map_err(|err| PyValueError::new_err(err.to_string()))
    }

    fn reset(&mut self) {
        self.inner.reset();
    }

    fn process<'py>(
        &mut self,
        py: Python<'py>,
        samples: Vec<f64>,
    ) -> Bound<'py, PyBytes> {
        PyBytes::new(py, &self.inner.process(&samples))
    }

    fn flush<'py>(&mut self, py: Python<'py>) -> Bound<'py, PyBytes> {
        PyBytes::new(py, &self.inner.flush())
    }
}

#[pyclass]
pub struct NativeWasapiOutput {
    inner: RustWasapiOutput,
}

#[pymethods]
impl NativeWasapiOutput {
    #[new]
    #[pyo3(signature = (sample_rate, buffer_size, queue_blocks=8))]
    fn new(sample_rate: u32, buffer_size: u32, queue_blocks: usize) -> PyResult<Self> {
        let inner = RustWasapiOutput::new(sample_rate, buffer_size, queue_blocks)
            .map_err(|error| pyo3::exceptions::PyRuntimeError::new_err(error.to_string()))?;
        Ok(Self { inner })
    }

    #[getter]
    fn sample_rate(&self) -> u32 {
        self.inner.sample_rate()
    }

    #[getter]
    fn buffer_size(&self) -> u32 {
        self.inner.buffer_size()
    }

    #[getter]
    fn underrun_count(&self) -> u64 {
        self.inner.underrun_count()
    }

    #[getter]
    fn dropped_block_count(&self) -> u64 {
        self.inner.dropped_block_count()
    }

    fn write_pcm(&self, pcm: &Bound<'_, PyBytes>) -> PyResult<()> {
        self.inner
            .write_pcm(pcm.as_bytes().to_vec())
            .map_err(|error| pyo3::exceptions::PyRuntimeError::new_err(error.to_string()))
    }

    fn close(&mut self) {
        self.inner.close();
    }
}
