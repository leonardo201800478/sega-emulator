//! Native SN76489AN PSG and timestamped audio-write synchronization.

use std::collections::VecDeque;

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

const CLOCK_DIVIDER: u64 = 16;
const TONE_CHANNELS: usize = 3;
const NOISE_CHANNEL: usize = 3;
const LFSR_MASK: u16 = 0x7FFF;
const LFSR_SEED: u16 = 0x4000;
const SAMPLE_BUFFER_SECONDS: u64 = 1;
const ATTENUATION: [f64; 16] = [
    1.0, 0.794328, 0.630957, 0.501187, 0.398107, 0.316228, 0.251189, 0.199526, 0.158489, 0.125893,
    0.1, 0.079433, 0.063096, 0.050119, 0.039811, 0.0,
];

#[pyclass(module = "sega_z80_core")]
pub(crate) struct NativeSn76489 {
    clock_hz: u64,
    sample_rate: u64,
    sample_buffer_capacity: usize,
    pending_samples: VecDeque<f64>,
    render_pending: VecDeque<f64>,
    dropped_samples: u64,
    tone_periods: [u16; TONE_CHANNELS],
    attenuations: [u8; 4],
    noise_control: u8,
    latched_register: u8,
    tone_counters: [u16; TONE_CHANNELS],
    tone_outputs: [u8; TONE_CHANNELS],
    noise_counter: u16,
    noise_counter_phase: u8,
    lfsr: u16,
    clock_divider_phase: u64,
    output_phase: u64,
    noise_period: u16,
}

#[pymethods]
impl NativeSn76489 {
    #[new]
    #[pyo3(signature = (clock_hz=3579545, sample_rate=44100))]
    fn new(clock_hz: u64, sample_rate: u64) -> PyResult<Self> {
        if clock_hz == 0 {
            return Err(PyValueError::new_err(
                "clock_hz deve ser um inteiro positivo.",
            ));
        }
        if sample_rate == 0 {
            return Err(PyValueError::new_err(
                "sample_rate deve ser um inteiro positivo.",
            ));
        }
        let sample_buffer_capacity = usize::try_from(sample_rate * SAMPLE_BUFFER_SECONDS)
            .map_err(|_| PyValueError::new_err("sample_rate excede a capacidade suportada."))?;
        let mut device = Self {
            clock_hz,
            sample_rate,
            sample_buffer_capacity,
            pending_samples: VecDeque::new(),
            render_pending: VecDeque::new(),
            dropped_samples: 0,
            tone_periods: [0; TONE_CHANNELS],
            attenuations: [15; 4],
            noise_control: 0,
            latched_register: 0,
            tone_counters: [1; TONE_CHANNELS],
            tone_outputs: [1; TONE_CHANNELS],
            noise_counter: 0,
            noise_counter_phase: 0,
            lfsr: LFSR_SEED,
            clock_divider_phase: 0,
            output_phase: 0,
            noise_period: 0x10,
        };
        device.reset_inner();
        Ok(device)
    }

    #[getter]
    fn clock_hz(&self) -> u64 {
        self.clock_hz
    }

    #[getter]
    fn sample_rate(&self) -> u64 {
        self.sample_rate
    }

    #[getter(_sample_buffer_capacity)]
    fn sample_buffer_capacity(&self) -> usize {
        self.sample_buffer_capacity
    }

    #[getter]
    fn dropped_samples(&self) -> u64 {
        self.dropped_samples
    }

    #[getter]
    fn tone_periods(&self) -> Vec<u16> {
        self.tone_periods.to_vec()
    }

    #[getter]
    fn attenuations(&self) -> Vec<u16> {
        self.attenuations
            .iter()
            .map(|value| u16::from(*value))
            .collect()
    }

    #[getter]
    fn noise_control(&self) -> u8 {
        self.noise_control
    }

    #[getter(_latched_register)]
    fn latched_register(&self) -> u8 {
        self.latched_register
    }

    #[getter(_tone_counters)]
    fn tone_counters(&self) -> Vec<u16> {
        self.tone_counters.to_vec()
    }

    #[getter(_tone_outputs)]
    fn tone_outputs(&self) -> Vec<u16> {
        self.tone_outputs
            .iter()
            .map(|value| u16::from(*value))
            .collect()
    }

    #[getter(_noise_counter)]
    fn noise_counter(&self) -> u16 {
        self.noise_counter
    }

    #[getter(_noise_counter_phase)]
    fn noise_counter_phase(&self) -> u8 {
        self.noise_counter_phase
    }

    #[getter(_lfsr)]
    fn lfsr(&self) -> u16 {
        self.lfsr
    }

    #[getter(_clock_divider_phase)]
    fn clock_divider_phase(&self) -> u64 {
        self.clock_divider_phase
    }

    #[getter(_output_phase)]
    fn output_phase(&self) -> u64 {
        self.output_phase
    }

    #[getter(_noise_period)]
    fn noise_period(&self) -> u16 {
        self.noise_period
    }

    fn reset(&mut self) {
        self.reset_inner();
    }

    fn write(&mut self, value: i64) -> PyResult<()> {
        if !(0..=0xFF).contains(&value) {
            return Err(PyValueError::new_err("value deve ser um byte inteiro."));
        }
        self.write_inner(value as u8);
        Ok(())
    }

    fn advance_clocks(&mut self, clocks: i64) -> PyResult<()> {
        if clocks < 0 {
            return Err(PyValueError::new_err(
                "clocks deve ser um inteiro não negativo.",
            ));
        }
        self.advance_clocks_inner(clocks as u64, false);
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
        let count = max_samples.map_or(self.pending_samples.len(), |value| {
            usize::try_from(value)
                .unwrap_or(usize::MAX)
                .min(self.pending_samples.len())
        });
        Ok((0..count)
            .filter_map(|_| self.pending_samples.pop_front())
            .collect())
    }

    fn render(&mut self, sample_count: i64) -> PyResult<Vec<f64>> {
        if sample_count < 0 {
            return Err(PyValueError::new_err(
                "sample_count deve ser um inteiro não negativo.",
            ));
        }
        let mut output = Vec::with_capacity((sample_count as u64).min(1_000_000) as usize);
        let target = sample_count as usize;
        while output.len() < target {
            if let Some(sample) = self.render_pending.pop_front() {
                output.push(sample);
                continue;
            }
            let clocks_to_next_tick = CLOCK_DIVIDER - self.clock_divider_phase;
            self.advance_clocks_inner(clocks_to_next_tick, true);
        }
        Ok(output)
    }
}

impl NativeSn76489 {
    fn reset_inner(&mut self) {
        self.tone_periods = [0; TONE_CHANNELS];
        self.attenuations = [15; 4];
        self.noise_control = 0;
        self.latched_register = 0;
        self.tone_counters = [1; TONE_CHANNELS];
        self.tone_outputs = [1; TONE_CHANNELS];
        self.noise_counter = 0;
        self.noise_counter_phase = 0;
        self.lfsr = LFSR_SEED;
        self.clock_divider_phase = 0;
        self.output_phase = 0;
        self.noise_period = 0x10;
        self.noise_counter = self.noise_period;
        self.pending_samples.clear();
        self.render_pending.clear();
        self.dropped_samples = 0;
        self.refresh_noise_period();
    }

    fn write_inner(&mut self, value: u8) {
        if value & 0x80 != 0 {
            let register = (value >> 4) & 0x07;
            self.latched_register = register;
            self.write_register(register, value & 0x0F, true);
        } else {
            self.write_register(self.latched_register, value & 0x3F, false);
        }
    }

    fn write_register(&mut self, register: u8, data: u8, latched: bool) {
        let channel = usize::from(register >> 1);
        let is_volume = register & 1 != 0;
        if channel < TONE_CHANNELS && !is_volume {
            let previous_period = self.tone_periods[channel];
            if latched {
                self.tone_periods[channel] =
                    (self.tone_periods[channel] & 0x03F0) | u16::from(data & 0x0F);
            } else {
                self.tone_periods[channel] =
                    (self.tone_periods[channel] & 0x000F) | (u16::from(data & 0x3F) << 4);
            }
            self.tone_periods[channel] &= 0x03FF;
            if previous_period <= 1 && self.tone_periods[channel] > 1 {
                self.tone_counters[channel] = self.tone_periods[channel];
            }
            if channel == 2 {
                self.refresh_noise_period();
            }
            return;
        }
        if channel < TONE_CHANNELS && is_volume {
            self.attenuations[channel] = data & 0x0F;
            return;
        }
        if channel == NOISE_CHANNEL && !is_volume {
            self.noise_control = data & 0x07;
            self.refresh_noise_period();
            self.reset_noise_generator();
            return;
        }
        self.attenuations[NOISE_CHANNEL] = data & 0x0F;
    }

    fn refresh_noise_period(&mut self) {
        let rate = self.noise_control & 0x03;
        self.noise_period = if rate == 3 {
            self.tone_periods[2].max(1)
        } else {
            0x10_u16 << rate
        };
    }

    fn reset_noise_generator(&mut self) {
        self.lfsr = LFSR_SEED;
        self.noise_counter = self.noise_period;
        self.noise_counter_phase = 0;
    }

    fn shift_noise_register(&mut self) {
        let feedback = if self.noise_control & 0x04 != 0 {
            (self.lfsr ^ (self.lfsr >> 1)) & 1
        } else {
            self.lfsr & 1
        };
        self.lfsr = ((self.lfsr >> 1) | (feedback << 14)) & LFSR_MASK;
    }

    fn tick_internal_clock(&mut self) {
        for channel in 0..TONE_CHANNELS {
            let period = self.tone_periods[channel];
            if period <= 1 {
                self.tone_outputs[channel] = 1;
                self.tone_counters[channel] = period;
            } else if self.tone_counters[channel] > 1 {
                self.tone_counters[channel] -= 1;
            } else {
                self.tone_outputs[channel] ^= 1;
                self.tone_counters[channel] = period;
            }
        }
        if self.noise_counter > 1 {
            self.noise_counter -= 1;
        } else {
            self.noise_counter = self.noise_period;
            self.noise_counter_phase ^= 1;
            if self.noise_counter_phase != 0 {
                self.shift_noise_register();
            }
        }
    }

    fn advance_clocks_inner(&mut self, clocks: u64, render_sink: bool) {
        let internal_total = self.clock_divider_phase + clocks;
        let internal_clocks = internal_total / CLOCK_DIVIDER;
        self.clock_divider_phase = internal_total % CLOCK_DIVIDER;
        for _ in 0..internal_clocks {
            self.tick_internal_clock();
            self.output_phase += self.sample_rate * CLOCK_DIVIDER;
            while self.output_phase >= self.clock_hz {
                let sample = self.mix();
                if render_sink {
                    self.render_pending.push_back(sample);
                } else {
                    if self.pending_samples.len() == self.sample_buffer_capacity {
                        self.pending_samples.pop_front();
                        self.dropped_samples += 1;
                    }
                    self.pending_samples.push_back(sample);
                }
                self.output_phase -= self.clock_hz;
            }
        }
    }

    fn mix(&self) -> f64 {
        let mut total = ATTENUATION[usize::from(self.attenuations[0])]
            * if self.tone_outputs[0] != 0 { 1.0 } else { -1.0 };
        total += ATTENUATION[usize::from(self.attenuations[1])]
            * if self.tone_outputs[1] != 0 { 1.0 } else { -1.0 };
        total += ATTENUATION[usize::from(self.attenuations[2])]
            * if self.tone_outputs[2] != 0 { 1.0 } else { -1.0 };
        total += ATTENUATION[usize::from(self.attenuations[NOISE_CHANNEL])]
            * if self.lfsr & 1 != 0 { 1.0 } else { -1.0 };
        total * 0.25
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
                    .advance_clocks_inner(delta, false);
            }
            self.current_tstate = event_tstate;
            self.device.bind(py).borrow_mut().write_inner(event_value);
        }
        let delta = target_tstate - self.current_tstate;
        if delta != 0 {
            self.device
                .bind(py)
                .borrow_mut()
                .advance_clocks_inner(delta, false);
        }
        self.current_tstate = target_tstate;
        Ok(())
    }
}

impl NativeSn76489 {
    pub(crate) fn new_native(clock_hz: u64, sample_rate: u64) -> PyResult<Self> {
        Self::new(clock_hz, sample_rate)
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
            .map_err(|_| PyValueError::new_err("target_tstate excede o intervalo suportado."))?;
        self.advance_to(py, target_tstate)
    }
}

#[cfg(test)]
mod tests {
    use super::NativeSn76489;

    #[test]
    fn reset_and_repeated_advancement_are_deterministic() {
        let mut first = NativeSn76489::new(3_579_545, 44_100).unwrap();
        let mut second = NativeSn76489::new(3_579_545, 44_100).unwrap();
        for device in [&mut first, &mut second] {
            device.write_inner(0x85);
            device.write_inner(0x12);
            device.write_inner(0x90);
            device.write_inner(0xE4);
            device.advance_clocks_inner(75_000, false);
        }
        assert_eq!(first.tone_periods, second.tone_periods);
        assert_eq!(first.lfsr, second.lfsr);
        assert_eq!(first.pending_samples, second.pending_samples);
        first.reset_inner();
        second.reset_inner();
        assert_eq!(first.tone_periods, [0; 3]);
        assert_eq!(first.attenuations, [15; 4]);
        assert_eq!(first.pending_samples.len(), 0);
        assert_eq!(first.pending_samples, second.pending_samples);
    }
}
