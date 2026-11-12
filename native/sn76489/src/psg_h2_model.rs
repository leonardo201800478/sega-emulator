//! Test-only event-domain reconstruction study for the SN76489 PSG.
//!
//! This module does not alter production synthesis. It captures digital
//! amplitude transitions at PSG-clock timestamps and reconstructs them with an
//! independent band-limited step kernel for reproducible A/B analysis.

use super::{PsgProfile, Sn76489Core};
use std::collections::HashSet;
use std::f64::consts::{PI, TAU};
use std::io::{BufWriter, Write};
use std::path::Path;
use std::time::Instant;

const CLOCK_HZ: u64 = 3_579_545;
const SAMPLE_RATE: u64 = 48_000;
const FULL_TRACE_CLOCKS: u64 = 16_000_000;
const TONE_TRACE_CLOCKS: u64 = 4_000_000;
const NOISE_TRACE_CLOCKS: u64 = 2_000_000;
const EVENT_TAPS: usize = 65;
const EVENT_HALF: f64 = 32.0;
const EVENT_OVERSAMPLE: usize = 1024;
const EVENT_CUTOFF: f64 = 0.46;
const EVENT_KAISER_BETA: f64 = 7.857_26;
const EVENT_FRACTIONAL_DELAY: f64 = -0.75;

const PSG_TRACE: [(u64, u8); 19] = [
    (0, 0x85),
    (0, 0x12),
    (0, 0x90),
    (0, 0xA8),
    (0, 0x05),
    (0, 0xB4),
    (0, 0xC0),
    (0, 0x10),
    (0, 0xD8),
    (0, 0xE4),
    (0, 0xF6),
    (2_000_000, 0x94),
    (4_000_000, 0xE7),
    (6_000_000, 0xF2),
    (8_000_000, 0x89),
    (8_000_000, 0x11),
    (10_000_000, 0xB7),
    (12_000_000, 0xE4),
    (14_000_000, 0xF4),
];

const TONE_SWEEP_PERIODS: [u16; 14] = [2, 3, 4, 5, 6, 7, 9, 14, 28, 56, 112, 255, 512, 1023];

#[derive(Clone, Copy, Debug, PartialEq)]
struct PsgAmplitudeEvent {
    clock: u64,
    delta: f64,
}

struct EventBasedKernel {
    step_response: Vec<f64>,
    impulse_response: Vec<f64>,
    discrete_coefficients: Vec<f64>,
    step_resolution: f64,
}

impl EventBasedKernel {
    fn new() -> Self {
        let denominator = super::bessel_i0(EVENT_KAISER_BETA);
        let sample_count = (2.0 * EVENT_HALF * EVENT_OVERSAMPLE as f64) as usize + 1;
        let step_resolution = 1.0 / EVENT_OVERSAMPLE as f64;
        let mut impulse_response = Vec::with_capacity(sample_count);
        let mut step_response = vec![0.0; sample_count];

        for index in 0..sample_count {
            let position = -EVENT_HALF + index as f64 * step_resolution;
            let normalized = position / EVENT_HALF;
            let window_argument =
                (1.0 - normalized * normalized).max(0.0).sqrt() * EVENT_KAISER_BETA;
            let window = super::bessel_i0(window_argument) / denominator;
            let scaled = 2.0 * EVENT_CUTOFF * position;
            let sinc = if scaled.abs() < 1.0e-12 {
                1.0
            } else {
                (PI * scaled).sin() / (PI * scaled)
            };
            impulse_response.push(2.0 * EVENT_CUTOFF * sinc * window);
        }

        for index in 1..sample_count {
            step_response[index] = step_response[index - 1]
                + 0.5 * (impulse_response[index - 1] + impulse_response[index]) * step_resolution;
        }

        let area = *step_response.last().expect("kernel has samples");
        assert!(area.is_finite() && area > 0.0);
        for value in &mut impulse_response {
            *value /= area;
        }
        for value in &mut step_response {
            *value /= area;
        }

        let mut discrete_coefficients = Vec::with_capacity(EVENT_TAPS);
        for index in 0..EVENT_TAPS {
            let sample_index = index * EVENT_OVERSAMPLE;
            discrete_coefficients.push(impulse_response[sample_index]);
        }
        let discrete_sum: f64 = discrete_coefficients.iter().sum();
        for coefficient in &mut discrete_coefficients {
            *coefficient /= discrete_sum;
        }

        Self {
            step_response,
            impulse_response,
            discrete_coefficients,
            step_resolution,
        }
    }

    fn response_to_step(&self, position: f64) -> f64 {
        if position <= -EVENT_HALF {
            return 0.0;
        }
        if position >= EVENT_HALF {
            return 1.0;
        }
        let coordinate = (position + EVENT_HALF) / self.step_resolution;
        let index = coordinate.floor() as usize;
        let fraction = coordinate - index as f64;
        let first = self.step_response[index];
        let second = self.step_response[index + 1];
        first + (second - first) * fraction
    }

    fn render(
        &self,
        events: &[PsgAmplitudeEvent],
        initial_level: f64,
        total_clocks: u64,
    ) -> Vec<f64> {
        let sample_count =
            (u128::from(total_clocks) * u128::from(SAMPLE_RATE) / u128::from(CLOCK_HZ)) as usize;
        let mut settled_deltas = vec![0.0; sample_count + 1];
        let mut transition_contributions = vec![0.0; sample_count];

        debug_assert!(events.windows(2).all(|pair| pair[0].clock <= pair[1].clock));
        let mut event_index = 0;
        let mut simultaneous_deltas = Vec::new();
        while event_index < events.len() {
            let event_clock = events[event_index].clock;
            simultaneous_deltas.clear();
            while event_index < events.len() && events[event_index].clock == event_clock {
                simultaneous_deltas.push(events[event_index].delta);
                event_index += 1;
            }

            // Canonicalize simultaneous deltas before convolution. Floating-point
            // accumulation must not depend on the order of same-clock bus writes.
            simultaneous_deltas.sort_by(f64::total_cmp);
            let event_delta = simultaneous_deltas.iter().sum::<f64>();
            if event_delta == 0.0 {
                continue;
            }

            let numerator = u128::from(event_clock) * u128::from(SAMPLE_RATE);
            let whole = (numerator / u128::from(CLOCK_HZ)) as f64;
            let remainder = (numerator % u128::from(CLOCK_HZ)) as f64;
            let event_position = whole + remainder / CLOCK_HZ as f64;

            // Sample zero is emitted at the first sample-clock crossing.
            let first = (event_position + EVENT_FRACTIONAL_DELAY - 1.0)
                .ceil()
                .max(0.0) as usize;
            let settled = (event_position + 2.0 * EVENT_HALF + EVENT_FRACTIONAL_DELAY - 1.0)
                .ceil()
                .max(0.0) as usize;

            if settled < sample_count {
                settled_deltas[settled] += event_delta;
            }
            let start = first.min(sample_count);
            let end = settled.min(sample_count);
            for (sample_index, contribution) in transition_contributions
                .iter_mut()
                .enumerate()
                .take(end)
                .skip(start)
            {
                let sample_time = sample_index as f64 + 1.0;
                let position = sample_time - event_position - EVENT_HALF - EVENT_FRACTIONAL_DELAY;
                *contribution += event_delta * self.response_to_step(position);
            }
        }

        let mut output = vec![0.0; sample_count];
        let mut level = initial_level;
        for index in 0..sample_count {
            level += settled_deltas[index];
            output[index] = level + transition_contributions[index];
        }
        output
    }

    fn discrete_frequency_response(&self, cycles_per_sample: f64) -> f64 {
        let mut real = 0.0;
        let mut imaginary = 0.0;
        for (index, coefficient) in self.discrete_coefficients.iter().enumerate() {
            let position = index as f64 - EVENT_HALF;
            let phase = TAU * cycles_per_sample * position;
            real += coefficient * phase.cos();
            imaginary -= coefficient * phase.sin();
        }
        real.hypot(imaginary)
    }

    fn continuous_frequency_response(&self, cycles_per_sample: f64) -> f64 {
        let resolution = self.step_resolution;
        let half = (self.impulse_response.len() - 1) / 2;
        let mut real = 0.0;
        let mut imaginary = 0.0;
        for (index, impulse) in self.impulse_response.iter().enumerate() {
            let position = (index as f64 - half as f64) * resolution;
            let weight = if index == 0 || index + 1 == self.impulse_response.len() {
                0.5
            } else {
                1.0
            };
            let phase = TAU * cycles_per_sample * position;
            real += weight * impulse * phase.cos() * resolution;
            imaginary -= weight * impulse * phase.sin() * resolution;
        }
        real.hypot(imaginary)
    }

    fn step_extrema(&self) -> (f64, f64) {
        self.step_response.iter().copied().fold(
            (f64::INFINITY, f64::NEG_INFINITY),
            |(minimum, maximum), value| (minimum.min(value), maximum.max(value)),
        )
    }

    fn settling_samples(&self, tolerance: f64) -> f64 {
        let Some(index) = (0..self.step_response.len()).find(|index| {
            self.step_response[*index..]
                .iter()
                .all(|value| (1.0 - value).abs() <= tolerance)
        }) else {
            return f64::INFINITY;
        };
        index as f64 * self.step_resolution
    }
}

fn record_event(
    psg: &Sn76489Core,
    events: &mut Vec<PsgAmplitudeEvent>,
    previous_level: &mut f64,
    clock: u64,
) {
    let current_level = psg.mix();
    if current_level != *previous_level {
        events.push(PsgAmplitudeEvent {
            clock,
            delta: current_level - *previous_level,
        });
        *previous_level = current_level;
    }
}

fn advance_digital_to(
    psg: &mut Sn76489Core,
    events: &mut Vec<PsgAmplitudeEvent>,
    previous_level: &mut f64,
    elapsed: &mut u64,
    target: u64,
    max_chunk: Option<u64>,
) {
    while *elapsed < target {
        let clocks_to_tick = super::CLOCK_DIVIDER - psg.clock_divider_phase;
        let chunk = (target - *elapsed)
            .min(clocks_to_tick)
            .min(max_chunk.unwrap_or(u64::MAX));
        psg.clock_divider_phase += chunk;
        *elapsed += chunk;
        if psg.clock_divider_phase == super::CLOCK_DIVIDER {
            psg.clock_divider_phase = 0;
            psg.tick_internal_clock();
            record_event(psg, events, previous_level, *elapsed);
        }
    }
}

fn capture_events(
    profile: PsgProfile,
    trace: &[(u64, u8)],
    total_clocks: u64,
    max_chunk: Option<u64>,
) -> Vec<PsgAmplitudeEvent> {
    let mut psg = Sn76489Core::new_with_profile(CLOCK_HZ, SAMPLE_RATE, profile).unwrap();
    let mut events = Vec::new();
    let mut previous_level = psg.mix();
    let mut elapsed = 0_u64;

    for &(clock, value) in trace {
        assert!(clock >= elapsed && clock <= total_clocks);
        advance_digital_to(
            &mut psg,
            &mut events,
            &mut previous_level,
            &mut elapsed,
            clock,
            max_chunk,
        );
        psg.write_inner(value);
        record_event(&psg, &mut events, &mut previous_level, elapsed);
    }
    advance_digital_to(
        &mut psg,
        &mut events,
        &mut previous_level,
        &mut elapsed,
        total_clocks,
        max_chunk,
    );
    events
}

fn advance_core_to(psg: &mut Sn76489Core, output: &mut Vec<f64>, elapsed: &mut u64, target: u64) {
    while *elapsed < target {
        let clocks = (target - *elapsed).min(16_384);
        psg.advance_clocks(clocks);
        output.extend(psg.drain_samples(None));
        *elapsed += clocks;
    }
}

fn render_core_trace(profile: PsgProfile, trace: &[(u64, u8)], total_clocks: u64) -> Vec<f64> {
    let mut psg = Sn76489Core::new_with_profile(CLOCK_HZ, SAMPLE_RATE, profile).unwrap();
    let mut output = Vec::new();
    let mut elapsed = 0_u64;
    for &(clock, value) in trace {
        advance_core_to(&mut psg, &mut output, &mut elapsed, clock);
        psg.write_inner(value);
    }
    advance_core_to(&mut psg, &mut output, &mut elapsed, total_clocks);
    output.extend(psg.drain_samples(None));
    output
}

fn tone_trace(period: u16) -> [(u64, u8); 6] {
    [
        (0, 0x80 | (period as u8 & 0x0f)),
        (0, (period >> 4) as u8),
        (0, 0x90),
        (0, 0xbf),
        (0, 0xdf),
        (0, 0xff),
    ]
}

fn noise_trace(control: u8) -> [(u64, u8); 7] {
    [
        (0, 0x9f),
        (0, 0xbf),
        (0, 0xc0),
        (0, 0x04),
        (0, 0xdf),
        (0, 0xe0 | (control & 0x07)),
        (0, 0xf0),
    ]
}

fn write_float_wav(path: &Path, samples: &[f64]) -> std::io::Result<()> {
    let data_bytes = u32::try_from(samples.len() * 4)
        .map_err(|_| std::io::Error::other("PSG WAV exceeds RIFF size"))?;
    let riff_size = 36_u32
        .checked_add(data_bytes)
        .ok_or_else(|| std::io::Error::other("PSG WAV exceeds RIFF size"))?;
    let mut writer = BufWriter::new(std::fs::File::create(path)?);
    writer.write_all(b"RIFF")?;
    writer.write_all(&riff_size.to_le_bytes())?;
    writer.write_all(b"WAVEfmt ")?;
    writer.write_all(&16_u32.to_le_bytes())?;
    writer.write_all(&3_u16.to_le_bytes())?;
    writer.write_all(&1_u16.to_le_bytes())?;
    writer.write_all(&(SAMPLE_RATE as u32).to_le_bytes())?;
    writer.write_all(&((SAMPLE_RATE * 4) as u32).to_le_bytes())?;
    writer.write_all(&4_u16.to_le_bytes())?;
    writer.write_all(&32_u16.to_le_bytes())?;
    writer.write_all(b"data")?;
    writer.write_all(&data_bytes.to_le_bytes())?;
    for sample in samples {
        writer.write_all(&(*sample as f32).to_le_bytes())?;
    }
    writer.flush()
}

fn artifact_directory() -> Option<std::path::PathBuf> {
    std::env::var_os("PSG_AUDIO_ARTIFACT_DIR").map(std::path::PathBuf::from)
}

fn export_wav(directory: &Path, name: &str, samples: &[f64]) {
    write_float_wav(&directory.join(name), samples).unwrap();
}

fn normalized_error(reference: &[f64], candidate: &[f64]) -> f64 {
    assert_eq!(reference.len(), candidate.len());
    let reference_rms =
        (reference.iter().map(|value| value * value).sum::<f64>() / reference.len() as f64).sqrt();
    let error_rms = (reference
        .iter()
        .zip(candidate)
        .map(|(expected, actual)| (actual - expected).powi(2))
        .sum::<f64>()
        / reference.len() as f64)
        .sqrt();
    error_rms / reference_rms.max(f64::MIN_POSITIVE)
}

fn goertzel_power(samples: &[f64], frequency_hz: f64) -> f64 {
    let omega = TAU * frequency_hz / SAMPLE_RATE as f64;
    let coefficient = 2.0 * omega.cos();
    let mut previous = 0.0;
    let mut previous_previous = 0.0;
    for sample in samples {
        let current = sample + coefficient * previous - previous_previous;
        previous_previous = previous;
        previous = current;
    }
    (previous * previous + previous_previous * previous_previous
        - coefficient * previous * previous_previous)
        .max(0.0)
}

fn write_runtime_metrics(
    directory: &Path,
    profile_name: &str,
    events: &[PsgAmplitudeEvent],
    kernel: &EventBasedKernel,
    sample_count: usize,
    capture_elapsed: std::time::Duration,
    event_render_elapsed: std::time::Duration,
    core_render_elapsed: std::time::Duration,
) {
    let event_storage_bytes = events.capacity() * std::mem::size_of::<PsgAmplitudeEvent>();
    let kernel_storage_bytes = (kernel.step_response.capacity()
        + kernel.impulse_response.capacity()
        + kernel.discrete_coefficients.capacity())
        * std::mem::size_of::<f64>();
    let renderer_scratch_bytes =
        (sample_count * 3 + 1) * std::mem::size_of::<f64>();
    let peak_working_set_estimate_bytes =
        event_storage_bytes + kernel_storage_bytes + renderer_scratch_bytes;
    let audio_duration_seconds = sample_count as f64 / SAMPLE_RATE as f64;
    let event_pipeline_seconds =
        capture_elapsed.as_secs_f64() + event_render_elapsed.as_secs_f64();
    let report = format!(
        concat!(
            "{{\n",
            "  \"profile\": \"{}\",\n",
            "  \"sample_count\": {},\n",
            "  \"audio_duration_seconds\": {:.9},\n",
            "  \"event_count\": {},\n",
            "  \"event_capture_ms\": {:.6},\n",
            "  \"event_render_ms\": {:.6},\n",
            "  \"current_core_render_ms\": {:.6},\n",
            "  \"event_pipeline_realtime_factor\": {:.9},\n",
            "  \"event_storage_bytes\": {},\n",
            "  \"kernel_storage_bytes\": {},\n",
            "  \"renderer_scratch_peak_bytes\": {},\n",
            "  \"peak_working_set_estimate_bytes\": {},\n",
            "  \"group_delay_samples\": {:.6},\n",
            "  \"causal_lookahead_frames\": 32,\n",
            "  \"causal_lookahead_ms\": {:.9},\n",
            "  \"streaming_ready\": false,\n",
            "  \"streaming_limitation\": \"offline renderer stores the complete event list and full-duration scratch/output buffers\"\n",
            "}}\n"
        ),
        profile_name,
        sample_count,
        audio_duration_seconds,
        events.len(),
        capture_elapsed.as_secs_f64() * 1000.0,
        event_render_elapsed.as_secs_f64() * 1000.0,
        core_render_elapsed.as_secs_f64() * 1000.0,
        event_pipeline_seconds / audio_duration_seconds,
        event_storage_bytes,
        kernel_storage_bytes,
        renderer_scratch_bytes,
        peak_working_set_estimate_bytes,
        EVENT_HALF + EVENT_FRACTIONAL_DELAY,
        32.0 * 1000.0 / SAMPLE_RATE as f64
    );
    std::fs::write(
        directory.join(format!("psg-h2-runtime-{profile_name}.json")),
        report,
    )
    .unwrap();
}

fn write_kernel_metrics(directory: &Path, kernel: &EventBasedKernel) {
    let (minimum, maximum) = kernel.step_extrema();
    let report = format!(
        concat!(
            "{{\n",
            "  \"model\": \"test-only event-based band-limited step reconstruction\",\n",
            "  \"sample_rate_hz\": {},\n",
            "  \"kernel_taps\": {},\n",
            "  \"kernel_half_width_samples\": {},\n",
            "  \"oversample_phases\": {},\n",
            "  \"cutoff_cycles_per_output_sample\": {},\n",
            "  \"kaiser_beta\": {},\n",
            "  \"fractional_delay_samples\": {},\n",
            "  \"group_delay_samples\": {},\n",
            "  \"passband_gain_at_0_42\": {},\n",
            "  \"stopband_gain_at_nyquist\": {},\n",
            "  \"step_undershoot\": {},\n",
            "  \"step_overshoot\": {},\n",
            "  \"settling_to_0_1_percent_samples\": {},\n",
            "  \"sms_white_lfsr_period\": 57337,\n",
            "  \"sg1000_white_lfsr_period\": 32767,\n",
            "  \"production_enabled\": false\n",
            "}}\n"
        ),
        SAMPLE_RATE,
        EVENT_TAPS,
        EVENT_HALF,
        EVENT_OVERSAMPLE,
        EVENT_CUTOFF,
        EVENT_KAISER_BETA,
        EVENT_FRACTIONAL_DELAY,
        EVENT_HALF + EVENT_FRACTIONAL_DELAY,
        kernel.continuous_frequency_response(0.42),
        kernel.continuous_frequency_response(0.5),
        minimum,
        maximum - 1.0,
        kernel.settling_samples(0.001)
    );
    std::fs::write(directory.join("psg-h2-model-metrics.json"), report).unwrap();
}

#[test]
fn event_kernel_step_and_frequency_responses_are_bounded() {
    let kernel = EventBasedKernel::new();
    let (minimum, maximum) = kernel.step_extrema();
    assert!(minimum > -0.10, "step undershoot {minimum}");
    assert!(maximum < 1.10, "step overshoot {maximum}");
    assert!(kernel.settling_samples(0.001) < 54.0);
    assert!(kernel.continuous_frequency_response(0.42) > 0.999);
    assert!(kernel.continuous_frequency_response(0.5) < 2.0e-5);
    for index in 0..=256 {
        let position = -EVENT_HALF + 2.0 * EVENT_HALF * index as f64 / 256.0;
        let symmetry = kernel.response_to_step(position) + kernel.response_to_step(-position);
        assert!((symmetry - 1.0).abs() < 2.0e-3);
    }
    if let Some(directory) = artifact_directory() {
        std::fs::create_dir_all(&directory).unwrap();
        write_kernel_metrics(&directory, &kernel);
    }
}

#[test]
fn event_kernel_sweeps_all_tone_periods_and_preserves_period_zero_one() {
    let kernel = EventBasedKernel::new();
    for period in 2_u16..=1023 {
        let frequency = CLOCK_HZ as f64 / (32.0 * f64::from(period));
        let wrapped = frequency.rem_euclid(SAMPLE_RATE as f64);
        let alias = wrapped.min(SAMPLE_RATE as f64 - wrapped);
        let response = if frequency >= SAMPLE_RATE as f64 / 2.0 || period == 5 {
            kernel.continuous_frequency_response(frequency / SAMPLE_RATE as f64)
        } else {
            kernel.discrete_frequency_response(alias / SAMPLE_RATE as f64)
        };
        assert!(response.is_finite() && (0.0..=1.10).contains(&response));
        if period <= 4 {
            assert!(response < 2.0e-4, "period={period}, stopband gain={response}");
        } else if period == 5 {
            assert!((0.25..0.40).contains(&response), "transition gain={response}");
        } else {
            assert!(response > 0.999, "period={period}, passband gain={response}");
        }
    }
    for period in [0_u16, 1] {
        let mut psg =
            Sn76489Core::new_with_profile(CLOCK_HZ, SAMPLE_RATE, PsgProfile::Sms).unwrap();
        psg.write_inner(0x80 | (period as u8 & 0x0f));
        psg.write_inner((period >> 4) as u8);
        psg.write_inner(0x90);
        for _ in 0..1024 {
            psg.tick_internal_clock();
            assert_eq!(psg.tone_outputs[0], 1, "period={period}");
        }
        assert_eq!(psg.tone_periods[0], period);
    }
}

#[test]
fn event_based_full_register_traces_are_deterministic_for_both_profiles() {
    let kernel = EventBasedKernel::new();
    let directory = artifact_directory();
    if let Some(path) = &directory {
        std::fs::create_dir_all(path).unwrap();
    }
    for (profile, name) in [(PsgProfile::Sms, "sms"), (PsgProfile::Sg1000, "sg1000")] {
        let capture_start = Instant::now();
        let events = capture_events(profile, &PSG_TRACE, FULL_TRACE_CLOCKS, None);
        let capture_elapsed = capture_start.elapsed();

        let event_render_start = Instant::now();
        let event_pcm = kernel.render(&events, 0.0, FULL_TRACE_CLOCKS);
        let event_render_elapsed = event_render_start.elapsed();

        let core_render_start = Instant::now();
        let core_pcm = render_core_trace(profile, &PSG_TRACE, FULL_TRACE_CLOCKS);
        let core_render_elapsed = core_render_start.elapsed();

        let expected = (u128::from(FULL_TRACE_CLOCKS) * u128::from(SAMPLE_RATE)
            / u128::from(CLOCK_HZ)) as usize;
        assert_eq!(event_pcm.len(), expected, "{profile:?}");
        assert_eq!(core_pcm.len(), expected, "{profile:?}");
        assert!(event_pcm.iter().all(|sample| sample.is_finite()));
        assert!(normalized_error(&core_pcm, &event_pcm) < 0.08);
        if let Some(path) = &directory {
            export_wav(path, &format!("event-{name}.wav"), &event_pcm);
            write_runtime_metrics(
                path,
                name,
                &events,
                &kernel,
                event_pcm.len(),
                capture_elapsed,
                event_render_elapsed,
                core_render_elapsed,
            );
        }
    }
}

#[test]
fn event_capture_and_reconstruction_are_invariant_to_clock_chunking() {
    const TRACE: [(u64, u8); 12] = [
        (0, 0x85),
        (0, 0x12),
        (0, 0x90),
        (0, 0xe4),
        (0, 0xf0),
        (250_003, 0x94),
        (500_009, 0xe7),
        (750_011, 0xf3),
        (1_000_013, 0x89),
        (1_000_013, 0x11),
        (1_500_017, 0xe4),
        (1_750_019, 0xf0),
    ];
    const TOTAL_CLOCKS: u64 = 2_000_000;
    let kernel = EventBasedKernel::new();
    for profile in [PsgProfile::Sms, PsgProfile::Sg1000] {
        let contiguous = capture_events(profile, &TRACE, TOTAL_CLOCKS, None);
        let reference_pcm = kernel.render(&contiguous, 0.0, TOTAL_CLOCKS);
        for chunk_size in [1_u64, 7, 997, 16_384] {
            let segmented = capture_events(profile, &TRACE, TOTAL_CLOCKS, Some(chunk_size));
            assert_eq!(segmented, contiguous, "{profile:?}, chunk={chunk_size}");
            assert_eq!(
                kernel.render(&segmented, 0.0, TOTAL_CLOCKS),
                reference_pcm,
                "{profile:?}, chunk={chunk_size}"
            );
        }
    }
}

#[test]
fn event_reconstruction_handles_sample_boundaries_simultaneous_deltas_and_end_edges() {
    let kernel = EventBasedKernel::new();
    const SAMPLE_BOUNDARY_CLOCK: u64 = 715_909;
    const TOTAL_CLOCKS: u64 = SAMPLE_BOUNDARY_CLOCK * 2;
    let ordered = [
        PsgAmplitudeEvent {
            clock: 0,
            delta: 0.25,
        },
        PsgAmplitudeEvent {
            clock: SAMPLE_BOUNDARY_CLOCK - 1,
            delta: -0.25,
        },
        PsgAmplitudeEvent {
            clock: SAMPLE_BOUNDARY_CLOCK,
            delta: 0.125,
        },
        PsgAmplitudeEvent {
            clock: SAMPLE_BOUNDARY_CLOCK,
            delta: -0.125,
        },
        PsgAmplitudeEvent {
            clock: SAMPLE_BOUNDARY_CLOCK + 1,
            delta: 0.25,
        },
        PsgAmplitudeEvent {
            clock: TOTAL_CLOCKS,
            delta: -0.125,
        },
    ];
    let reordered = [
        ordered[0], ordered[1], ordered[3], ordered[2], ordered[4], ordered[5],
    ];

    let first = kernel.render(&ordered, 0.0, TOTAL_CLOCKS);
    let second = kernel.render(&reordered, 0.0, TOTAL_CLOCKS);
    assert_eq!(first, second);
    assert_eq!(first.len(), 19_200);
    assert!(first.iter().all(|sample| sample.is_finite()));
    assert!(first.iter().map(|sample| sample.abs()).fold(0.0, f64::max) < 1.0);
}


#[test]
fn event_based_full_tone_period_sweep_exports_2_through_1023() {
    const SWEEP_CLOCKS: u64 = 357_955;
    const EXPECTED_SAMPLES: usize = 4_800;
    let kernel = EventBasedKernel::new();
    let directory = artifact_directory();
    let mut report = String::from("[\n");

    for period in 2_u16..=1023 {
        let trace = tone_trace(period);
        let events = capture_events(PsgProfile::Sms, &trace, SWEEP_CLOCKS, None);
        let samples = kernel.render(&events, 0.0, SWEEP_CLOCKS);
        assert_eq!(samples.len(), EXPECTED_SAMPLES, "period={period}");
        assert!(samples.iter().all(|sample| sample.is_finite()));

        let expected_frequency = CLOCK_HZ as f64 / (32.0 * f64::from(period));
        let wrapped = expected_frequency.rem_euclid(SAMPLE_RATE as f64);
        let alias_frequency = wrapped.min(SAMPLE_RATE as f64 - wrapped);
        let kernel_gain = if expected_frequency >= SAMPLE_RATE as f64 / 2.0 || period == 5 {
            kernel.continuous_frequency_response(expected_frequency / SAMPLE_RATE as f64)
        } else {
            kernel.discrete_frequency_response(alias_frequency / SAMPLE_RATE as f64)
        };
        let rms = (samples.iter().map(|sample| sample * sample).sum::<f64>()
            / samples.len() as f64)
            .sqrt();
        let peak = samples.iter().map(|sample| sample.abs()).fold(0.0, f64::max);
        let fundamental_power = goertzel_power(&samples, alias_frequency);

        assert!(peak < 1.0, "period={period}, peak={peak}");
        assert!(rms.is_finite() && fundamental_power.is_finite());
        if expected_frequency >= SAMPLE_RATE as f64 / 2.0 {
            assert!(kernel_gain < 2.0e-4, "period={period}, gain={kernel_gain}");
        } else if period == 5 {
            assert!((0.25..0.40).contains(&kernel_gain));
        } else {
            assert!(kernel_gain > 0.999, "period={period}, gain={kernel_gain}");
        }

        if period > 2 {
            report.push_str(",\n");
        }
        report.push_str(&format!(
            concat!(
                "  {{\"period\": {}, \"expected_frequency_hz\": {:.9}, ",
                "\"alias_frequency_hz\": {:.9}, \"kernel_gain\": {:.12}, ",
                "\"rms\": {:.12}, \"peak\": {:.12}, ",
                "\"fundamental_power_at_alias\": {:.12e}, \"event_count\": {}}}"
            ),
            period,
            expected_frequency,
            alias_frequency,
            kernel_gain,
            rms,
            peak,
            fundamental_power,
            events.len()
        ));
    }
    report.push_str("\n]\n");

    if let Some(path) = &directory {
        std::fs::write(path.join("psg-h2-tone-sweep.json"), report).unwrap();
    }
}

#[test]
fn event_based_tone_sweep_exports_period_zero_one_and_frequency_checkpoints() {
    let kernel = EventBasedKernel::new();
    let directory = artifact_directory();
    if let Some(path) = &directory {
        std::fs::create_dir_all(path).unwrap();
    }

    for period in [0_u16, 1] {
        let trace = tone_trace(period);
        let events = capture_events(PsgProfile::Sms, &trace, TONE_TRACE_CLOCKS, None);
        let samples = kernel.render(&events, 0.0, TONE_TRACE_CLOCKS);
        assert_eq!(events.len(), 1, "period={period}");
        assert!(samples.len() > 128);
        assert!(
            samples[samples.len() - 64..]
                .iter()
                .all(|sample| (*sample - 0.25).abs() < 1.0e-9)
        );
        if let Some(path) = &directory {
            export_wav(path, &format!("event-tone-p{period}.wav"), &samples);
        }
    }

    for period in TONE_SWEEP_PERIODS {
        let trace = tone_trace(period);
        let events = capture_events(PsgProfile::Sms, &trace, TONE_TRACE_CLOCKS, None);
        let samples = kernel.render(&events, 0.0, TONE_TRACE_CLOCKS);
        assert!(events.len() > 1, "period={period}");
        assert!(samples.iter().all(|sample| sample.is_finite()));
        assert!(
            samples
                .iter()
                .map(|sample| sample.abs())
                .fold(0.0, f64::max)
                < 1.0
        );
        if let Some(path) = &directory {
            export_wav(path, &format!("event-tone-p{period}.wav"), &samples);
        }
    }
}

#[test]
fn event_based_noise_profiles_export_periodic_and_white_noise_comparisons() {
    let kernel = EventBasedKernel::new();
    let directory = artifact_directory();
    if let Some(path) = &directory {
        std::fs::create_dir_all(path).unwrap();
    }

    for (profile, profile_name) in [(PsgProfile::Sms, "sms"), (PsgProfile::Sg1000, "sg1000")] {
        for (control, mode_name) in [(0_u8, "periodic"), (4_u8, "white")] {
            let trace = noise_trace(control);
            let events = capture_events(profile, &trace, NOISE_TRACE_CLOCKS, None);
            let event_pcm = kernel.render(&events, 0.0, NOISE_TRACE_CLOCKS);
            let core_pcm = render_core_trace(profile, &trace, NOISE_TRACE_CLOCKS);
            assert_eq!(event_pcm.len(), core_pcm.len());
            assert!(events.len() > 100);
            assert!(event_pcm.iter().all(|sample| sample.is_finite()));
            assert!(core_pcm.iter().all(|sample| sample.is_finite()));
            if let Some(path) = &directory {
                export_wav(
                    path,
                    &format!("event-noise-{profile_name}-{mode_name}.wav"),
                    &event_pcm,
                );
                export_wav(
                    path,
                    &format!("current-noise-{profile_name}-{mode_name}.wav"),
                    &core_pcm,
                );
            }
        }
    }
}

#[test]
fn sms_and_sg1000_white_noise_lfsr_periods_match_documented_profiles() {
    for (profile, period, seed, expected_ones) in [
        (PsgProfile::Sms, 57_337_usize, 0x8000_u16, 28_668_usize),
        (PsgProfile::Sg1000, 32_767_usize, 0x4000_u16, 16_384_usize),
    ] {
        let mut psg = Sn76489Core::new_with_profile(CLOCK_HZ, SAMPLE_RATE, profile).unwrap();
        psg.write_inner(0xe4);
        let mut seen = HashSet::with_capacity(period);
        let mut ones = 0_usize;
        for _ in 0..period {
            assert!(seen.insert(psg.lfsr), "{profile:?} repeated an LFSR state");
            ones += usize::from(psg.lfsr & 1);
            psg.shift_noise_register();
        }
        assert_eq!(seen.len(), period, "{profile:?}");
        assert_eq!(ones, expected_ones, "{profile:?}");
        assert_eq!(psg.lfsr, seed, "{profile:?}");
    }
}

#[test]
fn periodic_noise_recycles_the_seed_bit_for_each_hardware_profile() {
    for (profile, period, seed) in [
        (PsgProfile::Sms, 16_usize, 0x8000_u16),
        (PsgProfile::Sg1000, 15_usize, 0x4000_u16),
    ] {
        let mut psg = Sn76489Core::new_with_profile(CLOCK_HZ, SAMPLE_RATE, profile).unwrap();
        psg.write_inner(0xe0);
        let mut output_ones = 0_usize;
        for _ in 0..period {
            output_ones += usize::from(psg.lfsr & 1);
            psg.shift_noise_register();
        }
        assert_eq!(output_ones, 1, "{profile:?}");
        assert_eq!(psg.lfsr, seed, "{profile:?}");
    }
}
