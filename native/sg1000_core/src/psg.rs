//! Pure Rust SN76489AN profile for the SG-1000.
//!
//! Digital synthesis and sample buffering are independent of PyO3. This module
//! preserves the existing native algorithm while making ownership explicit in
//! the machine runtime. The FIR is an output filter, not an analog circuit model.

use std::collections::VecDeque;
use std::f64::consts::PI;
use std::fmt;

const CLOCK_DIVIDER: u64 = 16;
const TONE_CHANNELS: usize = 3;
const NOISE_CHANNEL: usize = 3;
const LFSR_MASK: u16 = 0x7FFF;
const LFSR_SEED: u16 = 0x4000;
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
    use std::collections::HashSet;

    use super::Sn76489;

    const WHITE_NOISE_GOLDEN_STATES: [u16; 32] = [
        0x4000, 0x2000, 0x1000, 0x0800, 0x0400, 0x0200, 0x0100, 0x0080, 0x0040, 0x0020, 0x0010,
        0x0008, 0x0004, 0x0002, 0x4001, 0x6000, 0x3000, 0x1800, 0x0C00, 0x0600, 0x0300, 0x0180,
        0x00C0, 0x0060, 0x0030, 0x0018, 0x000C, 0x0006, 0x4003, 0x2001, 0x5000, 0x2800,
    ];

    #[test]
    fn reset_and_repeated_advancement_are_deterministic() {
        let mut first = Sn76489::new(3_579_545, 44_100).unwrap();
        let mut second = Sn76489::new(3_579_545, 44_100).unwrap();
        for device in [&mut first, &mut second] {
            device.write_inner(0x85);
            device.write_inner(0x12);
            device.write_inner(0x90);
            device.write_inner(0xE4);
            device.advance_clocks_inner(75_000, false);
        }
        assert_eq!(first.tone_periods, second.tone_periods);
        assert_eq!(first.lfsr, second.lfsr);
        assert_eq!(first.noise_output, second.noise_output);
        assert_eq!(first.pending_samples, second.pending_samples);
        first.reset_inner();
        second.reset_inner();
        assert_eq!(first.tone_periods, [0; 3]);
        assert_eq!(first.attenuations, [15; 4]);
        assert_eq!(first.pending_samples.len(), 0);
        assert_eq!(first.pending_samples, second.pending_samples);
    }

    #[test]
    fn all_tone_period_register_values_round_trip() {
        for channel in 0..3 {
            for period in 0_u16..=0x03FF {
                let mut device = Sn76489::new(3_579_545, 44_100).unwrap();
                let register = (channel as u8) * 2;
                device.write_inner(0x80 | (register << 4) | (period as u8 & 0x0F));
                device.write_inner((period >> 4) as u8);
                assert_eq!(device.tone_periods[channel], period);
            }
        }
    }

    #[test]
    fn every_latch_register_and_nibble_is_decoded() {
        for register in 0_u8..8 {
            for nibble in 0_u8..16 {
                let mut device = Sn76489::new(3_579_545, 44_100).unwrap();
                device.write_inner(0x80 | (register << 4) | nibble);
                assert_eq!(device.latched_register, register);
                match register {
                    0 | 2 | 4 => {
                        assert_eq!(
                            device.tone_periods[usize::from(register / 2)],
                            u16::from(nibble)
                        );
                    }
                    1 | 3 | 5 | 7 => {
                        assert_eq!(device.attenuations[usize::from(register / 2)], nibble);
                    }
                    6 => assert_eq!(device.noise_control, nibble & 0x07),
                    _ => unreachable!("register is restricted to three bits"),
                }
            }
        }
    }

    #[test]
    fn white_noise_lfsr_matches_fixed_golden_vector() {
        let mut device = Sn76489::new(3_579_545, 44_100).unwrap();
        device.write_inner(0xE4);
        for expected in WHITE_NOISE_GOLDEN_STATES {
            assert_eq!(device.lfsr, expected);
            device.shift_noise_register();
        }
    }

    #[test]
    fn white_noise_lfsr_visits_all_nonzero_fifteen_bit_states_once() {
        let mut device = Sn76489::new(3_579_545, 44_100).unwrap();
        device.write_inner(0xE4);
        let mut seen = HashSet::with_capacity(0x7FFF);

        for _ in 0..0x7FFF {
            assert!(
                seen.insert(device.lfsr),
                "repeated LFSR state {:#06x}",
                device.lfsr
            );
            assert!((1..=0x7FFF).contains(&device.lfsr));
            device.shift_noise_register();
        }

        assert_eq!(seen.len(), 0x7FFF);
        assert_eq!(device.lfsr, 0x4000);
    }

    #[test]
    fn noise_mixer_outputs_the_bit_shifted_out_of_the_lfsr() {
        let mut device = Sn76489::new(3_579_545, 44_100).unwrap();
        device.write_inner(0xE0);
        device.write_inner(0xF0);

        for _ in 0..14 {
            device.shift_noise_register();
            assert_eq!(device.mix(), -0.25);
        }
        device.shift_noise_register();
        assert_eq!(device.mix(), 0.25);
    }

    #[test]
    fn periodic_noise_lfsr_recycles_the_single_one_bit() {
        let mut device = Sn76489::new(3_579_545, 44_100).unwrap();
        device.write_inner(0xE0);
        let mut states = Vec::with_capacity(15);

        for _ in 0..15 {
            states.push(device.lfsr);
            device.shift_noise_register();
        }

        assert_eq!(states[0], 0x4000);
        assert_eq!(states[14], 0x0001);
        assert_eq!(device.lfsr, 0x4000);
    }

    #[test]
    fn tone_counter_toggles_at_each_exhaustive_period_boundary() {
        for channel in 0..3 {
            for period in 2_u16..=0x03FF {
                let mut device = Sn76489::new(3_579_545, 44_100).unwrap();
                device.tone_periods[channel] = period;
                device.tone_counters[channel] = period;
                device.tone_outputs[channel] = 1;

                for _ in 1..period {
                    device.tick_internal_clock();
                    assert_eq!(device.tone_outputs[channel], 1);
                }
                device.tick_internal_clock();
                assert_eq!(device.tone_outputs[channel], 0);
            }
        }
    }

    #[test]
    fn rendered_tone_frequency_matches_clock_and_divider() {
        for period in [127_u16, 254, 511] {
            let mut device = Sn76489::new(3_579_545, 44_100).unwrap();
            device.write_inner(0x90);
            device.write_inner(0x80 | (period as u8 & 0x0F));
            device.write_inner((period >> 4) as u8);

            let half_second_clocks = device.clock_hz / 2;
            device.advance_clocks_inner(half_second_clocks, false);
            // Ignore the FIR startup transient and measure zero crossings rather
            // than every sample-value change; filtering makes adjacent samples
            // differ even when no fundamental-frequency transition occurred.
            let samples: Vec<f64> = device.pending_samples.iter().copied().skip(1_024).collect();
            let transitions = samples
                .windows(2)
                .filter(|pair| (pair[0] < 0.0) != (pair[1] < 0.0))
                .count();
            let expected_frequency = device.clock_hz as f64 / (32.0 * f64::from(period));
            let measured_frequency =
                transitions as f64 * device.sample_rate as f64 / (2.0 * (samples.len() - 1) as f64);

            assert!(
                (measured_frequency - expected_frequency).abs() <= 2.5,
                "period={period}: expected {expected_frequency:.3} Hz, measured {measured_frequency:.3} Hz from {transitions} transitions"
            );
        }
    }

    #[test]
    fn clock_advancement_is_invariant_under_chunking() {
        let mut whole = Sn76489::new(3_579_545, 44_100).unwrap();
        let mut chunked = Sn76489::new(3_579_545, 44_100).unwrap();
        for device in [&mut whole, &mut chunked] {
            device.write_inner(0x85);
            device.write_inner(0x12);
            device.write_inner(0x90);
            device.write_inner(0xE5);
            device.write_inner(0xF3);
        }

        whole.advance_clocks_inner(123_457, false);
        chunked.advance_clocks_inner(12_345, false);
        chunked.advance_clocks_inner(11_112, false);
        chunked.advance_clocks_inner(100_000, false);

        assert_eq!(chunked.tone_periods, whole.tone_periods);
        assert_eq!(chunked.tone_counters, whole.tone_counters);
        assert_eq!(chunked.tone_outputs, whole.tone_outputs);
        assert_eq!(chunked.noise_counter, whole.noise_counter);
        assert_eq!(chunked.noise_counter_phase, whole.noise_counter_phase);
        assert_eq!(chunked.lfsr, whole.lfsr);
        assert_eq!(chunked.noise_output, whole.noise_output);
        assert_eq!(chunked.clock_divider_phase, whole.clock_divider_phase);
        assert_eq!(chunked.output_phase, whole.output_phase);
        assert_eq!(chunked.pending_samples, whole.pending_samples);
    }
}
