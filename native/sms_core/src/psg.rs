//! Independent SMS PSG port based on the SG-1000 SN76489 implementation.
//!
//! This copy deliberately belongs to the SMS core: SMS-specific noise hardware
//! and future SMS audio changes must not couple the SG-1000 implementation.
//! The FIR is a digital output filter, not a claim of analog circuit accuracy.

use std::collections::VecDeque;
use std::f64::consts::PI;
use std::fmt;

const CLOCK_DIVIDER: u64 = 16;
const TONE_CHANNELS: usize = 3;
const NOISE_CHANNEL: usize = 3;
const LFSR_MASK: u16 = 0xFFFF;
const LFSR_SEED: u16 = 0x8000;
const SAMPLE_BUFFER_SECONDS: u64 = 1;
const FILTER_TAPS: usize = 63;
const ATTENUATION: [f64; 16] = [
    1.0, 0.794328, 0.630957, 0.501187, 0.398107, 0.316228, 0.251189, 0.199526, 0.158489, 0.125893,
    0.1, 0.079433, 0.063096, 0.050119, 0.039811, 0.0,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PsgError {
    ZeroClock,
    ZeroSampleRate,
    SampleRateTooLarge,
}

impl fmt::Display for PsgError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroClock => write!(f, "clock_hz must be positive"),
            Self::ZeroSampleRate => write!(f, "sample_rate must be positive"),
            Self::SampleRateTooLarge => write!(f, "sample_rate exceeds supported capacity"),
        }
    }
}

impl std::error::Error for PsgError {}

fn design_lowpass(clock_hz: u64, sample_rate: u64) -> Vec<f64> {
    let internal_rate = clock_hz as f64 / CLOCK_DIVIDER as f64;
    let cutoff = 0.45 * (sample_rate as f64 / internal_rate).min(1.0);
    let center = (FILTER_TAPS - 1) as f64 / 2.0;
    let mut coefficients = Vec::with_capacity(FILTER_TAPS);
    for index in 0..FILTER_TAPS {
        let offset = index as f64 - center;
        let ideal = if offset == 0.0 {
            2.0 * cutoff
        } else {
            (2.0 * PI * cutoff * offset).sin() / (PI * offset)
        };
        let position = index as f64 / (FILTER_TAPS - 1) as f64;
        let window = 0.42 - 0.5 * (2.0 * PI * position).cos() + 0.08 * (4.0 * PI * position).cos();
        coefficients.push(ideal * window);
    }
    let total: f64 = coefficients.iter().sum();
    for coefficient in &mut coefficients {
        *coefficient /= total;
    }
    coefficients
}

#[derive(Debug, Clone, PartialEq)]
pub struct Sn76489 {
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
    noise_output: u8,
    clock_divider_phase: u64,
    output_phase: u64,
    noise_period: u16,
    filter_coefficients: Vec<f64>,
    filter_history: Vec<f64>,
    filter_position: usize,
}

impl Sn76489 {
    pub fn new(clock_hz: u64, sample_rate: u64) -> Result<Self, PsgError> {
        if clock_hz == 0 {
            return Err(PsgError::ZeroClock);
        }
        if sample_rate == 0 {
            return Err(PsgError::ZeroSampleRate);
        }
        let capacity = sample_rate
            .checked_mul(SAMPLE_BUFFER_SECONDS)
            .ok_or(PsgError::SampleRateTooLarge)?;
        let sample_buffer_capacity =
            usize::try_from(capacity).map_err(|_| PsgError::SampleRateTooLarge)?;
        let filter_coefficients = design_lowpass(clock_hz, sample_rate);
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
            noise_output: 0,
            clock_divider_phase: 0,
            output_phase: 0,
            noise_period: 0x10,
            filter_coefficients,
            filter_history: vec![0.0; FILTER_TAPS],
            filter_position: 0,
        };
        device.reset_inner();
        Ok(device)
    }

    pub fn clock_hz(&self) -> u64 {
        self.clock_hz
    }

    pub fn sample_rate(&self) -> u64 {
        self.sample_rate
    }

    pub fn dropped_samples(&self) -> u64 {
        self.dropped_samples
    }

    pub fn tone_periods(&self) -> [u16; 3] {
        self.tone_periods
    }

    pub fn attenuations(&self) -> [u8; 4] {
        self.attenuations
    }

    pub fn noise_control(&self) -> u8 {
        self.noise_control
    }

    pub fn lfsr(&self) -> u16 {
        self.lfsr
    }

    pub fn noise_period(&self) -> u16 {
        self.noise_period
    }

    pub fn pending_sample_count(&self) -> usize {
        self.pending_samples.len()
    }

    pub fn sample_buffer_capacity(&self) -> usize {
        self.sample_buffer_capacity
    }

    pub fn latched_register(&self) -> u8 {
        self.latched_register
    }

    pub fn tone_counters(&self) -> [u16; 3] {
        self.tone_counters
    }

    pub fn tone_outputs(&self) -> [u8; 3] {
        self.tone_outputs
    }

    pub fn noise_counter(&self) -> u16 {
        self.noise_counter
    }

    pub fn noise_counter_phase(&self) -> u8 {
        self.noise_counter_phase
    }

    pub fn noise_output(&self) -> u8 {
        self.noise_output
    }

    pub fn clock_divider_phase(&self) -> u64 {
        self.clock_divider_phase
    }

    pub fn output_phase(&self) -> u64 {
        self.output_phase
    }

    pub fn reset(&mut self) {
        self.reset_inner();
    }

    pub fn write(&mut self, value: u8) {
        self.write_inner(value);
    }

    pub fn advance_clocks(&mut self, clocks: u64) {
        self.advance_clocks_inner(clocks, false);
    }

    pub fn drain_samples(&mut self, max_samples: Option<usize>) -> Vec<f64> {
        let count = max_samples
            .unwrap_or(self.pending_samples.len())
            .min(self.pending_samples.len());
        (0..count)
            .filter_map(|_| self.pending_samples.pop_front())
            .collect()
    }

    pub fn render(&mut self, sample_count: usize) -> Vec<f64> {
        let mut output = Vec::with_capacity(sample_count.min(1_000_000));
        while output.len() < sample_count {
            if let Some(sample) = self.render_pending.pop_front() {
                output.push(sample);
                continue;
            }
            let clocks_to_next_tick = CLOCK_DIVIDER - self.clock_divider_phase;
            self.advance_clocks_inner(clocks_to_next_tick, true);
        }
        output
    }
}

impl Sn76489 {
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
        self.noise_output = 0;
        self.clock_divider_phase = 0;
        self.output_phase = 0;
        self.filter_history.fill(0.0);
        self.filter_position = 0;
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
        self.noise_output = (self.lfsr & 1) as u8;
        self.noise_counter = self.noise_period;
        self.noise_counter_phase = 0;
    }

    fn shift_noise_register(&mut self) {
        self.noise_output = (self.lfsr & 1) as u8;
        let feedback = if self.noise_control & 0x04 != 0 {
            // SMS/GG integrated PSG white-noise polynomial: taps at bits 0 and 3.
            ((self.lfsr & 0x0009).count_ones() as u16) & 1
        } else {
            self.lfsr & 1
        };
        self.lfsr = ((self.lfsr >> 1) | (feedback << 15)) & LFSR_MASK;
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
            self.push_filter_sample();
            self.output_phase += self.sample_rate * CLOCK_DIVIDER;
            if self.output_phase >= self.clock_hz {
                let sample = self.filtered_output();
                while self.output_phase >= self.clock_hz {
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
    }

    fn push_filter_sample(&mut self) {
        let taps = self.filter_coefficients.len();
        let current_sample = self.mix();
        self.filter_history[self.filter_position] = current_sample;
        self.filter_position = (self.filter_position + 1) % taps;
    }

    fn filtered_output(&self) -> f64 {
        let taps = self.filter_coefficients.len();
        let mut total = 0.0;
        for index in 0..taps {
            let history_index = (self.filter_position + taps - 1 - index) % taps;
            total += self.filter_coefficients[index] * self.filter_history[history_index];
        }
        total
    }

    fn mix(&self) -> f64 {
        let mut total = ATTENUATION[usize::from(self.attenuations[0])]
            * if self.tone_outputs[0] != 0 { 1.0 } else { -1.0 };
        total += ATTENUATION[usize::from(self.attenuations[1])]
            * if self.tone_outputs[1] != 0 { 1.0 } else { -1.0 };
        total += ATTENUATION[usize::from(self.attenuations[2])]
            * if self.tone_outputs[2] != 0 { 1.0 } else { -1.0 };
        total += ATTENUATION[usize::from(self.attenuations[NOISE_CHANNEL])]
            * if self.noise_output != 0 { 1.0 } else { -1.0 };
        total * 0.25
    }
}


#[cfg(test)]
mod tests {
    use super::Sn76489;

    #[test]
    fn sms_profile_starts_with_a_sixteen_bit_lfsr() {
        let psg = Sn76489::new(3_579_545, 44_100).unwrap();
        assert_eq!(psg.lfsr(), 0x8000);
    }

    #[test]
    fn sms_white_noise_uses_taps_zero_and_three() {
        let mut psg = Sn76489::new(3_579_545, 44_100).unwrap();
        psg.write(0xE4);
        let expected = [
            0x8000_u16, 0x4000, 0x2000, 0x1000, 0x0800, 0x0400, 0x0200, 0x0100,
            0x0080, 0x0040, 0x0020, 0x0010, 0x0008, 0x8004, 0x4002, 0x2001,
        ];
        for state in expected {
            assert_eq!(psg.lfsr(), state);
            psg.shift_noise_register();
        }
    }

    #[test]
    fn sms_periodic_noise_shifts_the_low_bit_into_bit_fifteen() {
        let mut psg = Sn76489::new(3_579_545, 44_100).unwrap();
        psg.write(0xE0);
        for _ in 0..15 {
            psg.shift_noise_register();
        }
        assert_eq!(psg.lfsr(), 1);
        psg.shift_noise_register();
        assert_eq!(psg.lfsr(), 0x8000);
    }

    #[test]
    fn tone_registers_and_attenuation_keep_sn76489_protocol() {
        let mut psg = Sn76489::new(3_579_545, 44_100).unwrap();
        psg.write(0x85);
        psg.write(0x12);
        psg.write(0x90);
        assert_eq!(psg.tone_periods()[0], 0x125);
        assert_eq!(psg.attenuations()[0], 0);
    }

    #[test]
    fn audio_generation_is_deterministic_and_chunk_invariant() {
        let mut whole = Sn76489::new(3_579_545, 44_100).unwrap();
        let mut chunked = whole.clone();
        for psg in [&mut whole, &mut chunked] {
            psg.write(0x90);
            psg.write(0x85);
            psg.write(0x12);
        }
        whole.advance_clocks(120_000);
        chunked.advance_clocks(40_000);
        chunked.advance_clocks(80_000);
        assert_eq!(whole, chunked);
        let samples = whole.drain_samples(None);
        assert!(!samples.is_empty());
        assert!(samples.iter().any(|sample| sample.abs() > 0.001));
    }

    #[test]
    fn constructor_rejects_zero_clock_and_sample_rate() {
        assert_eq!(
            Sn76489::new(0, 44_100).unwrap_err(),
            super::PsgError::ZeroClock
        );
        assert_eq!(
            Sn76489::new(3_579_545, 0).unwrap_err(),
            super::PsgError::ZeroSampleRate
        );
    }
}
