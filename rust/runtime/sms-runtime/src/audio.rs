//! Host-independent audio session interface for the SMS worker.
//!
//! The runtime owns when samples are mixed and submitted. The application
//! supplies a concrete session backed by its selected native audio host.

use sega_audio_runtime::{NativeAudioSession, PcmSink};

/// Audio session accepted by the SMS runtime worker.
pub trait AudioSession {
    /// Process one machine-generated audio sample batch.
    fn process_samples(&mut self, samples: &[f64]) -> Result<(), String>;

    /// Advance DSP state but discard PCM at the host boundary (debugger StepFrame).
    fn process_samples_discarding_output(&mut self, samples: &[f64]) -> Result<(), String>;

    /// Suspend the endpoint and synchronously invalidate its queued audio epoch.
    fn suspend_and_flush(&mut self) -> Result<(), String>;

    /// Begin a fresh endpoint epoch; playback may wait for prebuffer.
    fn resume(&mut self) -> Result<(), String>;

    /// Reset resampler history at Reset/LoadRom boundaries.
    fn reset_dsp(&mut self);

    /// Update output volume without replacing the native host session.
    fn set_volume(&mut self, volume: u8) -> Result<(), String>;

    /// Return host underrun and dropped-block counters.
    fn metrics(&self) -> (u64, u64);

    /// Close the host session and release its resources.
    fn close(&mut self);
}

impl<S: PcmSink> AudioSession for NativeAudioSession<S> {
    fn process_samples(&mut self, samples: &[f64]) -> Result<(), String> {
        NativeAudioSession::process_samples(self, samples)
    }

    fn process_samples_discarding_output(&mut self, samples: &[f64]) -> Result<(), String> {
        NativeAudioSession::process_samples_discarding_output(self, samples)
    }

    fn suspend_and_flush(&mut self) -> Result<(), String> {
        NativeAudioSession::suspend_and_flush(self)
    }

    fn resume(&mut self) -> Result<(), String> {
        NativeAudioSession::resume(self)
    }

    fn reset_dsp(&mut self) {
        NativeAudioSession::reset_dsp(self)
    }

    fn set_volume(&mut self, volume: u8) -> Result<(), String> {
        NativeAudioSession::set_volume(self, volume)
    }

    fn metrics(&self) -> (u64, u64) {
        NativeAudioSession::metrics(self)
    }

    fn close(&mut self) {
        NativeAudioSession::close(self);
    }
}
