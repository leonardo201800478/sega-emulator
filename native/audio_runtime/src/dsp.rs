//! Deterministic, host-independent DSP primitives for emulated audio sources.
//! Hardware-specific analog models require independent evidence and are not
//! enabled by this generic mixer.

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DspError {
    InvalidMasterGain,
    InvalidSourceGain,
    InconsistentSourceLength,
    InvalidFilterConfiguration,
}

impl std::fmt::Display for DspError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidMasterGain => write!(f, "master gain must be finite and in 0..=4"),
            Self::InvalidSourceGain => write!(f, "source gain must be finite and in 0..=4"),
            Self::InconsistentSourceLength => {
                write!(f, "all mixed sources must have the same frame count")
            }
            Self::InvalidFilterConfiguration => write!(
                f,
                "audio filter requires 0 < cutoff < Nyquist and a positive sample rate"
            ),
        }
    }
}

impl std::error::Error for DspError {}

/// One mono hardware source entering the common mixer.
#[derive(Clone, Copy, Debug)]
pub struct MixSource<'a> {
    pub samples: &'a [f64],
    pub gain: f64,
}

/// Reusable multi-source mono mixer. It preserves headroom and does not clip;
/// final PCM quantization/saturation remains the resampler's responsibility.
#[derive(Clone, Copy, Debug)]
pub struct AudioMixer {
    master_gain: f64,
}

impl Default for AudioMixer {
    fn default() -> Self {
        Self { master_gain: 1.0 }
    }
}

impl AudioMixer {
    pub fn new(master_gain: f64) -> Result<Self, DspError> {
        if !master_gain.is_finite() || !(0.0..=4.0).contains(&master_gain) {
            return Err(DspError::InvalidMasterGain);
        }
        Ok(Self { master_gain })
    }

    pub fn master_gain(&self) -> f64 {
        self.master_gain
    }

    /// Mix equally sized mono source blocks into a caller-owned scratch buffer.
    /// Reusing output avoids a new allocation on each audio batch.
    pub fn mix_mono_into(
        &self,
        sources: &[MixSource<'_>],
        output: &mut Vec<f64>,
    ) -> Result<(), DspError> {
        let Some(first) = sources.first() else {
            output.clear();
            return Ok(());
        };
        let frames = first.samples.len();
        if sources.iter().any(|source| source.samples.len() != frames) {
            return Err(DspError::InconsistentSourceLength);
        }
        if sources
            .iter()
            .any(|source| !source.gain.is_finite() || !(0.0..=4.0).contains(&source.gain))
        {
            return Err(DspError::InvalidSourceGain);
        }

        output.clear();
        output.resize(frames, 0.0);
        for source in sources {
            let gain = source.gain * self.master_gain;
            for (mixed, sample) in output.iter_mut().zip(source.samples) {
                *mixed += sample * gain;
            }
        }
        Ok(())
    }
}

/// Optional one-pole low-pass. Not enabled by default; chip response needs calibration.
#[derive(Clone, Copy, Debug)]
pub struct OnePoleLowPass {
    alpha: f64,
    previous: f64,
}

impl OnePoleLowPass {
    pub fn new(sample_rate: u32, cutoff_hz: f64) -> Result<Self, DspError> {
        if sample_rate == 0
            || !cutoff_hz.is_finite()
            || cutoff_hz <= 0.0
            || cutoff_hz >= sample_rate as f64 / 2.0
        {
            return Err(DspError::InvalidFilterConfiguration);
        }
        let alpha = 1.0 - (-std::f64::consts::TAU * cutoff_hz / sample_rate as f64).exp();
        Ok(Self {
            alpha,
            previous: 0.0,
        })
    }

    pub fn reset(&mut self) {
        self.previous = 0.0;
    }

    pub fn process_sample(&mut self, input: f64) -> f64 {
        self.previous += self.alpha * (input - self.previous);
        self.previous
    }

    pub fn process_block(&mut self, input: &[f64], output: &mut Vec<f64>) {
        output.clear();
        output.reserve(input.len());
        for sample in input {
            output.push(self.process_sample(*sample));
        }
    }
}

/// User-selectable analog output approximation. Values are host-independent and
/// intentionally separate from the emulated PSG and machine timing.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AnalogOutputConfig {
    pub enabled: bool,
    pub high_pass_hz: Option<f64>,
    pub low_pass_hz: Option<f64>,
}


impl AnalogOutputConfig {
    /// Experimental broad-band approximation for an SMS1-style analog output.
    /// The service manual specifies 20 Hz–20 kHz at the output stage, but does
    /// not publish a complete transfer function; these values are not a
    /// component-level reconstruction or a calibrated board profile.
    pub const fn sms1_experimental() -> Self {
        Self {
            enabled: true,
            high_pass_hz: Some(20.0),
            low_pass_hz: Some(18_000.0),
        }
    }

    pub fn validate(self, sample_rate: u32) -> Result<Self, DspError> {
        if sample_rate == 0
            || self.high_pass_hz.is_some_and(|cutoff| {
                !cutoff.is_finite() || cutoff <= 0.0 || cutoff >= sample_rate as f64 / 2.0
            })
            || self.low_pass_hz.is_some_and(|cutoff| {
                !cutoff.is_finite() || cutoff <= 0.0 || cutoff >= sample_rate as f64 / 2.0
            })
        {
            return Err(DspError::InvalidFilterConfiguration);
        }
        Ok(self)
    }
}

/// One-pole high-pass used for AC coupling / DC blocking.
#[derive(Clone, Copy, Debug)]
pub struct OnePoleHighPass {
    alpha: f64,
    previous_input: f64,
    previous_output: f64,
}

impl OnePoleHighPass {
    pub fn new(sample_rate: u32, cutoff_hz: f64) -> Result<Self, DspError> {
        if sample_rate == 0
            || !cutoff_hz.is_finite()
            || cutoff_hz <= 0.0
            || cutoff_hz >= sample_rate as f64 / 2.0
        {
            return Err(DspError::InvalidFilterConfiguration);
        }
        let alpha = (-std::f64::consts::TAU * cutoff_hz / sample_rate as f64).exp();
        Ok(Self {
            alpha,
            previous_input: 0.0,
            previous_output: 0.0,
        })
    }

    pub fn reset(&mut self) {
        self.previous_input = 0.0;
        self.previous_output = 0.0;
    }

    pub fn process_sample(&mut self, input: f64) -> f64 {
        let output = self.alpha * (self.previous_output + input - self.previous_input);
        self.previous_input = input;
        self.previous_output = output;
        output
    }
}

/// Stateful analog output stage, applied after mixing and before PCM resampling.
#[derive(Clone, Debug, Default)]
pub struct AnalogOutputProcessor {
    config: AnalogOutputConfig,
    high_pass: Option<OnePoleHighPass>,
    low_pass: Option<OnePoleLowPass>,
}

impl AnalogOutputProcessor {
    pub fn new(sample_rate: u32, config: AnalogOutputConfig) -> Result<Self, DspError> {
        let mut processor = Self::default();
        processor.set_config(sample_rate, config)?;
        Ok(processor)
    }

    pub fn config(&self) -> AnalogOutputConfig {
        self.config
    }

    pub fn set_config(
        &mut self,
        sample_rate: u32,
        config: AnalogOutputConfig,
    ) -> Result<(), DspError> {
        config.validate(sample_rate)?;
        let high_pass = if config.enabled {
            config
                .high_pass_hz
                .map(|cutoff| OnePoleHighPass::new(sample_rate, cutoff))
                .transpose()?
        } else {
            None
        };
        let low_pass = if config.enabled {
            config
                .low_pass_hz
                .map(|cutoff| OnePoleLowPass::new(sample_rate, cutoff))
                .transpose()?
        } else {
            None
        };
        self.config = config;
        self.high_pass = high_pass;
        self.low_pass = low_pass;
        Ok(())
    }

    pub fn reset(&mut self) {
        if let Some(filter) = &mut self.high_pass {
            filter.reset();
        }
        if let Some(filter) = &mut self.low_pass {
            filter.reset();
        }
    }

    pub fn process_block(&mut self, input: &[f64], output: &mut Vec<f64>) {
        output.clear();
        output.reserve(input.len());
        for &sample in input {
            let mut value = self
                .high_pass
                .as_mut()
                .map_or(sample, |filter| filter.process_sample(sample));
            if let Some(filter) = &mut self.low_pass {
                value = filter.process_sample(value);
            }
            output.push(value);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn high_pass_blocks_dc_and_preserves_state_across_samples() {
        let mut filter = OnePoleHighPass::new(48_000, 20.0).unwrap();
        let mut output = 0.0;
        for _ in 0..48_000 {
            output = filter.process_sample(1.0);
        }
        assert!(output.abs() < 1.0e-6, "DC component should decay to zero");
        filter.reset();
        assert_eq!(filter.process_sample(0.0), 0.0);
    }

    #[test]
    fn analog_output_is_bit_exact_bypass_when_disabled() {
        let input = [0.0, 0.25, -0.5, 0.75, -1.0];
        let mut processor =
            AnalogOutputProcessor::new(48_000, AnalogOutputConfig::default()).unwrap();
        let mut output = Vec::new();
        processor.process_block(&input, &mut output);
        assert_eq!(output, input);
    }

    #[test]
    fn sms1_profile_is_explicitly_experimental_and_valid_at_internal_rate() {
        let config = AnalogOutputConfig::sms1_experimental();
        assert!(config.enabled);
        assert_eq!(config.high_pass_hz, Some(20.0));
        assert_eq!(config.low_pass_hz, Some(18_000.0));
        assert!(config.validate(48_000).is_ok());
    }

    #[test]
    fn analog_filter_block_processing_preserves_filter_state_between_blocks() {
        let config = AnalogOutputConfig::sms1_experimental();
        let mut split = AnalogOutputProcessor::new(48_000, config).unwrap();
        let mut whole = AnalogOutputProcessor::new(48_000, config).unwrap();
        let first = [0.0, 0.5, 0.25];
        let second = [-0.25, 0.0, 0.75];
        let mut split_output = Vec::new();
        let mut whole_output = Vec::new();
        split.process_block(&first, &mut split_output);
        let mut tail = Vec::new();
        split.process_block(&second, &mut tail);
        split_output.extend(tail);
        whole.process_block(&[0.0, 0.5, 0.25, -0.25, 0.0, 0.75], &mut whole_output);
        assert_eq!(split_output, whole_output);
    }

    #[test]
    fn mixer_sums_sources_and_reuses_output() {
        let mixer = AudioMixer::new(0.5).unwrap();
        let first = [0.5, -0.5, 0.25];
        let second = [0.5, 0.5, -0.25];
        let mut output = vec![99.0; 8];
        mixer
            .mix_mono_into(
                &[
                    MixSource {
                        samples: &first,
                        gain: 1.0,
                    },
                    MixSource {
                        samples: &second,
                        gain: 2.0,
                    },
                ],
                &mut output,
            )
            .unwrap();
        assert_eq!(output, vec![0.75, 0.25, -0.125]);
    }

    #[test]
    fn mixer_rejects_mismatched_blocks_and_invalid_gain() {
        let mixer = AudioMixer::default();
        let mut output = Vec::new();
        assert_eq!(
            mixer.mix_mono_into(
                &[
                    MixSource {
                        samples: &[0.0],
                        gain: 1.0,
                    },
                    MixSource {
                        samples: &[],
                        gain: 1.0,
                    },
                ],
                &mut output,
            ),
            Err(DspError::InconsistentSourceLength)
        );
        assert_eq!(
            mixer.mix_mono_into(
                &[MixSource {
                    samples: &[0.0],
                    gain: f64::NAN,
                }],
                &mut output,
            ),
            Err(DspError::InvalidSourceGain)
        );
    }

    #[test]
    fn low_pass_is_stateful_and_resettable() {
        let mut filter = OnePoleLowPass::new(48_000, 1_000.0).unwrap();
        let first = filter.process_sample(1.0);
        let second = filter.process_sample(0.0);
        assert!(first > 0.0 && first < 1.0 && second < first);
        filter.reset();
        assert_eq!(filter.process_sample(0.0), 0.0);
    }

    #[test]
    fn low_pass_rejects_nyquist() {
        assert_eq!(
            OnePoleLowPass::new(48_000, 24_000.0).err(),
            Some(DspError::InvalidFilterConfiguration)
        );
    }
}
