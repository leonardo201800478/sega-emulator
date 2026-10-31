use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyBytes;
use sega_audio_output::{
    NativeWasapiOutput as RustWasapiOutput, NativeXAudio2Output as RustXAudio2Output,
};
use sega_audio_runtime::{NativeAudioSession, PcmResampler, PcmSink};
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



enum NativeAudioBackend {
    Wasapi(RustWasapiOutput),
    XAudio2(RustXAudio2Output),
}

impl NativeAudioBackend {
    fn new(
        backend: &str,
        sample_rate: u32,
        buffer_size: u32,
        queue_blocks: usize,
        balance: i8,
    ) -> Result<Self, String> {
        match backend {
            "wasapi" => RustWasapiOutput::new_with_balance(
                sample_rate, buffer_size, queue_blocks, balance,
            )
            .map(Self::Wasapi)
            .map_err(|error| error.to_string()),
            "xaudio2" => RustXAudio2Output::new_with_balance(
                sample_rate, buffer_size, queue_blocks, balance,
            )
            .map(Self::XAudio2)
            .map_err(|error| error.to_string()),
            _ => Err(format!("unsupported native audio backend: {backend}")),
        }
    }
}

impl PcmSink for NativeAudioBackend {
    fn write_pcm(&mut self, pcm: Vec<u8>) -> Result<(), String> {
        match self {
            Self::Wasapi(output) => output.write_pcm(pcm).map_err(|error| error.to_string()),
            Self::XAudio2(output) => output.write_pcm(pcm).map_err(|error| error.to_string()),
        }
    }

    fn metrics(&self) -> (u64, u64) {
        match self {
            Self::Wasapi(output) => (output.underrun_count(), output.dropped_block_count()),
            Self::XAudio2(output) => (output.underrun_count(), output.dropped_block_count()),
        }
    }

    fn close(&mut self) {
        match self {
            Self::Wasapi(output) => output.close(),
            Self::XAudio2(output) => output.close(),
        }
    }
}

impl NativeAudioSession<NativeAudioBackend> {
    pub(crate) fn open(
        backend: &str,
        sample_rate: u32,
        buffer_size: u32,
        queue_blocks: usize,
        volume: u8,
        balance: i8,
    ) -> Result<Self, String> {
        let output = NativeAudioBackend::new(
            backend, sample_rate, buffer_size, queue_blocks, balance,
        )?;
        NativeAudioSession::new(sample_rate, volume, output)
    }
}
