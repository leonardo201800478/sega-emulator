//! Host-independent audio session interface for the SMS worker.
//!
//! The runtime owns when samples are mixed and submitted. The application
//! supplies a concrete session backed by its selected native audio host.

use sega_audio_runtime::{NativeAudioSession, PcmSink};

/// Audio session accepted by the SMS runtime worker.
pub trait AudioSession {
    /// Process one machine-generated audio sample batch.
    fn process_samples(&mut self, samples: &[f64]) -> Result<(), String>;

    /// Return host underrun and dropped-block counters.
    fn metrics(&self) -> (u64, u64);

    /// Close the host session and release its resources.
    fn close(&mut self);
}

impl<S: PcmSink> AudioSession for NativeAudioSession<S> {
    fn process_samples(&mut self, samples: &[f64]) -> Result<(), String> {
        NativeAudioSession::process_samples(self, samples)
    }

    fn metrics(&self) -> (u64, u64) {
        NativeAudioSession::metrics(self)
    }

    fn close(&mut self) {
        NativeAudioSession::close(self);
    }
}
