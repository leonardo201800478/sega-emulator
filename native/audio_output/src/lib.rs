//! Native Windows audio output. Qt/Python owns configuration only.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, mpsc};
use std::thread::{self, JoinHandle};
use std::time::Duration;

#[cfg(windows)]
use std::collections::VecDeque;
#[cfg(windows)]
use wasapi::{
    DeviceEnumerator, Direction, SampleType, StreamMode, WaveFormat, initialize_mta,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AudioOutputError {
    InvalidSampleRate,
    InvalidBufferSize,
    InvalidQueueCapacity,
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
            Self::InvalidPcmBlock => write!(f, "PCM16 blocks must contain an even byte count"),
            Self::Device(message) => write!(f, "WASAPI output failed: {message}"),
            Self::WorkerStopped => write!(f, "WASAPI output worker has stopped"),
        }
    }
}

impl std::error::Error for AudioOutputError {}

/// Bounded producer queue and event-driven WASAPI consumer owned entirely by Rust.
pub struct NativeWasapiOutput {
    #[cfg(windows)]
    sender: Option<mpsc::SyncSender<Vec<u8>>>,
    #[cfg(windows)]
    worker: Option<JoinHandle<()>>,
    shutdown: Arc<AtomicBool>,
    underruns: Arc<AtomicU64>,
    dropped_blocks: Arc<AtomicU64>,
    sample_rate: u32,
    buffer_size: u32,
}

impl NativeWasapiOutput {
    pub fn new(
        sample_rate: u32,
        buffer_size: u32,
        queue_blocks: usize,
    ) -> Result<Self, AudioOutputError> {
        if sample_rate == 0 {
            return Err(AudioOutputError::InvalidSampleRate);
        }
        if buffer_size == 0 {
            return Err(AudioOutputError::InvalidBufferSize);
        }
        if queue_blocks == 0 {
            return Err(AudioOutputError::InvalidQueueCapacity);
        }

        #[cfg(windows)]
        {
            let (sender, receiver) = mpsc::sync_channel(queue_blocks);
            let (ready_sender, ready_receiver) = mpsc::sync_channel(1);
            let shutdown = Arc::new(AtomicBool::new(false));
            let underruns = Arc::new(AtomicU64::new(0));
            let dropped_blocks = Arc::new(AtomicU64::new(0));
            let worker_shutdown = Arc::clone(&shutdown);
            let worker_underruns = Arc::clone(&underruns);
            let worker = thread::Builder::new()
                .name("sega-wasapi-output".to_owned())
                .spawn(move || {
                    run_wasapi_worker(
                        receiver,
                        ready_sender,
                        worker_shutdown,
                        worker_underruns,
                        sample_rate,
                        buffer_size,
                    );
                })
                .map_err(|error| AudioOutputError::Device(error.to_string()))?;

            match ready_receiver.recv_timeout(Duration::from_secs(5)) {
                Ok(Ok(())) => Ok(Self {
                    sender: Some(sender),
                    worker: Some(worker),
                    shutdown,
                    underruns,
                    dropped_blocks,
                    sample_rate,
                    buffer_size,
                }),
                Ok(Err(message)) => {
                    shutdown.store(true, Ordering::Release);
                    drop(sender);
                    let _ = worker.join();
                    Err(AudioOutputError::Device(message))
                }
                Err(error) => {
                    shutdown.store(true, Ordering::Release);
                    drop(sender);
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

    pub fn write_pcm(&self, pcm: Vec<u8>) -> Result<(), AudioOutputError> {
        if pcm.len() % 2 != 0 {
            return Err(AudioOutputError::InvalidPcmBlock);
        }
        if pcm.is_empty() {
            return Ok(());
        }
        #[cfg(windows)]
        {
            if self.shutdown.load(Ordering::Acquire) {
                return Err(AudioOutputError::WorkerStopped);
            }
            let sender = self.sender.as_ref().ok_or(AudioOutputError::WorkerStopped)?;
            match sender.try_send(pcm) {
                Ok(()) => Ok(()),
                Err(mpsc::TrySendError::Full(_block)) => {
                    self.dropped_blocks.fetch_add(1, Ordering::Relaxed);
                    Ok(())
                }
                Err(mpsc::TrySendError::Disconnected(_block)) => {
                    Err(AudioOutputError::WorkerStopped)
                }
            }
        }
        #[cfg(not(windows))]
        {
            let _ = pcm;
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

    pub fn underrun_count(&self) -> u64 {
        self.underruns.load(Ordering::Relaxed)
    }

    pub fn dropped_block_count(&self) -> u64 {
        self.dropped_blocks.load(Ordering::Relaxed)
    }

    pub fn close(&mut self) {
        if self.shutdown.swap(true, Ordering::AcqRel) {
            return;
        }
        #[cfg(windows)]
        {
            self.sender.take();
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

#[cfg(windows)]
fn run_wasapi_worker(
    receiver: mpsc::Receiver<Vec<u8>>,
    ready: mpsc::SyncSender<Result<(), String>>,
    shutdown: Arc<AtomicBool>,
    underruns: Arc<AtomicU64>,
    sample_rate: u32,
    buffer_size: u32,
) {
    let setup = (|| -> Result<_, String> {
        initialize_mta().ok().map_err(|error| error.to_string())?;
        let enumerator = DeviceEnumerator::new().map_err(|error| error.to_string())?;
        let device = enumerator
            .get_default_device(&Direction::Render)
            .map_err(|error| error.to_string())?;
        let mut audio_client = device
            .get_iaudioclient()
            .map_err(|error| error.to_string())?;
        let format = WaveFormat::new(
            16,
            16,
            &SampleType::Int,
            sample_rate,
            1,
            None,
        );
        let block_align = format.get_blockalign() as usize;
        let (default_period, _) = audio_client
            .get_device_period()
            .map_err(|error| error.to_string())?;
        let mode = StreamMode::EventsShared {
            autoconvert: true,
            buffer_duration_hns: default_period,
        };
        audio_client
            .initialize_client(&format, &Direction::Render, &mode)
            .map_err(|error| error.to_string())?;
        let event = audio_client
            .set_get_eventhandle()
            .map_err(|error| error.to_string())?;
        let render_client = audio_client
            .get_audiorenderclient()
            .map_err(|error| error.to_string())?;
        audio_client
            .start_stream()
            .map_err(|error| error.to_string())?;
        Ok((audio_client, event, render_client, block_align))
    })();

    let (mut audio_client, event, render_client, block_align) = match setup {
        Ok(state) => {
            let _ = ready.send(Ok(()));
            state
        }
        Err(message) => {
            let _ = ready.send(Err(message));
            return;
        }
    };

    let mut pending = VecDeque::<u8>::with_capacity(buffer_size as usize * block_align * 8);
    loop {
        if shutdown.load(Ordering::Acquire) {
            break;
        }
        if event.wait_for_event(1000).is_err() {
            if shutdown.load(Ordering::Acquire) {
                break;
            }
            underruns.fetch_add(1, Ordering::Relaxed);
            continue;
        }

        let frames = match audio_client.get_available_space_in_frames() {
            Ok(frames) => frames as usize,
            Err(_) => break,
        };
        if frames == 0 {
            continue;
        }
        let required = frames.saturating_mul(block_align);
        let mut disconnected = false;
        while pending.len() < required {
            match receiver.try_recv() {
                Ok(block) => pending.extend(block),
                Err(mpsc::TryRecvError::Empty) => {
                    pending.resize(required, 0);
                    underruns.fetch_add(1, Ordering::Relaxed);
                    break;
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    disconnected = true;
                    break;
                }
            }
        }
        if disconnected || shutdown.load(Ordering::Acquire) {
            break;
        }
        if render_client
            .write_to_device_from_deque(frames, &mut pending, None)
            .is_err()
        {
            break;
        }
    }
    let _ = audio_client.stop_stream();
}
