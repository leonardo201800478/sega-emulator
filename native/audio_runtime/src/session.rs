//! Rust-owned audio session shared by machine runtimes.
//!
//! This module has no PyO3, Python, or host API dependency. Output is supplied
//! through a small sink contract, allowing the complete DSP path to be tested
//! headlessly with deterministic in-memory sinks.

use crate::{AudioMixer, MixSource, PcmResampler};

/// Copyable snapshot of host PCM output telemetry.
///
/// Queue counters use stereo PCM frames, not scalar samples or bytes.
/// The queue-duration helpers estimate only the backlog held in the SPSC
/// transport; they are not end-to-end device latency measurements.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AudioOutputMetrics {
    /// Configured host output rate in frames per second; zero means unavailable.
    pub sample_rate_hz: u32,
    /// Fixed SPSC queue capacity in stereo frames.
    pub queue_capacity_frames: u64,
    /// Approximate instantaneous SPSC queue occupancy in stereo frames.
    pub queue_queued_frames: u64,
    /// Maximum observed SPSC queue occupancy in stereo frames.
    pub queue_high_water_frames: u64,
    /// Total frames successfully published by the producer.
    pub accepted_frames: u64,
    /// Total frames removed from the SPSC queue by the backend consumer.
    pub consumed_frames: u64,
    /// Total newest frames rejected because the SPSC queue was full.
    pub dropped_frames: u64,
    /// Total frames replaced with digital silence when the consumer needed data.
    pub underrun_frames: u64,
    /// Consumer read operations that needed at least one silence frame.
    pub underrun_events: u64,
    /// PCM write calls in which at least one frame was rejected.
    pub dropped_blocks: u64,
    /// Frames intentionally discarded from the FIFO during an epoch flush.
    pub flushed_frames: u64,
    /// XAudio2 source-voice starvation transitions (not FIFO underrun events).
    pub endpoint_starvation_events: u64,
    /// Frames removed from the FIFO but rejected by XAudio2 submission.
    pub submission_failed_frames: u64,
}

impl AudioOutputMetrics {
    /// Estimate the duration represented by the current SPSC queue backlog.
    ///
    /// This excludes endpoint/driver/device buffering and is not a physical
    /// playback-latency measurement. Returns None when the sample rate is
    /// unavailable.
    pub fn queue_duration_micros(self) -> Option<u64> {
        (self.sample_rate_hz > 0).then(|| {
            self.queue_queued_frames.saturating_mul(1_000_000) / u64::from(self.sample_rate_hz)
        })
    }

    /// Estimate the maximum observed SPSC queue backlog duration.
    pub fn queue_high_water_duration_micros(self) -> Option<u64> {
        (self.sample_rate_hz > 0).then(|| {
            self.queue_high_water_frames.saturating_mul(1_000_000) / u64::from(self.sample_rate_hz)
        })
    }
}

/// Host-output boundary consumed by the Rust audio session.
pub trait PcmSink {
    /// Enqueue one interleaved PCM16 block. Implementations own any host buffers.
    fn write_pcm(&mut self, pcm: Vec<u8>) -> Result<(), String>;

    /// Invalidate queued and submitted host audio before acknowledging a
    /// pause, stop, reset or content/ROM transition.
    ///
    /// The implementation must perform the operation on the backend-owning
    /// thread and return only after old-epoch audio can no longer be emitted.
    fn suspend_and_flush(&mut self) -> Result<(), String>;

    /// Allow the backend to begin a new audio epoch. Device playback may wait
    /// for its configured prebuffer threshold before starting.
    fn resume(&mut self) -> Result<(), String>;

    /// Return a nonblocking snapshot of queue and host-output counters.
    fn metrics(&self) -> AudioOutputMetrics;

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
    suspended: bool,
    discarded_since_suspend: bool,
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
        sega_observability::tracing::info!(
            target: "sega::audio::session",
            sample_rate,
            volume,
            "native audio session initialization requested"
        );
        let resampler =
            PcmResampler::new(sample_rate, volume).map_err(|error| error.to_string())?;
        let sink = make_sink()?;
        Ok(Self {
            mixer: AudioMixer::default(),
            mixed_samples: Vec::new(),
            resampler,
            sink,
            closed: false,
            suspended: false,
            discarded_since_suspend: false,
        })
    }

    /// Mix, resample and enqueue one native hardware sample batch.
    pub fn process_samples(&mut self, samples: &[f64]) -> Result<(), String> {
        self.process_samples_with_policy(samples, true)
    }

    /// Advance the same mixer/resampler state without publishing PCM to the
    /// physical endpoint. Used by deterministic debugger StepFrame execution.
    pub fn process_samples_discarding_output(&mut self, samples: &[f64]) -> Result<(), String> {
        if samples.is_empty() {
            return Ok(());
        }
        self.process_samples_with_policy(samples, false)?;
        self.discarded_since_suspend = true;
        Ok(())
    }

    fn process_samples_with_policy(
        &mut self,
        samples: &[f64],
        publish: bool,
    ) -> Result<(), String> {
        if self.closed {
            return Err("native audio session is closed".to_owned());
        }
        if samples.is_empty() {
            return Ok(());
        }
        self.mixer
            .mix_mono_into(&[MixSource { samples, gain: 1.0 }], &mut self.mixed_samples)
            .map_err(|error| error.to_string())?;
        let pcm = self.resampler.process(&self.mixed_samples);
        if publish && !pcm.is_empty() {
            self.sink.write_pcm(pcm)?;
        }
        Ok(())
    }

    /// Stop host presentation and wait for the backend to invalidate its
    /// current queue epoch. DSP state is intentionally preserved for Pause.
    pub fn suspend_and_flush(&mut self) -> Result<(), String> {
        sega_observability::tracing::debug!(
            target: "sega::audio::session",
            already_suspended = self.suspended,
            closed = self.closed,
            "audio session suspend requested"
        );
        if self.closed {
            return Err("native audio session is closed".to_owned());
        }
        self.sink.suspend_and_flush()?;
        self.suspended = true;
        Ok(())
    }

    /// Start a fresh presentation epoch without resetting DSP history.
    pub fn resume(&mut self) -> Result<(), String> {
        sega_observability::tracing::debug!(
            target: "sega::audio::session",
            suspended = self.suspended,
            closed = self.closed,
            discarded_since_suspend = self.discarded_since_suspend,
            "audio session resume requested"
        );
        if self.closed {
            return Err("native audio session is closed".to_owned());
        }
        if self.discarded_since_suspend {
            // Debugger-stepped samples advanced the emulated sound source but
            // were intentionally not presented. Do not let their FIR history
            // leak into the first audible block of the resumed epoch.
            self.resampler.reset();
            self.mixed_samples.clear();
        }
        self.sink.resume()?;
        self.suspended = false;
        self.discarded_since_suspend = false;
        Ok(())
    }

    /// Reset FIR/resampler history at a machine-content boundary such as
    /// Reset or LoadRom. Pause/Resume must not call this method.
    pub fn reset_dsp(&mut self) {
        sega_observability::tracing::debug!(
            target: "sega::audio::session",
            "audio DSP history reset"
        );
        self.resampler.reset();
        self.mixed_samples.clear();
        self.discarded_since_suspend = false;
    }

    /// Update output volume without rebuilding the host stream.
    pub fn set_volume(&mut self, volume: u8) -> Result<(), String> {
        self.resampler
            .set_volume(volume)
            .map_err(|error| error.to_string())
    }

    /// Return backend underrun and dropped-block counters.
    pub fn metrics(&self) -> AudioOutputMetrics {
        self.sink.metrics()
    }

    /// Flush the resampler tail and close the selected native output once.
    pub fn close(&mut self) {
        if self.closed {
            sega_observability::tracing::trace!(
                target: "sega::audio::session",
                "duplicate audio session close ignored"
            );
            return;
        }
        if self.suspended || self.discarded_since_suspend {
            // A successful suspend/flush invalidates the previous output epoch.
            // Closing must not publish a resampler tail into that invalidated epoch.
            self.resampler.reset();
        } else {
            let tail = self.resampler.flush();
            if !tail.is_empty() {
                let _ = self.sink.write_pcm(tail);
            }
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
    use super::{AudioOutputMetrics, NativeAudioSession, PcmSink};

    #[derive(Default)]
    struct RecordingSink {
        blocks: Vec<Vec<u8>>,
        underruns: u64,
        dropped: u64,
        closed: bool,
        flush_count: usize,
        resume_count: usize,
        suspended: bool,
    }

    impl PcmSink for RecordingSink {
        fn write_pcm(&mut self, pcm: Vec<u8>) -> Result<(), String> {
            if self.closed {
                return Err("sink is closed".to_owned());
            }
            self.blocks.push(pcm);
            Ok(())
        }

        fn suspend_and_flush(&mut self) -> Result<(), String> {
            self.flush_count += 1;
            self.suspended = true;
            Ok(())
        }

        fn resume(&mut self) -> Result<(), String> {
            self.resume_count += 1;
            self.suspended = false;
            Ok(())
        }

        fn metrics(&self) -> AudioOutputMetrics {
            AudioOutputMetrics {
                underrun_events: self.underruns,
                dropped_blocks: self.dropped,
                ..AudioOutputMetrics::default()
            }
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
    fn close_after_suspend_does_not_publish_resampler_tail() {
        let mut session = NativeAudioSession::new(44_100, 100, RecordingSink::default()).unwrap();
        session.process_samples(&[0.0, 0.5, -0.5]).unwrap();
        session.suspend_and_flush().unwrap();
        let blocks_at_flush = session.sink.blocks.clone();

        session.close();

        assert_eq!(session.sink.blocks, blocks_at_flush);
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
    fn changing_volume_keeps_the_existing_output_sink_open() {
        let sink = RecordingSink::default();
        let mut session = NativeAudioSession::new(44_100, 100, sink).unwrap();

        session.process_samples(&[0.5]).unwrap();
        session.set_volume(50).unwrap();
        session.process_samples(&[0.5]).unwrap();

        assert_eq!(session.sink.blocks.len(), 2);
        assert!(!session.sink.closed);
        let full_volume =
            i16::from_le_bytes([session.sink.blocks[0][0], session.sink.blocks[0][1]]);
        let half_volume =
            i16::from_le_bytes([session.sink.blocks[1][0], session.sink.blocks[1][1]]);
        assert!(half_volume.abs() < full_volume.abs());
    }

    #[test]
    fn invalid_volume_does_not_change_the_active_session() {
        let mut session = NativeAudioSession::new(44_100, 100, RecordingSink::default()).unwrap();
        assert!(session.set_volume(101).is_err());
        assert_eq!(session.resampler.volume(), 100);
    }

    #[test]
    fn session_reports_sink_metrics() {
        let sink = RecordingSink {
            underruns: 3,
            dropped: 2,
            ..RecordingSink::default()
        };
        let session = NativeAudioSession::new(44_100, 100, sink).unwrap();
        let metrics = session.metrics();
        assert_eq!(metrics.underrun_events, 3);
        assert_eq!(metrics.dropped_blocks, 2);
    }

    #[test]
    fn queue_duration_estimates_only_the_fifo_backlog() {
        let metrics = AudioOutputMetrics {
            sample_rate_hz: 48_000,
            queue_queued_frames: 480,
            queue_high_water_frames: 960,
            ..AudioOutputMetrics::default()
        };

        assert_eq!(metrics.queue_duration_micros(), Some(10_000));
        assert_eq!(metrics.queue_high_water_duration_micros(), Some(20_000));
        assert_eq!(AudioOutputMetrics::default().queue_duration_micros(), None);
    }

    #[test]
    fn discarded_step_frame_does_not_publish_or_leak_fir_history_after_resume() {
        let first: Vec<f64> = (0..160).map(|i| (i as f64 * 0.07).sin()).collect();
        let second: Vec<f64> = (160..320).map(|i| (i as f64 * 0.07).sin()).collect();

        let mut stepped = NativeAudioSession::new(48_000, 100, RecordingSink::default()).unwrap();
        stepped.process_samples_discarding_output(&first).unwrap();
        assert!(stepped.sink.blocks.is_empty());
        stepped.resume().unwrap();
        stepped.process_samples(&second).unwrap();

        let mut reference = NativeAudioSession::new(48_000, 100, RecordingSink::default()).unwrap();
        reference.process_samples(&second).unwrap();

        assert_eq!(stepped.sink.blocks, reference.sink.blocks);
    }

    #[test]
    fn pause_flush_resume_preserves_dsp_history_and_acknowledges_sink_controls() {
        let mut session = NativeAudioSession::new(48_000, 100, RecordingSink::default()).unwrap();
        let first: Vec<f64> = (0..160).map(|i| (i as f64 * 0.05).sin()).collect();
        let second: Vec<f64> = (160..320).map(|i| (i as f64 * 0.05).sin()).collect();
        session.process_samples(&first).unwrap();
        session.suspend_and_flush().unwrap();
        assert!(session.sink.suspended);
        session.resume().unwrap();
        assert!(!session.sink.suspended);
        session.process_samples(&second).unwrap();

        let mut reference = NativeAudioSession::new(48_000, 100, RecordingSink::default()).unwrap();
        reference.process_samples(&first).unwrap();
        reference.process_samples(&second).unwrap();

        assert_eq!(session.sink.flush_count, 1);
        assert_eq!(session.sink.resume_count, 1);
        assert_eq!(session.sink.blocks, reference.sink.blocks);
    }

    #[test]
    fn reset_dsp_discards_filter_history_at_machine_content_boundary() {
        let input: Vec<f64> = (0..160).map(|i| (i as f64 * 0.03).sin()).collect();
        let mut session = NativeAudioSession::new(48_000, 100, RecordingSink::default()).unwrap();
        session.process_samples(&input).unwrap();
        session.reset_dsp();
        session.process_samples(&input).unwrap();

        let mut fresh = NativeAudioSession::new(48_000, 100, RecordingSink::default()).unwrap();
        fresh.process_samples(&input).unwrap();

        assert_eq!(session.sink.blocks[1], fresh.sink.blocks[0]);
    }

    #[test]
    fn invalid_dsp_configuration_does_not_construct_the_host_sink() {
        let mut sink_created = false;
        let result = NativeAudioSession::<RecordingSink>::with_sink_factory(0, 100, || {
            sink_created = true;
            Ok(RecordingSink::default())
        });
        assert!(result.is_err());
        assert!(!sink_created);
    }
}
