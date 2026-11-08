//! Rust-owned continuous execution worker for the temporal SMS machine.
//!
//! The worker constructs and owns the complete hardware machine on its own
//! thread. The host sends only coarse-grained commands; CPU, Scheduler, VDP, PSG,
//! audio pacing and state transitions remain inside Rust.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender, SyncSender};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crate::audio::AudioSession;
use sega_sms_core::{
    ControllerButtons, SmsTemporalMachine, SmsVdpProfile, SmsVdpRevision, SpriteLimitMode,
    Territory, VideoStandard,
};
#[cfg(windows)]
use sega_video_runtime::d3d12::D3d12MailboxSession;
use sega_video_runtime::{FrameMailbox, PixelFormat, VideoFrame};

const SMS_TEMPORAL_SAMPLE_RATE: u64 = 44_100;
const SMS_TEMPORAL_MASTER_TICKS_PER_TSTATE: u64 = 1;
const DEFAULT_MAX_FRAME_INSTRUCTIONS: usize = 100_000;

pub fn sms_temporal_region_profile(
    region: &str,
) -> Result<(Territory, SmsVdpProfile, u64), &'static str> {
    let sms1 = SmsVdpRevision::Sega3155124;
    let sprite_limit = SpriteLimitMode::HardwareLimit;
    match region {
        "usa" => Ok((Territory::Export, SmsVdpProfile::sms1_t0_ntsc(), 3_579_545)),
        "brazil" => Ok((
            Territory::Export,
            SmsVdpProfile::new(sms1, VideoStandard::PalM, sprite_limit),
            3_579_545,
        )),
        "europe" => Ok((
            Territory::Export,
            SmsVdpProfile::new(sms1, VideoStandard::Pal, sprite_limit),
            3_546_895,
        )),
        "japan" => Ok((
            Territory::Japan,
            SmsVdpProfile::new(sms1, VideoStandard::Ntsc, sprite_limit),
            3_579_545,
        )),
        _ => Err("region deve ser usa, brazil, europe ou japan"),
    }
}

fn build_sms_temporal_machine(rom: Vec<u8>, region: &str) -> Result<SmsTemporalMachine, String> {
    let (territory, vdp_profile, psg_clock_hz) =
        sms_temporal_region_profile(region).map_err(str::to_owned)?;
    SmsTemporalMachine::new_with_hardware_profile(
        rom,
        SMS_TEMPORAL_MASTER_TICKS_PER_TSTATE,
        psg_clock_hz,
        SMS_TEMPORAL_SAMPLE_RATE,
        vdp_profile,
        territory,
    )
    .map_err(|error| error.to_string())
}

#[derive(Debug, Default)]
struct WorkerTelemetry {
    running: AtomicBool,
    frame_number: AtomicU64,
    total_instructions: AtomicU64,
    audio_underruns: AtomicU64,
    audio_dropped_blocks: AtomicU64,
    audio_output_error: Mutex<Option<String>>,
    video_output_error: Mutex<Option<String>>,
    video_last_published_sequence: AtomicU64,
    video_replaced_frames: AtomicU64,
    runtime_error: Mutex<Option<String>>,
    frame_signal: (Mutex<u64>, Condvar),
}

impl WorkerTelemetry {
    fn set_frame_number(&self, frame_number: u64) {
        self.frame_number.store(frame_number, Ordering::Release);
        if let Ok(mut observed) = self.frame_signal.0.lock() {
            *observed = frame_number;
            self.frame_signal.1.notify_all();
        }
    }

    fn set_audio_error(&self, error: Option<String>) {
        if let Ok(mut current) = self.audio_output_error.lock() {
            *current = error;
        }
    }

    fn set_video_error(&self, error: Option<String>) {
        if let Ok(mut current) = self.video_output_error.lock() {
            *current = error;
        }
    }

    fn set_runtime_error(&self, error: Option<String>) {
        if let Ok(mut current) = self.runtime_error.lock() {
            *current = error;
        }
    }

    fn set_audio_metrics(&self, metrics: (u64, u64)) {
        self.audio_underruns.store(metrics.0, Ordering::Release);
        self.audio_dropped_blocks
            .store(metrics.1, Ordering::Release);
    }

    fn reset_machine_counters(&self) {
        self.total_instructions.store(0, Ordering::Release);
        self.set_frame_number(0);
        self.set_runtime_error(None);
    }
}

type AudioSessionFactory = Box<dyn FnOnce() -> Result<Box<dyn AudioSession>, String> + Send>;

enum WorkerCommand {
    Start(SyncSender<Result<(), String>>),
    Pause(SyncSender<Result<(), String>>),
    Stop(SyncSender<Result<(), String>>),
    Reset(SyncSender<Result<(), String>>),
    LoadRom(Vec<u8>, SyncSender<Result<(), String>>),
    StepFrame(usize, SyncSender<Result<usize, String>>),
    SetController(u8, ControllerButtons, SyncSender<Result<(), String>>),
    RequestNmi,
    ConfigureAudio(AudioSessionFactory, SyncSender<Result<(), String>>),
    SetAudioVolume(u8, SyncSender<Result<(), String>>),
    CloseAudio(SyncSender<()>),
    #[cfg(any(windows, test))]
    SetVideoMailbox(Option<FrameMailbox>, SyncSender<Result<(), String>>),
    Shutdown,
}

/// Handle to the thread that exclusively owns the temporal SMS machine.
pub struct SmsTemporalWorker {
    commands: Sender<WorkerCommand>,
    worker: Option<JoinHandle<()>>,
    telemetry: Arc<WorkerTelemetry>,
    #[cfg(windows)]
    video_session: Mutex<Option<D3d12MailboxSession>>,
}

impl SmsTemporalWorker {
    /// Start the native worker and wait until the first machine is constructed.
    pub fn new(rom: Vec<u8>, region: &str) -> Result<Self, String> {
        let region = region.to_owned();
        let telemetry = Arc::new(WorkerTelemetry::default());
        let worker_telemetry = Arc::clone(&telemetry);
        let (commands, receiver) = mpsc::channel();
        let (ready_sender, ready_receiver) = mpsc::sync_channel(1);
        let worker_region = region.clone();

        let worker = thread::Builder::new()
            .name("sms-temporal-machine".to_owned())
            .spawn(move || {
                let machine = match build_sms_temporal_machine(rom.clone(), &worker_region) {
                    Ok(machine) => {
                        let _ = ready_sender.send(Ok(()));
                        machine
                    }
                    Err(error) => {
                        let _ = ready_sender.send(Err(error));
                        return;
                    }
                };
                worker_loop(machine, rom, worker_region, receiver, worker_telemetry);
            })
            .map_err(|error| format!("could not start SMS machine worker: {error}"))?;

        match ready_receiver.recv() {
            Ok(Ok(())) => Ok(Self {
                commands,
                worker: Some(worker),
                telemetry,
                #[cfg(windows)]
                video_session: Mutex::new(None),
            }),
            Ok(Err(error)) => {
                let _ = worker.join();
                Err(error)
            }
            Err(error) => {
                let _ = worker.join();
                Err(format!(
                    "SMS machine worker startup channel closed: {error}"
                ))
            }
        }
    }

    fn request_unit(
        &self,
        command: impl FnOnce(SyncSender<Result<(), String>>) -> WorkerCommand,
    ) -> Result<(), String> {
        let (reply_sender, reply_receiver) = mpsc::sync_channel(1);
        self.commands
            .send(command(reply_sender))
            .map_err(|error| format!("SMS machine worker is unavailable: {error}"))?;
        reply_receiver
            .recv()
            .map_err(|error| format!("SMS machine worker did not acknowledge command: {error}"))?
    }

    pub fn load_rom(&self, rom: Vec<u8>) -> Result<(), String> {
        self.request_unit(|reply| WorkerCommand::LoadRom(rom, reply))
    }

    pub fn reset(&self) -> Result<(), String> {
        self.request_unit(WorkerCommand::Reset)
    }

    pub fn start(&self) -> Result<(), String> {
        self.request_unit(WorkerCommand::Start)
    }

    pub fn pause(&self) -> Result<(), String> {
        self.request_unit(WorkerCommand::Pause)
    }

    pub fn stop(&self) -> Result<(), String> {
        self.request_unit(WorkerCommand::Stop)
    }

    pub fn step_frame(&self, max_instructions: usize) -> Result<usize, String> {
        let (reply_sender, reply_receiver) = mpsc::sync_channel(1);
        self.commands
            .send(WorkerCommand::StepFrame(max_instructions, reply_sender))
            .map_err(|error| format!("SMS machine worker is unavailable: {error}"))?;
        reply_receiver
            .recv()
            .map_err(|error| format!("SMS machine worker did not return a frame: {error}"))?
    }

    pub fn set_controller(&self, player: u8, buttons: ControllerButtons) -> Result<(), String> {
        self.request_unit(|reply| WorkerCommand::SetController(player, buttons, reply))
    }

    pub fn request_nmi(&self) -> Result<(), String> {
        self.commands
            .send(WorkerCommand::RequestNmi)
            .map_err(|error| format!("SMS machine worker is unavailable: {error}"))
    }

    /// Configure host audio on the worker thread using an application-supplied factory.
    ///
    /// The factory is transferred as a coarse-grained command and invoked by the
    /// machine-owning thread, so host objects never cross thread boundaries.
    pub fn configure_audio_output<F>(&self, factory: F) -> Result<(), String>
    where
        F: FnOnce() -> Result<Box<dyn AudioSession>, String> + Send + 'static,
    {
        self.request_unit(|reply| WorkerCommand::ConfigureAudio(Box::new(factory), reply))
    }

    /// Update the configured output volume without restarting the audio backend.
    pub fn set_audio_volume(&self, volume: u8) -> Result<(), String> {
        self.request_unit(|reply| WorkerCommand::SetAudioVolume(volume, reply))
    }

    pub fn close_audio_output(&self) -> Result<(), String> {
        let (reply_sender, reply_receiver) = mpsc::sync_channel(1);
        self.commands
            .send(WorkerCommand::CloseAudio(reply_sender))
            .map_err(|error| format!("SMS machine worker is unavailable: {error}"))?;
        reply_receiver
            .recv()
            .map_err(|error| format!("SMS machine worker did not close audio: {error}"))
    }

    #[cfg(any(windows, test))]
    fn set_video_mailbox(&self, mailbox: Option<FrameMailbox>) -> Result<(), String> {
        let (reply_sender, reply_receiver) = mpsc::sync_channel(1);
        self.commands
            .send(WorkerCommand::SetVideoMailbox(mailbox, reply_sender))
            .map_err(|error| format!("SMS machine worker is unavailable: {error}"))?;
        reply_receiver
            .recv()
            .map_err(|error| format!("SMS machine worker did not configure video: {error}"))?
    }

    /// Attach an externally owned presentation mailbox to the SMS worker.
    ///
    /// The application may own the D3D12 session and HWND while the machine
    /// worker publishes completed frames through a clone of its mailbox.
    /// Attaching or detaching the mailbox is a coarse-grained worker command;
    /// it never advances MachineTime.
    #[cfg(any(windows, test))]
    pub fn attach_video_mailbox(&self, mailbox: FrameMailbox) -> Result<(), String> {
        self.set_video_mailbox(Some(mailbox))
    }

    /// Detach the presentation mailbox without closing its owner-managed queue.
    ///
    /// The caller remains responsible for shutting down the presentation
    /// session after this command has been acknowledged.
    #[cfg(any(windows, test))]
    pub fn detach_video_mailbox(&self) -> Result<(), String> {
        self.set_video_mailbox(None)
    }

    #[cfg(windows)]
    fn start_video_session<E>(
        &self,
        start: impl FnOnce() -> Result<D3d12MailboxSession, E>,
    ) -> Result<(), String>
    where
        E: std::fmt::Display,
    {
        let mut session_guard = self
            .video_session
            .lock()
            .map_err(|_| "video session lock is poisoned".to_owned())?;

        if session_guard
            .as_ref()
            .is_some_and(D3d12MailboxSession::is_running)
        {
            return Ok(());
        }

        if let Some(stale_session) = session_guard.take() {
            let _ = stale_session.shutdown();
        }

        let session = start().map_err(|error| error.to_string())?;
        let mailbox = session.mailbox();
        if let Err(error) = self.set_video_mailbox(Some(mailbox)) {
            let _ = session.shutdown();
            return Err(error);
        }

        self.telemetry.set_video_error(None);
        self.telemetry
            .video_replaced_frames
            .store(0, Ordering::Release);
        *session_guard = Some(session);
        Ok(())
    }

    #[cfg(windows)]
    pub fn open_video_output(&self) -> Result<(), String> {
        self.start_video_session(|| D3d12MailboxSession::start(256, 192, 768, 576))
    }

    #[cfg(windows)]
    pub fn open_video_output_in_viewport(&self, window_handle: usize) -> Result<(), String> {
        self.start_video_session(|| {
            D3d12MailboxSession::start_for_existing_window(256, 192, window_handle)
        })
    }

    #[cfg(not(windows))]
    pub fn open_video_output(&self) -> Result<(), String> {
        Err("D3D12 video output is available only on Windows".to_owned())
    }

    #[cfg(not(windows))]
    pub fn open_video_output_in_viewport(&self, _window_handle: usize) -> Result<(), String> {
        Err("D3D12 video output is available only on Windows".to_owned())
    }

    pub fn close_video_output(&self) -> Result<(), String> {
        #[cfg(windows)]
        {
            let mut session_guard = self
                .video_session
                .lock()
                .map_err(|_| "video session lock is poisoned".to_owned())?;
            if session_guard.is_none() {
                return Ok(());
            }
            self.set_video_mailbox(None)?;
            if let Some(session) = session_guard.take() {
                session.shutdown().map_err(|error| error.to_string())?;
            }
        }
        Ok(())
    }

    pub fn video_output_is_running(&self) -> bool {
        #[cfg(windows)]
        {
            self.video_session
                .lock()
                .ok()
                .and_then(|session| session.as_ref().map(D3d12MailboxSession::is_running))
                .unwrap_or(false)
        }
        #[cfg(not(windows))]
        {
            false
        }
    }

    pub fn video_output_error(&self) -> Option<String> {
        self.telemetry
            .video_output_error
            .lock()
            .ok()
            .and_then(|error| error.clone())
    }

    pub fn video_last_published_sequence(&self) -> u64 {
        self.telemetry
            .video_last_published_sequence
            .load(Ordering::Acquire)
    }

    pub fn video_replaced_frames(&self) -> u64 {
        self.telemetry.video_replaced_frames.load(Ordering::Acquire)
    }

    pub fn video_last_presented_sequence(&self) -> u64 {
        #[cfg(windows)]
        {
            self.video_session
                .lock()
                .ok()
                .and_then(|session| {
                    session
                        .as_ref()
                        .map(D3d12MailboxSession::last_presented_sequence)
                })
                .unwrap_or(0)
        }
        #[cfg(not(windows))]
        {
            0
        }
    }

    pub fn video_device_recovery_count(&self) -> u64 {
        #[cfg(windows)]
        {
            self.video_session
                .lock()
                .ok()
                .and_then(|session| {
                    session
                        .as_ref()
                        .map(D3d12MailboxSession::device_recovery_count)
                })
                .unwrap_or(0)
        }
        #[cfg(not(windows))]
        {
            0
        }
    }

    pub fn is_running(&self) -> bool {
        self.telemetry.running.load(Ordering::Acquire)
    }

    pub fn frame_number(&self) -> u64 {
        self.telemetry.frame_number.load(Ordering::Acquire)
    }

    pub fn total_instructions(&self) -> u64 {
        self.telemetry.total_instructions.load(Ordering::Acquire)
    }

    pub fn audio_output_error(&self) -> Option<String> {
        self.telemetry
            .audio_output_error
            .lock()
            .ok()
            .and_then(|error| error.clone())
    }

    pub fn runtime_error(&self) -> Option<String> {
        self.telemetry
            .runtime_error
            .lock()
            .ok()
            .and_then(|error| error.clone())
    }

    pub fn audio_output_metrics(&self) -> (u64, u64) {
        (
            self.telemetry.audio_underruns.load(Ordering::Acquire),
            self.telemetry.audio_dropped_blocks.load(Ordering::Acquire),
        )
    }

    #[cfg(test)]
    fn wait_for_frame_after(&self, frame_number: u64, timeout: Duration) -> bool {
        let Ok(observed) = self.telemetry.frame_signal.0.lock() else {
            return false;
        };
        let Ok((observed, timeout_result)) =
            self.telemetry
                .frame_signal
                .1
                .wait_timeout_while(observed, timeout, |current| *current <= frame_number)
        else {
            return false;
        };
        !timeout_result.timed_out() && *observed > frame_number
    }
}

impl Drop for SmsTemporalWorker {
    fn drop(&mut self) {
        let _ = self.close_video_output();
        let _ = self.commands.send(WorkerCommand::Shutdown);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn execute_frame(
    machine: &mut SmsTemporalMachine,
    max_instructions: usize,
    publish_audio: bool,
    audio_session: &mut Option<Box<dyn AudioSession>>,
    video_mailbox: &mut Option<FrameMailbox>,
    video_sequence: &mut u64,
    telemetry: &WorkerTelemetry,
) -> Result<(usize, Duration), String> {
    let frame_started = Instant::now();
    let instructions = machine
        .step_frame(max_instructions)
        .map_err(|error| error.to_string())?;
    telemetry
        .total_instructions
        .fetch_add(instructions as u64, Ordering::AcqRel);
    telemetry.set_frame_number(machine.frame_number());

    *video_sequence = video_sequence
        .checked_add(1)
        .ok_or_else(|| "SMS video frame sequence exhausted".to_owned())?;
    let mut video_error = None;
    if let Some(mailbox) = video_mailbox.as_ref() {
        let frame = VideoFrame::new(
            *video_sequence,
            256,
            192,
            PixelFormat::Rgb888,
            256 * 3,
            machine.framebuffer().to_vec(),
        )
        .map_err(|error| error.to_string())
        .and_then(|frame| mailbox.publish(frame).map_err(|error| error.to_string()));

        match frame {
            Ok(()) => {
                telemetry
                    .video_last_published_sequence
                    .store(*video_sequence, Ordering::Release);
                match mailbox.replaced_frames() {
                    Ok(replaced) => telemetry
                        .video_replaced_frames
                        .store(replaced, Ordering::Release),
                    Err(error) => video_error = Some(error.to_string()),
                }
            }
            Err(error) => video_error = Some(error),
        }
    }
    if let Some(error) = video_error {
        telemetry.set_video_error(Some(error));
        *video_mailbox = None;
    }

    let samples = machine.drain_audio_samples(None);
    let audio_result = audio_session.as_mut().map(|session| {
        if publish_audio {
            session.process_samples(&samples)
        } else {
            session.process_samples_discarding_output(&samples)
        }
    });
    match audio_result {
        Some(Ok(())) => {
            if let Some(session) = audio_session.as_ref() {
                telemetry.set_audio_metrics(session.metrics());
            }
        }
        Some(Err(error)) => {
            if let Some(mut failed_session) = audio_session.take() {
                failed_session.close();
            }
            telemetry.set_audio_error(Some(error));
            telemetry.set_audio_metrics((0, 0));
        }
        None => {}
    }

    let sample_count = samples.len().max(1) as f64;
    let frame_duration = Duration::from_secs_f64(sample_count / SMS_TEMPORAL_SAMPLE_RATE as f64);
    Ok((
        instructions,
        frame_duration.saturating_sub(frame_started.elapsed()),
    ))
}

fn worker_loop(
    mut machine: SmsTemporalMachine,
    mut rom: Vec<u8>,
    region: String,
    commands: Receiver<WorkerCommand>,
    telemetry: Arc<WorkerTelemetry>,
) {
    let mut running = false;
    let mut audio_session: Option<Box<dyn AudioSession>> = None;
    let mut video_mailbox: Option<FrameMailbox> = None;
    let mut video_sequence = 0_u64;

    loop {
        let command = if running {
            match commands.recv_timeout(Duration::from_millis(0)) {
                Ok(command) => Some(command),
                Err(RecvTimeoutError::Timeout) => None,
                Err(RecvTimeoutError::Disconnected) => break,
            }
        } else {
            match commands.recv() {
                Ok(command) => Some(command),
                Err(_) => break,
            }
        };

        if let Some(command) = command {
            if !handle_command(
                command,
                &mut machine,
                &mut rom,
                &region,
                &mut running,
                &mut audio_session,
                &mut video_mailbox,
                &mut video_sequence,
                &telemetry,
            ) {
                break;
            }
            continue;
        }

        match execute_frame(
            &mut machine,
            DEFAULT_MAX_FRAME_INSTRUCTIONS,
            true,
            &mut audio_session,
            &mut video_mailbox,
            &mut video_sequence,
            &telemetry,
        ) {
            Ok((_, frame_wait)) => {
                if !frame_wait.is_zero() {
                    match commands.recv_timeout(frame_wait) {
                        Ok(command) => {
                            if !handle_command(
                                command,
                                &mut machine,
                                &mut rom,
                                &region,
                                &mut running,
                                &mut audio_session,
                                &mut video_mailbox,
                                &mut video_sequence,
                                &telemetry,
                            ) {
                                break;
                            }
                        }
                        Err(RecvTimeoutError::Timeout) => {}
                        Err(RecvTimeoutError::Disconnected) => break,
                    }
                }
            }
            Err(error) => {
                running = false;
                telemetry.running.store(false, Ordering::Release);
                telemetry.set_runtime_error(Some(error));
                if let Some(mut session) = audio_session.take() {
                    session.close();
                }
            }
        }
    }

    telemetry.running.store(false, Ordering::Release);
    if let Some(mut session) = audio_session.take() {
        session.close();
    }
    // The worker only borrows the publisher handle. Mailbox closure belongs
    // to the host that created the presentation session.
    drop(video_mailbox);
}

/// Dispatch one coarse worker command over the worker-owned machine and outputs.
///
/// The explicit state references keep machine, audio, video and timing ownership
/// visible at this boundary instead of hiding them in shared mutable globals.
#[allow(clippy::too_many_arguments)]
fn handle_command(
    command: WorkerCommand,
    machine: &mut SmsTemporalMachine,
    rom: &mut Vec<u8>,
    region: &str,
    running: &mut bool,
    audio_session: &mut Option<Box<dyn AudioSession>>,
    video_mailbox: &mut Option<FrameMailbox>,
    video_sequence: &mut u64,
    telemetry: &WorkerTelemetry,
) -> bool {
    match command {
        WorkerCommand::Start(reply) => {
            let result = audio_session
                .as_mut()
                .map_or(Ok(()), |session| session.resume());
            match result {
                Ok(()) => {
                    *running = true;
                    telemetry.running.store(true, Ordering::Release);
                    let _ = reply.send(Ok(()));
                }
                Err(error) => {
                    telemetry.set_audio_error(Some(error.clone()));
                    let _ = reply.send(Err(error));
                }
            }
        }
        WorkerCommand::Pause(reply) | WorkerCommand::Stop(reply) => {
            *running = false;
            telemetry.running.store(false, Ordering::Release);
            let result = audio_session
                .as_mut()
                .map_or(Ok(()), |session| session.suspend_and_flush());
            if let Err(error) = &result {
                telemetry.set_audio_error(Some(error.clone()));
            }
            let _ = reply.send(result);
        }
        WorkerCommand::Reset(reply) => match build_sms_temporal_machine(rom.clone(), region) {
            Ok(reset_machine) => {
                *running = false;
                telemetry.running.store(false, Ordering::Release);
                let result = audio_session.as_mut().map_or(Ok(()), |session| {
                    session.suspend_and_flush()?;
                    session.reset_dsp();
                    Ok(())
                });
                match result {
                    Ok(()) => {
                        *machine = reset_machine;
                        telemetry.reset_machine_counters();
                        let _ = reply.send(Ok(()));
                    }
                    Err(error) => {
                        telemetry.set_audio_error(Some(error.clone()));
                        let _ = reply.send(Err(error));
                    }
                }
            }
            Err(error) => {
                let _ = reply.send(Err(error));
            }
        },
        WorkerCommand::LoadRom(new_rom, reply) => {
            match build_sms_temporal_machine(new_rom.clone(), region) {
                Ok(new_machine) => {
                    *running = false;
                    telemetry.running.store(false, Ordering::Release);
                    let result = audio_session.as_mut().map_or(Ok(()), |session| {
                        session.suspend_and_flush()?;
                        session.reset_dsp();
                        Ok(())
                    });
                    match result {
                        Ok(()) => {
                            *machine = new_machine;
                            *rom = new_rom;
                            telemetry.reset_machine_counters();
                            let _ = reply.send(Ok(()));
                        }
                        Err(error) => {
                            telemetry.set_audio_error(Some(error.clone()));
                            let _ = reply.send(Err(error));
                        }
                    }
                }
                Err(error) => {
                    let _ = reply.send(Err(error));
                }
            }
        }
        WorkerCommand::StepFrame(max_instructions, reply) => {
            if *running {
                let _ = reply.send(Err(
                    "step_frame is available only while the native worker is paused".to_owned(),
                ));
            } else if max_instructions == 0 {
                let _ = reply.send(Err("max_instructions must be positive".to_owned()));
            } else {
                match execute_frame(
                    machine,
                    max_instructions,
                    false,
                    audio_session,
                    video_mailbox,
                    video_sequence,
                    telemetry,
                ) {
                    Ok((instructions, _)) => {
                        let _ = reply.send(Ok(instructions));
                    }
                    Err(error) => {
                        telemetry.set_runtime_error(Some(error.clone()));
                        let _ = reply.send(Err(error));
                    }
                }
            }
        }
        WorkerCommand::SetController(player, buttons, reply) => {
            let result = machine
                .set_controller(player, buttons)
                .map_err(|error| error.to_string());
            let _ = reply.send(result);
        }
        WorkerCommand::RequestNmi => machine.request_nmi(),
        WorkerCommand::ConfigureAudio(factory, reply) => match factory() {
            Ok(mut session) => {
                let previous_result = audio_session
                    .as_mut()
                    .map_or(Ok(()), |previous| previous.suspend_and_flush());
                if let Err(error) = previous_result {
                    if let Some(mut previous) = audio_session.take() {
                        previous.close();
                    }
                    session.close();
                    telemetry.set_audio_error(Some(error.clone()));
                    let _ = reply.send(Err(error));
                } else {
                    if let Some(mut previous) = audio_session.take() {
                        previous.close();
                    }
                    let result = if *running {
                        session.resume()
                    } else {
                        session.suspend_and_flush()
                    };
                    match result {
                        Ok(()) => {
                            *audio_session = Some(session);
                            telemetry.set_audio_error(None);
                            telemetry.set_audio_metrics((0, 0));
                            let _ = reply.send(Ok(()));
                        }
                        Err(error) => {
                            session.close();
                            telemetry.set_audio_error(Some(error.clone()));
                            let _ = reply.send(Err(error));
                        }
                    }
                }
            }
            Err(error) => {
                telemetry.set_audio_error(Some(error.clone()));
                let _ = reply.send(Err(error));
            }
        },
        WorkerCommand::SetAudioVolume(volume, reply) => {
            let result = audio_session
                .as_mut()
                .ok_or_else(|| "native audio output is not configured".to_owned())
                .and_then(|session| session.set_volume(volume));
            let _ = reply.send(result);
        }
        WorkerCommand::CloseAudio(reply) => {
            let close_error = audio_session.take().and_then(|mut session| {
                let error = session.suspend_and_flush().err();
                session.close();
                error
            });
            telemetry.set_audio_error(close_error);
            telemetry.set_audio_metrics((0, 0));
            let _ = reply.send(());
        }
        #[cfg(any(windows, test))]
        WorkerCommand::SetVideoMailbox(mailbox, reply) => {
            if let Some(attached_mailbox) = mailbox.as_ref() {
                match attached_mailbox.last_sequence() {
                    Ok(Some(last_sequence)) => {
                        *video_sequence = (*video_sequence).max(last_sequence);
                    }
                    Ok(None) => {}
                    Err(error) => {
                        let _ = reply.send(Err(format!(
                            "could not read video mailbox sequence: {error}"
                        )));
                        return true;
                    }
                }
            }
            *video_mailbox = mailbox;
            telemetry.set_video_error(None);
            let _ = reply.send(Ok(()));
        }
        WorkerCommand::Shutdown => {
            *running = false;
            telemetry.running.store(false, Ordering::Release);
            if let Some(mut session) = audio_session.take() {
                let _ = session.suspend_and_flush();
                session.close();
            }
            // Do not close a mailbox supplied by an external presentation host.
            // The host detaches us and closes its own mailbox after worker shutdown.
            drop(video_mailbox.take());
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::{
        DEFAULT_MAX_FRAME_INSTRUCTIONS, Ordering, SMS_TEMPORAL_SAMPLE_RATE, SmsTemporalWorker,
        WorkerTelemetry, build_sms_temporal_machine, execute_frame,
    };
    use sega_audio_runtime::{NativeAudioSession, PcmSink};
    use sega_sms_core::ControllerButtons;
    use sega_video_runtime::{FrameMailbox, PixelFormat};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    #[derive(Clone, Default)]
    struct RecordingSink(Arc<Mutex<Vec<Vec<u8>>>>);

    impl PcmSink for RecordingSink {
        fn write_pcm(&mut self, pcm: Vec<u8>) -> Result<(), String> {
            self.0
                .lock()
                .map_err(|_| "recording sink lock poisoned".to_owned())?
                .push(pcm);
            Ok(())
        }

        fn suspend_and_flush(&mut self) -> Result<(), String> {
            Ok(())
        }

        fn resume(&mut self) -> Result<(), String> {
            Ok(())
        }

        fn metrics(&self) -> (u64, u64) {
            (0, 0)
        }

        fn close(&mut self) {}
    }

    #[derive(Clone, Default)]
    struct ControlSink(Arc<Mutex<Vec<&'static str>>>);

    impl PcmSink for ControlSink {
        fn write_pcm(&mut self, _pcm: Vec<u8>) -> Result<(), String> {
            Ok(())
        }

        fn suspend_and_flush(&mut self) -> Result<(), String> {
            self.0.lock().unwrap().push("flush");
            Ok(())
        }

        fn resume(&mut self) -> Result<(), String> {
            self.0.lock().unwrap().push("resume");
            Ok(())
        }

        fn metrics(&self) -> (u64, u64) {
            (0, 0)
        }

        fn close(&mut self) {}
    }

    const SYNTHETIC_ROM: [u8; 0x4000] = [0; 0x4000];

    fn av_test_rom() -> Vec<u8> {
        let mut rom = vec![0; 0x4000];
        let mut program = vec![
            0xF3, // DI
            0x31, 0xF0, 0xDF, // LD SP,$DFF0
            0x3E, 0x06, 0xD3, 0xBF, 0x3E, 0x80, 0xD3, 0xBF, // R0: Mode 4
            0x3E, 0xC0, 0xD3, 0xBF, 0x3E, 0x81, 0xD3, 0xBF, // R1: 16 KB, display
            0x3E, 0x0E, 0xD3, 0xBF, 0x3E, 0x82, 0xD3, 0xBF, // R2: name table
            0x3E, 0x01, 0xD3, 0xBF, 0x3E, 0xC0, 0xD3, 0xBF, 0x3E, 0x3F, 0xD3, 0xBE, 0x3E, 0x00,
            0xD3, 0xBF, 0x3E, 0x40, 0xD3, 0xBF,
        ];
        for _ in 0..8 {
            // One byte per bitplane per row: only plane 0 is set, so every
            // pixel selects background palette entry 1 (the white CRAM color).
            program.extend([
                0x3E, 0xFF, 0xD3, 0xBE, // Plane 0
                0x3E, 0x00, 0xD3, 0xBE, // Plane 1
                0x3E, 0x00, 0xD3, 0xBE, // Plane 2
                0x3E, 0x00, 0xD3, 0xBE, // Plane 3
            ]);
        }
        program.extend([
            0x3E, 0x84, 0xD3, 0x7E, 0x3E, 0x00, 0xD3, 0x7E, 0x3E, 0x90, 0xD3, 0x7E, 0x18, 0xFE,
        ]);
        rom[..program.len()].copy_from_slice(&program);
        rom
    }

    #[test]
    fn native_worker_runs_pauses_and_steps_through_coarse_commands() {
        let worker = SmsTemporalWorker::new(SYNTHETIC_ROM.to_vec(), "usa").unwrap();
        let initial = worker.frame_number();

        worker.start().unwrap();
        assert!(worker.wait_for_frame_after(initial, Duration::from_secs(2)));
        worker.pause().unwrap();

        let paused_frame = worker.frame_number();
        std::thread::sleep(Duration::from_millis(30));
        assert_eq!(worker.frame_number(), paused_frame);

        worker.step_frame(100_000).unwrap();
        assert_eq!(worker.frame_number(), paused_frame + 1);
    }

    #[test]
    fn reset_and_load_rom_restore_power_on_machine_counters() {
        let worker = SmsTemporalWorker::new(SYNTHETIC_ROM.to_vec(), "usa").unwrap();
        worker.step_frame(100_000).unwrap();
        assert_eq!(worker.frame_number(), 1);

        worker.reset().unwrap();
        assert_eq!(worker.frame_number(), 0);

        worker.step_frame(100_000).unwrap();
        worker.load_rom(SYNTHETIC_ROM.to_vec()).unwrap();
        assert_eq!(worker.frame_number(), 0);
    }

    #[test]
    fn controller_levels_do_not_advance_machine_time() {
        let worker = SmsTemporalWorker::new(SYNTHETIC_ROM.to_vec(), "usa").unwrap();
        let initial = worker.frame_number();
        worker
            .set_controller(
                1,
                ControllerButtons {
                    up: true,
                    down: false,
                    left: false,
                    right: false,
                    button_1: true,
                    button_2: false,
                },
            )
            .unwrap();
        worker.request_nmi().unwrap();
        assert_eq!(worker.frame_number(), initial);
    }

    #[test]
    fn audio_factory_runs_on_worker_thread_and_isolates_host_errors() {
        let worker = SmsTemporalWorker::new(SYNTHETIC_ROM.to_vec(), "usa").unwrap();
        let caller_thread = std::thread::current().id();
        let observed_thread = Arc::new(Mutex::new(None));
        let factory_thread = Arc::clone(&observed_thread);

        let result = worker.configure_audio_output(move || {
            *factory_thread
                .lock()
                .map_err(|_| "audio test lock poisoned".to_owned())? =
                Some(std::thread::current().id());
            Err("audio host unavailable".to_owned())
        });

        assert_eq!(result, Err("audio host unavailable".to_owned()));
        assert_ne!(
            observed_thread.lock().unwrap().as_ref(),
            Some(&caller_thread)
        );
        assert_eq!(
            worker.audio_output_error().as_deref(),
            Some("audio host unavailable")
        );
        assert!(!worker.is_running());
    }

    #[test]
    fn worker_serializes_audio_flush_resume_and_step_frame_boundaries() {
        let worker = SmsTemporalWorker::new(av_test_rom(), "usa").unwrap();
        let events = Arc::new(Mutex::new(Vec::new()));
        let factory_events = Arc::clone(&events);

        worker
            .configure_audio_output(move || -> Result<Box<dyn super::AudioSession>, String> {
                let sink = ControlSink(Arc::clone(&factory_events));
                let session =
                    NativeAudioSession::new(SMS_TEMPORAL_SAMPLE_RATE as u32, 100, sink)?;
                Ok(Box::new(session))
            })
            .unwrap();
        worker.start().unwrap();
        worker.pause().unwrap();

        let before_step = events.lock().unwrap().clone();
        worker.step_frame(100_000).unwrap();
        assert_eq!(
            *events.lock().unwrap(),
            before_step,
            "StepFrame must not resume or flush the host endpoint"
        );

        worker.start().unwrap();
        worker.reset().unwrap();
        worker.load_rom(av_test_rom()).unwrap();

        assert_eq!(
            *events.lock().unwrap(),
            ["flush", "resume", "flush", "resume", "flush", "flush"]
        );
        assert!(!worker.is_running());
    }

    #[test]
    fn completed_frames_publish_rgb888_and_keep_sequence_monotonic_across_reset() {
        let worker = SmsTemporalWorker::new(SYNTHETIC_ROM.to_vec(), "usa").unwrap();
        let mailbox = FrameMailbox::new();
        worker.attach_video_mailbox(mailbox.clone()).unwrap();

        worker.step_frame(100_000).unwrap();
        let first = mailbox.take_latest().unwrap().unwrap();
        assert_eq!(first.sequence(), 1);
        assert_eq!(first.width(), 256);
        assert_eq!(first.height(), 192);
        assert_eq!(first.pixel_format(), PixelFormat::Rgb888);
        assert_eq!(first.stride_bytes(), 256 * 3);
        assert_eq!(first.pixels().len(), 256 * 192 * 3);

        worker.reset().unwrap();
        worker.step_frame(100_000).unwrap();
        let second = mailbox.take_latest().unwrap().unwrap();
        assert_eq!(second.sequence(), 2);
        assert_eq!(worker.video_last_published_sequence(), 2);

        worker.detach_video_mailbox().unwrap();
        worker.step_frame(100_000).unwrap();
        assert!(
            mailbox.take_latest().unwrap().is_none(),
            "detaching must stop frame publication"
        );
        assert!(
            !mailbox.is_closed().unwrap(),
            "detaching must not close the externally owned mailbox"
        );
    }

    #[test]
    fn replacement_worker_continues_existing_mailbox_sequence() {
        let mailbox = FrameMailbox::new();

        let first_worker = SmsTemporalWorker::new(SYNTHETIC_ROM.to_vec(), "usa").unwrap();
        first_worker.attach_video_mailbox(mailbox.clone()).unwrap();
        first_worker.step_frame(100_000).unwrap();
        let first = mailbox.take_latest().unwrap().unwrap();
        assert_eq!(first.sequence(), 1);
        drop(first_worker);

        let replacement_worker = SmsTemporalWorker::new(SYNTHETIC_ROM.to_vec(), "usa").unwrap();
        replacement_worker
            .attach_video_mailbox(mailbox.clone())
            .unwrap();
        replacement_worker.step_frame(100_000).unwrap();

        let second = mailbox.take_latest().unwrap().unwrap();
        assert_eq!(second.sequence(), 2);
        assert_eq!(replacement_worker.video_last_published_sequence(), 2);
        assert_eq!(replacement_worker.video_output_error(), None);
    }

    #[test]
    fn worker_publishes_vdp_generated_non_black_pixels_to_video_mailbox() {
        let worker = SmsTemporalWorker::new(av_test_rom(), "usa").unwrap();
        let mailbox = FrameMailbox::new();
        worker.set_video_mailbox(Some(mailbox.clone())).unwrap();

        worker.step_frame(100_000).unwrap();

        let frame = mailbox.take_latest().unwrap().unwrap();
        assert_eq!(frame.width(), 256);
        assert_eq!(frame.height(), 192);
        assert_eq!(frame.pixel_format(), PixelFormat::Rgb888);
        assert_eq!(
            &frame.pixels()[32 * 256 * 3..32 * 256 * 3 + 3],
            &[255, 255, 255],
            "the worker must preserve the Mode 4 background pixel at line 32"
        );
        assert!(
            frame
                .pixels()
                .as_chunks::<3>()
                .0
                .iter()
                .any(|pixel| pixel.iter().any(|channel| *channel != 0))
        );
    }

    #[test]
    fn debugger_step_frame_advances_machine_without_publishing_pcm() {
        let mut machine = build_sms_temporal_machine(av_test_rom(), "usa").unwrap();
        let sink = RecordingSink::default();
        let captured_blocks = Arc::clone(&sink.0);
        let session = NativeAudioSession::new(SMS_TEMPORAL_SAMPLE_RATE as u32, 100, sink).unwrap();
        let mut audio_session: Option<Box<dyn super::AudioSession>> = Some(Box::new(session));
        let telemetry = WorkerTelemetry::default();
        let mut video_mailbox = None;
        let mut video_sequence = 0;

        let (instructions, _) = execute_frame(
            &mut machine,
            DEFAULT_MAX_FRAME_INSTRUCTIONS,
            false,
            &mut audio_session,
            &mut video_mailbox,
            &mut video_sequence,
            &telemetry,
        )
        .unwrap();

        assert!(instructions > 0);
        assert_eq!(machine.frame_number(), 1);
        assert!(
            captured_blocks.lock().unwrap().is_empty(),
            "debugger StepFrame must advance DSP but never submit PCM to the physical sink"
        );
    }

    #[test]
    fn worker_frame_submits_non_silent_pcm_to_native_audio_session_sink() {
        let mut machine = build_sms_temporal_machine(av_test_rom(), "usa").unwrap();
        let sink = RecordingSink::default();
        let captured_blocks = Arc::clone(&sink.0);
        let session = NativeAudioSession::new(SMS_TEMPORAL_SAMPLE_RATE as u32, 100, sink).unwrap();
        let mut audio_session: Option<Box<dyn super::AudioSession>> = Some(Box::new(session));
        let telemetry = WorkerTelemetry::default();
        let mut video_mailbox = None;
        let mut video_sequence = 0;

        let (instructions, _) = execute_frame(
            &mut machine,
            DEFAULT_MAX_FRAME_INSTRUCTIONS,
            true,
            &mut audio_session,
            &mut video_mailbox,
            &mut video_sequence,
            &telemetry,
        )
        .unwrap();

        assert!(instructions > 0);
        assert_eq!(telemetry.audio_underruns.load(Ordering::Acquire), 0);
        assert_eq!(telemetry.audio_dropped_blocks.load(Ordering::Acquire), 0);

        let blocks_before_close = captured_blocks.lock().unwrap();
        assert!(
            !blocks_before_close.is_empty(),
            "process_samples must submit a PCM block before the session is flushed"
        );
        assert!(
            blocks_before_close
                .iter()
                .flatten()
                .any(|sample| *sample != 0),
            "worker must submit non-silent PCM, not only initialize the sink"
        );
        drop(blocks_before_close);

        audio_session.as_mut().unwrap().close();
    }

    #[test]
    fn dropping_worker_does_not_close_externally_owned_video_mailbox() {
        let mailbox = FrameMailbox::new();
        {
            let worker = SmsTemporalWorker::new(SYNTHETIC_ROM.to_vec(), "usa").unwrap();
            worker.attach_video_mailbox(mailbox.clone()).unwrap();
        }

        assert!(
            !mailbox.is_closed().unwrap(),
            "the external host retains ownership of mailbox shutdown"
        );
    }

    #[test]
    fn closed_video_mailbox_does_not_stop_machine_execution() {
        let worker = SmsTemporalWorker::new(SYNTHETIC_ROM.to_vec(), "usa").unwrap();
        let mailbox = FrameMailbox::new();
        mailbox.close().unwrap();
        worker.set_video_mailbox(Some(mailbox)).unwrap();

        worker.step_frame(100_000).unwrap();

        assert_eq!(worker.frame_number(), 1);
        assert!(worker.video_output_error().is_some());
        assert_eq!(worker.runtime_error(), None);
    }
}
