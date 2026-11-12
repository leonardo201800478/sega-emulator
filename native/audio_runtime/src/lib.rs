//! Native audio DSP and PCM conversion shared by host audio backends.

mod dsp;
mod psg;
mod session;

pub use dsp::{AudioMixer, DspError, MixSource, OnePoleLowPass};
pub use psg::{PsgClockError, ScheduledPsg};
pub use session::{AudioOutputMetrics, NativeAudioSession, PcmSink};

use std::f64::consts::PI;

const DEFAULT_TEST_INPUT_RATE: u32 = 48_000;
const TAPS: usize = 63;
const HALF: isize = 31;
const PHASES: usize = 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AudioError {
    InvalidSampleRate,
    InvalidVolume,
}

impl std::fmt::Display for AudioError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidSampleRate => write!(f, "input and output sample rates must be positive"),
            Self::InvalidVolume => write!(f, "volume must be in 0..=100"),
        }
    }
}

impl std::error::Error for AudioError {}

#[derive(Debug, Clone)]
pub struct PcmResampler {
    input_rate: u32,
    output_rate: u32,
    step: f64,
    volume: u8,
    coefficients: Vec<[f64; TAPS]>,
    buffer: Vec<f64>,
    start: usize,
    available: usize,
    real: usize,
    next: f64,
    emitted: usize,
}

impl PcmResampler {
    pub fn new(input_rate: u32, output_rate: u32, volume: u8) -> Result<Self, AudioError> {
        if input_rate == 0 || output_rate == 0 {
            return Err(AudioError::InvalidSampleRate);
        }
        if volume > 100 {
            return Err(AudioError::InvalidVolume);
        }

        let cutoff = (output_rate as f64 / input_rate as f64).min(1.0) * 0.94;
        let mut coefficients = Vec::with_capacity(PHASES);
        for phase in 0..PHASES {
            let fraction = phase as f64 / PHASES as f64;
            let mut weights = [0.0; TAPS];
            let mut sum = 0.0;
            for (index, weight) in weights.iter_mut().enumerate() {
                let distance = index as f64 - HALF as f64 - fraction;
                let window = 0.42
                    + 0.5 * (PI * distance / (HALF as f64 + 1.0)).cos()
                    + 0.08 * (2.0 * PI * distance / (HALF as f64 + 1.0)).cos();
                let scaled_distance = cutoff * distance;
                let sinc = if scaled_distance.abs() < 1.0e-12 {
                    1.0
                } else {
                    (PI * scaled_distance).sin() / (PI * scaled_distance)
                };
                *weight = cutoff * sinc * window;
                sum += *weight;
            }
            for weight in &mut weights {
                *weight /= sum;
            }
            coefficients.push(weights);
        }

        sega_observability::tracing::info!(
            target: "sega::audio::dsp",
            input_rate,
            output_rate,
            volume,
            filter_taps = coefficients.len(),
            "PCM resampler initialized"
        );
        Ok(Self {
            input_rate,
            output_rate,
            step: input_rate as f64 / output_rate as f64,
            volume,
            coefficients,
            buffer: Vec::new(),
            start: 0,
            available: 0,
            real: 0,
            next: 0.0,
            emitted: 0,
        })
    }

    pub fn set_volume(&mut self, volume: u8) -> Result<(), AudioError> {
        if volume > 100 {
            return Err(AudioError::InvalidVolume);
        }
        let previous = self.volume;
        self.volume = volume;
        sega_observability::tracing::debug!(
            target: "sega::audio::dsp",
            previous,
            volume,
            "PCM output volume changed"
        );
        Ok(())
    }

    pub fn output_rate(&self) -> u32 {
        self.output_rate
    }

    pub fn volume(&self) -> u8 {
        self.volume
    }

    pub fn reset(&mut self) {
        self.buffer.clear();
        self.start = 0;
        self.available = 0;
        self.real = 0;
        self.next = 0.0;
        self.emitted = 0;
    }

    pub fn process(&mut self, input: &[f64]) -> Vec<u8> {
        if input.is_empty() {
            return Vec::new();
        }

        let output = if self.output_rate == self.input_rate {
            self.encode(input.iter().copied())
        } else {
            self.buffer.extend_from_slice(input);
            self.available += input.len();
            self.real += input.len();
            let output = self.emit(false);
            self.discard_old();
            output
        };
        sega_observability::tracing::trace!(
            target: "sega::audio::dsp",
            input_samples = input.len(),
            output_pcm_bytes = output.len(),
            output_samples = output.len() / 2,
            output_rate = self.output_rate,
            buffered_samples = self.available.saturating_sub(self.start),
            "PCM resampler processed sample batch"
        );
        output
    }

    pub fn flush(&mut self) -> Vec<u8> {
        if self.output_rate == INPUT_RATE || self.real == 0 {
            self.reset();
            return Vec::new();
        }

        self.buffer.extend(std::iter::repeat_n(0.0, TAPS / 2 + 1));
        self.available += TAPS / 2 + 1;
        let output = self.emit(true);
        sega_observability::tracing::trace!(
            target: "sega::audio::dsp",
            flushed_pcm_bytes = output.len(),
            flushed_samples = output.len() / 2,
            "PCM resampler flushed pending samples"
        );
        self.reset();
        output
    }

    fn emit(&mut self, flushing: bool) -> Vec<u8> {
        let target =
            (self.real as f64 * self.output_rate as f64 / self.input_rate as f64).round_ties_even() as usize;
        let mut output = Vec::new();

        loop {
            if flushing {
                if self.next >= self.real as f64 || self.emitted >= target {
                    break;
                }
            } else if self.next + HALF as f64 >= self.available as f64 || self.emitted >= target {
                break;
            }

            let base = self.next.floor() as isize;
            let phase = (((self.next - base as f64) * PHASES as f64) as usize).min(PHASES - 1);
            let mut value = 0.0;
            for (index, coefficient) in self.coefficients[phase].iter().enumerate() {
                let source_index = base + index as isize - HALF;
                if source_index >= self.start as isize && source_index < self.available as isize {
                    value += self.buffer[source_index as usize - self.start] * coefficient;
                }
            }
            output.extend_from_slice(&self.pcm16(value).to_le_bytes());
            self.emitted += 1;
            self.next += self.step;
        }

        output
    }

    fn encode(&self, values: impl Iterator<Item = f64>) -> Vec<u8> {
        let mut output = Vec::new();
        for value in values {
            output.extend_from_slice(&self.pcm16(value).to_le_bytes());
        }
        output
    }

    fn pcm16(&self, value: f64) -> i16 {
        (value * self.volume as f64 / 100.0 * 32767.0)
            .round_ties_even()
            .clamp(-32768.0, 32767.0) as i16
    }

    fn discard_old(&mut self) {
        let keep = (self.next.floor() as isize - HALF).max(0) as usize;
        let count = keep.saturating_sub(self.start).min(self.buffer.len());
        if count > 0 {
            self.buffer.drain(..count);
            self.start += count;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pcm16_clips_and_scales() {
        let mut resampler = PcmResampler::new(DEFAULT_TEST_INPUT_RATE, DEFAULT_TEST_INPUT_RATE, 100).unwrap();
        let bytes = resampler.process(&[-2.0, -1.0, 0.0, 1.0, 2.0]);
        let values: Vec<i16> = bytes
            .as_chunks::<2>()
            .0
            .iter()
            .map(|bytes| i16::from_le_bytes(*bytes))
            .collect();
        assert_eq!(values, vec![-32768, -32767, 0, 32767, 32767]);

        resampler.set_volume(50).unwrap();
        let bytes = resampler.process(&[0.5]);
        assert_eq!(i16::from_le_bytes([bytes[0], bytes[1]]), 8192);
    }

    #[test]
    fn resampling_is_repeatable() {
        let input: Vec<f64> = (0..2048).map(|index| (index as f64 * 0.03).sin()).collect();
        let mut whole = PcmResampler::new(48_000, 44_100, 100).unwrap();
        let mut chunked = PcmResampler::new(48_000, 44_100, 100).unwrap();
        let expected = [whole.process(&input), whole.flush()].concat();

        let mut actual = Vec::new();
        for block in input.chunks(113) {
            actual.extend(chunked.process(block));
        }
        actual.extend(chunked.flush());
        assert_eq!(actual, expected);
    }

    #[test]
    fn resampling_is_invariant_across_rates_and_input_chunk_sizes() {
        let input: Vec<f64> = (0..4096)
            .map(|index| 0.6 * (index as f64 * 0.017).sin())
            .collect();

        for rate in [22_050, 44_100, 48_000, 96_000] {
            let mut whole = PcmResampler::new(48_000, rate, 100).unwrap();
            let expected = [whole.process(&input), whole.flush()].concat();

            for chunk_size in [1, 17, 113, 1024] {
                let mut chunked = PcmResampler::new(48_000, rate, 100).unwrap();
                let mut actual = Vec::new();
                for block in input.chunks(chunk_size) {
                    actual.extend(chunked.process(block));
                }
                actual.extend(chunked.flush());
                assert_eq!(
                    actual, expected,
                    "PCM output changed with chunk size {chunk_size} at {rate} Hz"
                );
            }
        }
    }

    #[test]
    fn resampler_preserves_output_length_and_does_not_start_one_half_filter_early() {
        let input = vec![1.0; 2048];

        for rate in [22_050, 44_100, 48_000] {
            let mut resampler = PcmResampler::new(48_000, rate, 100).unwrap();
            let mut pcm = resampler.process(&input);
            pcm.extend(resampler.flush());

            let expected_samples =
                (input.len() as f64 * rate as f64 / 48_000.0).round_ties_even() as usize;
            assert_eq!(pcm.len(), expected_samples * 2);

            let first_sample = i16::from_le_bytes([pcm[0], pcm[1]]);
            if rate != 48_000 {
                assert!(
                    first_sample > 10_000,
                    "FIR output should be centered on the first input sample; rate={rate}, first={first_sample}"
                );
            }
        }
    }

    #[test]
    fn rejects_bad_configuration() {
        assert!(PcmResampler::new(0, 48_000, 100).is_err());
        assert!(PcmResampler::new(48_000, 0, 100).is_err());
        assert!(PcmResampler::new(48_000, 44_100, 101).is_err());
    }
    #[test]
    fn downsampling_fir_attenuates_tones_above_output_nyquist() {
        let count = 8192;
        let low_tone: Vec<f64> = (0..count)
            .map(|index| 0.5 * (std::f64::consts::TAU * 0.10 * index as f64).sin())
            .collect();
        let high_tone: Vec<f64> = (0..count)
            .map(|index| 0.5 * (std::f64::consts::TAU * 0.35 * index as f64).sin())
            .collect();

        let mut low_resampler = PcmResampler::new(48_000, 22_050, 100).unwrap();
        let mut high_resampler = PcmResampler::new(48_000, 22_050, 100).unwrap();
        let low_pcm = [low_resampler.process(&low_tone), low_resampler.flush()].concat();
        let high_pcm = [high_resampler.process(&high_tone), high_resampler.flush()].concat();
        let decode = |pcm: &[u8]| -> Vec<i16> {
            let (samples, remainder) = pcm.as_chunks::<2>();
            assert!(
                remainder.is_empty(),
                "PCM16 output must contain complete samples"
            );
            samples
                .iter()
                .map(|sample| i16::from_le_bytes(*sample))
                .collect()
        };
        let low = decode(&low_pcm);
        let high = decode(&high_pcm);
        let rms = |samples: &[i16]| -> f64 {
            let tail = &samples[100..];
            (tail
                .iter()
                .map(|sample| f64::from(*sample).powi(2))
                .sum::<f64>()
                / tail.len() as f64)
                .sqrt()
        };

        assert!(rms(&low) > 8_000.0);
        assert!(rms(&high) < rms(&low) * 0.03);
    }
}
