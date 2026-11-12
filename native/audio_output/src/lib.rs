//! CPAL-backed physical PCM output for the Sega audio runtime.
//!
//! The emulated PSG and DSP remain upstream of this crate. The CPAL callback
//! only consumes completed stereo PCM16 frames from a bounded SPSC queue,
//! converts them to the negotiated device sample format, and fills missing
//! frames with digital silence. It never advances emulated time or calls the
//! MachineScheduler.

mod pcm_convert;
mod pcm_fifo;

pub use pcm_fifo::{
    PcmFrameQueueError, PcmFrameQueueMetrics, PcmFrameReadReport, PcmFrameWriteReport,
    StereoFrame, pcm_frame_queue,
};

use std::str::FromStr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample, SizedSample};
use sega_audio_runtime::{AudioOutputMetrics, NativeAudioSession, PcmSink};

use crate::pcm_convert::mono_sample_to_stereo_frame;
use crate::pcm_fifo::{PcmFrameConsumer, PcmFrameProducer};

const CALLBACK_SCRATCH_FRAMES: usize = 4096;

/// Stable output-device identity and human-readable name supplied by CPAL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioOutputDeviceInfo {
    /// Stable CPAL device identifier suitable for persistence.
    pub id: String,
    /// Human-readable device name.
    pub name: String,
}

/// CPAL output stream and bounded PCM transport.
pub struct NativeCpalOutput {
    stream: Option<cpal::Stream>,
    producer: PcmFrameProducer,
    sample_rate_hz: u32,
    channels: u16,
    started: bool,
    closed: bool,
    flush_requested: Arc<AtomicBool>,
    callback_error: Arc<Mutex<Option<String>>>,
    frame_scratch: Vec<StereoFrame>,
    dropped_blocks: u64,
}

impl NativeCpalOutput {
    /// Enumerate available output devices and their stable CPAL identifiers.
    pub fn output_devices() -> Result<Vec<AudioOutputDeviceInfo>, String> {
        let host = cpal::default_host();
        let devices = host
            .output_devices()
            .map_err(|error| format!("CPAL output-device enumeration failed: {error}"))?;

        devices
            .map(|device| {
                let id = device
                    .id()
                    .map_err(|error| format!("CPAL device identifier unavailable: {error}"))?
                    .to_string();
                let name = device
                    .description()
                    .map_err(|error| format!("CPAL device description unavailable: {error}"))?
                    .name()
                    .to_owned();
                Ok(AudioOutputDeviceInfo { id, name })
            })
            .collect()
    }

    /// Open the operating system's default output device.
    pub fn new(buffer_frames: u32, queue_blocks: usize) -> Result<Self, String> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or_else(|| "CPAL did not find a default output device".to_owned())?;
        Self::new_with_device(device, buffer_frames, queue_blocks)
    }

    /// Open a previously enumerated output device by its stable CPAL identifier.
    pub fn new_for_device(
        device_id: &str,
        buffer_frames: u32,
        queue_blocks: usize,
    ) -> Result<Self, String> {
        let host = cpal::default_host();
        let id = cpal::DeviceId::from_str(device_id)
            .map_err(|error| format!("Invalid CPAL device identifier: {error}"))?;
        let device = host
            .device_by_id(&id)
            .ok_or_else(|| format!("CPAL output device '{device_id}' is no longer available"))?;
        Self::new_with_device(device, buffer_frames, queue_blocks)
    }

    fn new_with_device(
        device: cpal::Device,
        buffer_frames: u32,
        queue_blocks: usize,
    ) -> Result<Self, String> {
        if buffer_frames == 0 {
            return Err("CPAL buffer size must be positive".to_owned());
        }
        if queue_blocks == 0 {
            return Err("CPAL queue block count must be positive".to_owned());
        }

        let supported = device
            .default_output_config()
            .map_err(|error| format!("CPAL default output configuration failed: {error}"))?;
        let sample_rate_hz = supported.sample_rate();
        let channels = supported.channels();
        if channels == 0 {
            return Err("CPAL output device reported zero channels".to_owned());
        }

        let mut config: cpal::StreamConfig = supported.clone().into();
        config.buffer_size = cpal::BufferSize::Fixed(buffer_frames);

        let queue_capacity = usize::try_from(buffer_frames)
            .ok()
            .and_then(|frames| frames.checked_mul(queue_blocks))
            .filter(|capacity| *capacity > 0)
            .ok_or_else(|| "CPAL PCM queue capacity overflow".to_owned())?;
        let (producer, consumer) =
            crate::pcm_fifo::pcm_frame_queue(queue_capacity).map_err(|error| error.to_string())?;

        let flush_requested = Arc::new(AtomicBool::new(false));
        let callback_error = Arc::new(Mutex::new(None));
        let stream = build_stream(
            &device,
            config,
            supported.sample_format(),
            channels,
            consumer,
            Arc::clone(&flush_requested),
            Arc::clone(&callback_error),
        )
        .map_err(|error| format!("CPAL output stream creation failed: {error}"))?;

        Ok(Self {
            stream: Some(stream),
            producer,
            sample_rate_hz,
            channels,
            started: false,
            closed: false,
            flush_requested,
            callback_error,
            frame_scratch: Vec::new(),
            dropped_blocks: 0,
        })
    }

    /// Return the actual sample rate selected by the output device.
    pub const fn sample_rate_hz(&self) -> u32 {
        self.sample_rate_hz
    }

    /// Wrap this sink in the shared Rust DSP/resampler session.
    pub fn into_audio_session(
        self,
        volume: u8,
    ) -> Result<NativeAudioSession<Self>, String> {
        NativeAudioSession::new(self.sample_rate_hz, volume, self)
    }

    fn check_callback_error(&self) -> Result<(), String> {
        let error = self
            .callback_error
            .lock()
            .map_err(|_| "CPAL callback error state is poisoned".to_owned())?
            .clone();
        match error {
            Some(error) => Err(format!("CPAL output stream failed: {error}")),
            None => Ok(()),
        }
    }
}

impl PcmSink for NativeCpalOutput {
    fn write_pcm(&mut self, pcm: Vec<u8>) -> Result<(), String> {
        if self.closed {
            return Err("CPAL output is closed".to_owned());
        }
        self.check_callback_error()?;
        if pcm.len() % 2 != 0 {
            return Err("PCM16 block contains an incomplete sample".to_owned());
        }

        self.frame_scratch.clear();
        self.frame_scratch.reserve(pcm.len() / 2);
        for sample in pcm.chunks_exact(2) {
            let mono = i16::from_le_bytes([sample[0], sample[1]]);
            self.frame_scratch.push(mono_sample_to_stereo_frame(mono, 0));
        }

        let report = self
            .producer
            .push_frames(&self.frame_scratch)
            .map_err(|error| error.to_string())?;
        if report.dropped_frames > 0 {
            self.dropped_blocks = self.dropped_blocks.saturating_add(1);
        }
        Ok(())
    }

    fn suspend_and_flush(&mut self) -> Result<(), String> {
        if self.closed {
            return Err("CPAL output is closed".to_owned());
        }
        self.check_callback_error()?;
        if self.started {
            self.stream
                .as_ref()
                .ok_or_else(|| "CPAL output stream is unavailable".to_owned())?
                .pause()
                .map_err(|error| format!("CPAL stream pause failed: {error}"))?;
            self.started = false;
        }
        // The callback performs the consumer-side SPSC discard before it reads
        // any samples in the next playback epoch. CPAL does not expose a
        // portable command to purge samples already buffered by the OS/device.
        self.flush_requested.store(true, Ordering::Release);
        Ok(())
    }

    fn resume(&mut self) -> Result<(), String> {
        if self.closed {
            return Err("CPAL output is closed".to_owned());
        }
        self.check_callback_error()?;
        if !self.started {
            self.stream
                .as_ref()
                .ok_or_else(|| "CPAL output stream is unavailable".to_owned())?
                .play()
                .map_err(|error| format!("CPAL stream start failed: {error}"))?;
            self.started = true;
        }
        Ok(())
    }

    fn metrics(&self) -> AudioOutputMetrics {
        let queue = self.producer.metrics();
        AudioOutputMetrics {
            sample_rate_hz: self.sample_rate_hz,
            queue_capacity_frames: queue.capacity_frames as u64,
            queue_queued_frames: queue.queued_frames as u64,
            queue_high_water_frames: queue.high_water_frames as u64,
            accepted_frames: queue.accepted_frames,
            consumed_frames: queue.consumed_frames,
            dropped_frames: queue.dropped_frames,
            underrun_frames: queue.underrun_frames,
            underrun_events: queue.underrun_events,
            dropped_blocks: self.dropped_blocks,
            flushed_frames: queue.flushed_frames,
        }
    }

    fn close(&mut self) {
        if self.closed {
            return;
        }
        if self.started {
            if let Some(stream) = self.stream.as_ref() {
                let _ = stream.pause();
            }
            self.started = false;
        }
        self.stream.take();
        self.closed = true;
    }
}

fn build_stream<T>(
    device: &cpal::Device,
    config: cpal::StreamConfig,
    channels: u16,
    mut consumer: PcmFrameConsumer,
    flush_requested: Arc<AtomicBool>,
    callback_error: Arc<Mutex<Option<String>>>,
) -> Result<cpal::Stream, cpal::Error>
where
    T: SizedSample + Sample + FromSample<i16>,
{
    let channel_count = usize::from(channels);
    let mut scratch = vec![[0_i16; 2]; CALLBACK_SCRATCH_FRAMES];

    device.build_output_stream(
        config,
        move |output: &mut [T], _info: &cpal::OutputCallbackInfo| {
            if flush_requested.swap(false, Ordering::AcqRel) {
                consumer.discard_queued_frames();
            }

            let frame_count = output.len() / channel_count;
            let mut frame_offset = 0;
            while frame_offset < frame_count {
                let count = (frame_count - frame_offset).min(scratch.len());
                consumer.read_frames(&mut scratch[..count]);
                let sample_start = frame_offset * channel_count;
                let sample_end = sample_start + count * channel_count;
                let output_chunk = &mut output[sample_start..sample_end];

                for (frame, destination) in scratch[..count]
                    .iter()
                    .zip(output_chunk.chunks_exact_mut(channel_count))
                {
                    if channel_count == 1 {
                        let mono = ((i32::from(frame[0]) + i32::from(frame[1])) / 2) as i16;
                        destination[0] = T::from_sample(mono);
                    } else {
                        destination[0] = T::from_sample(frame[0]);
                        destination[1] = T::from_sample(frame[1]);
                        destination[2..].fill(T::EQUILIBRIUM);
                    }
                }
                frame_offset += count;
            }

            output[frame_count * channel_count..].fill(T::EQUILIBRIUM);
        },
        move |error| {
            if let Ok(mut current) = callback_error.lock() {
                *current = Some(error.to_string());
            }
        },
        None,
    )
}

fn build_stream_for_format(
    device: &cpal::Device,
    config: cpal::StreamConfig,
    format: cpal::SampleFormat,
    channels: u16,
    consumer: PcmFrameConsumer,
    flush_requested: Arc<AtomicBool>,
    callback_error: Arc<Mutex<Option<String>>>,
) -> Result<cpal::Stream, cpal::Error> {
    macro_rules! build {
        ($sample:ty) => {
            build_stream::<$sample>(
                device,
                config,
                channels,
                consumer,
                flush_requested,
                callback_error,
            )
        };
    }

    match format {
        cpal::SampleFormat::I8 => build!(i8),
        cpal::SampleFormat::I16 => build!(i16),
        cpal::SampleFormat::I24 => build!(cpal::I24),
        cpal::SampleFormat::I32 => build!(i32),
        cpal::SampleFormat::I64 => build!(i64),
        cpal::SampleFormat::U8 => build!(u8),
        cpal::SampleFormat::U16 => build!(u16),
        cpal::SampleFormat::U24 => build!(cpal::U24),
        cpal::SampleFormat::U32 => build!(u32),
        cpal::SampleFormat::U64 => build!(u64),
        cpal::SampleFormat::F32 => build!(f32),
        cpal::SampleFormat::F64 => build!(f64),
        unsupported => Err(cpal::Error::new(
            cpal::ErrorKind::UnsupportedOperation,
            format!("unsupported CPAL output sample format: {unsupported}"),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_device_identifiers_are_non_empty_when_enumeration_succeeds() {
        match NativeCpalOutput::output_devices() {
            Ok(devices) => {
                for device in devices {
                    assert!(!device.id.is_empty());
                    assert!(!device.name.is_empty());
                }
            }
            Err(error) => assert!(!error.is_empty()),
        }
    }

    #[test]
    fn pcm_sink_metrics_start_at_zero_without_opening_a_device() {
        let (producer, _consumer) = pcm_frame_queue(64).unwrap();
        let metrics = producer.metrics();
        assert_eq!(metrics.accepted_frames, 0);
        assert_eq!(metrics.consumed_frames, 0);
        assert_eq!(metrics.underrun_frames, 0);
    }

    #[test]
    fn mono_pcm16_conversion_preserves_signed_extremes() {
        for sample in [i16::MIN, -1, 0, 1, i16::MAX] {
            assert_eq!(mono_sample_to_stereo_frame(sample, 0), [sample, sample]);
        }
    }
}
