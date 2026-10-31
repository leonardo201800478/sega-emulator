//! Rust-owned audio session shared by machine runtimes.
//!
//! This module has no PyO3, Python, or host API dependency. Output is supplied
//! through a small sink contract, allowing the complete DSP path to be tested
//! headlessly with deterministic in-memory sinks.

use crate::{AudioMixer, MixSource, PcmResampler};

/// Host-output boundary consumed by the Python-independent audio session.
pub trait PcmSink {
    /// Enqueue one interleaved PCM16 block. Implementations own any host buffers.
    fn write_pcm(&mut self, pcm: Vec<u8>) -> Result<(), String>;

    /// Return (underruns, dropped blocks) observed by the sink.
    fn metrics(&self) -> (u64, u64);

    /// Stop the host stream and release its resources.
    fn close(&mut self);
}

/// Complete per-machine DSP session, independent from PyO3 and Qt.
pub struct NativeAudioSession<S: PcmSink> {
    mixer: AudioMixer,
    mixed_samples: Vec<f64>,
    resampler: PcmResampler,
    sink: S,
    closed: bool,
}

impl<S: PcmSink> NativeAudioSession<S> {
    /// Construct a session around an explicitly selected output sink.
    pub fn new(sample_rate: u32, volume: u8, sink: S) -> Result<Self, String> {
        Self::with_sink_factory(sample_rate, volume, || Ok(sink))
    }

    /// Validate DSP configuration before opening the host sink.
    pub fn with_sink_factory<F>(sample_rate: u32, volume: u8, make_sink: F) -> Result<Self, String>
    where
        F: FnOnce() -> Result<S, String>,
    {
        let resampler = PcmResampler::new(sample_rate, volume)
            .map_err(|error| error.to_string())?;
        let sink = make_sink()?;
        Ok(Self {
            mixer: AudioMixer::default(),
            mixed_samples: Vec::new(),
            resampler,
            sink,
            closed: false,
        })
    }

    /// Mix, resample and enqueue one native hardware sample batch.
    pub fn process_samples(&mut self, samples: &[f64]) -> Result<(), String> {
        if self.closed {
            return Err("native audio session is closed".to_owned());
        }
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
        self.sink.write_pcm(pcm)
    }

    /// Return backend underrun and dropped-block counters.
    pub fn metrics(&self) -> (u64, u64) {
        self.sink.metrics()
    }

    /// Flush the resampler tail and close the selected native output once.
    pub fn close(&mut self) {
        if self.closed {
            return;
        }
        let tail = self.resampler.flush();
        if !tail.is_empty() {
            let _ = self.sink.write_pcm(tail);
        }
        self.sink.close();
        self.closed = true;
    }
}

impl<S: PcmSink> Drop for NativeAudioSession<S> {
    fn drop(&mut self) {
        self.close();
    }
}

#[cfg(test)]
mod tests {
    use super::{NativeAudioSession, PcmSink};

    #[derive(Default)]
    struct RecordingSink {
        blocks: Vec<Vec<u8>>,
        underruns: u64,
        dropped: u64,
        closed: bool,
    }

    impl PcmSink for RecordingSink {
        fn write_pcm(&mut self, pcm: Vec<u8>) -> Result<(), String> {
            if self.closed {
                return Err("sink is closed".to_owned());
            }
            self.blocks.push(pcm);
            Ok(())
        }

        fn metrics(&self) -> (u64, u64) {
            (self.underruns, self.dropped)
        }

        fn close(&mut self) {
            self.closed = true;
        }
    }

    #[test]
    fn session_runs_pcm_conversion_without_a_host_or_python_runtime() {
        let sink = RecordingSink::default();
        let mut session = NativeAudioSession::new(44_100, 100, sink).unwrap();
        session.process_samples(&[0.0, 0.5, -0.5]).unwrap();
        session.close();
        assert_eq!(session.sink.blocks, vec![vec![0, 0, 0, 64, 0, 192]]);
        assert!(session.sink.closed);
    }

    #[test]
    fn session_ignores_empty_batches_and_closes_idempotently() {
        let mut session = NativeAudioSession::new(44_100, 100, RecordingSink::default()).unwrap();
        session.process_samples(&[]).unwrap();
        assert!(session.sink.blocks.is_empty());
        session.close();
        session.close();
        assert!(session.sink.closed);
        assert!(session.process_samples(&[0.25]).is_err());
    }

    #[test]
    fn session_reports_sink_metrics() {
        let sink = RecordingSink {
            underruns: 3,
            dropped: 2,
            ..RecordingSink::default()
        };
        let session = NativeAudioSession::new(44_100, 100, sink).unwrap();
        assert_eq!(session.metrics(), (3, 2));
    }
}
