//! Thin PyO3 adapter for the pure-Rust SG-1000 PSG and audio timeline.

use std::collections::VecDeque;

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use sega_sg1000_core::Sn76489;

#[pyclass(module = "sega_z80_core")]
pub(crate) struct NativeSn76489 {
    inner: Sn76489,
}

#[pymethods]
impl NativeSn76489 {
    #[new]
    #[pyo3(signature = (clock_hz=3579545, sample_rate=44100))]
    fn new(clock_hz: u64, sample_rate: u64) -> PyResult<Self> {
        let inner = Sn76489::new(clock_hz, sample_rate)
            .map_err(|error| PyValueError::new_err(error.to_string()))?;
        Ok(Self { inner })
    }

    #[getter]
    fn clock_hz(&self) -> u64 {
        self.inner.clock_hz()
    }
    #[getter]
    fn sample_rate(&self) -> u64 {
        self.inner.sample_rate()
    }
    #[getter(_sample_buffer_capacity)]
    fn sample_buffer_capacity(&self) -> usize {
        self.inner.sample_buffer_capacity()
    }
    #[getter]
    fn dropped_samples(&self) -> u64 {
        self.inner.dropped_samples()
    }
    #[getter]
    fn tone_periods(&self) -> Vec<u16> {
        self.inner.tone_periods().to_vec()
    }
    #[getter]
    fn attenuations(&self) -> Vec<u16> {
        self.inner
            .attenuations()
            .iter()
            .map(|value| u16::from(*value))
            .collect()
    }
    #[getter]
    fn noise_control(&self) -> u8 {
        self.inner.noise_control()
    }
    #[getter(_latched_register)]
    fn latched_register(&self) -> u8 {
        self.inner.latched_register()
    }
    #[getter(_tone_counters)]
    fn tone_counters(&self) -> Vec<u16> {
        self.inner.tone_counters().to_vec()
    }
    #[getter(_tone_outputs)]
    fn tone_outputs(&self) -> Vec<u16> {
        self.inner
            .tone_outputs()
            .iter()
            .map(|value| u16::from(*value))
            .collect()
    }
    #[getter(_noise_counter)]
    fn noise_counter(&self) -> u16 {
        self.inner.noise_counter()
    }
    #[getter(_noise_counter_phase)]
    fn noise_counter_phase(&self) -> u8 {
        self.inner.noise_counter_phase()
    }
    #[getter(_lfsr)]
    fn lfsr(&self) -> u16 {
        self.inner.lfsr()
    }
    #[getter(_noise_output)]
    fn noise_output(&self) -> u8 {
        self.inner.noise_output()
    }
    #[getter(_clock_divider_phase)]
    fn clock_divider_phase(&self) -> u64 {
        self.inner.clock_divider_phase()
    }
    #[getter(_output_phase)]
    fn output_phase(&self) -> u64 {
        self.inner.output_phase()
    }
    #[getter(_noise_period)]
    fn noise_period(&self) -> u16 {
        self.inner.noise_period()
    }

    fn reset(&mut self) {
        self.inner.reset();
    }

    fn write(&mut self, value: i64) -> PyResult<()> {
        if !(0..=0xFF).contains(&value) {
            return Err(PyValueError::new_err("value deve ser um byte inteiro."));
        }
        self.inner.write(value as u8);
        Ok(())
    }

    fn advance_clocks(&mut self, clocks: i64) -> PyResult<()> {
        if clocks < 0 {
            return Err(PyValueError::new_err(
                "clocks deve ser um inteiro não negativo.",
            ));
        }
        self.inner.advance_clocks(clocks as u64);
        Ok(())
    }

    #[pyo3(signature = (max_samples=None))]
    fn drain_samples(&mut self, max_samples: Option<i64>) -> PyResult<Vec<f64>> {
        if let Some(count) = max_samples
            && count < 0
        {
            return Err(PyValueError::new_err(
                "max_samples deve ser None ou inteiro não negativo.",
            ));
        }
        let count = max_samples.map(|value| usize::try_from(value).unwrap_or(usize::MAX));
        Ok(self.inner.drain_samples(count))
    }

    fn render(&mut self, sample_count: i64) -> PyResult<Vec<f64>> {
        if sample_count < 0 {
            return Err(PyValueError::new_err(
                "sample_count deve ser um inteiro não negativo.",
            ));
        }
        let count = usize::try_from(sample_count).unwrap_or(usize::MAX);
        Ok(self.inner.render(count))
    }
}

impl NativeSn76489 {
    pub(crate) fn new_native(clock_hz: u64, sample_rate: u64) -> PyResult<Self> {
        Self::new(clock_hz, sample_rate)
    }
}

#[pyclass(module = "sega_z80_core")]
pub(crate) struct NativeAudioSynchronizer {
    device: Py<NativeSn76489>,
    current_tstate: u64,
    events: VecDeque<(u64, u8)>,
}

#[pymethods]
impl NativeAudioSynchronizer {
    #[new]
    fn new(device: Py<NativeSn76489>) -> Self {
        Self {
            device,
            current_tstate: 0,
            events: VecDeque::new(),
        }
    }

    #[getter]
    fn current_tstate(&self) -> u64 {
        self.current_tstate
    }

    #[getter]
    fn pending_events(&self) -> Vec<(u64, u8)> {
        self.events.iter().copied().collect()
    }

    fn schedule_write(&mut self, tstate: i64, value: i64) -> PyResult<()> {
        if tstate < 0 {
            return Err(PyValueError::new_err(
                "tstate deve ser um inteiro não negativo.",
            ));
        }
        if !(0..=0xFF).contains(&value) {
            return Err(PyValueError::new_err("value deve ser um byte inteiro."));
        }
        let tstate = tstate as u64;
        if tstate < self.current_tstate {
            return Err(PyValueError::new_err(
                "Não é possível agendar uma escrita no passado.",
            ));
        }
        if let Some((last_tstate, _)) = self.events.back()
            && tstate < *last_tstate
        {
            return Err(PyValueError::new_err(
                "Os eventos devem ser agendados em ordem cronológica.",
            ));
        }
        self.events.push_back((tstate, value as u8));
        Ok(())
    }

    fn advance_to(&mut self, py: Python<'_>, target_tstate: i64) -> PyResult<()> {
        if target_tstate < 0 {
            return Err(PyValueError::new_err(
                "target_tstate deve ser um inteiro não negativo.",
            ));
        }
        let target_tstate = target_tstate as u64;
        if target_tstate < self.current_tstate {
            return Err(PyValueError::new_err(
                "A linha temporal de áudio não pode retroceder.",
            ));
        }
        while let Some(&(event_tstate, event_value)) = self.events.front() {
            if event_tstate > target_tstate {
                break;
            }
            self.events.pop_front();
            let delta = event_tstate - self.current_tstate;
            if delta != 0 {
                self.device
                    .bind(py)
                    .borrow_mut()
                    .inner
                    .advance_clocks(delta);
            }
            self.current_tstate = event_tstate;
            self.device.bind(py).borrow_mut().inner.write(event_value);
        }
        let delta = target_tstate - self.current_tstate;
        if delta != 0 {
            self.device
                .bind(py)
                .borrow_mut()
                .inner
                .advance_clocks(delta);
        }
        self.current_tstate = target_tstate;
        Ok(())
    }
}

impl NativeAudioSynchronizer {
    pub(crate) fn new_native(device: Py<NativeSn76489>) -> Self {
        Self::new(device)
    }

    pub(crate) fn schedule_write_native(&mut self, tstate: u64, value: u8) -> PyResult<()> {
        let tstate = i64::try_from(tstate)
            .map_err(|_| PyValueError::new_err("tstate excede o intervalo suportado."))?;
        self.schedule_write(tstate, i64::from(value))
    }

    pub(crate) fn advance_to_native(&mut self, py: Python<'_>, target_tstate: u64) -> PyResult<()> {
        let target_tstate = i64::try_from(target_tstate)
            .map_err(|_| PyValueError::new_err("tstate excede o intervalo suportado."))?;
        self.advance_to(py, target_tstate)
    }
}
