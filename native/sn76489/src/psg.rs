//! Shared SN76489-family PSG core with explicit console hardware profiles.
//!
//! Digital synthesis and sample buffering are independent of PyO3. This module
//! preserves the existing native algorithm while making ownership explicit in
//! the machine runtime. The FIR is an output filter, not an analog circuit model.

use std::collections::VecDeque;
use std::f64::consts::PI;
use std::fmt;

const CLOCK_DIVIDER: u64 = 16;

/// Split a master-clock delta without overflowing when `clocks` is near u64::MAX.
fn split_clock_advance(clocks: u64, phase: u64) -> (u64, u64) {
    debug_assert!(phase < CLOCK_DIVIDER);
    let partial = clocks % CLOCK_DIVIDER + phase;
    (
        clocks / CLOCK_DIVIDER + partial / CLOCK_DIVIDER,
        partial % CLOCK_DIVIDER,
    )
}

/// Accumulate the resampler phase using a wider intermediate.
fn advance_output_phase(phase: u64, sample_rate: u64, clock_hz: u64) -> (u64, u64) {
    debug_assert!(phase < clock_hz);
    let total = u128::from(phase) + u128::from(sample_rate) * u128::from(CLOCK_DIVIDER);
    let divisor = u128::from(clock_hz);
    let samples = u64::try_from(total / divisor)
        .expect("validated sample rate and phase produce a u64 sample count");
    let remainder =
        u64::try_from(total % divisor).expect("remainder is smaller than the u64 clock frequency");
    (samples, remainder)
}

/// Interpolate a filtered PSG signal at the exact fractional output-clock edge.
///
/// The phase is the output-rate accumulator immediately before the current
/// internal PSG tick. Each output sample crosses one clock_hz boundary inside
/// the interval between the previous and current filtered samples.
fn interpolate_output_sample(
    previous: f64,
    current: f64,
    phase: u64,
    sample_rate: u64,
    clock_hz: u64,
    sample_index: u64,
) -> f64 {
    debug_assert!(phase < clock_hz);
    let increment = u128::from(sample_rate) * u128::from(CLOCK_DIVIDER);
    let crossing = u128::from(clock_hz - phase) + u128::from(sample_index) * u128::from(clock_hz);
    let fraction = crossing as f64 / increment as f64;
    debug_assert!((0.0..=1.0).contains(&fraction));
    previous + (current - previous) * fraction
}

const TONE_CHANNELS: usize = 3;
const NOISE_CHANNEL: usize = 3;
const SAMPLE_BUFFER_SECONDS: u64 = 1;
const MIN_FILTER_TAPS: usize = 63;
const KAISER_ATTENUATION_DB: f64 = 80.0;
const KAISER_BETA: f64 = 7.857_26;
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

/// Approximate the modified Bessel function I0 for the Kaiser window.
fn bessel_i0(value: f64) -> f64 {
    let quarter_square = value * value * 0.25;
    let mut sum = 1.0;
    let mut term = 1.0;
    for order in 1..=32 {
        term *= quarter_square / (order as f64 * order as f64);
        sum += term;
        if term <= sum * f64::EPSILON {
            break;
        }
    }
    sum
}

/// Design a normalized low-pass FIR for PSG clock-to-host-rate conversion.
///
/// For downsampling, the passband ends at 84% of the output Nyquist frequency
/// and the stopband begins at output Nyquist. The Kaiser order is selected for
/// approximately 80 dB stopband attenuation. This avoids asking a short windowed
/// FIR to make an unrealistically sharp transition at the SMS 5:1 rate ratio.
fn design_lowpass(clock_hz: u64, sample_rate: u64) -> Vec<f64> {
    let internal_rate = clock_hz as f64 / CLOCK_DIVIDER as f64;
    let rate_ratio = (sample_rate as f64 / internal_rate).min(1.0);

    let (cutoff, taps) = if (sample_rate as f64) < internal_rate {
        let transition_width = 0.08 * rate_ratio;
        let order =
            ((KAISER_ATTENUATION_DB - 8.0) / (2.285 * 2.0 * PI * transition_width)).ceil() as usize;
        let mut taps = (order + 1).max(MIN_FILTER_TAPS);
        if taps.is_multiple_of(2) {
            taps += 1;
        }
        (0.46 * rate_ratio, taps)
    } else {
        (0.45, MIN_FILTER_TAPS)
    };

    let center = (taps - 1) as f64 / 2.0;
    let window_denominator = bessel_i0(KAISER_BETA);
    let mut coefficients = Vec::with_capacity(taps);
    for index in 0..taps {
        let offset = index as f64 - center;
        let ideal = if offset == 0.0 {
            2.0 * cutoff
        } else {
            (2.0 * PI * cutoff * offset).sin() / (PI * offset)
        };
        let normalized_position = offset / center;
        let window = bessel_i0(
            KAISER_BETA
                * (1.0 - normalized_position * normalized_position)
                    .max(0.0)
                    .sqrt(),
        ) / window_denominator;
        coefficients.push(ideal * window);
    }

    let total: f64 = coefficients.iter().sum();
    for coefficient in &mut coefficients {
        *coefficient /= total;
    }
    coefficients
}

/// Hardware-specific noise generator behavior for compatible SN76489-family chips.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PsgProfile {
    Sg1000,
    Sms,
}
impl PsgProfile {
    const fn lfsr_seed(self) -> u16 {
        match self {
            Self::Sg1000 => 0x4000,
            Self::Sms => 0x8000,
        }
    }
    const fn lfsr_mask(self) -> u16 {
        match self {
            Self::Sg1000 => 0x7FFF,
            Self::Sms => 0xFFFF,
        }
    }
    const fn feedback_shift(self) -> u32 {
        match self {
            Self::Sg1000 => 14,
            Self::Sms => 15,
        }
    }
    fn white_noise_feedback(self, value: u16) -> u16 {
        match self {
            Self::Sg1000 => (value ^ (value >> 1)) & 1,
            Self::Sms => ((value & 0x0009).count_ones() as u16) & 1,
        }
    }
}
/// Current version of the complete, deterministic PSG persistence state.
pub const SN76489_STATE_FORMAT_VERSION: u16 = 1;

/// Complete versioned state required to resume the digital PSG at an arbitrary clock boundary.
///
/// FIR coefficients are derived from the immutable clock/sample-rate configuration;
/// FIR history, resampler phase, pending PCM and all mutable chip counters are retained.
#[derive(Debug, Clone, PartialEq)]
pub struct Sn76489StateV1 {
    pub format_version: u16,
    pub profile: PsgProfile,
    pub clock_hz: u64,
    pub sample_rate: u64,
    pub sample_buffer_capacity: usize,
    pub dropped_samples: u64,
    pub tone_periods: [u16; TONE_CHANNELS],
    pub attenuations: [u8; 4],
    pub noise_control: u8,
    pub latched_register: u8,
    pub tone_counters: [u16; TONE_CHANNELS],
    pub tone_outputs: [u8; TONE_CHANNELS],
    pub noise_counter: u16,
    pub noise_counter_phase: u8,
    pub lfsr: u16,
    pub noise_output: u8,
    pub clock_divider_phase: u64,
    pub output_phase: u64,
    pub noise_period: u16,
    pub filter_history: Vec<f64>,
    pub filter_position: usize,
    pub pending_samples: Vec<f64>,
    pub render_pending: Vec<f64>,
}

/// Failure while validating or restoring a PSG persistence state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PsgStateError {
    UnsupportedVersion { expected: u16, actual: u16 },
    InvalidConfiguration(PsgError),
    InvalidState(&'static str),
}

impl fmt::Display for PsgStateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedVersion { expected, actual } => write!(
                f,
                "SN76489 state version {actual} is unsupported (expected {expected})"
            ),
            Self::InvalidConfiguration(error) => {
                write!(f, "SN76489 state configuration is invalid: {error}")
            }
            Self::InvalidState(reason) => write!(f, "SN76489 state is invalid: {reason}"),
        }
    }
}

impl std::error::Error for PsgStateError {}

impl Sn76489StateV1 {
    fn validate_for(&self, core: &Sn76489Core) -> Result<(), PsgStateError> {
        if self.format_version != SN76489_STATE_FORMAT_VERSION {
            return Err(PsgStateError::UnsupportedVersion {
                expected: SN76489_STATE_FORMAT_VERSION,
                actual: self.format_version,
            });
        }
        if self.sample_buffer_capacity != core.sample_buffer_capacity {
            return Err(PsgStateError::InvalidState(
                "sample buffer capacity does not match the configured sample rate",
            ));
        }
        if self.tone_periods.iter().any(|period| *period > 0x03FF)
            || self.tone_counters.iter().any(|counter| *counter > 0x03FF)
        {
            return Err(PsgStateError::InvalidState(
                "tone period or counter exceeds the 10-bit hardware range",
            ));
        }
        if self.attenuations.iter().any(|level| *level > 0x0F)
            || self.noise_control > 0x07
            || self.latched_register > 0x07
        {
            return Err(PsgStateError::InvalidState(
                "register value exceeds its hardware bit width",
            ));
        }
        if self.tone_outputs.iter().any(|output| *output > 1)
            || self.noise_output > 1
            || self.noise_counter_phase > 1
        {
            return Err(PsgStateError::InvalidState(
                "oscillator output or noise divider phase is not binary",
            ));
        }
        if self.lfsr == 0 || self.lfsr & !self.profile.lfsr_mask() != 0 {
            return Err(PsgStateError::InvalidState(
                "LFSR state is zero or exceeds the selected hardware profile",
            ));
        }
        let expected_noise_period = if self.noise_control & 0x03 == 3 {
            self.tone_periods[2].max(1)
        } else {
            0x10_u16 << (self.noise_control & 0x03)
        };
        if self.noise_period != expected_noise_period || self.noise_counter == 0 {
            return Err(PsgStateError::InvalidState(
                "noise divider period or counter is inconsistent",
            ));
        }
        if self.clock_divider_phase >= CLOCK_DIVIDER
            || self.output_phase >= self.clock_hz
        {
            return Err(PsgStateError::InvalidState(
                "clock divider or output sampling phase is out of range",
            ));
        }
        if self.filter_history.len() != core.filter_coefficients.len()
            || self.filter_position >= self.filter_history.len()
        {
            return Err(PsgStateError::InvalidState(
                "FIR history length or cursor is inconsistent with the filter configuration",
            ));
        }
        if self.pending_samples.len() > self.sample_buffer_capacity
            || self.render_pending.len() > self.sample_buffer_capacity
        {
            return Err(PsgStateError::InvalidState(
                "pending PCM exceeds the configured sample buffer capacity",
            ));
        }
        if self
            .filter_history
            .iter()
            .chain(self.pending_samples.iter())
            .chain(self.render_pending.iter())
            .any(|sample| !sample.is_finite())
        {
            return Err(PsgStateError::InvalidState(
                "FIR history or pending PCM contains a non-finite sample",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Sn76489Core {
    profile: PsgProfile,
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

impl Sn76489Core {
    pub fn new(clock_hz: u64, sample_rate: u64) -> Result<Self, PsgError> {
        Self::new_with_profile(clock_hz, sample_rate, PsgProfile::Sg1000)
    }
    pub fn new_with_profile(
        clock_hz: u64,
        sample_rate: u64,
        profile: PsgProfile,
    ) -> Result<Self, PsgError> {
        if clock_hz == 0 {
            return Err(PsgError::ZeroClock);
        }
        if sample_rate == 0 {
            return Err(PsgError::ZeroSampleRate);
        }
        let output_clock_numerator = sample_rate
            .checked_mul(CLOCK_DIVIDER)
            .ok_or(PsgError::SampleRateTooLarge)?;
        if output_clock_numerator > clock_hz {
            return Err(PsgError::SampleRateTooLarge);
        }
        let capacity = sample_rate
            .checked_mul(SAMPLE_BUFFER_SECONDS)
            .ok_or(PsgError::SampleRateTooLarge)?;
        let sample_buffer_capacity =
            usize::try_from(capacity).map_err(|_| PsgError::SampleRateTooLarge)?;
        let filter_coefficients = design_lowpass(clock_hz, sample_rate);
        let filter_history = vec![0.0; filter_coefficients.len()];
        sega_observability::tracing::info!(
            target: "sega::psg",
            clock_hz,
            sample_rate,
            profile = ?profile,
            sample_buffer_capacity,
            "SN76489 PSG initialized"
        );
        let mut device = Self {
            profile,
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
            lfsr: profile.lfsr_seed(),
            noise_output: 0,
            clock_divider_phase: 0,
            output_phase: 0,
            noise_period: 0x10,
            filter_coefficients,
            filter_history,
            filter_position: 0,
        };
        device.reset_inner();
        Ok(device)
    }

    pub fn profile(&self) -> PsgProfile {
        self.profile
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

    /// Capture every mutable chip, divider, resampler, FIR and PCM-queue field.
    pub fn snapshot_state(&self) -> Sn76489StateV1 {
        Sn76489StateV1 {
            format_version: SN76489_STATE_FORMAT_VERSION,
            profile: self.profile,
            clock_hz: self.clock_hz,
            sample_rate: self.sample_rate,
            sample_buffer_capacity: self.sample_buffer_capacity,
            dropped_samples: self.dropped_samples,
            tone_periods: self.tone_periods,
            attenuations: self.attenuations,
            noise_control: self.noise_control,
            latched_register: self.latched_register,
            tone_counters: self.tone_counters,
            tone_outputs: self.tone_outputs,
            noise_counter: self.noise_counter,
            noise_counter_phase: self.noise_counter_phase,
            lfsr: self.lfsr,
            noise_output: self.noise_output,
            clock_divider_phase: self.clock_divider_phase,
            output_phase: self.output_phase,
            noise_period: self.noise_period,
            filter_history: self.filter_history.clone(),
            filter_position: self.filter_position,
            pending_samples: self.pending_samples.iter().copied().collect(),
            render_pending: self.render_pending.iter().copied().collect(),
        }
    }

    /// Construct a PSG from a validated state without losing any partial phase.
    pub fn from_state(state: Sn76489StateV1) -> Result<Self, PsgStateError> {
        let mut core = Self::new_with_profile(state.clock_hz, state.sample_rate, state.profile)
            .map_err(PsgStateError::InvalidConfiguration)?;
        state.validate_for(&core)?;

        core.dropped_samples = state.dropped_samples;
        core.tone_periods = state.tone_periods;
        core.attenuations = state.attenuations;
        core.noise_control = state.noise_control;
        core.latched_register = state.latched_register;
        core.tone_counters = state.tone_counters;
        core.tone_outputs = state.tone_outputs;
        core.noise_counter = state.noise_counter;
        core.noise_counter_phase = state.noise_counter_phase;
        core.lfsr = state.lfsr;
        core.noise_output = state.noise_output;
        core.clock_divider_phase = state.clock_divider_phase;
        core.output_phase = state.output_phase;
        core.noise_period = state.noise_period;
        core.filter_history = state.filter_history;
        core.filter_position = state.filter_position;
        core.pending_samples = state.pending_samples.into_iter().collect();
        core.render_pending = state.render_pending.into_iter().collect();
        Ok(core)
    }

    /// Restore atomically: invalid state leaves the current device untouched.
    pub fn restore_state(&mut self, state: Sn76489StateV1) -> Result<(), PsgStateError> {
        let restored = Self::from_state(state)?;
        *self = restored;
        Ok(())
    }

    pub fn reset(&mut self) {
        sega_observability::tracing::debug!(
            target: "sega::psg",
            "SN76489 PSG reset"
        );
        self.reset_inner();
    }

    pub fn write(&mut self, value: u8) {
        sega_observability::tracing::trace!(
            target: "sega::psg",
            value,
            "SN76489 register write"
        );
        self.write_inner(value);
    }

    pub fn advance_clocks(&mut self, clocks: u64) {
        self.advance_clocks_inner(clocks, false);
    }

    pub fn drain_samples(&mut self, max_samples: Option<usize>) -> Vec<f64> {
        let available_samples = self.pending_samples.len();
        let count = max_samples
            .unwrap_or(available_samples)
            .min(available_samples);
        let samples = (0..count)
            .filter_map(|_| self.pending_samples.pop_front())
            .collect::<Vec<_>>();
        sega_observability::tracing::trace!(
            target: "sega::psg",
            available_samples,
            drained_samples = samples.len(),
            remaining_samples = self.pending_samples.len(),
            "SN76489 sample queue drained"
        );
        samples
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

impl Sn76489Core {
    fn reset_inner(&mut self) {
        self.tone_periods = [0; TONE_CHANNELS];
        self.attenuations = [15; 4];
        self.noise_control = 0;
        self.latched_register = 0;
        self.tone_counters = [1; TONE_CHANNELS];
        self.tone_outputs = [1; TONE_CHANNELS];
        self.noise_counter = 0;
        self.noise_counter_phase = 0;
        self.lfsr = self.profile.lfsr_seed();
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
        // A noise-register write resets the shift register, not the running
        // divider. Keep the counter and its phase continuous across the write.
        self.lfsr = self.profile.lfsr_seed();
        self.noise_output = (self.lfsr & 1) as u8;
    }

    fn shift_noise_register(&mut self) {
        self.noise_output = (self.lfsr & 1) as u8;
        let feedback = if self.noise_control & 0x04 != 0 {
            self.profile.white_noise_feedback(self.lfsr)
        } else {
            self.lfsr & 1
        };
        self.lfsr = ((self.lfsr >> 1) | (feedback << self.profile.feedback_shift()))
            & self.profile.lfsr_mask();
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
        let (internal_clocks, phase) = split_clock_advance(clocks, self.clock_divider_phase);
        self.clock_divider_phase = phase;
        for _ in 0..internal_clocks {
            let previous_phase = self.output_phase;
            let (sample_count, next_phase) =
                advance_output_phase(previous_phase, self.sample_rate, self.clock_hz);
            let previous_filtered = if sample_count > 0 {
                self.filtered_output()
            } else {
                0.0
            };

            self.tick_internal_clock();
            self.push_filter_sample();
            self.output_phase = next_phase;

            if sample_count > 0 {
                let current_filtered = self.filtered_output();
                for sample_index in 0..sample_count {
                    let sample = interpolate_output_sample(
                        previous_filtered,
                        current_filtered,
                        previous_phase,
                        self.sample_rate,
                        self.clock_hz,
                        sample_index,
                    );
                    if render_sink {
                        self.render_pending.push_back(sample);
                    } else {
                        if self.pending_samples.len() == self.sample_buffer_capacity {
                            sega_observability::tracing::trace!(
                                target: "sega::psg",
                                capacity = self.sample_buffer_capacity,
                                "PSG sample queue overflow; oldest pending sample discarded"
                            );
                            self.pending_samples.pop_front();
                            self.dropped_samples += 1;
                        }
                        self.pending_samples.push_back(sample);
                    }
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

    use super::{PsgProfile, Sn76489Core as Sn76489};

    const WHITE_NOISE_GOLDEN_STATES: [u16; 32] = [
        0x4000, 0x2000, 0x1000, 0x0800, 0x0400, 0x0200, 0x0100, 0x0080, 0x0040, 0x0020, 0x0010,
        0x0008, 0x0004, 0x0002, 0x4001, 0x6000, 0x3000, 0x1800, 0x0C00, 0x0600, 0x0300, 0x0180,
        0x00C0, 0x0060, 0x0030, 0x0018, 0x000C, 0x0006, 0x4003, 0x2001, 0x5000, 0x2800,
    ];

    fn frequency_response(coefficients: &[f64], frequency: f64) -> f64 {
        let (real, imaginary) = coefficients.iter().enumerate().fold(
            (0.0, 0.0),
            |(real, imaginary), (index, coefficient)| {
                let phase = std::f64::consts::TAU * frequency * index as f64;
                (
                    real + coefficient * phase.cos(),
                    imaginary - coefficient * phase.sin(),
                )
            },
        );
        real.hypot(imaginary)
    }

    #[test]
    fn output_samples_are_invariant_to_scheduler_clock_chunking() {
        let clock_hz = 3_579_545_u64;
        let sample_rate = 44_100_u64;
        let total_clocks = 1_000_000_u64;
        let mut whole = Sn76489::new_with_profile(clock_hz, sample_rate, PsgProfile::Sms).unwrap();
        let mut chunked =
            Sn76489::new_with_profile(clock_hz, sample_rate, PsgProfile::Sms).unwrap();

        for device in [&mut whole, &mut chunked] {
            device.write_inner(0x85);
            device.write_inner(0x12);
            device.write_inner(0x90);
            device.write_inner(0xE4);
        }

        whole.advance_clocks(total_clocks);
        let chunk_sizes = [1_u64, 7, 15, 16, 17, 113, 257, 4_096];
        let mut remaining = total_clocks;
        let mut chunk_index = 0;
        while remaining > 0 {
            let clocks = chunk_sizes[chunk_index % chunk_sizes.len()].min(remaining);
            chunked.advance_clocks(clocks);
            remaining -= clocks;
            chunk_index += 1;
        }

        let expected_internal_ticks = total_clocks / super::CLOCK_DIVIDER;
        let expected_samples = (u128::from(expected_internal_ticks)
            * u128::from(sample_rate)
            * u128::from(super::CLOCK_DIVIDER)
            / u128::from(clock_hz)) as usize;
        let whole_samples = whole.drain_samples(None);
        let chunked_samples = chunked.drain_samples(None);

        assert_eq!(whole_samples.len(), expected_samples);
        assert_eq!(chunked_samples, whole_samples);
        assert_eq!(chunked.clock_divider_phase, whole.clock_divider_phase);
        assert_eq!(chunked.output_phase, whole.output_phase);
        assert_eq!(chunked.filter_history, whole.filter_history);
        assert_eq!(chunked.filter_position, whole.filter_position);
        assert_eq!(chunked.tone_counters, whole.tone_counters);
        assert_eq!(chunked.lfsr, whole.lfsr);
    }

    #[test]
    fn fractional_output_sampling_reduces_clock_quantization_error() {
        let clock_hz = 3_579_545_u64;
        let sample_rate = 44_100_u64;
        let frequency_hz = 10_000.0_f64;
        let internal_rate = clock_hz as f64 / super::CLOCK_DIVIDER as f64;
        let coefficients = super::design_lowpass(clock_hz, sample_rate);
        let taps = coefficients.len();
        let mut history = vec![0.0_f64; taps];
        let mut filter_position = 0_usize;
        let mut output_phase = 0_u64;
        let mut previous_filtered = 0.0_f64;
        let mut output_index = 0_u64;
        let mut nearest_error_squared = 0.0_f64;
        let mut interpolated_error_squared = 0.0_f64;
        let mut reference_squared = 0.0_f64;
        let mut measured_samples = 0_u64;
        let internal_ticks = (internal_rate * 0.25) as u64;
        let angular_frequency = std::f64::consts::TAU * frequency_hz / internal_rate;

        for tick in 0..internal_ticks {
            // The first FIR sample is captured after the first internal clock edge.
            let input = (angular_frequency * (tick as f64 + 1.0)).sin();
            history[filter_position] = input;
            filter_position = (filter_position + 1) % taps;

            let current_filtered = coefficients
                .iter()
                .enumerate()
                .map(|(index, coefficient)| {
                    let history_index = (filter_position + taps - 1 - index) % taps;
                    coefficient * history[history_index]
                })
                .sum::<f64>();

            let previous_phase = output_phase;
            let (sample_count, next_phase) =
                super::advance_output_phase(previous_phase, sample_rate, clock_hz);
            for sample_index in 0..sample_count {
                let quantized_tick_sample = current_filtered;
                let interpolated_sample = super::interpolate_output_sample(
                    previous_filtered,
                    current_filtered,
                    previous_phase,
                    sample_rate,
                    clock_hz,
                    sample_index,
                );
                let target_tick = (output_index + 1) as f64 * internal_rate / sample_rate as f64;
                let reference = coefficients
                    .iter()
                    .enumerate()
                    .map(|(index, coefficient)| {
                        coefficient * (angular_frequency * (target_tick - index as f64)).sin()
                    })
                    .sum::<f64>();

                if tick > (taps * 2) as u64 {
                    nearest_error_squared += (quantized_tick_sample - reference).powi(2);
                    interpolated_error_squared += (interpolated_sample - reference).powi(2);
                    reference_squared += reference.powi(2);
                    measured_samples += 1;
                }
                output_index += 1;
            }
            previous_filtered = current_filtered;
            output_phase = next_phase;
        }

        let expected_samples = (u128::from(internal_ticks)
            * u128::from(sample_rate)
            * u128::from(super::CLOCK_DIVIDER)
            / u128::from(clock_hz)) as u64;
        assert_eq!(output_index, expected_samples);
        assert!(measured_samples > 0);

        let reference_rms = (reference_squared / measured_samples as f64).sqrt();
        let nearest_error_rms =
            (nearest_error_squared / measured_samples as f64).sqrt() / reference_rms;
        let interpolated_error_rms =
            (interpolated_error_squared / measured_samples as f64).sqrt() / reference_rms;

        assert!(
            nearest_error_rms > 0.05,
            "test must reproduce material tick-quantization error, got {nearest_error_rms:.4}"
        );
        assert!(
            interpolated_error_rms < 0.01,
            "fractional sampling error remains too high: {interpolated_error_rms:.4}"
        );
    }

    #[test]
    fn psg_fir_preserves_audio_band_and_rejects_the_full_stopband() {
        for clock_hz in [3_579_545_u64, 3_546_893] {
            for output_rate in [22_050_u64, 44_100, 48_000, 96_000] {
                let coefficients = super::design_lowpass(clock_hz, output_rate);
                let internal_rate = clock_hz as f64 / 16.0;
                let ratio = output_rate as f64 / internal_rate;
                let passband = frequency_response(&coefficients, 0.42 * ratio);
                let stopband = (0..=256)
                    .map(|index| {
                        let fraction = index as f64 / 256.0;
                        let frequency = 0.5 * ratio + (0.5 - 0.5 * ratio) * fraction;
                        frequency_response(&coefficients, frequency)
                    })
                    .fold(0.0_f64, f64::max);

                assert!(
                    passband > 0.999,
                    "passband gain too low at {clock_hz} Hz / {output_rate} Hz: {passband:.6}"
                );
                assert!(
                    stopband < 0.000_12,
                    "stopband attenuation below 78 dB at {clock_hz} Hz / {output_rate} Hz: {stopband:.8}"
                );
                assert!(coefficients.len() >= super::MIN_FILTER_TAPS);
                assert!(!coefficients.len().is_multiple_of(2));
                assert!(
                    coefficients
                        .iter()
                        .all(|coefficient| coefficient.is_finite())
                );

                let sum: f64 = coefficients.iter().sum();
                assert!(
                    (sum - 1.0).abs() < 1.0e-12,
                    "DC gain differs from unity at {clock_hz} Hz / {output_rate} Hz"
                );
                for index in 0..coefficients.len() / 2 {
                    let mirrored = coefficients.len() - 1 - index;
                    assert!(
                        (coefficients[index] - coefficients[mirrored]).abs() < 1.0e-12,
                        "FIR is not symmetric at tap {index} for {clock_hz} Hz / {output_rate} Hz"
                    );
                }
            }
        }
    }

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
    fn noise_rate_selects_fixed_dividers_or_tracks_tone_three() {
        let mut device = Sn76489::new(3_579_545, 44_100).unwrap();
        for (control, expected) in [(0_u8, 0x10_u16), (1, 0x20), (2, 0x40)] {
            device.write_inner(0xE0 | control);
            assert_eq!(device.noise_period, expected);
        }

        device.write_inner(0xE3);
        assert_eq!(device.noise_period, 1);
        device.write_inner(0xC3); // tone channel 3 period low nibble = 3
        device.write_inner(0x12); // period high bits = 0x12
        assert_eq!(device.tone_periods[2], 0x123);
        assert_eq!(device.noise_period, 0x123);
    }

    #[test]
    fn sms_tone_two_noise_rate_tracks_period_without_resetting_partial_divider() {
        let mut device = Sn76489::new_with_profile(3_579_545, 44_100, PsgProfile::Sms).unwrap();

        // Tone 2 period = 3; Noise Rate = Tone 2.
        device.write_inner(0xC3);
        device.write_inner(0x00);
        device.write_inner(0xE3);
        assert_eq!(device.tone_periods[2], 3);
        assert_eq!(device.noise_period, 3);

        // Put the independent noise divider at a known boundary, then advance
        // one tick so a Tone 2 write occurs with two divider ticks remaining.
        device.noise_counter = 3;
        device.noise_counter_phase = 0;
        device.tick_internal_clock();
        assert_eq!(device.noise_counter, 2);
        let before_write = device.lfsr;

        // Change Tone 2 to period 2. The current divider remainder is retained;
        // subsequent reloads use the new period, as in the reference's live
        // pointer to the Tone 2 period and persistent oscillator delay.
        device.write_inner(0xC2);
        assert_eq!(device.noise_period, 2);
        assert_eq!(device.noise_counter, 2);
        assert_eq!(device.lfsr, before_write);

        device.tick_internal_clock();
        assert_eq!(device.lfsr, before_write);
        device.tick_internal_clock();
        assert_ne!(device.lfsr, before_write);
        let first_shift = device.lfsr;

        // Two expirations of the new period (2 + 2 internal ticks) produce
        // the next shift; the intervening expiration only changes divider phase.
        for _ in 0..3 {
            device.tick_internal_clock();
            assert_eq!(device.lfsr, first_shift);
        }
        device.tick_internal_clock();
        assert_ne!(device.lfsr, first_shift);
    }

    #[test]
    fn noise_register_write_resets_lfsr_without_restarting_noise_divider() {
        let mut device = Sn76489::new_with_profile(3_579_545, 44_100, PsgProfile::Sms).unwrap();
        device.noise_counter = 7;
        device.noise_counter_phase = 1;
        device.lfsr = 0x1234;
        device.noise_output = 1;

        device.write_inner(0xE4); // Latch white noise, fixed rate 0x10.
        assert_eq!(device.lfsr, 0x8000);
        assert_eq!(device.noise_output, 0);
        assert_eq!(device.noise_counter, 7);
        assert_eq!(device.noise_counter_phase, 1);

        device.noise_counter = 5;
        device.noise_counter_phase = 0;
        device.write_inner(0x07); // Data byte updates the latched noise register.
        assert_eq!(device.noise_control, 0x07);
        assert_eq!(device.lfsr, 0x8000);
        assert_eq!(device.noise_counter, 5);
        assert_eq!(device.noise_counter_phase, 0);
    }

    #[test]
    fn attenuation_zero_is_full_scale_and_fifteen_mutes_each_channel() {
        let mut device = Sn76489::new(3_579_545, 44_100).unwrap();
        device.attenuations = [0, 15, 15, 15];
        device.tone_outputs = [1; 3];
        device.noise_output = 1;
        assert_eq!(device.mix(), 0.25);

        device.attenuations[0] = 1;
        assert!((device.mix() - 0.198582).abs() < 0.000001);
        device.attenuations[0] = 15;
        assert_eq!(device.mix(), 0.0);
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
    fn sms_white_noise_trace_matches_independent_gear_system_reference() {
        // Gearsystem Sms_Apu.cpp: seed 0x8000, 16-bit register, taps 0 and 3,
        // output is the pre-shift bit 0, and feedback is inserted at bit 15.
        const TRACE: [(u16, u8, u16); 32] = [
            (0x8000, 0, 0x4000),
            (0x4000, 0, 0x2000),
            (0x2000, 0, 0x1000),
            (0x1000, 0, 0x0800),
            (0x0800, 0, 0x0400),
            (0x0400, 0, 0x0200),
            (0x0200, 0, 0x0100),
            (0x0100, 0, 0x0080),
            (0x0080, 0, 0x0040),
            (0x0040, 0, 0x0020),
            (0x0020, 0, 0x0010),
            (0x0010, 0, 0x0008),
            (0x0008, 0, 0x8004),
            (0x8004, 0, 0x4002),
            (0x4002, 0, 0x2001),
            (0x2001, 1, 0x9000),
            (0x9000, 0, 0x4800),
            (0x4800, 0, 0x2400),
            (0x2400, 0, 0x1200),
            (0x1200, 0, 0x0900),
            (0x0900, 0, 0x0480),
            (0x0480, 0, 0x0240),
            (0x0240, 0, 0x0120),
            (0x0120, 0, 0x0090),
            (0x0090, 0, 0x0048),
            (0x0048, 0, 0x8024),
            (0x8024, 0, 0x4012),
            (0x4012, 0, 0x2009),
            (0x2009, 1, 0x1004),
            (0x1004, 0, 0x0802),
            (0x0802, 0, 0x0401),
            (0x0401, 1, 0x8200),
        ];

        let mut device = Sn76489::new_with_profile(3_579_545, 44_100, PsgProfile::Sms).unwrap();
        device.write_inner(0xE4);

        for (state, output, next_state) in TRACE {
            assert_eq!(device.lfsr(), state);
            device.shift_noise_register();
            assert_eq!(device.noise_output(), output);
            assert_eq!(device.lfsr(), next_state);
        }
    }

    #[test]
    fn sms_periodic_noise_trace_matches_independent_gear_system_reference() {
        // Periodic mode feeds the outgoing bit 0 back into bit 15 without XOR.
        const TRACE: [(u16, u8, u16); 16] = [
            (0x8000, 0, 0x4000),
            (0x4000, 0, 0x2000),
            (0x2000, 0, 0x1000),
            (0x1000, 0, 0x0800),
            (0x0800, 0, 0x0400),
            (0x0400, 0, 0x0200),
            (0x0200, 0, 0x0100),
            (0x0100, 0, 0x0080),
            (0x0080, 0, 0x0040),
            (0x0040, 0, 0x0020),
            (0x0020, 0, 0x0010),
            (0x0010, 0, 0x0008),
            (0x0008, 0, 0x0004),
            (0x0004, 0, 0x0002),
            (0x0002, 0, 0x0001),
            (0x0001, 1, 0x8000),
        ];

        let mut device = Sn76489::new_with_profile(3_579_545, 44_100, PsgProfile::Sms).unwrap();
        device.write_inner(0xE0);

        for (state, output, next_state) in TRACE {
            assert_eq!(device.lfsr(), state);
            device.shift_noise_register();
            assert_eq!(device.noise_output(), output);
            assert_eq!(device.lfsr(), next_state);
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
        for period in [11_u16, 12, 127, 254, 511] {
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

    #[test]
    fn public_api_preserves_render_and_drain_buffer_contracts() {
        let mut psg = Sn76489::new(1_600, 100).unwrap();
        psg.write(0x90);
        psg.write(0x88);
        psg.advance_clocks(16 * 250);
        assert_eq!(psg.pending_sample_count(), 100);
        assert_eq!(psg.dropped_samples(), 150);
        assert_eq!(psg.drain_samples(Some(3)).len(), 3);
        assert_eq!(psg.drain_samples(None).len(), 97);
        assert!(psg.drain_samples(None).is_empty());

        let rendered = psg.render(32);
        assert_eq!(rendered.len(), 32);
        assert!(psg.drain_samples(None).is_empty());
    }

    #[test]
    fn clock_arithmetic_handles_u64_boundary_without_intermediate_overflow() {
        let (internal_clocks, phase) = super::split_clock_advance(u64::MAX, 15);
        assert_eq!(internal_clocks, u64::MAX / super::CLOCK_DIVIDER + 1);
        assert_eq!(phase, 14);

        let sample_rate = u64::MAX / super::CLOCK_DIVIDER;
        let (samples, phase) = super::advance_output_phase(u64::MAX - 1, sample_rate, u64::MAX);
        assert_eq!(samples, 1);
        assert_eq!(phase, u64::MAX - 16);
    }

    #[test]
    fn constructor_rejects_output_rate_above_internal_psg_rate() {
        assert!(Sn76489::new(1_600, 100).is_ok());
        assert_eq!(
            Sn76489::new(1_600, 101).unwrap_err(),
            super::PsgError::SampleRateTooLarge
        );
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
        assert_eq!(
            Sn76489::new(u64::MAX, u64::MAX).unwrap_err(),
            super::PsgError::SampleRateTooLarge
        );
    }
    #[test]
    fn sms_profile_uses_sixteen_bit_lfsr_and_white_noise_taps_zero_and_three() {
        let mut psg = Sn76489::new_with_profile(3_579_545, 44_100, PsgProfile::Sms).unwrap();
        assert_eq!(psg.lfsr(), 0x8000);
        psg.write(0xE4);
        let expected = [
            0x8000_u16, 0x4000, 0x2000, 0x1000, 0x0800, 0x0400, 0x0200, 0x0100, 0x0080, 0x0040,
            0x0020, 0x0010, 0x0008, 0x8004, 0x4002, 0x2001,
        ];
        for state in expected {
            assert_eq!(psg.lfsr(), state);
            psg.shift_noise_register();
        }
        assert_eq!(psg.profile(), PsgProfile::Sms);
    }
    #[test]
    fn sms_periodic_noise_shifts_feedback_into_bit_fifteen() {
        let mut psg = Sn76489::new_with_profile(3_579_545, 44_100, PsgProfile::Sms).unwrap();
        psg.write(0xE0);
        for _ in 0..15 {
            psg.shift_noise_register();
        }
        assert_eq!(psg.lfsr(), 1);
        psg.shift_noise_register();
        assert_eq!(psg.lfsr(), 0x8000);
    }
    #[test]
    fn profile_specific_noise_sequences_remain_distinct_and_repeatable() {
        let mut sms_a = Sn76489::new_with_profile(3_579_545, 44_100, PsgProfile::Sms).unwrap();
        let mut sms_b = sms_a.clone();
        let mut sg_a = Sn76489::new_with_profile(3_579_545, 44_100, PsgProfile::Sg1000).unwrap();
        let mut sg_b = sg_a.clone();

        for device in [&mut sms_a, &mut sms_b, &mut sg_a, &mut sg_b] {
            device.write(0xE4);
            for _ in 0..64 {
                device.shift_noise_register();
            }
        }

        assert_eq!(sms_a.lfsr(), sms_b.lfsr());
        assert_eq!(sg_a.lfsr(), sg_b.lfsr());
        assert_ne!(sms_a.lfsr(), sg_a.lfsr());
        assert_eq!(sms_a.profile(), PsgProfile::Sms);
        assert_eq!(sg_a.profile(), PsgProfile::Sg1000);
    }

    #[test]
    fn reset_restores_the_selected_profile_seed() {
        for (profile, seed) in [(PsgProfile::Sg1000, 0x4000), (PsgProfile::Sms, 0x8000)] {
            let mut psg = Sn76489::new_with_profile(3_579_545, 44_100, profile).unwrap();
            psg.write(0xE4);
            psg.shift_noise_register();
            psg.reset();
            assert_eq!(psg.lfsr(), seed);
            assert_eq!(psg.profile(), profile);
        }
    }
    #[test]
    fn sms_tone_register_protocol_is_shared_without_profile_drift() {
        let mut psg = Sn76489::new_with_profile(3_579_545, 44_100, PsgProfile::Sms).unwrap();
        psg.write(0x85);
        psg.write(0x12);
        psg.write(0x90);
        assert_eq!(psg.tone_periods(), [0x125, 0, 0]);
        assert_eq!(psg.attenuations()[0], 0);
    }
    #[test]
    fn sms_audio_generation_is_deterministic_and_chunk_invariant() {
        let mut whole = Sn76489::new_with_profile(3_579_545, 44_100, PsgProfile::Sms).unwrap();
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
        assert!(!whole.drain_samples(None).is_empty());
    }

    #[test]
    fn sms_attenuation_levels_match_independent_reference_table() {
        // SMS Power specifies a 2 dB attenuation step. Voltage ratios are
        // therefore 10^(-0.1 * level), with level 15 forced to silence.
        const REFERENCE: [f64; 16] = [
            1.0,
            0.794_328_23,
            0.630_957_34,
            0.501_187_23,
            0.398_107_17,
            0.316_227_77,
            0.251_188_64,
            0.199_526_23,
            0.158_489_32,
            0.125_892_54,
            0.1,
            0.079_432_82,
            0.063_095_73,
            0.050_118_72,
            0.039_810_72,
            0.0,
        ];

        let mut psg = Sn76489::new_with_profile(3_579_545, 44_100, PsgProfile::Sms).unwrap();
        psg.tone_outputs = [1; 3];
        psg.noise_output = 1;
        psg.attenuations = [15; 4];

        for (level, expected) in REFERENCE.into_iter().enumerate() {
            psg.attenuations[0] = level as u8;
            let measured = psg.mix() * 4.0;
            assert!(
                (measured - expected).abs() < 0.000_001,
                "SMS attenuation level {level}: expected {expected:.8}, got {measured:.8}"
            );
        }
    }

    #[test]
    fn sms_reference_latch_data_vectors_update_registers_immediately() {
        let mut psg = Sn76489::new_with_profile(3_579_545, 44_100, PsgProfile::Sms).unwrap();

        // SMS Power example: 0x8E followed by 0x0F produces Tone 0 = 0x0FE.
        psg.write(0x8E);
        assert_eq!(psg.tone_periods()[0], 0x00E);
        psg.write(0x0F);
        assert_eq!(psg.tone_periods()[0], 0x0FE);

        // SMS Power specifies that the latched register persists for data bytes.
        // A volume data byte updates the low four bits of the selected volume.
        psg.write(0xBF);
        assert_eq!(psg.attenuations()[1], 0x0F);
        psg.write(0x00);
        assert_eq!(psg.attenuations()[1], 0x00);

        // Noise data writes update all three meaningful bits.
        psg.write(0xE5);
        assert_eq!(psg.noise_control(), 0x05);
        psg.write(0x04);
        assert_eq!(psg.noise_control(), 0x04);
    }
}
