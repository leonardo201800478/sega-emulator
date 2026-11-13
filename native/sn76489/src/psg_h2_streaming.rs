//! Incremental event-domain H2 reconstruction for the SN76489 digital mix.
//!
//! This renderer consumes transitions captured from the existing PSG core. It
//! does not simulate PSG hardware and never runs in the CPAL device callback.
//! The 65-tap Kaiser-sinc study kernel is retained as an integrated step
//! response so event contributions can be accumulated into a bounded ring.

use super::{bessel_i0, PsgAmplitudeEvent};
use std::f64::consts::PI;

const KERNEL_HALF: f64 = 32.0;
const KERNEL_OVERSAMPLE: usize = 1024;
const KERNEL_CUTOFF: f64 = 0.46;
const KAISER_BETA: f64 = 7.857_26;
const FRACTIONAL_DELAY: f64 = -0.75;
const PENDING_CAPACITY: usize = 128;
const MAX_SIMULTANEOUS_EVENTS: usize = 64;

/// Errors reported by the experimental H2 streaming renderer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum H2StreamingError {
    ZeroClock,
    ZeroSampleRate,
    UnsupportedSampleRate,
    InvalidInitialLevel,
    ClockWentBackwards,
    EventOutOfOrder,
    EventAfterRenderHorizon,
    LateEvent,
    PendingHorizonExceeded,
    TooManySimultaneousEvents,
    SampleIndexOverflow,
}

impl std::fmt::Display for H2StreamingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ZeroClock => write!(f, "H2 PSG clock must be positive"),
            Self::ZeroSampleRate => write!(f, "H2 sample rate must be positive"),
            Self::UnsupportedSampleRate => write!(f, "H2 sample rate exceeds the PSG clock domain"),
            Self::InvalidInitialLevel => write!(f, "H2 initial amplitude must be finite"),
            Self::ClockWentBackwards => write!(f, "H2 render clock moved backwards"),
            Self::EventOutOfOrder => write!(f, "H2 amplitude events are not ordered by PSG clock"),
            Self::EventAfterRenderHorizon => write!(f, "H2 event lies beyond the supplied render horizon"),
            Self::LateEvent => write!(f, "H2 event arrived after an affected PCM sample was emitted"),
            Self::PendingHorizonExceeded => write!(f, "H2 event exceeds the bounded pending-sample horizon"),
            Self::TooManySimultaneousEvents => write!(f, "H2 same-clock event group exceeds the fixed limit"),
            Self::SampleIndexOverflow => write!(f, "H2 absolute sample index exceeds u64"),
        }
    }
}

impl std::error::Error for H2StreamingError {}

/// Bounded diagnostics for one H2 renderer epoch.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct H2StreamingMetrics {
    pub events_processed: u64,
    pub output_samples: u64,
    pub max_pending_samples: usize,
}

/// Incremental H2 renderer with a fixed pending-sample horizon.
///
/// The renderer retains the reference study's 1024x integrated Kaiser-sinc
/// step response, 65-tap source kernel, 0.46 cycles/sample cutoff and -0.75
/// sample fractional alignment. Its two 128-entry accumulation rings bound
/// transient storage independently of session duration.
#[derive(Debug, Clone)]
pub struct H2StreamingRenderer {
    clock_hz: u64,
    sample_rate: u64,
    step_response: Vec<f64>,
    transition_ring: [f64; PENDING_CAPACITY],
    settled_ring: [f64; PENDING_CAPACITY],
    next_sample_index: u64,
    settled_level: f64,
    epoch_start_clock: u64,
    last_event_clock: u64,
    metrics: H2StreamingMetrics,
}

impl H2StreamingRenderer {
    /// Build immutable H2 kernel tables for one PSG clock and PCM sample rate.
    pub fn new(clock_hz: u64, sample_rate: u64) -> Result<Self, H2StreamingError> {
        if clock_hz == 0 {
            return Err(H2StreamingError::ZeroClock);
        }
        if sample_rate == 0 {
            return Err(H2StreamingError::ZeroSampleRate);
        }
        if u128::from(sample_rate) * 16 > u128::from(clock_hz) {
            return Err(H2StreamingError::UnsupportedSampleRate);
        }

        let denominator = bessel_i0(KAISER_BETA);
        let sample_count = (2.0 * KERNEL_HALF * KERNEL_OVERSAMPLE as f64) as usize + 1;
        let step_resolution = 1.0 / KERNEL_OVERSAMPLE as f64;
        let mut impulse_response = Vec::with_capacity(sample_count);
        let mut step_response = vec![0.0; sample_count];

        for index in 0..sample_count {
            let position = -KERNEL_HALF + index as f64 * step_resolution;
            let normalized = position / KERNEL_HALF;
            let window_argument =
                (1.0 - normalized * normalized).max(0.0).sqrt() * KAISER_BETA;
            let window = bessel_i0(window_argument) / denominator;
            let scaled = 2.0 * KERNEL_CUTOFF * position;
            let sinc = if scaled.abs() < 1.0e-12 {
                1.0
            } else {
                (PI * scaled).sin() / (PI * scaled)
            };
            impulse_response.push(2.0 * KERNEL_CUTOFF * sinc * window);
        }

        for index in 1..sample_count {
            step_response[index] = step_response[index - 1]
                + 0.5 * (impulse_response[index - 1] + impulse_response[index]) * step_resolution;
        }

        let area = *step_response.last().expect("H2 kernel has samples");
        if !area.is_finite() || area <= 0.0 {
            return Err(H2StreamingError::InvalidInitialLevel);
        }
        for value in &mut step_response {
            *value /= area;
        }

        let mut renderer = Self {
            clock_hz,
            sample_rate,
            step_response,
            transition_ring: [0.0; PENDING_CAPACITY],
            settled_ring: [0.0; PENDING_CAPACITY],
            next_sample_index: 0,
            settled_level: 0.0,
            epoch_start_clock: 0,
            last_event_clock: 0,
            metrics: H2StreamingMetrics::default(),
        };
        renderer.begin_epoch(0, 0.0)?;
        Ok(renderer)
    }

    /// Start a fresh event timeline at a machine-owned PSG clock.
    ///
    /// One preceding sample is retained as a warm-up point for the reference
    /// kernel's pre-ringing. A renderer switch should fade this boundary rather
    /// than treating that warm-up sample as a hardware-timed sample.
    pub fn begin_epoch(
        &mut self,
        start_clock: u64,
        initial_level: f64,
    ) -> Result<(), H2StreamingError> {
        if !initial_level.is_finite() {
            return Err(H2StreamingError::InvalidInitialLevel);
        }
        self.transition_ring.fill(0.0);
        self.settled_ring.fill(0.0);
        let first_sample = self.sample_index_at_clock(start_clock)?;
        self.next_sample_index = first_sample.saturating_sub(1);
        self.settled_level = initial_level;
        self.epoch_start_clock = start_clock;
        self.last_event_clock = start_clock;
        self.metrics = H2StreamingMetrics::default();
        Ok(())
    }

    /// Consume ordered digital transitions and emit samples whose event
    /// horizon is closed. The caller reuses `output` to avoid per-block growth.
    pub fn render_until(
        &mut self,
        events: &[PsgAmplitudeEvent],
        end_clock: u64,
        output: &mut Vec<f64>,
    ) -> Result<usize, H2StreamingError> {
        self.validate_horizon(end_clock)?;
        output.clear();

        let mut index = 0;
        while index < events.len() {
            let clock = events[index].clock;
            if clock < self.last_event_clock {
                return Err(H2StreamingError::EventOutOfOrder);
            }
            if clock < self.epoch_start_clock {
                return Err(H2StreamingError::EventOutOfOrder);
            }
            if clock > end_clock {
                return Err(H2StreamingError::EventAfterRenderHorizon);
            }

            let mut deltas = [0.0_f64; MAX_SIMULTANEOUS_EVENTS];
            let mut delta_count = 0;
            while index < events.len() && events[index].clock == clock {
                if delta_count == MAX_SIMULTANEOUS_EVENTS {
                    return Err(H2StreamingError::TooManySimultaneousEvents);
                }
                if !events[index].delta.is_finite() {
                    return Err(H2StreamingError::InvalidInitialLevel);
                }
                deltas[delta_count] = events[index].delta;
                delta_count += 1;
                index += 1;
            }

            // Same-clock events are summed in canonical order, independent of
            // the order in which bus writes were observed.
            deltas[..delta_count].sort_by(f64::total_cmp);
            let delta = deltas[..delta_count].iter().sum::<f64>();
            self.last_event_clock = clock;
            if delta != 0.0 {
                self.accumulate_event(clock, delta)?;
                self.metrics.events_processed = self.metrics.events_processed.saturating_add(
                    u64::try_from(delta_count).unwrap_or(u64::MAX),
                );
            }
        }

        let safe_exclusive = self.sample_index_at_clock(end_clock)?.saturating_sub(1);
        self.emit_until(safe_exclusive, output)?;
        Ok(output.len())
    }

    /// Flush the remaining samples through the supplied final clock. Call only
    /// when no later events in this epoch can arrive.
    pub fn finish(
        &mut self,
        end_clock: u64,
        output: &mut Vec<f64>,
    ) -> Result<usize, H2StreamingError> {
        self.validate_horizon(end_clock)?;
        output.clear();
        let final_exclusive = self.sample_index_at_clock(end_clock)?;
        self.emit_until(final_exclusive, output)?;
        Ok(output.len())
    }

    /// Return renderer counters without changing the audio timeline.
    pub fn metrics(&self) -> H2StreamingMetrics {
        self.metrics
    }

    /// Number of samples still waiting for the safe emission horizon.
    pub fn pending_sample_count(&self, end_clock: u64) -> Result<usize, H2StreamingError> {
        self.validate_horizon(end_clock)?;
        let safe_exclusive = self.sample_index_at_clock(end_clock)?.saturating_sub(1);
        usize::try_from(safe_exclusive.saturating_sub(self.next_sample_index))
            .map_err(|_| H2StreamingError::SampleIndexOverflow)
    }

    fn validate_horizon(&self, end_clock: u64) -> Result<(), H2StreamingError> {
        if end_clock < self.epoch_start_clock || end_clock < self.last_event_clock {
            return Err(H2StreamingError::ClockWentBackwards);
        }
        Ok(())
    }

    fn sample_index_at_clock(&self, clock: u64) -> Result<u64, H2StreamingError> {
        let numerator = u128::from(clock) * u128::from(self.sample_rate);
        u64::try_from(numerator / u128::from(self.clock_hz))
            .map_err(|_| H2StreamingError::SampleIndexOverflow)
    }

    fn sample_position_at_clock(&self, clock: u64) -> Result<f64, H2StreamingError> {
        let numerator = u128::from(clock) * u128::from(self.sample_rate);
        let divisor = u128::from(self.clock_hz);
        let whole = u64::try_from(numerator / divisor)
            .map_err(|_| H2StreamingError::SampleIndexOverflow)?;
        let remainder = numerator % divisor;
        Ok(whole as f64 + remainder as f64 / self.clock_hz as f64)
    }

    fn response_to_step(&self, position: f64) -> f64 {
        if position <= -KERNEL_HALF {
            return 0.0;
        }
        if position >= KERNEL_HALF {
            return 1.0;
        }
        let coordinate = (position + KERNEL_HALF) * KERNEL_OVERSAMPLE as f64;
        let index = coordinate.floor() as usize;
        let fraction = coordinate - index as f64;
        let first = self.step_response[index];
        let second = self.step_response[index + 1];
        first + (second - first) * fraction
    }

    fn accumulate_event(
        &mut self,
        clock: u64,
        delta: f64,
    ) -> Result<(), H2StreamingError> {
        let event_position = self.sample_position_at_clock(clock)?;
        let first_value = (event_position + FRACTIONAL_DELAY - 1.0).ceil().max(0.0);
        let settled_value =
            (event_position + 2.0 * KERNEL_HALF + FRACTIONAL_DELAY - 1.0)
                .ceil()
                .max(0.0);
        if !first_value.is_finite()
            || !settled_value.is_finite()
            || settled_value > u64::MAX as f64
        {
            return Err(H2StreamingError::SampleIndexOverflow);
        }
        let first = first_value as u64;
        let settled = settled_value as u64;
        if first < self.next_sample_index {
            return Err(H2StreamingError::LateEvent);
        }
        if settled.saturating_sub(self.next_sample_index) >= PENDING_CAPACITY as u64 {
            return Err(H2StreamingError::PendingHorizonExceeded);
        }

        for sample_index in first..settled {
            let slot = sample_index as usize % PENDING_CAPACITY;
            let sample_time = sample_index as f64 + 1.0;
            let position =
                sample_time - event_position - KERNEL_HALF - FRACTIONAL_DELAY;
            self.transition_ring[slot] += delta * self.response_to_step(position);
        }
        let settled_slot = settled as usize % PENDING_CAPACITY;
        self.settled_ring[settled_slot] += delta;
        Ok(())
    }

    fn emit_until(
        &mut self,
        end_exclusive: u64,
        output: &mut Vec<f64>,
    ) -> Result<(), H2StreamingError> {
        while self.next_sample_index < end_exclusive {
            let slot = self.next_sample_index as usize % PENDING_CAPACITY;
            self.settled_level += self.settled_ring[slot];
            let sample = self.settled_level + self.transition_ring[slot];
            if !sample.is_finite() {
                return Err(H2StreamingError::InvalidInitialLevel);
            }
            output.push(sample);
            self.settled_ring[slot] = 0.0;
            self.transition_ring[slot] = 0.0;
            self.next_sample_index = self
                .next_sample_index
                .checked_add(1)
                .ok_or(H2StreamingError::SampleIndexOverflow)?;
            self.metrics.output_samples = self.metrics.output_samples.saturating_add(1);
        }
        let safe_exclusive = end_exclusive;
        let pending = safe_exclusive.saturating_sub(self.next_sample_index);
        self.metrics.max_pending_samples = self.metrics.max_pending_samples.max(
            usize::try_from(pending).unwrap_or(usize::MAX),
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{H2StreamingError, H2StreamingRenderer};
    use crate::PsgAmplitudeEvent;

    const CLOCK_HZ: u64 = 3_579_545;
    const SAMPLE_RATE: u64 = 48_000;

    fn fixture_events() -> Vec<PsgAmplitudeEvent> {
        vec![
            PsgAmplitudeEvent { clock: 0, delta: 0.25 },
            PsgAmplitudeEvent { clock: 12_345, delta: -0.5 },
            PsgAmplitudeEvent { clock: 12_345, delta: 0.125 },
            PsgAmplitudeEvent { clock: 98_765, delta: 0.25 },
            PsgAmplitudeEvent { clock: 200_000, delta: -0.125 },
            PsgAmplitudeEvent { clock: 400_001, delta: 0.25 },
        ]
    }

    #[test]
    fn streaming_output_is_invariant_to_block_boundaries() {
        let events = fixture_events();
        let total_clock = 500_000;
        let mut continuous = H2StreamingRenderer::new(CLOCK_HZ, SAMPLE_RATE).unwrap();
        let mut chunked = H2StreamingRenderer::new(CLOCK_HZ, SAMPLE_RATE).unwrap();
        continuous.begin_epoch(0, 0.0).unwrap();
        chunked.begin_epoch(0, 0.0).unwrap();

        let mut continuous_block = Vec::with_capacity(1024);
        let mut chunked_block = Vec::with_capacity(1024);
        let mut continuous_output = Vec::new();
        let mut chunked_output = Vec::new();

        continuous
            .render_until(&events, total_clock, &mut continuous_block)
            .unwrap();
        continuous_output.extend_from_slice(&continuous_block);
        continuous.finish(total_clock, &mut continuous_block).unwrap();
        continuous_output.extend_from_slice(&continuous_block);

        let mut previous_clock = 0;
        for end_clock in [1_000, 12_345, 50_000, 100_000, 200_000, 300_000, total_clock] {
            let block_events: Vec<_> = events
                .iter()
                .copied()
                .filter(|event| event.clock >= previous_clock && event.clock <= end_clock)
                .collect();
            chunked
                .render_until(&block_events, end_clock, &mut chunked_block)
                .unwrap();
            chunked_output.extend_from_slice(&chunked_block);
            previous_clock = end_clock;
        }
        chunked.finish(total_clock, &mut chunked_block).unwrap();
        chunked_output.extend_from_slice(&chunked_block);

        assert_eq!(continuous_output, chunked_output);
        assert_eq!(
            continuous_output.len(),
            (u128::from(total_clock) * u128::from(SAMPLE_RATE) / u128::from(CLOCK_HZ)) as usize
        );
    }

    #[test]
    fn same_clock_events_are_summed_independent_of_input_order() {
        let mut first = H2StreamingRenderer::new(CLOCK_HZ, SAMPLE_RATE).unwrap();
        let mut second = H2StreamingRenderer::new(CLOCK_HZ, SAMPLE_RATE).unwrap();
        let a = [
            PsgAmplitudeEvent { clock: 0, delta: 0.25 },
            PsgAmplitudeEvent { clock: 0, delta: -0.5 },
            PsgAmplitudeEvent { clock: 10_000, delta: 0.125 },
        ];
        let b = [
            PsgAmplitudeEvent { clock: 0, delta: -0.5 },
            PsgAmplitudeEvent { clock: 0, delta: 0.25 },
            PsgAmplitudeEvent { clock: 10_000, delta: 0.125 },
        ];
        let mut out_a = Vec::new();
        let mut out_b = Vec::new();
        first.render_until(&a, 100_000, &mut out_a).unwrap();
        second.render_until(&b, 100_000, &mut out_b).unwrap();
        first.finish(100_000, &mut out_a).unwrap();
        second.finish(100_000, &mut out_b).unwrap();
        assert_eq!(out_a, out_b);
    }

    #[test]
    fn late_events_fail_instead_of_corrupting_already_emitted_samples() {
        let mut renderer = H2StreamingRenderer::new(CLOCK_HZ, SAMPLE_RATE).unwrap();
        let mut output = Vec::new();
        renderer.render_until(&[], 100_000, &mut output).unwrap();
        let late = [PsgAmplitudeEvent { clock: 0, delta: 0.25 }];
        assert_eq!(
            renderer.render_until(&late, 110_000, &mut output),
            Err(H2StreamingError::LateEvent)
        );
    }

    #[test]
    fn renderer_rejects_events_after_the_supplied_horizon() {
        let mut renderer = H2StreamingRenderer::new(CLOCK_HZ, SAMPLE_RATE).unwrap();
        let mut output = Vec::new();
        let future = [PsgAmplitudeEvent { clock: 10_000, delta: 0.25 }];
        assert_eq!(
            renderer.render_until(&future, 9_999, &mut output),
            Err(H2StreamingError::EventAfterRenderHorizon)
        );
    }
}
