//! Native Windows audio output. Qt/Python owns configuration only.

mod pcm_convert;
mod pcm_fifo;

fn record_audio_underrun(
    underruns: &AtomicU64,
    backend: &'static str,
    inserted_silence_frames: u64,
) {
    let count = underruns.fetch_add(1, Ordering::Relaxed) + 1;
    if count.is_power_of_two() {
        sega_observability::tracing::warn!(
            target: "sega::audio::backend",
            backend,
            underrun_events = count,
            inserted_silence_frames,
            "audio output underrun detected"
        );
    }
}

fn audio_output_metrics_from_queue(
    sample_rate_hz: u32,
    queue: PcmFrameQueueMetrics,
    dropped_blocks: u64,
    endpoint_starvation_events: u64,
    submission_failed_frames: u64,
) -> sega_audio_runtime::AudioOutputMetrics {
    sega_audio_runtime::AudioOutputMetrics {
        sample_rate_hz,
        queue_capacity_frames: queue.capacity_frames as u64,
        queue_queued_frames: queue.queued_frames as u64,
        queue_high_water_frames: queue.high_water_frames as u64,
        accepted_frames: queue.accepted_frames,
        consumed_frames: queue.consumed_frames,
        dropped_frames: queue.dropped_frames,
        underrun_frames: queue.underrun_frames,
        underrun_events: queue.underrun_events,
        dropped_blocks,
        flushed_frames: queue.flushed_frames,
        endpoint_starvation_events,
        submission_failed_frames,
    }
}

pub use pcm_fifo::{
    PcmFrameConsumer, PcmFrameProducer, PcmFrameQueueError, PcmFrameQueueMetrics,
    PcmFrameReadReport, PcmFrameWriteReport, StereoFrame, pcm_frame_queue,
};

#[cfg(any(windows, test))]
use pcm_convert::mono_sample_to_stereo_frame;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
#[cfg(windows)]
use std::sync::mpsc;
#[cfg(any(windows, test))]
use std::thread;
#[cfg(windows)]
use std::thread::JoinHandle;
#[cfg(any(windows, test))]
use std::time::Duration;

#[cfg(any(windows, test))]
use std::collections::VecDeque;
#[cfg(windows)]
use wasapi::{
    DeviceEnumerator, Direction, SampleType, StreamMode, TimingMode, WaveFormat, deinitialize,
    initialize_mta,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AudioOutputError {
    InvalidSampleRate,
    InvalidBufferSize,
    InvalidQueueCapacity,
    InvalidBalance,
    InvalidPcmBlock,
    Device(String),
    WorkerStopped,
}

impl std::fmt::Display for AudioOutputError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidSampleRate => write!(f, "sample_rate must be positive"),
            Self::InvalidBufferSize => write!(f, "buffer_size must be positive"),
            Self::InvalidQueueCapacity => write!(f, "queue_blocks must be positive"),
            Self::InvalidBalance => write!(f, "balance must be between -100 and 100"),
            Self::InvalidPcmBlock => write!(f, "PCM16 blocks must contain an even byte count"),
            Self::Device(message) => write!(f, "WASAPI output failed: {message}"),
            Self::WorkerStopped => write!(f, "WASAPI output worker has stopped"),
        }
    }
}

impl std::error::Error for AudioOutputError {}

#[cfg(test)]
/// Convert mono PCM16 bytes into a producer-side batch of stereo frames.
/// The allocation is confined to the producer path, never the WASAPI consumer.
fn mono_pcm16_to_stereo_frames(pcm: &[u8], balance: i8) -> Vec<StereoFrame> {
    let mut frames = Vec::with_capacity(pcm.len() / 2);
    for sample in pcm.as_chunks::<2>().0 {
        let value = i16::from_le_bytes([sample[0], sample[1]]);
        frames.push(mono_sample_to_stereo_frame(value, balance));
    }
    frames
}

/// Return whether a backend producer may publish another PCM block.
#[cfg(any(windows, test))]
fn backend_accepts_pcm(shutdown: bool, failed: bool, worker_finished: bool) -> bool {
    !shutdown && !failed && !worker_finished
}

/// Maximum number of PCM frames converted before publishing to the SPSC queue.
/// This bounds temporary conversion storage independently of caller block size.
#[cfg(any(windows, test))]
const PCM_FRAME_BATCH_SIZE: usize = 256;

/// Convert and publish mono PCM16 in fixed-size batches without allocating.
/// Returns the exact number of frames rejected by the queue's drop-newest policy.
#[cfg(any(windows, test))]
fn push_mono_pcm16_bounded(
    producer: &mut PcmFrameProducer,
    pcm: &[u8],
    balance: i8,
) -> Result<usize, PcmFrameQueueError> {
    debug_assert!(pcm.len().is_multiple_of(2));

    let mut frames = [[0_i16; 2]; PCM_FRAME_BATCH_SIZE];
    let mut dropped_frames = 0_usize;

    for batch in pcm.chunks(PCM_FRAME_BATCH_SIZE * 2) {
        let frame_count = batch.len() / 2;
        for (index, sample) in batch.as_chunks::<2>().0.iter().enumerate() {
            let mono = i16::from_le_bytes([sample[0], sample[1]]);
            frames[index] = mono_sample_to_stereo_frame(mono, balance);
        }

        let report = producer.push_frames(&frames[..frame_count])?;
        dropped_frames += report.dropped_frames;
    }

    Ok(dropped_frames)
}

/// Legacy byte-layout oracle retained only for equivalence tests.
#[cfg(test)]
fn mono_pcm16_to_stereo(pcm: &[u8], balance: i8) -> Vec<u8> {
    let mut stereo = Vec::with_capacity(pcm.len().saturating_mul(2));
    for sample in pcm.as_chunks::<2>().0 {
        let value = i16::from_le_bytes([sample[0], sample[1]]);
        let [left, right] = mono_sample_to_stereo_frame(value, balance);
        stereo.extend_from_slice(&left.to_le_bytes());
        stereo.extend_from_slice(&right.to_le_bytes());
    }
    stereo
}

/// Fixed staging pool for XAudio2 buffers. Submitted allocations remain owned
/// by the worker until matching normal-playback completion notifications arrive.
#[cfg(any(windows, test))]
struct XAudio2SubmittedBuffers {
    available: VecDeque<Vec<StereoFrame>>,
    submitted: VecDeque<Vec<StereoFrame>>,
    reclaimed_completions: u64,
}

#[cfg(any(windows, test))]
impl XAudio2SubmittedBuffers {
    fn new(max_buffers: usize, frames_per_buffer: usize) -> Self {
        let mut available = VecDeque::with_capacity(max_buffers);
        for _ in 0..max_buffers {
            available.push_back(Vec::with_capacity(frames_per_buffer));
        }
        Self {
            available,
            submitted: VecDeque::with_capacity(max_buffers),
            reclaimed_completions: 0,
        }
    }

    fn stage_next(
        &mut self,
        consumer: &mut PcmFrameConsumer,
        max_frames: usize,
    ) -> Option<(*const u8, u32)> {
        let mut staging = self.available.pop_front()?;
        let frame_count = consumer
            .metrics()
            .queued_frames
            .min(max_frames)
            .min(staging.capacity());
        if frame_count == 0 {
            self.available.push_front(staging);
            return None;
        }
        let audio_bytes = frame_count
            .checked_mul(std::mem::size_of::<StereoFrame>())
            .and_then(|bytes| u32::try_from(bytes).ok());
        let Some(audio_bytes) = audio_bytes else {
            self.available.push_front(staging);
            return None;
        };

        staging.resize(frame_count, [0, 0]);
        let report = consumer.read_frames(&mut staging);
        debug_assert_eq!(report.queued_frames, frame_count);
        debug_assert_eq!(report.silence_frames, 0);
        self.submitted.push_back(staging);
        let submitted = self.submitted.back().expect("just staged XAudio2 frames");
        Some((submitted.as_ptr().cast::<u8>(), audio_bytes))
    }

    fn rollback_last_submission(&mut self) -> usize {
        let Some(mut staging) = self.submitted.pop_back() else {
            return 0;
        };
        let lost_frames = staging.len();
        staging.clear();
        self.available.push_back(staging);
        lost_frames
    }

    fn reclaim_completed(&mut self, completed_buffers: u64) -> usize {
        let completion_delta = completed_buffers.wrapping_sub(self.reclaimed_completions);
        let reclaim_count = completion_delta.min(self.submitted.len() as u64) as usize;
        for _ in 0..reclaim_count {
            if let Some(mut staging) = self.submitted.pop_front() {
                staging.clear();
                self.available.push_back(staging);
            }
        }
        self.reclaimed_completions = self
            .reclaimed_completions
            .wrapping_add(reclaim_count as u64);
        reclaim_count
    }

    fn submitted_len(&self) -> usize {
        self.submitted.len()
    }

    #[cfg(test)]
    fn available_len(&self) -> usize {
        self.available.len()
    }

    #[cfg(test)]
    fn seed_reclaimed_completions(&mut self, value: u64) {
        self.reclaimed_completions = value;
    }
}

#[cfg(any(windows, test))]
#[derive(Default)]
struct XAudio2CallbackSignals {
    completed_buffers: AtomicU64,
    voice_error: AtomicBool,
}

#[cfg(any(windows, test))]
impl XAudio2CallbackSignals {
    fn signal_buffer_end(&self) {
        self.completed_buffers.fetch_add(1, Ordering::Release);
    }

    fn signal_voice_error(&self) {
        self.voice_error.store(true, Ordering::Release);
    }
}

#[cfg(windows)]
struct BackendEpochControl {
    suspended: AtomicBool,
    flush_requested: AtomicBool,
    flush_acknowledged: AtomicBool,
    failed: AtomicBool,
}

#[cfg(windows)]
impl BackendEpochControl {
    fn new() -> Self {
        Self {
            suspended: AtomicBool::new(true),
            flush_requested: AtomicBool::new(false),
            flush_acknowledged: AtomicBool::new(true),
            failed: AtomicBool::new(false),
        }
    }
}

#[cfg(windows)]
#[derive(Clone, Copy)]
struct BackendConfig {
    sample_rate: u32,
    buffer_size: u32,
    queue_blocks: usize,
}

#[cfg(any(windows, test))]
fn wait_for_flush_acknowledgement(
    acknowledged: &AtomicBool,
    failed: &AtomicBool,
    shutdown: &AtomicBool,
    timeout: Duration,
) -> Result<(), AudioOutputError> {
    let deadline = std::time::Instant::now() + timeout;
    while !acknowledged.load(Ordering::Acquire) {
        if failed.load(Ordering::Acquire) {
            return Err(AudioOutputError::Device(
                "audio backend worker failed during an epoch transition".to_owned(),
            ));
        }
        if shutdown.load(Ordering::Acquire) {
            return Err(AudioOutputError::WorkerStopped);
        }
        if std::time::Instant::now() >= deadline {
            return Err(AudioOutputError::Device(
                "audio backend did not acknowledge the flush epoch within 5 seconds".to_owned(),
            ));
        }
        thread::sleep(Duration::from_millis(1));
    }
    if failed.load(Ordering::Acquire) {
        return Err(AudioOutputError::Device(
            "audio backend failed while invalidating the previous epoch".to_owned(),
        ));
    }
    Ok(())
}

#[cfg(any(windows, test))]
fn xaudio2_flush_barrier_reached(
    completed_before_flush: u64,
    completed_now: u64,
    pending_buffers: usize,
    buffers_queued: u32,
) -> bool {
    completed_now.wrapping_sub(completed_before_flush) >= pending_buffers as u64
        && buffers_queued == 0
}

/// Bounded producer queue and event-driven WASAPI consumer owned entirely by Rust.
pub struct NativeWasapiOutput {
    #[cfg(windows)]
    producer: Option<PcmFrameProducer>,
    #[cfg(windows)]
    worker: Option<JoinHandle<()>>,
    shutdown: Arc<AtomicBool>,
    underruns: Arc<AtomicU64>,
    dropped_blocks: Arc<AtomicU64>,
    sample_rate: u32,
    buffer_size: u32,
    #[cfg(windows)]
    balance: i8,
    #[cfg(windows)]
    control: Arc<BackendEpochControl>,
}

impl NativeWasapiOutput {
    pub fn new(
        sample_rate: u32,
        buffer_size: u32,
        queue_blocks: usize,
    ) -> Result<Self, AudioOutputError> {
        Self::new_with_balance(sample_rate, buffer_size, queue_blocks, 0)
    }

    pub fn new_with_balance(
        sample_rate: u32,
        buffer_size: u32,
        queue_blocks: usize,
        balance: i8,
    ) -> Result<Self, AudioOutputError> {
        sega_observability::tracing::info!(
            target: "sega::audio::backend",
            backend = "WASAPI",
            sample_rate,
            buffer_size,
            queue_blocks,
            balance,
            "native audio backend initialization requested"
        );
        if sample_rate == 0 {
            return Err(AudioOutputError::InvalidSampleRate);
        }
        if buffer_size == 0 || buffer_size > 16_384 {
            return Err(AudioOutputError::InvalidBufferSize);
        }
        if queue_blocks == 0 || queue_blocks > 64 {
            return Err(AudioOutputError::InvalidQueueCapacity);
        }
        if !(-100..=100).contains(&balance) {
            return Err(AudioOutputError::InvalidBalance);
        }

        #[cfg(windows)]
        {
            let capacity_frames = (buffer_size as usize) * queue_blocks;
            let (producer, consumer) = pcm_frame_queue(capacity_frames)
                .map_err(|error| AudioOutputError::Device(error.to_string()))?;
            let (ready_sender, ready_receiver) = mpsc::sync_channel(1);
            let shutdown = Arc::new(AtomicBool::new(false));
            let underruns = Arc::new(AtomicU64::new(0));
            let dropped_blocks = Arc::new(AtomicU64::new(0));
            let control = Arc::new(BackendEpochControl::new());
            let worker_shutdown = Arc::clone(&shutdown);
            let worker_underruns = Arc::clone(&underruns);
            let worker_control = Arc::clone(&control);
            let worker = thread::Builder::new()
                .name("sega-wasapi-output".to_owned())
                .spawn(move || {
                    run_wasapi_worker(
                        consumer,
                        ready_sender,
                        worker_shutdown,
                        worker_underruns,
                        worker_control,
                        BackendConfig {
                            sample_rate,
                            buffer_size,
                            queue_blocks,
                        },
                    );
                })
                .map_err(|error| AudioOutputError::Device(error.to_string()))?;

            match ready_receiver.recv_timeout(Duration::from_secs(5)) {
                Ok(Ok(())) => {
                    sega_observability::tracing::info!(
                        target: "sega::audio::backend",
                        backend = "WASAPI",
                        sample_rate,
                        buffer_size,
                        queue_blocks,
                        balance,
                        "WASAPI render endpoint initialized"
                    );
                    Ok(Self {
                    producer: Some(producer),
                    worker: Some(worker),
                    shutdown,
                    underruns,
                    dropped_blocks,
                    sample_rate,
                    buffer_size,
                    balance,
                    control,
                    })
                },
                Ok(Err(message)) => {
                    sega_observability::tracing::error!(
                        target: "sega::audio::backend",
                        backend = "WASAPI",
                        error = %message,
                        "WASAPI initialization failed"
                    );
                    shutdown.store(true, Ordering::Release);
                    drop(producer);
                    let _ = worker.join();
                    Err(AudioOutputError::Device(message))
                }
                Err(error) => {
                    sega_observability::tracing::error!(
                        target: "sega::audio::backend",
                        backend = "WASAPI",
                        error = %error,
                        "WASAPI initialization timed out"
                    );
                    shutdown.store(true, Ordering::Release);
                    drop(producer);
                    let _ = worker.join();
                    Err(AudioOutputError::Device(format!(
                        "audio worker initialization timed out: {error}"
                    )))
                }
            }
        }

        #[cfg(not(windows))]
        {
            let _ = queue_blocks;
            Err(AudioOutputError::Device(
                "native WASAPI output is available only on Windows".to_owned(),
            ))
        }
    }

    pub fn write_pcm(&mut self, pcm: Vec<u8>) -> Result<(), AudioOutputError> {
        if !pcm.len().is_multiple_of(2) || pcm.len() > (u32::MAX as usize / 2) {
            return Err(AudioOutputError::InvalidPcmBlock);
        }
        if pcm.is_empty() {
            return Ok(());
        }
        #[cfg(windows)]
        {
            if !backend_accepts_pcm(
                self.shutdown.load(Ordering::Acquire),
                self.control.failed.load(Ordering::Acquire),
                self.worker
                    .as_ref()
                    .is_none_or(|worker| worker.is_finished()),
            ) {
                return Err(AudioOutputError::WorkerStopped);
            }
            let producer = self
                .producer
                .as_mut()
                .ok_or(AudioOutputError::WorkerStopped)?;
            let dropped_frames = push_mono_pcm16_bounded(producer, &pcm, self.balance)
                .map_err(|_| AudioOutputError::WorkerStopped)?;
            if dropped_frames > 0 {
                let dropped_blocks = self.dropped_blocks.fetch_add(1, Ordering::Relaxed) + 1;
                if dropped_blocks.is_power_of_two() {
                    let metrics = producer.metrics();
                    sega_observability::tracing::warn!(
                        target: "sega::audio::backend",
                        backend = "WASAPI",
                        dropped_frames,
                        dropped_blocks,
                        queued_frames = metrics.queued_frames,
                        capacity_frames = metrics.capacity_frames,
                        high_water_mark_frames = metrics.high_water_mark_frames,
                        "WASAPI producer queue saturated; oldest frames discarded"
                    );
                }
            }
            Ok(())
        }
        #[cfg(not(windows))]
        {
            Err(AudioOutputError::Device(
                "native WASAPI output is available only on Windows".to_owned(),
            ))
        }
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub fn buffer_size(&self) -> u32 {
        self.buffer_size
    }

    /// Number of consumer reads that inserted one or more silence frames.
    pub fn underrun_count(&self) -> u64 {
        self.underruns.load(Ordering::Relaxed)
    }

    pub fn dropped_block_count(&self) -> u64 {
        self.dropped_blocks.load(Ordering::Relaxed)
    }

    pub fn suspend_and_flush(&mut self) -> Result<(), AudioOutputError> {
        #[cfg(windows)]
        {
            if self.shutdown.load(Ordering::Acquire) {
                return Err(AudioOutputError::WorkerStopped);
            }
            self.control.suspended.store(true, Ordering::Release);
            self.control
                .flush_acknowledged
                .store(false, Ordering::Release);
            self.control.flush_requested.store(true, Ordering::Release);
            wait_for_flush_acknowledgement(
                &self.control.flush_acknowledged,
                &self.control.failed,
                &self.shutdown,
                Duration::from_secs(5),
            )
        }
        #[cfg(not(windows))]
        {
            Err(AudioOutputError::WorkerStopped)
        }
    }

    pub fn resume(&mut self) -> Result<(), AudioOutputError> {
        #[cfg(windows)]
        {
            if self.shutdown.load(Ordering::Acquire) {
                return Err(AudioOutputError::WorkerStopped);
            }
            if self.control.failed.load(Ordering::Acquire) {
                return Err(AudioOutputError::Device(
                    "audio backend control is in a failed state".to_owned(),
                ));
            }
            self.control.suspended.store(false, Ordering::Release);
            Ok(())
        }
        #[cfg(not(windows))]
        {
            Err(AudioOutputError::WorkerStopped)
        }
    }

    pub fn close(&mut self) {
        self.shutdown.store(true, Ordering::Release);
        #[cfg(windows)]
        {
            drop(self.producer.take());
            if let Some(worker) = self.worker.take() {
                let _ = worker.join();
            }
        }
    }
}

impl Drop for NativeWasapiOutput {
    fn drop(&mut self) {
        self.close();
    }
}

/// Select an IAudioClient3 period in frames, respecting endpoint limits and alignment.
#[cfg(any(windows, test))]
fn select_shared_period(requested: u32, min: u32, max: u32, fundamental: u32) -> Option<u32> {
    if fundamental == 0 || min == 0 || min > max {
        return None;
    }
    let target = requested.max(1).clamp(min, max);
    let aligned = target.div_ceil(fundamental).saturating_mul(fundamental);
    let selected = if aligned <= max {
        aligned
    } else {
        (max / fundamental) * fundamental
    };
    (selected >= min && selected <= max).then_some(selected)
}

#[cfg(windows)]
struct ComApartment;

#[cfg(windows)]
impl Drop for ComApartment {
    fn drop(&mut self) {
        deinitialize();
    }
}

#[cfg(windows)]
fn run_wasapi_worker(
    mut consumer: PcmFrameConsumer,
    ready: mpsc::SyncSender<Result<(), String>>,
    shutdown: Arc<AtomicBool>,
    underruns: Arc<AtomicU64>,
    control: Arc<BackendEpochControl>,
    config: BackendConfig,
) {
    let sample_rate = config.sample_rate;
    let buffer_size = config.buffer_size;
    if let Err(error) = initialize_mta().ok() {
        sega_observability::tracing::error!(
            target: "sega::audio::backend",
            backend = "WASAPI",
            error = %error,
            "WASAPI COM apartment initialization failed"
        );
        let _ = ready.send(Err(error.to_string()));
        return;
    }
    let _com_apartment = ComApartment;
    let mut first_start = true;

    // Re-enumerate and reopen the default render endpoint after stream loss.
    // The first initialization failure is reported to the constructor; later
    // failures retry until shutdown or the producer closes the queue.
    loop {
        if shutdown.load(Ordering::Acquire) {
            break;
        }

        let setup = (|| {
            let enumerator = DeviceEnumerator::new().map_err(|error| error.to_string())?;
            let device = enumerator
                .get_default_device(&Direction::Render)
                .map_err(|error| error.to_string())?;
            let mut audio_client = device
                .get_iaudioclient()
                .map_err(|error| error.to_string())?;
            let format = WaveFormat::new(16, 16, &SampleType::Int, sample_rate as usize, 2, None);
            let block_align = format.get_blockalign() as usize;
            if block_align != 4 {
                return Err("WASAPI format must use stereo PCM16 (4 bytes per frame)".to_owned());
            }
            let (default_period, _minimum_period) = audio_client
                .get_device_period()
                .map_err(|error| error.to_string())?;

            let requested_frames = buffer_size.max(1);
            let selected_period = audio_client
                .get_shared_mode_engine_period(&format)
                .ok()
                .and_then(|periods| {
                    select_shared_period(
                        requested_frames,
                        periods.min,
                        periods.max,
                        periods.fundamental,
                    )
                });
            let initialized_with_client3 = selected_period.is_some_and(|period| {
                audio_client
                    .initialize_shared_audio_stream(&format, TimingMode::Events, period)
                    .is_ok()
            });
            if !initialized_with_client3 {
                let requested_period =
                    (buffer_size as u64 * 10_000_000 / sample_rate as u64) as i64;
                let mode = StreamMode::EventsShared {
                    autoconvert: true,
                    buffer_duration_hns: requested_period.max(default_period),
                };
                audio_client
                    .initialize_client(&format, &Direction::Render, &mode)
                    .map_err(|error| error.to_string())?;
            }
            let event = audio_client
                .set_get_eventhandle()
                .map_err(|error| error.to_string())?;
            let render_client = audio_client
                .get_audiorenderclient()
                .map_err(|error| error.to_string())?;
            let initial_buffer_frames = audio_client
                .get_buffer_size()
                .map_err(|error| error.to_string())?
                as usize;
            Ok((
                audio_client,
                event,
                render_client,
                block_align,
                initial_buffer_frames,
            ))
        })();

        let (audio_client, event, render_client, block_align, initial_buffer_frames) = match setup {
            Ok(state) => state,
            Err(message) => {
                if first_start {
                    sega_observability::tracing::error!(
                        target: "sega::audio::backend",
                        backend = "WASAPI",
                        error = %message,
                        "default render endpoint initialization failed"
                    );
                    let _ = ready.send(Err(message));
                    return;
                }
                sega_observability::tracing::warn!(
                    target: "sega::audio::backend",
                    backend = "WASAPI",
                    error = %message,
                    "render endpoint reconnect attempt failed"
                );
                thread::sleep(Duration::from_millis(250));
                continue;
            }
        };

        let staging_bytes = match initial_buffer_frames.checked_mul(block_align) {
            Some(bytes) => bytes,
            None => {
                if first_start {
                    let _ = ready.send(Err("WASAPI staging size overflow".to_owned()));
                    return;
                }
                thread::sleep(Duration::from_millis(250));
                continue;
            }
        };
        let prebuffer_frames = initial_buffer_frames
            .min(consumer.metrics().capacity_frames)
            .max(1);
        sega_observability::tracing::info!(
            target: "sega::audio::backend",
            backend = "WASAPI",
            endpoint = "default render",
            sample_rate,
            channels = 2,
            bits_per_sample = 16,
            block_align,
            initial_buffer_frames,
            prebuffer_frames,
            "WASAPI PCM16 stereo format negotiated"
        );
        let mut scratch_frames = vec![[0_i16; 2]; initial_buffer_frames];
        let mut pending = VecDeque::<u8>::with_capacity(staging_bytes);
        pending.resize(staging_bytes, 0);
        if let Err(error) =
            render_client.write_to_device_from_deque(initial_buffer_frames, &mut pending, None)
        {
            sega_observability::tracing::error!(
                target: "sega::audio::backend",
                backend = "WASAPI",
                error = %error,
                frames = initial_buffer_frames,
                "failed to prime the WASAPI render buffer"
            );
            if first_start {
                let _ = ready.send(Err("failed to prime the WASAPI render buffer".to_owned()));
                return;
            }
            thread::sleep(Duration::from_millis(250));
            continue;
        }
        pending.clear();
        if let Err(error) = audio_client.start_stream() {
            sega_observability::tracing::error!(
                target: "sega::audio::backend",
                backend = "WASAPI",
                error = %error,
                "WASAPI render stream failed to start"
            );
            if first_start {
                let _ = ready.send(Err(error.to_string()));
                return;
            }
            thread::sleep(Duration::from_millis(250));
            continue;
        }
        let mut stream_started = true;
        if first_start {
            let _ = ready.send(Ok(()));
            first_start = false;
        } else {
            sega_observability::tracing::info!(
                target: "sega::audio::backend",
                backend = "WASAPI",
                sample_rate,
                buffer_size,
                "WASAPI render endpoint reconnected"
            );
        }

        let mut disconnected = false;
        loop {
            if shutdown.load(Ordering::Acquire) {
                break;
            }

            if control.flush_requested.swap(false, Ordering::AcqRel) {
                let flush_result = (|| {
                    if stream_started {
                        audio_client
                            .stop_stream()
                            .map_err(|error| error.to_string())?;
                        audio_client
                            .reset_stream()
                            .map_err(|error| error.to_string())?;
                        stream_started = false;
                    }
                    consumer.discard_queued_frames();
                    Ok::<(), String>(())
                })();
                if let Err(error) = &flush_result {
                    sega_observability::tracing::error!(
                        target: "sega::audio::backend",
                        backend = "WASAPI",
                        error = %error,
                        "WASAPI suspend/flush operation failed"
                    );
                    control.failed.store(true, Ordering::Release);
                    disconnected = true;
                }
                control.flush_acknowledged.store(true, Ordering::Release);
                if flush_result.is_err() {
                    break;
                }
            }

            if control.suspended.load(Ordering::Acquire) {
                if stream_started {
                    let stopped = audio_client
                        .stop_stream()
                        .and_then(|()| audio_client.reset_stream());
                    if let Err(error) = stopped {
                        sega_observability::tracing::error!(
                            target: "sega::audio::backend",
                            backend = "WASAPI",
                            error = %error,
                            "WASAPI stream failed to stop/reset during suspend"
                        );
                        control.failed.store(true, Ordering::Release);
                        disconnected = true;
                        break;
                    }
                    stream_started = false;
                }
                thread::sleep(Duration::from_millis(2));
                continue;
            }

            if !stream_started {
                if consumer.metrics().queued_frames < prebuffer_frames {
                    thread::sleep(Duration::from_millis(2));
                    continue;
                }
                let prebuffer = &mut scratch_frames[..prebuffer_frames];
                let report = consumer.read_frames(prebuffer);
                if report.silence_frames > 0 {
                    record_audio_underrun(
                        &underruns,
                        "WASAPI",
                        report.silence_frames as u64,
                    );
                }
                pending.clear();
                for [left, right] in prebuffer.iter() {
                    pending.extend(left.to_le_bytes());
                    pending.extend(right.to_le_bytes());
                }
                if pending.len() != prebuffer_frames.saturating_mul(block_align) {
                    sega_observability::tracing::error!(
                        target: "sega::audio::backend",
                        backend = "WASAPI",
                        pending_bytes = pending.len(),
                        expected_bytes = prebuffer_frames.saturating_mul(block_align),
                        "WASAPI prebuffer byte count invariant violated"
                    );
                    disconnected = true;
                    break;
                }
                if let Err(error) =
                    render_client.write_to_device_from_deque(prebuffer_frames, &mut pending, None)
                {
                    sega_observability::tracing::error!(
                        target: "sega::audio::backend",
                        backend = "WASAPI",
                        error = %error,
                        frames = prebuffer_frames,
                        "WASAPI prebuffer submission failed"
                    );
                    disconnected = true;
                    break;
                }
                pending.clear();
                if let Err(error) = audio_client.start_stream() {
                    sega_observability::tracing::error!(
                        target: "sega::audio::backend",
                        backend = "WASAPI",
                        error = %error,
                        "WASAPI render stream failed to restart"
                    );
                    disconnected = true;
                    break;
                }
                stream_started = true;
                continue;
            }

            if event.wait_for_event(10).is_err() {
                // The short timeout is intentional: it bounds control-command
                // latency while retaining event-driven rendering. A failed
                // endpoint is detected by the next client-space query.
                if let Err(error) = audio_client.get_available_space_in_frames() {
                    sega_observability::tracing::warn!(
                        target: "sega::audio::backend",
                        backend = "WASAPI",
                        error = %error,
                        "WASAPI endpoint stopped responding after event timeout"
                    );
                    disconnected = true;
                    break;
                }
                continue;
            }

            let frames = match audio_client.get_available_space_in_frames() {
                Ok(frames) => frames as usize,
                Err(error) => {
                    sega_observability::tracing::warn!(
                        target: "sega::audio::backend",
                        backend = "WASAPI",
                        error = %error,
                        "WASAPI available-buffer query failed; endpoint will be reopened"
                    );
                    disconnected = true;
                    break;
                }
            };
            if frames == 0 {
                continue;
            }
            if frames > scratch_frames.len() {
                sega_observability::tracing::error!(
                    target: "sega::audio::backend",
                    backend = "WASAPI",
                    available_frames = frames,
                    scratch_capacity = scratch_frames.len(),
                    "WASAPI endpoint reported more frames than the staging buffer can hold"
                );
                disconnected = true;
                break;
            }
            let report = consumer.read_frames(&mut scratch_frames[..frames]);
            if report.silence_frames > 0 {
                record_audio_underrun(
                    &underruns,
                    "WASAPI",
                    report.silence_frames as u64,
                );
            }

            pending.clear();
            for [left, right] in &scratch_frames[..frames] {
                pending.extend(left.to_le_bytes());
                pending.extend(right.to_le_bytes());
            }
            if pending.len() != frames.saturating_mul(block_align) {
                sega_observability::tracing::error!(
                    target: "sega::audio::backend",
                    backend = "WASAPI",
                    pending_bytes = pending.len(),
                    expected_bytes = frames.saturating_mul(block_align),
                    "WASAPI render buffer byte count invariant violated"
                );
                disconnected = true;
                break;
            }
            if let Err(error) =
                render_client.write_to_device_from_deque(frames, &mut pending, None)
            {
                sega_observability::tracing::warn!(
                    target: "sega::audio::backend",
                    backend = "WASAPI",
                    error = %error,
                    frames,
                    "WASAPI render buffer submission failed; endpoint will be reopened"
                );
                disconnected = true;
                break;
            }
            sega_observability::tracing::trace!(
                target: "sega::audio::backend",
                backend = "WASAPI",
                submitted_frames = frames,
                consumed_frames = report.consumed_frames,
                silence_frames = report.silence_frames,
                queued_frames = consumer.metrics().queued_frames,
                "WASAPI render buffer submitted"
            );
            if report.producer_closed {
                disconnected = true;
                break;
            }
        }

        if stream_started {
            let _ = audio_client.stop_stream();
        }
        if shutdown.load(Ordering::Acquire)
            || (disconnected && control.failed.load(Ordering::Acquire))
        {
            break;
        }
        if disconnected {
            sega_observability::tracing::warn!(
                target: "sega::audio::backend",
                backend = "WASAPI",
                "WASAPI endpoint disconnected; retrying default render device"
            );
            thread::sleep(Duration::from_millis(250));
        }
    }
}

#[cfg(windows)]
use std::ffi::c_void;
#[cfg(windows)]
use windows::Win32::Media::Audio::XAudio2::{
    IXAudio2, IXAudio2MasteringVoice, IXAudio2SourceVoice, IXAudio2VoiceCallback,
    IXAudio2VoiceCallback_Impl, XAUDIO2_BUFFER, XAUDIO2_DEFAULT_PROCESSOR, XAUDIO2_VOICE_STATE,
    XAudio2CreateWithVersionInfo,
};
#[cfg(windows)]
use windows::Win32::Media::Audio::{AudioCategory_GameEffects, WAVE_FORMAT_PCM, WAVEFORMATEX};
#[cfg(windows)]
use windows::Win32::System::SystemInformation::NTDDI_WIN10_RS5;
#[cfg(windows)]
use windows::core::{HRESULT, PCWSTR};

/// Native XAudio2 output using Rust-owned source buffers and a bounded producer queue.
pub struct NativeXAudio2Output {
    #[cfg(windows)]
    producer: Option<PcmFrameProducer>,
    #[cfg(windows)]
    worker: Option<JoinHandle<()>>,
    shutdown: Arc<AtomicBool>,
    underruns: Arc<AtomicU64>,
    dropped_blocks: Arc<AtomicU64>,
    submission_failed_frames: Arc<AtomicU64>,
    sample_rate: u32,
    buffer_size: u32,
    #[cfg(windows)]
    balance: i8,
    #[cfg(windows)]
    control: Arc<BackendEpochControl>,
}

impl NativeXAudio2Output {
    pub fn new(
        sample_rate: u32,
        buffer_size: u32,
        queue_blocks: usize,
    ) -> Result<Self, AudioOutputError> {
        Self::new_with_balance(sample_rate, buffer_size, queue_blocks, 0)
    }

    pub fn new_with_balance(
        sample_rate: u32,
        buffer_size: u32,
        queue_blocks: usize,
        balance: i8,
    ) -> Result<Self, AudioOutputError> {
        sega_observability::tracing::info!(
            target: "sega::audio::backend",
            backend = "XAudio2",
            sample_rate,
            buffer_size,
            queue_blocks,
            balance,
            "native audio backend initialization requested"
        );
        if sample_rate == 0 {
            return Err(AudioOutputError::InvalidSampleRate);
        }
        if buffer_size == 0 || buffer_size > 16_384 {
            return Err(AudioOutputError::InvalidBufferSize);
        }
        if queue_blocks == 0 || queue_blocks > 64 {
            return Err(AudioOutputError::InvalidQueueCapacity);
        }
        if !(-100..=100).contains(&balance) {
            return Err(AudioOutputError::InvalidBalance);
        }

        #[cfg(windows)]
        {
            let capacity_frames = (buffer_size as usize) * queue_blocks;
            let (producer, consumer) = pcm_frame_queue(capacity_frames)
                .map_err(|error| AudioOutputError::Device(error.to_string()))?;
            let (ready_sender, ready_receiver) = mpsc::sync_channel(1);
            let shutdown = Arc::new(AtomicBool::new(false));
            let underruns = Arc::new(AtomicU64::new(0));
            let dropped_blocks = Arc::new(AtomicU64::new(0));
            let submission_failed_frames = Arc::new(AtomicU64::new(0));
            let control = Arc::new(BackendEpochControl::new());
            let worker_shutdown = Arc::clone(&shutdown);
            let worker_underruns = Arc::clone(&underruns);
            let worker_submission_failed_frames = Arc::clone(&submission_failed_frames);
            let worker_control = Arc::clone(&control);
            let worker = thread::Builder::new()
                .name("sega-xaudio2-output".to_owned())
                .spawn(move || {
                    run_xaudio2_worker(
                        consumer,
                        ready_sender,
                        worker_shutdown,
                        worker_underruns,
                        worker_submission_failed_frames,
                        worker_control,
                        BackendConfig {
                            sample_rate,
                            buffer_size,
                            queue_blocks,
                        },
                    );
                })
                .map_err(|error| AudioOutputError::Device(error.to_string()))?;

            match ready_receiver.recv_timeout(Duration::from_secs(5)) {
                Ok(Ok(())) => {
                    sega_observability::tracing::info!(
                        target: "sega::audio::backend",
                        backend = "XAudio2",
                        sample_rate,
                        buffer_size,
                        queue_blocks,
                        balance,
                        "XAudio2 mastering and source voices initialized"
                    );
                    Ok(Self {
                    producer: Some(producer),
                    worker: Some(worker),
                    shutdown,
                    underruns,
                    dropped_blocks,
                    submission_failed_frames,
                    sample_rate,
                    buffer_size,
                    balance,
                    control,
                    })
                },
                Ok(Err(message)) => {
                    sega_observability::tracing::error!(
                        target: "sega::audio::backend",
                        backend = "XAudio2",
                        error = %message,
                        "XAudio2 initialization failed"
                    );
                    shutdown.store(true, Ordering::Release);
                    drop(producer);
                    let _ = worker.join();
                    Err(AudioOutputError::Device(message))
                }
                Err(error) => {
                    sega_observability::tracing::error!(
                        target: "sega::audio::backend",
                        backend = "XAudio2",
                        error = %error,
                        "XAudio2 initialization timed out"
                    );
                    shutdown.store(true, Ordering::Release);
                    drop(producer);
                    let _ = worker.join();
                    Err(AudioOutputError::Device(format!(
                        "XAudio2 initialization timed out: {error}"
                    )))
                }
            }
        }

        #[cfg(not(windows))]
        {
            let _ = queue_blocks;
            Err(AudioOutputError::Device(
                "native XAudio2 output is available only on Windows".to_owned(),
            ))
        }
    }

    pub fn write_pcm(&mut self, pcm: Vec<u8>) -> Result<(), AudioOutputError> {
        if !pcm.len().is_multiple_of(2) || pcm.len() > (u32::MAX as usize / 2) {
            return Err(AudioOutputError::InvalidPcmBlock);
        }
        if pcm.is_empty() {
            return Ok(());
        }
        #[cfg(windows)]
        {
            if !backend_accepts_pcm(
                self.shutdown.load(Ordering::Acquire),
                self.control.failed.load(Ordering::Acquire),
                self.worker
                    .as_ref()
                    .is_none_or(|worker| worker.is_finished()),
            ) {
                return Err(AudioOutputError::WorkerStopped);
            }
            let producer = self
                .producer
                .as_mut()
                .ok_or(AudioOutputError::WorkerStopped)?;
            let dropped_frames = push_mono_pcm16_bounded(producer, &pcm, self.balance)
                .map_err(|_| AudioOutputError::WorkerStopped)?;
            if dropped_frames > 0 {
                let dropped_blocks = self.dropped_blocks.fetch_add(1, Ordering::Relaxed) + 1;
                if dropped_blocks.is_power_of_two() {
                    let metrics = producer.metrics();
                    sega_observability::tracing::warn!(
                        target: "sega::audio::backend",
                        backend = "XAudio2",
                        dropped_frames,
                        dropped_blocks,
                        queued_frames = metrics.queued_frames,
                        capacity_frames = metrics.capacity_frames,
                        high_water_mark_frames = metrics.high_water_mark_frames,
                        "XAudio2 producer queue saturated; oldest frames discarded"
                    );
                }
            }
            Ok(())
        }
        #[cfg(not(windows))]
        {
            Err(AudioOutputError::Device(
                "native XAudio2 output is available only on Windows".to_owned(),
            ))
        }
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub fn buffer_size(&self) -> u32 {
        self.buffer_size
    }

    /// Count transitions to XAudio2 source-voice starvation, not FIFO underflow.
    pub fn underrun_count(&self) -> u64 {
        self.underruns.load(Ordering::Relaxed)
    }

    pub fn dropped_block_count(&self) -> u64 {
        self.dropped_blocks.load(Ordering::Relaxed)
    }

    pub fn suspend_and_flush(&mut self) -> Result<(), AudioOutputError> {
        #[cfg(windows)]
        {
            if self.shutdown.load(Ordering::Acquire) {
                return Err(AudioOutputError::WorkerStopped);
            }
            self.control.suspended.store(true, Ordering::Release);
            self.control
                .flush_acknowledged
                .store(false, Ordering::Release);
            self.control.flush_requested.store(true, Ordering::Release);
            wait_for_flush_acknowledgement(
                &self.control.flush_acknowledged,
                &self.control.failed,
                &self.shutdown,
                Duration::from_secs(5),
            )
        }
        #[cfg(not(windows))]
        {
            Err(AudioOutputError::WorkerStopped)
        }
    }

    pub fn resume(&mut self) -> Result<(), AudioOutputError> {
        #[cfg(windows)]
        {
            if self.shutdown.load(Ordering::Acquire) {
                return Err(AudioOutputError::WorkerStopped);
            }
            if self.control.failed.load(Ordering::Acquire) {
                return Err(AudioOutputError::Device(
                    "audio backend control is in a failed state".to_owned(),
                ));
            }
            self.control.suspended.store(false, Ordering::Release);
            Ok(())
        }
        #[cfg(not(windows))]
        {
            Err(AudioOutputError::WorkerStopped)
        }
    }

    pub fn close(&mut self) {
        self.shutdown.store(true, Ordering::Release);
        #[cfg(windows)]
        {
            drop(self.producer.take());
            if let Some(worker) = self.worker.take() {
                let _ = worker.join();
            }
        }
    }
}

impl Drop for NativeXAudio2Output {
    fn drop(&mut self) {
        self.close();
    }
}

#[cfg(windows)]
struct XAudio2VoiceCallbackState {
    signals: Arc<XAudio2CallbackSignals>,
}

#[cfg(windows)]
#[allow(non_snake_case)]
impl IXAudio2VoiceCallback_Impl for XAudio2VoiceCallbackState {
    fn OnVoiceProcessingPassStart(&self, _bytesrequired: u32) {}
    fn OnVoiceProcessingPassEnd(&self) {}
    fn OnStreamEnd(&self) {}
    fn OnBufferStart(&self, _pbuffercontext: *mut c_void) {}
    fn OnBufferEnd(&self, _pbuffercontext: *mut c_void) {
        self.signals.signal_buffer_end();
    }
    fn OnLoopEnd(&self, _pbuffercontext: *mut c_void) {}
    fn OnVoiceError(&self, _pbuffercontext: *mut c_void, _error: HRESULT) {
        self.signals.signal_voice_error();
    }
}

#[cfg(windows)]
fn run_xaudio2_worker(
    mut consumer: PcmFrameConsumer,
    ready: mpsc::SyncSender<Result<(), String>>,
    shutdown: Arc<AtomicBool>,
    underruns: Arc<AtomicU64>,
    submission_failed_frames: Arc<AtomicU64>,
    control: Arc<BackendEpochControl>,
    config: BackendConfig,
) {
    let sample_rate = config.sample_rate;
    let frames_per_buffer = config.buffer_size as usize;
    let max_queued_buffers = config.queue_blocks;
    if let Err(error) = initialize_mta().ok() {
        sega_observability::tracing::error!(
            target: "sega::audio::backend",
            backend = "XAudio2",
            error = %error,
            "XAudio2 COM apartment initialization failed"
        );
        let _ = ready.send(Err(error.to_string()));
        return;
    }
    let _com_apartment = ComApartment;

    let signals = Arc::new(XAudio2CallbackSignals::default());
    let callback_state = XAudio2VoiceCallbackState {
        signals: Arc::clone(&signals),
    };
    // `new` builds the scoped COM interface adapter for this classic XAudio2
    // callback. Keep both the adapter and its borrowed state alive until after
    // DestroyVoice has completed.
    let callback = IXAudio2VoiceCallback::new(&callback_state);

    let setup = (|| {
        let mut engine: Option<IXAudio2> = None;
        unsafe {
            XAudio2CreateWithVersionInfo(
                &mut engine,
                0,
                XAUDIO2_DEFAULT_PROCESSOR,
                NTDDI_WIN10_RS5,
            )
            .map_err(|error| error.to_string())?;
        }
        let engine = engine.ok_or_else(|| "XAudio2Create returned no engine".to_owned())?;
        let mut mastering_voice: Option<IXAudio2MasteringVoice> = None;
        // The source voice is stereo PCM16. Keep the mastering voice stereo too;
        // declaring it mono forces a downmix and defeats the output balance control.
        unsafe {
            engine
                .CreateMasteringVoice(
                    &mut mastering_voice,
                    2,
                    sample_rate,
                    0,
                    PCWSTR::null(),
                    None,
                    AudioCategory_GameEffects,
                )
                .map_err(|error| error.to_string())?;
        }
        let mastering_voice =
            mastering_voice.ok_or_else(|| "XAudio2 did not create a mastering voice".to_owned())?;
        let format = WAVEFORMATEX {
            wFormatTag: WAVE_FORMAT_PCM as u16,
            nChannels: 2,
            nSamplesPerSec: sample_rate,
            nAvgBytesPerSec: sample_rate.saturating_mul(4),
            nBlockAlign: 4,
            wBitsPerSample: 16,
            cbSize: 0,
        };
        let mut source_voice: Option<IXAudio2SourceVoice> = None;
        let create_result = unsafe {
            engine.CreateSourceVoice(
                &mut source_voice,
                &format,
                0,
                2.0,
                Some(&*callback),
                None,
                None,
            )
        };
        if let Err(error) = create_result {
            unsafe { mastering_voice.DestroyVoice() };
            return Err(error.to_string());
        }
        let Some(source_voice) = source_voice else {
            unsafe { mastering_voice.DestroyVoice() };
            return Err("XAudio2 did not create a source voice".to_owned());
        };

        if let Err(error) = unsafe { engine.StartEngine() } {
            unsafe {
                source_voice.DestroyVoice();
                mastering_voice.DestroyVoice();
                engine.StopEngine();
            }
            return Err(error.to_string());
        }
        Ok((engine, mastering_voice, source_voice))
    })();

    let (engine, mastering_voice, source_voice) = match setup {
        Ok(state) => state,
        Err(message) => {
            sega_observability::tracing::error!(
                target: "sega::audio::backend",
                backend = "XAudio2",
                error = %message,
                "XAudio2 device and voice setup failed"
            );
            let _ = ready.send(Err(message));
            return;
        }
    };
    sega_observability::tracing::info!(
        target: "sega::audio::backend",
        backend = "XAudio2",
        sample_rate,
        channels = 2,
        bits_per_sample = 16,
        frames_per_buffer,
        max_queued_buffers,
        "XAudio2 PCM16 stereo format negotiated"
    );
    let _ = ready.send(Ok(()));

    let mut submitted = XAudio2SubmittedBuffers::new(max_queued_buffers, frames_per_buffer);
    let mut was_underrun = false;
    let mut voice_started = false;
    let mut voice_muted = false;
    loop {
        if shutdown.load(Ordering::Acquire) {
            break;
        }
        if signals.voice_error.load(Ordering::Acquire) || control.failed.load(Ordering::Acquire) {
            sega_observability::tracing::error!(
                target: "sega::audio::backend",
                backend = "XAudio2",
                callback_error = signals.voice_error.load(Ordering::Relaxed),
                "XAudio2 voice callback or backend control reported failure"
            );
            control.failed.store(true, Ordering::Release);
            break;
        }

        if control.flush_requested.swap(false, Ordering::AcqRel) {
            let completed_before_flush = signals.completed_buffers.load(Ordering::Acquire);
            submitted.reclaim_completed(completed_before_flush);
            let pending_before_flush = submitted.submitted_len();
            let flush_result = (|| {
                if voice_started {
                    // FlushSourceBuffers leaves the current buffer running.
                    // Mute first, then let its OnBufferEnd arrive normally;
                    // Stop would suppress voice callbacks and break the
                    // callback-based staging lifetime barrier.
                    unsafe { source_voice.SetVolume(0.0, 0) }.map_err(|error| error.to_string())?;
                    voice_muted = true;
                }
                unsafe { source_voice.FlushSourceBuffers() }.map_err(|error| error.to_string())?;

                let deadline = std::time::Instant::now() + Duration::from_secs(5);
                loop {
                    let completed = signals.completed_buffers.load(Ordering::Acquire);
                    let mut state = XAUDIO2_VOICE_STATE::default();
                    unsafe { source_voice.GetState(&mut state, 0) };
                    if xaudio2_flush_barrier_reached(
                        completed_before_flush,
                        completed,
                        pending_before_flush,
                        state.BuffersQueued,
                    ) {
                        submitted.reclaim_completed(completed);
                        break;
                    }
                    if std::time::Instant::now() >= deadline {
                        return Err("XAudio2 flush callback barrier timed out".to_owned());
                    }
                    thread::sleep(Duration::from_millis(1));
                }
                if submitted.submitted_len() != 0 {
                    return Err(
                        "XAudio2 flush left submitted staging buffers unreclaimed".to_owned()
                    );
                }
                consumer.discard_queued_frames();
                Ok::<(), String>(())
            })();
            if let Err(error) = &flush_result {
                sega_observability::tracing::error!(
                    target: "sega::audio::backend",
                    backend = "XAudio2",
                    error = %error,
                    "XAudio2 flush barrier failed"
                );
                control.failed.store(true, Ordering::Release);
            }
            control.flush_acknowledged.store(true, Ordering::Release);
            if flush_result.is_err() {
                break;
            }
        }

        if control.suspended.load(Ordering::Acquire) {
            thread::sleep(Duration::from_millis(2));
            continue;
        }

        if (!voice_started || voice_muted) && consumer.metrics().queued_frames < frames_per_buffer {
            thread::sleep(Duration::from_millis(2));
            continue;
        }

        if voice_muted {
            if let Err(error) = unsafe { source_voice.SetVolume(1.0, 0) } {
                sega_observability::tracing::error!(
                    target: "sega::audio::backend",
                    backend = "XAudio2",
                    error = %error,
                    "XAudio2 source voice could not be unmuted"
                );
                control.failed.store(true, Ordering::Release);
                break;
            }
            voice_muted = false;
        }

        let completed = signals.completed_buffers.load(Ordering::Acquire);
        submitted.reclaim_completed(completed);

        let mut submitted_new = false;
        while submitted.submitted_len() < max_queued_buffers {
            let Some((audio_data, audio_bytes)) =
                submitted.stage_next(&mut consumer, frames_per_buffer)
            else {
                break;
            };
            let buffer = XAUDIO2_BUFFER {
                AudioBytes: audio_bytes,
                pAudioData: audio_data,
                ..Default::default()
            };
            if let Err(error) = unsafe { source_voice.SubmitSourceBuffer(&buffer, None) } {
                let lost_frames = submitted.rollback_last_submission();
                let total_failed_frames =
                    submission_failed_frames.fetch_add(lost_frames as u64, Ordering::Relaxed)
                        + lost_frames as u64;
                sega_observability::tracing::error!(
                    target: "sega::audio::backend",
                    backend = "XAudio2",
                    lost_frames,
                    total_failed_frames,
                    error = %error,
                    "XAudio2 buffer submission failed"
                );
                control.failed.store(true, Ordering::Release);
                break;
            }
            submitted_new = true;
            sega_observability::tracing::trace!(
                target: "sega::audio::backend",
                backend = "XAudio2",
                submitted_frames = frames_per_buffer,
                submitted_bytes = audio_bytes,
                queued_buffers = submitted.submitted_len(),
                "XAudio2 source buffer submitted"
            );
        }

        if !voice_started && submitted_new {
            if let Err(error) = unsafe { source_voice.Start(0, 0) } {
                sega_observability::tracing::error!(
                    target: "sega::audio::backend",
                    backend = "XAudio2",
                    error = %error,
                    "XAudio2 source voice failed to start"
                );
                control.failed.store(true, Ordering::Release);
                break;
            }
            voice_started = true;
        }

        let mut current_state = XAUDIO2_VOICE_STATE::default();
        unsafe {
            source_voice.GetState(&mut current_state, 0);
        }
        let is_underrun = voice_started && current_state.BuffersQueued == 0 && !submitted_new;
        if is_underrun && !was_underrun {
            record_audio_underrun(&underruns, "XAudio2", 0);
        }
        was_underrun = is_underrun;
        thread::sleep(Duration::from_millis(2));
    }

    unsafe {
        // Keep callback and all submitted audio allocations alive through voice
        // destruction. Flush callbacks may complete out of submission order;
        // no per-buffer reclamation is attempted during shutdown.
        let _ = source_voice.Stop(0, 0);
        let _ = source_voice.FlushSourceBuffers();
        // DestroyVoice waits for the processing thread to become idle. Once it
        // returns, no callback can run and no submitted audio memory is accessed.
        source_voice.DestroyVoice();
        mastering_voice.DestroyVoice();
        engine.StopEngine();
    }
    drop(submitted);
    drop(callback);
    drop(callback_state);
}
// The output crate owns these concrete sink types, so it implements the
// runtime's sink contract here without introducing a dependency cycle.
impl sega_audio_runtime::PcmSink for NativeWasapiOutput {
    fn write_pcm(&mut self, pcm: Vec<u8>) -> Result<(), String> {
        NativeWasapiOutput::write_pcm(self, pcm).map_err(|error| error.to_string())
    }

    fn suspend_and_flush(&mut self) -> Result<(), String> {
        NativeWasapiOutput::suspend_and_flush(self).map_err(|error| error.to_string())
    }

    fn resume(&mut self) -> Result<(), String> {
        NativeWasapiOutput::resume(self).map_err(|error| error.to_string())
    }

    fn metrics(&self) -> sega_audio_runtime::AudioOutputMetrics {
        #[cfg(windows)]
        let queue = self
            .producer
            .as_ref()
            .map(PcmFrameProducer::metrics)
            .unwrap_or_default();
        #[cfg(not(windows))]
        let queue = PcmFrameQueueMetrics::default();

        audio_output_metrics_from_queue(self.sample_rate, queue, self.dropped_block_count(), 0, 0)
    }

    fn close(&mut self) {
        NativeWasapiOutput::close(self);
    }
}

impl NativeWasapiOutput {
    /// Compose this native endpoint with the Rust DSP/resampling session.
    pub fn into_audio_session(
        self,
        output_sample_rate: u32,
        volume: u8,
    ) -> Result<sega_audio_runtime::NativeAudioSession<Self>, String> {
        if self.sample_rate() != output_sample_rate {
            return Err("WASAPI device rate must match the DSP output rate".to_owned());
        }
        sega_audio_runtime::NativeAudioSession::new(output_sample_rate, volume, self)
    }
}

impl sega_audio_runtime::PcmSink for NativeXAudio2Output {
    fn write_pcm(&mut self, pcm: Vec<u8>) -> Result<(), String> {
        NativeXAudio2Output::write_pcm(self, pcm).map_err(|error| error.to_string())
    }

    fn suspend_and_flush(&mut self) -> Result<(), String> {
        NativeXAudio2Output::suspend_and_flush(self).map_err(|error| error.to_string())
    }

    fn resume(&mut self) -> Result<(), String> {
        NativeXAudio2Output::resume(self).map_err(|error| error.to_string())
    }

    fn metrics(&self) -> sega_audio_runtime::AudioOutputMetrics {
        #[cfg(windows)]
        let queue = self
            .producer
            .as_ref()
            .map(PcmFrameProducer::metrics)
            .unwrap_or_default();
        #[cfg(not(windows))]
        let queue = PcmFrameQueueMetrics::default();

        audio_output_metrics_from_queue(
            self.sample_rate,
            queue,
            self.dropped_block_count(),
            self.underrun_count(),
            self.submission_failed_frames.load(Ordering::Relaxed),
        )
    }

    fn close(&mut self) {
        NativeXAudio2Output::close(self);
    }
}

impl NativeXAudio2Output {
    /// Compose this native endpoint with the Rust DSP/resampling session.
    pub fn into_audio_session(
        self,
        output_sample_rate: u32,
        volume: u8,
    ) -> Result<sega_audio_runtime::NativeAudioSession<Self>, String> {
        if self.sample_rate() != output_sample_rate {
            return Err("XAudio2 device rate must match the DSP output rate".to_owned());
        }
        sega_audio_runtime::NativeAudioSession::new(output_sample_rate, volume, self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queue_metrics_are_mapped_without_changing_frame_units() {
        let queue = PcmFrameQueueMetrics {
            capacity_frames: 2_048,
            queued_frames: 480,
            high_water_frames: 960,
            accepted_frames: 12_000,
            dropped_frames: 7,
            consumed_frames: 11_513,
            underrun_frames: 32,
            underrun_events: 4,
            flushed_frames: 5,
        };
        let metrics = audio_output_metrics_from_queue(48_000, queue, 3, 7, 11);

        assert_eq!(metrics.sample_rate_hz, 48_000);
        assert_eq!(metrics.queue_capacity_frames, 2_048);
        assert_eq!(metrics.queue_queued_frames, 480);
        assert_eq!(metrics.queue_high_water_frames, 960);
        assert_eq!(metrics.accepted_frames, 12_000);
        assert_eq!(metrics.consumed_frames, 11_513);
        assert_eq!(metrics.dropped_frames, 7);
        assert_eq!(metrics.underrun_frames, 32);
        assert_eq!(metrics.underrun_events, 4);
        assert_eq!(metrics.dropped_blocks, 3);
        assert_eq!(metrics.flushed_frames, 5);
        assert_eq!(metrics.endpoint_starvation_events, 7);
        assert_eq!(metrics.submission_failed_frames, 11);
        assert_eq!(metrics.queue_duration_micros(), Some(10_000));
    }

    #[test]
    fn failed_or_stopped_backend_rejects_new_pcm_writes() {
        assert!(backend_accepts_pcm(false, false, false));
        assert!(!backend_accepts_pcm(true, false, false));
        assert!(!backend_accepts_pcm(false, true, false));
        assert!(!backend_accepts_pcm(false, false, true));
        assert!(!backend_accepts_pcm(true, true, true));
    }

    #[test]
    fn xaudio2_failed_submission_rollback_reports_lost_frame_count() {
        let (mut producer, mut consumer) = pcm_frame_queue(8).unwrap();
        producer.push_frames(&[[1, 2], [3, 4], [5, 6]]).unwrap();
        let mut staging = XAudio2SubmittedBuffers::new(1, 8);
        assert!(staging.stage_next(&mut consumer, 8).is_some());
        assert_eq!(staging.submitted_len(), 1);
        assert_eq!(staging.rollback_last_submission(), 3);
        assert_eq!(staging.submitted_len(), 0);
        assert_eq!(staging.available_len(), 1);
        assert_eq!(consumer.metrics().consumed_frames, 3);
        assert_eq!(consumer.metrics().queued_frames, 0);
    }

    #[test]
    fn flush_acknowledgement_reports_failure_and_timeout() {
        let shutdown = AtomicBool::new(false);
        let failed = AtomicBool::new(false);
        let acknowledged = AtomicBool::new(true);
        assert!(
            wait_for_flush_acknowledgement(
                &acknowledged,
                &failed,
                &shutdown,
                Duration::from_millis(1),
            )
            .is_ok()
        );

        failed.store(true, Ordering::Release);
        assert!(
            wait_for_flush_acknowledgement(
                &acknowledged,
                &failed,
                &shutdown,
                Duration::from_millis(1),
            )
            .is_err()
        );

        failed.store(false, Ordering::Release);
        acknowledged.store(false, Ordering::Release);
        assert!(
            wait_for_flush_acknowledgement(&acknowledged, &failed, &shutdown, Duration::ZERO,)
                .is_err()
        );
    }

    #[test]
    fn xaudio2_flush_barrier_uses_aggregate_callbacks_not_callback_order() {
        assert!(xaudio2_flush_barrier_reached(10, 13, 3, 0));
        assert!(!xaudio2_flush_barrier_reached(10, 12, 3, 0));
        assert!(!xaudio2_flush_barrier_reached(10, 13, 3, 1));
        assert!(xaudio2_flush_barrier_reached(u64::MAX - 1, 1, 3, 0));
    }

    #[test]
    fn iaudio_client3_period_is_clamped_and_aligned_to_fundamental() {
        assert_eq!(select_shared_period(512, 48, 448, 4), Some(448));
        assert_eq!(select_shared_period(49, 48, 448, 4), Some(52));
        assert_eq!(select_shared_period(1, 48, 448, 4), Some(48));
        assert_eq!(select_shared_period(512, 49, 50, 48), None);
        assert_eq!(select_shared_period(100, 0, 448, 4), None);
        assert_eq!(select_shared_period(100, 48, 448, 0), None);
    }

    #[test]
    fn wasapi_mono_conversion_produces_stereo_frames_and_preserves_extremes() {
        let mono = [0x00, 0x80, 0xff, 0x7f, 0x00, 0x00];
        assert_eq!(
            mono_pcm16_to_stereo_frames(&mono, 0),
            vec![[i16::MIN, i16::MIN], [i16::MAX, i16::MAX], [0, 0]]
        );
        assert_eq!(
            mono_pcm16_to_stereo_frames(&mono, 100),
            vec![[0, i16::MIN], [0, i16::MAX], [0, 0]]
        );
    }

    #[test]
    fn mono_pcm16_is_duplicated_to_left_and_right_stereo_channels() {
        let mono = [
            0x34, 0x12, // +0x1234
            0x00, 0x80, // -32768
            0xff, 0x7f, // +32767
        ];
        assert_eq!(
            mono_pcm16_to_stereo(&mono, 0),
            [
                0x34, 0x12, 0x34, 0x12, 0x00, 0x80, 0x00, 0x80, 0xff, 0x7f, 0xff, 0x7f,
            ]
        );
    }

    #[test]
    fn mono_pcm16_balance_routes_samples_to_both_channels() {
        let mono = [0x00, 0x40, 0x00, 0xc0];
        assert_eq!(
            mono_pcm16_to_stereo(&mono, 0),
            [0x00, 0x40, 0x00, 0x40, 0x00, 0xc0, 0x00, 0xc0]
        );
        assert_eq!(
            mono_pcm16_to_stereo(&mono, 100),
            [0x00, 0x00, 0x00, 0x40, 0x00, 0x00, 0x00, 0xc0]
        );
        assert_eq!(
            mono_pcm16_to_stereo(&mono, -100),
            [0x00, 0x40, 0x00, 0x00, 0x00, 0xc0, 0x00, 0x00]
        );
    }

    #[test]
    fn wasapi_rejects_invalid_configuration_before_device_access() {
        assert!(matches!(
            NativeWasapiOutput::new(0, 512, 4),
            Err(AudioOutputError::InvalidSampleRate)
        ));
        assert!(matches!(
            NativeWasapiOutput::new(44_100, 0, 4),
            Err(AudioOutputError::InvalidBufferSize)
        ));
        assert!(matches!(
            NativeWasapiOutput::new(44_100, 16_385, 4),
            Err(AudioOutputError::InvalidBufferSize)
        ));
        assert!(matches!(
            NativeWasapiOutput::new(44_100, 512, 0),
            Err(AudioOutputError::InvalidQueueCapacity)
        ));
        assert!(matches!(
            NativeWasapiOutput::new_with_balance(44_100, 512, 4, 101),
            Err(AudioOutputError::InvalidBalance)
        ));
    }

    #[test]
    fn xaudio2_rejects_invalid_configuration_before_device_access() {
        assert!(matches!(
            NativeXAudio2Output::new(0, 512, 4),
            Err(AudioOutputError::InvalidSampleRate)
        ));
        assert!(matches!(
            NativeXAudio2Output::new(44_100, 0, 4),
            Err(AudioOutputError::InvalidBufferSize)
        ));
        assert!(matches!(
            NativeXAudio2Output::new(44_100, 16_385, 4),
            Err(AudioOutputError::InvalidBufferSize)
        ));
        assert!(matches!(
            NativeXAudio2Output::new(44_100, 512, 0),
            Err(AudioOutputError::InvalidQueueCapacity)
        ));
        assert!(matches!(
            NativeXAudio2Output::new_with_balance(44_100, 512, 4, -101),
            Err(AudioOutputError::InvalidBalance)
        ));
    }

    #[test]
    fn bounded_pcm_conversion_preserves_samples_across_batch_boundary() {
        let (mut producer, mut consumer) = pcm_frame_queue(PCM_FRAME_BATCH_SIZE + 1).unwrap();
        let pcm: Vec<u8> = (0..=PCM_FRAME_BATCH_SIZE as i16)
            .flat_map(|sample| sample.to_le_bytes())
            .collect();

        assert_eq!(push_mono_pcm16_bounded(&mut producer, &pcm, 0), Ok(0));
        let mut output = vec![[0_i16; 2]; PCM_FRAME_BATCH_SIZE + 1];
        let report = consumer.read_frames(&mut output);
        assert_eq!(report.queued_frames, PCM_FRAME_BATCH_SIZE + 1);
        assert_eq!(report.silence_frames, 0);
        for (index, frame) in output.iter().enumerate() {
            let expected = index as i16;
            assert_eq!(*frame, [expected, expected]);
        }
    }

    #[test]
    fn bounded_pcm_conversion_counts_all_frames_rejected_by_small_fifo() {
        let (mut producer, mut consumer) = pcm_frame_queue(3).unwrap();
        let frame_count = PCM_FRAME_BATCH_SIZE * 2 + 1;
        let pcm: Vec<u8> = (0..frame_count as i16)
            .flat_map(|sample| sample.to_le_bytes())
            .collect();

        assert_eq!(
            push_mono_pcm16_bounded(&mut producer, &pcm, 0),
            Ok(frame_count - 3)
        );
        let mut output = [[0_i16; 2]; 3];
        assert_eq!(consumer.read_frames(&mut output).queued_frames, 3);
        assert_eq!(output, [[0, 0], [1, 1], [2, 2]]);
        assert_eq!(consumer.metrics().dropped_frames, (frame_count - 3) as u64);
    }

    #[test]
    fn xaudio2_staging_keeps_frame_memory_stable_until_callback_completion() {
        let (mut producer, mut consumer) = pcm_frame_queue(8).unwrap();
        producer.push_frames(&[[1, 2], [3, 4], [5, 6]]).unwrap();
        let mut staging = XAudio2SubmittedBuffers::new(2, 2);
        let (first_ptr, first_bytes) = staging.stage_next(&mut consumer, 2).unwrap();
        assert_eq!(first_bytes, 8);
        assert_eq!(staging.submitted_len(), 1);
        unsafe {
            assert_eq!(
                std::slice::from_raw_parts(first_ptr.cast::<i16>(), 4),
                &[1, 2, 3, 4]
            );
        }

        let (second_ptr, second_bytes) = staging.stage_next(&mut consumer, 2).unwrap();
        assert_eq!(second_bytes, 4);
        assert_eq!(staging.submitted_len(), 2);
        unsafe {
            assert_eq!(
                std::slice::from_raw_parts(first_ptr.cast::<i16>(), 4),
                &[1, 2, 3, 4]
            );
            assert_eq!(
                std::slice::from_raw_parts(second_ptr.cast::<i16>(), 2),
                &[5, 6]
            );
        }

        assert_eq!(staging.reclaim_completed(0), 0);
        assert_eq!(staging.submitted_len(), 2);
        assert_eq!(staging.reclaim_completed(2), 2);
        assert_eq!(staging.submitted_len(), 0);
        assert_eq!(staging.available_len(), 2);
    }

    #[test]
    fn xaudio2_submit_failure_rolls_back_staging_without_callback() {
        let (mut producer, mut consumer) = pcm_frame_queue(4).unwrap();
        producer.push_frames(&[[10, 20]]).unwrap();
        let mut staging = XAudio2SubmittedBuffers::new(1, 4);
        assert!(staging.stage_next(&mut consumer, 4).is_some());
        assert_eq!(staging.submitted_len(), 1);
        staging.rollback_last_submission();
        assert_eq!(staging.submitted_len(), 0);
        assert_eq!(staging.available_len(), 1);
    }

    #[test]
    fn xaudio2_completion_counter_handles_u64_wrapping() {
        let (mut producer, mut consumer) = pcm_frame_queue(2).unwrap();
        producer.push_frames(&[[7, 8]]).unwrap();
        let mut staging = XAudio2SubmittedBuffers::new(1, 2);
        staging.seed_reclaimed_completions(u64::MAX);
        assert!(staging.stage_next(&mut consumer, 2).is_some());
        assert_eq!(staging.reclaim_completed(0), 1);
        assert_eq!(staging.submitted_len(), 0);
    }

    #[test]
    fn xaudio2_callback_signals_survive_concurrent_worker_observation() {
        const COMPLETIONS: u64 = 50_000;
        let signals = Arc::new(XAudio2CallbackSignals::default());
        let callback_signals = Arc::clone(&signals);
        let worker_signals = Arc::clone(&signals);
        let callback = std::thread::spawn(move || {
            for _ in 0..COMPLETIONS {
                callback_signals.signal_buffer_end();
            }
        });
        let worker = std::thread::spawn(move || {
            let mut observed = 0;
            while observed < COMPLETIONS {
                observed = worker_signals.completed_buffers.load(Ordering::Acquire);
                std::thread::yield_now();
            }
            observed
        });
        callback.join().unwrap();
        assert_eq!(worker.join().unwrap(), COMPLETIONS);
        signals.signal_voice_error();
        assert!(signals.voice_error.load(Ordering::Acquire));
    }

    #[test]
    fn xaudio2_frame_bytes_match_interleaved_stereo_pcm16_layout() {
        let frames = mono_pcm16_to_stereo_frames(&[0x34, 0x12, 0x00, 0x80], 0);
        let mut bytes = Vec::new();
        for [left, right] in frames {
            bytes.extend_from_slice(&left.to_le_bytes());
            bytes.extend_from_slice(&right.to_le_bytes());
        }
        assert_eq!(bytes, [0x34, 0x12, 0x34, 0x12, 0x00, 0x80, 0x00, 0x80]);
    }
}
