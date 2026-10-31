use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyBytes;
use sega_audio_output::{
    NativeWasapiOutput as RustWasapiOutput, NativeXAudio2Output as RustXAudio2Output,
};
use sega_audio_runtime::PcmResampler;
use std::sync::Mutex;

#[pyclass]
pub struct NativePcmResampler {
    inner: PcmResampler,
}

impl NativePcmResampler {
    pub(crate) fn process_native(&mut self, samples: &[f64]) -> Vec<u8> {
        self.inner.process(samples)
    }
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

    fn process<'py>(&mut self, py: Python<'py>, samples: Vec<f64>) -> Bound<'py, PyBytes> {
        PyBytes::new(py, &self.inner.process(&samples))
    }

    fn flush<'py>(&mut self, py: Python<'py>) -> Bound<'py, PyBytes> {
        PyBytes::new(py, &self.inner.flush())
    }
}

#[pyclass]
pub struct NativeWasapiOutput {
    inner: Mutex<RustWasapiOutput>,
}

#[pymethods]
impl NativeWasapiOutput {
    #[new]
    #[pyo3(signature = (sample_rate, buffer_size, queue_blocks=4, balance=0))]
    fn new(sample_rate: u32, buffer_size: u32, queue_blocks: usize, balance: i8) -> PyResult<Self> {
        let inner =
            RustWasapiOutput::new_with_balance(sample_rate, buffer_size, queue_blocks, balance)
                .map_err(|error| pyo3::exceptions::PyRuntimeError::new_err(error.to_string()))?;
        Ok(Self {
            inner: Mutex::new(inner),
        })
    }

    #[getter]
    fn sample_rate(&self) -> PyResult<u32> {
        self.inner
            .lock()
            .map(|inner| inner.sample_rate())
            .map_err(|_| {
                pyo3::exceptions::PyRuntimeError::new_err("native audio output lock poisoned")
            })
    }

    #[getter]
    fn buffer_size(&self) -> PyResult<u32> {
        self.inner
            .lock()
            .map(|inner| inner.buffer_size())
            .map_err(|_| {
                pyo3::exceptions::PyRuntimeError::new_err("native audio output lock poisoned")
            })
    }

    #[getter]
    fn underrun_count(&self) -> PyResult<u64> {
        self.inner
            .lock()
            .map(|inner| inner.underrun_count())
            .map_err(|_| {
                pyo3::exceptions::PyRuntimeError::new_err("native audio output lock poisoned")
            })
    }

    #[getter]
    fn dropped_block_count(&self) -> PyResult<u64> {
        self.inner
            .lock()
            .map(|inner| inner.dropped_block_count())
            .map_err(|_| {
                pyo3::exceptions::PyRuntimeError::new_err("native audio output lock poisoned")
            })
    }

    fn write_pcm(&mut self, pcm: &Bound<'_, PyBytes>) -> PyResult<()> {
        self.inner
            .lock()
            .map_err(|_| {
                pyo3::exceptions::PyRuntimeError::new_err("native audio output lock poisoned")
            })?
            .write_pcm(pcm.as_bytes().to_vec())
            .map_err(|error| pyo3::exceptions::PyRuntimeError::new_err(error.to_string()))
    }

    fn close(&mut self) -> PyResult<()> {
        self.inner
            .lock()
            .map_err(|_| {
                pyo3::exceptions::PyRuntimeError::new_err("native audio output lock poisoned")
            })?
            .close();
        Ok(())
    }
}

#[pyclass]
pub struct NativeXAudio2Output {
    inner: Mutex<RustXAudio2Output>,
}

#[pymethods]
impl NativeXAudio2Output {
    #[new]
    #[pyo3(signature = (sample_rate, buffer_size, queue_blocks=8, balance=0))]
    fn new(sample_rate: u32, buffer_size: u32, queue_blocks: usize, balance: i8) -> PyResult<Self> {
        let inner =
            RustXAudio2Output::new_with_balance(sample_rate, buffer_size, queue_blocks, balance)
                .map_err(|error| pyo3::exceptions::PyRuntimeError::new_err(error.to_string()))?;
        Ok(Self {
            inner: Mutex::new(inner),
        })
    }

    #[getter]
    fn sample_rate(&self) -> PyResult<u32> {
        self.inner
            .lock()
            .map(|inner| inner.sample_rate())
            .map_err(|_| {
                pyo3::exceptions::PyRuntimeError::new_err("native audio output lock poisoned")
            })
    }

    #[getter]
    fn buffer_size(&self) -> PyResult<u32> {
        self.inner
            .lock()
            .map(|inner| inner.buffer_size())
            .map_err(|_| {
                pyo3::exceptions::PyRuntimeError::new_err("native audio output lock poisoned")
            })
    }

    #[getter]
    fn underrun_count(&self) -> PyResult<u64> {
        self.inner
            .lock()
            .map(|inner| inner.underrun_count())
            .map_err(|_| {
                pyo3::exceptions::PyRuntimeError::new_err("native audio output lock poisoned")
            })
    }

    #[getter]
    fn dropped_block_count(&self) -> PyResult<u64> {
        self.inner
            .lock()
            .map(|inner| inner.dropped_block_count())
            .map_err(|_| {
                pyo3::exceptions::PyRuntimeError::new_err("native audio output lock poisoned")
            })
    }

    fn write_pcm(&mut self, pcm: &Bound<'_, PyBytes>) -> PyResult<()> {
        self.inner
            .lock()
            .map_err(|_| {
                pyo3::exceptions::PyRuntimeError::new_err("native audio output lock poisoned")
            })?
            .write_pcm(pcm.as_bytes().to_vec())
            .map_err(|error| pyo3::exceptions::PyRuntimeError::new_err(error.to_string()))
    }

    fn close(&mut self) -> PyResult<()> {
        self.inner
            .lock()
            .map_err(|_| {
                pyo3::exceptions::PyRuntimeError::new_err("native audio output lock poisoned")
            })?
            .close();
        Ok(())
    }
}

/// Complete per-machine audio session. PCM never crosses the Python boundary:
/// machine wrappers feed native samples directly into this session after a
/// coarse-grained execution batch.
pub(crate) struct NativeAudioSession {
    resampler: PcmResampler,
    output: NativeAudioBackend,
}

enum NativeAudioBackend {
    Wasapi(RustWasapiOutput),
    XAudio2(RustXAudio2Output),
}

impl NativeAudioSession {
    pub(crate) fn new(
        backend: &str,
        sample_rate: u32,
        buffer_size: u32,
        queue_blocks: usize,
        volume: u8,
        balance: i8,
    ) -> Result<Self, String> {
        let resampler =
            PcmResampler::new(sample_rate, volume).map_err(|error| error.to_string())?;
        let output = match backend {
            "wasapi" => NativeAudioBackend::Wasapi(
                RustWasapiOutput::new_with_balance(sample_rate, buffer_size, queue_blocks, balance)
                    .map_err(|error| error.to_string())?,
            ),
            "xaudio2" => NativeAudioBackend::XAudio2(
                RustXAudio2Output::new_with_balance(
                    sample_rate,
                    buffer_size,
                    queue_blocks,
                    balance,
                )
                .map_err(|error| error.to_string())?,
            ),
            _ => return Err(format!("unsupported native audio backend: {backend}")),
        };
        Ok(Self { resampler, output })
    }

    pub(crate) fn process_samples(&mut self, samples: &[f64]) -> Result<(), String> {
        if samples.is_empty() {
            return Ok(());
        }
        let pcm = self.resampler.process(samples);
        if pcm.is_empty() {
            return Ok(());
        }
        match &mut self.output {
            NativeAudioBackend::Wasapi(output) => {
                output.write_pcm(pcm).map_err(|error| error.to_string())
            }
            NativeAudioBackend::XAudio2(output) => {
                output.write_pcm(pcm).map_err(|error| error.to_string())
            }
        }
    }

    pub(crate) fn metrics(&self) -> (u64, u64) {
        match &self.output {
            NativeAudioBackend::Wasapi(output) => {
                (output.underrun_count(), output.dropped_block_count())
            }
            NativeAudioBackend::XAudio2(output) => {
                (output.underrun_count(), output.dropped_block_count())
            }
        }
    }

    pub(crate) fn close(&mut self) {
        let tail = self.resampler.flush();
        if !tail.is_empty() {
            match &mut self.output {
                NativeAudioBackend::Wasapi(output) => {
                    let _ = output.write_pcm(tail);
                }
                NativeAudioBackend::XAudio2(output) => {
                    let _ = output.write_pcm(tail);
                }
            }
        }
        match &mut self.output {
            NativeAudioBackend::Wasapi(output) => output.close(),
            NativeAudioBackend::XAudio2(output) => output.close(),
        }
    }
}

impl Drop for NativeAudioSession {
    fn drop(&mut self) {
        self.close();
    }
}

#[cfg(test)]
mod audio_session_tests {
    use super::*;

    #[test]
    fn rejects_unknown_backend_without_opening_a_device() {
        let result = NativeAudioSession::new("unknown", 48_000, 512, 4, 100, 0);
        assert!(
            result
                .err()
                .expect("unknown backend must fail")
                .contains("unsupported native audio backend")
        );
    }
}
