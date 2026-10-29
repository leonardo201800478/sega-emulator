//! Native Windows audio output. Qt/Python owns configuration only.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
#[cfg(windows)]
use std::sync::mpsc;
#[cfg(windows)]
use std::thread::{self, JoinHandle};
#[cfg(windows)]
use std::time::Duration;

#[cfg(windows)]
use std::collections::VecDeque;
#[cfg(windows)]
use wasapi::{
    DeviceEnumerator, Direction, SampleType, StreamMode, WaveFormat, deinitialize, initialize_mta,
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

/// Expand mono signed PCM16 to interleaved stereo, duplicating each sample.
/// The emulated PSG remains mono; channel routing is handled only at the output boundary.
fn mono_pcm16_to_stereo(pcm: &[u8], balance: i8) -> Vec<u8> {
    let mut stereo = Vec::with_capacity(pcm.len().saturating_mul(2));
    let left_gain = if balance > 0 { 100 - balance as i32 } else { 100 };
    let right_gain = if balance < 0 { 100 + balance as i32 } else { 100 };
    for sample in pcm.chunks_exact(2) {
        let value = i16::from_le_bytes([sample[0], sample[1]]) as i32;
        let left = ((value * left_gain) / 100).clamp(i16::MIN as i32, i16::MAX as i32) as i16;
        let right = ((value * right_gain) / 100).clamp(i16::MIN as i32, i16::MAX as i32) as i16;
        stereo.extend_from_slice(&left.to_le_bytes());
        stereo.extend_from_slice(&right.to_le_bytes());
    }
    stereo
}

/// Bounded producer queue and event-driven WASAPI consumer owned entirely by Rust.
pub struct NativeWasapiOutput {
    #[cfg(windows)]
    producer: Option<rtrb::Producer<Vec<u8>>>,
    #[cfg(windows)]
    worker: Option<JoinHandle<()>>,
    shutdown: Arc<AtomicBool>,
    underruns: Arc<AtomicU64>,
    dropped_blocks: Arc<AtomicU64>,
    sample_rate: u32,
    buffer_size: u32,
    balance: i8,
    balance: i8,
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
        if sample_rate == 0 {
            return Err(AudioOutputError::InvalidSampleRate);
        }
        if buffer_size == 0 || buffer_size > 16_384 {
            return Err(AudioOutputError::InvalidBufferSize);
        }
        if queue_blocks == 0 || queue_blocks > 64 {
            return Err(AudioOutputError::InvalidQueueCapacity);
        }

        #[cfg(windows)]
        {
            let (producer, consumer) = rtrb::RingBuffer::<Vec<u8>>::new(queue_blocks);
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
                        consumer,
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
                    producer: Some(producer),
                    worker: Some(worker),
                    shutdown,
                    underruns,
                    dropped_blocks,
                    sample_rate,
                    buffer_size,
                    balance,
                }),
                Ok(Err(message)) => {
                    shutdown.store(true, Ordering::Release);
                    drop(producer);
                    let _ = worker.join();
                    Err(AudioOutputError::Device(message))
                }
                Err(error) => {
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
        let pcm = mono_pcm16_to_stereo(&pcm, self.balance);
        #[cfg(windows)]
        {
            if self.shutdown.load(Ordering::Acquire) {
                return Err(AudioOutputError::WorkerStopped);
            }
            let producer = self
                .producer
                .as_mut()
                .ok_or(AudioOutputError::WorkerStopped)?;
            if producer.is_abandoned() {
                return Err(AudioOutputError::WorkerStopped);
            }
            match producer.push(pcm) {
                Ok(()) => Ok(()),
                Err(rtrb::PushError::Full(_block)) => {
                    self.dropped_blocks.fetch_add(1, Ordering::Relaxed);
                    Ok(())
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
    mut consumer: rtrb::Consumer<Vec<u8>>,
    ready: mpsc::SyncSender<Result<(), String>>,
    shutdown: Arc<AtomicBool>,
    underruns: Arc<AtomicU64>,
    sample_rate: u32,
    buffer_size: u32,
) {
    if let Err(error) = initialize_mta().ok() {
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
            let (default_period, _minimum_period) = audio_client
                .get_device_period()
                .map_err(|error| error.to_string())?;
            let requested_period = (buffer_size as u64 * 10_000_000 / sample_rate as u64) as i64;
            let mode = StreamMode::EventsShared {
                autoconvert: true,
                buffer_duration_hns: requested_period.max(default_period),
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
                    let _ = ready.send(Err(message));
                    return;
                }
                thread::sleep(Duration::from_millis(250));
                continue;
            }
        };

        let mut pending = VecDeque::<u8>::with_capacity(buffer_size as usize * block_align * 8);
        pending.resize(initial_buffer_frames.saturating_mul(block_align), 0);
        if render_client
            .write_to_device_from_deque(initial_buffer_frames, &mut pending, None)
            .is_err()
        {
            if first_start {
                let _ = ready.send(Err("failed to prime the WASAPI render buffer".to_owned()));
                return;
            }
            thread::sleep(Duration::from_millis(250));
            continue;
        }
        if let Err(error) = audio_client.start_stream() {
            if first_start {
                let _ = ready.send(Err(error.to_string()));
                return;
            }
            thread::sleep(Duration::from_millis(250));
            continue;
        }
        if first_start {
            let _ = ready.send(Ok(()));
            first_start = false;
        }

        let mut disconnected = false;
        loop {
            if shutdown.load(Ordering::Acquire) {
                break;
            }
            if event.wait_for_event(1000).is_err() {
                // A failed event wait usually means that the endpoint stopped.
                break;
            }

            let frames = match audio_client.get_available_space_in_frames() {
                Ok(frames) => frames as usize,
                Err(_) => break,
            };
            if frames == 0 {
                continue;
            }
            let required = frames.saturating_mul(block_align);
            while pending.len() < required {
                match consumer.pop() {
                    Ok(block) => pending.extend(block),
                    Err(rtrb::PopError::Empty) => {
                        if consumer.is_abandoned() {
                            disconnected = true;
                            break;
                        }
                        pending.resize(required, 0);
                        underruns.fetch_add(1, Ordering::Relaxed);
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
        if shutdown.load(Ordering::Acquire) || disconnected {
            break;
        }
        thread::sleep(Duration::from_millis(250));
    }
}

#[cfg(windows)]
use windows::Win32::Media::Audio::XAudio2::{
    IXAudio2, IXAudio2MasteringVoice, IXAudio2SourceVoice, IXAudio2VoiceCallback, XAUDIO2_BUFFER,
    XAUDIO2_DEFAULT_PROCESSOR, XAUDIO2_VOICE_STATE, XAudio2CreateWithVersionInfo,
};
#[cfg(windows)]
use windows::Win32::Media::Audio::{AudioCategory_GameEffects, WAVE_FORMAT_PCM, WAVEFORMATEX};
#[cfg(windows)]
use windows::Win32::System::SystemInformation::NTDDI_WIN10_RS5;
#[cfg(windows)]
use windows::core::PCWSTR;

/// Native XAudio2 output using Rust-owned source buffers and a bounded producer queue.
pub struct NativeXAudio2Output {
    #[cfg(windows)]
    producer: Option<rtrb::Producer<Vec<u8>>>,
    #[cfg(windows)]
    worker: Option<JoinHandle<()>>,
    shutdown: Arc<AtomicBool>,
    underruns: Arc<AtomicU64>,
    dropped_blocks: Arc<AtomicU64>,
    sample_rate: u32,
    buffer_size: u32,
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
        if sample_rate == 0 {
            return Err(AudioOutputError::InvalidSampleRate);
        }
        if buffer_size == 0 || buffer_size > 16_384 {
            return Err(AudioOutputError::InvalidBufferSize);
        }
        if queue_blocks == 0 || queue_blocks > 64 {
            return Err(AudioOutputError::InvalidQueueCapacity);
        }

        #[cfg(windows)]
        {
            let (producer, consumer) = rtrb::RingBuffer::<Vec<u8>>::new(queue_blocks);
            let (ready_sender, ready_receiver) = mpsc::sync_channel(1);
            let shutdown = Arc::new(AtomicBool::new(false));
            let underruns = Arc::new(AtomicU64::new(0));
            let dropped_blocks = Arc::new(AtomicU64::new(0));
            let worker_shutdown = Arc::clone(&shutdown);
            let worker_underruns = Arc::clone(&underruns);
            let worker = thread::Builder::new()
                .name("sega-xaudio2-output".to_owned())
                .spawn(move || {
                    run_xaudio2_worker(
                        consumer,
                        ready_sender,
                        worker_shutdown,
                        worker_underruns,
                        sample_rate,
                        queue_blocks,
                    );
                })
                .map_err(|error| AudioOutputError::Device(error.to_string()))?;

            match ready_receiver.recv_timeout(Duration::from_secs(5)) {
                Ok(Ok(())) => Ok(Self {
                    producer: Some(producer),
                    worker: Some(worker),
                    shutdown,
                    underruns,
                    dropped_blocks,
                    sample_rate,
                    buffer_size,
                    balance,
                }),
                Ok(Err(message)) => {
                    shutdown.store(true, Ordering::Release);
                    drop(producer);
                    let _ = worker.join();
                    Err(AudioOutputError::Device(message))
                }
                Err(error) => {
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
        let pcm = mono_pcm16_to_stereo(&pcm, self.balance);
        #[cfg(windows)]
        {
            if self.shutdown.load(Ordering::Acquire) {
                return Err(AudioOutputError::WorkerStopped);
            }
            let producer = self
                .producer
                .as_mut()
                .ok_or(AudioOutputError::WorkerStopped)?;
            let max_block_bytes = (self.buffer_size as usize).saturating_mul(4).max(4);
            for block in pcm.chunks(max_block_bytes) {
                if producer.is_abandoned() {
                    return Err(AudioOutputError::WorkerStopped);
                }
                match producer.push(block.to_vec()) {
                    Ok(()) => {}
                    Err(rtrb::PushError::Full(_block)) => {
                        self.dropped_blocks.fetch_add(1, Ordering::Relaxed);
                    }
                }
            }
            Ok(())
        }
        #[cfg(not(windows))]
        {
            let _ = pcm;
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

    pub fn underrun_count(&self) -> u64 {
        self.underruns.load(Ordering::Relaxed)
    }

    pub fn dropped_block_count(&self) -> u64 {
        self.dropped_blocks.load(Ordering::Relaxed)
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
fn run_xaudio2_worker(
    mut consumer: rtrb::Consumer<Vec<u8>>,
    ready: mpsc::SyncSender<Result<(), String>>,
    shutdown: Arc<AtomicBool>,
    underruns: Arc<AtomicU64>,
    sample_rate: u32,
    max_queued_buffers: usize,
) {
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
        unsafe {
            engine
                .CreateMasteringVoice(
                    &mut mastering_voice,
                    1,
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
        unsafe {
            engine
                .CreateSourceVoice(
                    &mut source_voice,
                    &format,
                    0,
                    2.0,
                    None::<&IXAudio2VoiceCallback>,
                    None,
                    None,
                )
                .map_err(|error| error.to_string())?;
            engine.StartEngine().map_err(|error| error.to_string())?;
        }
        let source_voice =
            source_voice.ok_or_else(|| "XAudio2 did not create a source voice".to_owned())?;
        unsafe {
            source_voice
                .Start(0, 0)
                .map_err(|error| error.to_string())?;
        }
        Ok((engine, mastering_voice, source_voice))
    })();

    let (engine, _mastering_voice, source_voice) = match setup {
        Ok(state) => state,
        Err(message) => {
            let _ = ready.send(Err(message));
            return;
        }
    };
    let _ = ready.send(Ok(()));

    let mut submitted = VecDeque::<Vec<u8>>::with_capacity(max_queued_buffers);
    let mut was_underrun = false;
    loop {
        if shutdown.load(Ordering::Acquire) {
            break;
        }

        let mut state = XAUDIO2_VOICE_STATE::default();
        unsafe {
            source_voice.GetState(&mut state, 0);
        }
        while submitted.len() > state.BuffersQueued as usize {
            submitted.pop_front();
        }

        let mut submitted_new = false;
        while submitted.len() < max_queued_buffers {
            match consumer.pop() {
                Ok(block) => {
                    submitted.push_back(block);
                    let block = submitted.back().expect("just inserted XAudio2 block");
                    let buffer = XAUDIO2_BUFFER {
                        AudioBytes: block.len() as u32,
                        pAudioData: block.as_ptr(),
                        ..Default::default()
                    };
                    let result = unsafe { source_voice.SubmitSourceBuffer(&buffer, None) };
                    if result.is_err() {
                        submitted.pop_back();
                        break;
                    }
                    submitted_new = true;
                }
                Err(_) => break,
            }
        }

        let mut current_state = XAUDIO2_VOICE_STATE::default();
        unsafe {
            source_voice.GetState(&mut current_state, 0);
        }
        let is_underrun = current_state.BuffersQueued == 0 && !submitted_new;
        if is_underrun && !was_underrun {
            underruns.fetch_add(1, Ordering::Relaxed);
        }
        was_underrun = is_underrun;
        thread::sleep(Duration::from_millis(2));
    }

    unsafe {
        let _ = source_voice.Stop(0, 0);
        let _ = source_voice.FlushSourceBuffers();
        source_voice.DestroyVoice();
        engine.StopEngine();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mono_pcm16_is_duplicated_to_left_and_right_stereo_channels() {
        let mono = [
            0x34, 0x12, // +0x1234
            0x00, 0x80, // -32768
            0xff, 0x7f, // +32767
        ];
        assert_eq!(
            mono_pcm16_to_stereo(&mono),
            [
                0x34, 0x12, 0x34, 0x12,
                0x00, 0x80, 0x00, 0x80,
                0xff, 0x7f, 0xff, 0x7f,
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
    }
}
