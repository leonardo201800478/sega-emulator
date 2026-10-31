//! Rust-owned audio session shared by the emulated machine adapters.
//!
//! This module has no PyO3 or Python dependency. It owns DSP, resampling and
//! the selected host output backend. Machine adapters pass native sample
//! slices directly; PCM buffers never cross the Python boundary.

use crate::{AudioMixer, MixSource, PcmResampler};
use sega_audio_output::{
    NativeWasapiOutput as RustWasapiOutput, NativeXAudio2Output as RustXAudio2Output,
};

/// Complete per-machine audio session, independent from the Python binding.
pub struct NativeAudioSession {
    mixer: AudioMixer,
    mixed_samples: Vec<f64>,
    resampler: PcmResampler,
    output: NativeAudioBackend,
}

enum NativeAudioBackend {
    Wasapi(RustWasapiOutput),
    XAudio2(RustXAudio2Output),
}

impl NativeAudioSession {
    pub fn new(
        backend: &str,
        sample_rate: u32,
        buffer_size: u32,
        queue_blocks: usize,
        volume: u8,
        balance: i8,
    ) -> Result<Self, String> {
        let resampler = PcmResampler::new(sample_rate, volume)
            .map_err(|error| error.to_string())?;
        let output = match backend {
            "wasapi" => NativeAudioBackend::Wasapi(
                RustWasapiOutput::new_with_balance(sample_rate, buffer_size, queue_blocks, balance)
                    .map_err(|error| error.to_string())?,
            ),
            "xaudio2" => NativeAudioBackend::XAudio2(
                RustXAudio2Output::new_with_balance(
                    sample_rate, buffer_size, queue_blocks, balance,
                )
                .map_err(|error| error.to_string())?,
            ),
            _ => return Err(format!("unsupported native audio backend: {backend}")),
        };
        Ok(Self {
            mixer: AudioMixer::default(),
            mixed_samples: Vec::new(),
            resampler,
            output,
        })
    }

    /// Mix, resample and enqueue one native hardware sample batch.
    pub fn process_samples(&mut self, samples: &[f64]) -> Result<(), String> {
        if samples.is_empty() {
            return Ok(());
        }
        self.mixer
            .mix_mono_into(
                &[MixSource { samples, gain: 1.0 }],
                &mut self.mixed_samples,
            )
            .map_err(|error| error.to_string())?;
        let pcm = self.resampler.process(&self.mixed_samples);
        if pcm.is_empty() {
            return Ok(());
        }
        match &mut self.output {
            NativeAudioBackend::Wasapi(output) => {
                output.write_pcm(pcm).map_err(|error| error.to_string())
            }
            NativeAudioBackend::XAudio2(output) => {
                output.write_pcm(pcm).map_err(|error| error.to_string())
            }
        }
    }

    /// Return backend underrun and dropped-block counters.
    pub fn metrics(&self) -> (u64, u64) {
        match &self.output {
            NativeAudioBackend::Wasapi(output) => {
                (output.underrun_count(), output.dropped_block_count())
            }
            NativeAudioBackend::XAudio2(output) => {
                (output.underrun_count(), output.dropped_block_count())
            }
        }
    }

    /// Flush the resampler tail and close the selected native output.
    pub fn close(&mut self) {
        let tail = self.resampler.flush();
        if !tail.is_empty() {
            match &mut self.output {
                NativeAudioBackend::Wasapi(output) => {
                    let _ = output.write_pcm(tail);
                }
                NativeAudioBackend::XAudio2(output) => {
                    let _ = output.write_pcm(tail);
                }
            }
        }
        match &mut self.output {
            NativeAudioBackend::Wasapi(output) => output.close(),
            NativeAudioBackend::XAudio2(output) => output.close(),
        }
    }
}

impl Drop for NativeAudioSession {
    fn drop(&mut self) {
        self.close();
    }
}

#[cfg(test)]
mod tests {
    use super::NativeAudioSession;

    #[test]
    fn rejects_unknown_backend_without_opening_a_device() {
        let result = NativeAudioSession::new("unknown", 48_000, 512, 4, 100, 0);
        assert!(result
            .err()
            .expect("unknown backend must fail")
            .contains("unsupported native audio backend"));
    }
}
