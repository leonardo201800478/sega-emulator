//! Rust-owned continuous execution worker for the temporal SMS machine.
//!
//! The worker constructs and owns the complete hardware machine on its own
//! thread. The host sends only coarse-grained commands; CPU, Scheduler, VDP, PSG,
//! audio pacing and state transitions remain inside Rust.

#[cfg(test)]
use std::sync::Condvar;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender, TrySendError};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crate::audio::AudioSession;
use sega_audio_runtime::AudioOutputMetrics;
use sega_debugger_core::{BoundedHistory, BreakpointSet};
use sega_sms_core::{
    ControllerButtons, SmsTemporalMachine, SmsTemporalTraceEvent, SmsVdpProfile, SmsVdpRevision,
    SpriteLimitMode, Territory, VideoStandard,
};
#[cfg(windows)]
use sega_video_runtime::d3d12::D3d12MailboxSession;
use sega_video_runtime::{FrameMailbox, PixelFormat, VideoFrame};

const SMS_TEMPORAL_SAMPLE_RATE: u64 = 44_100;
const SMS_TEMPORAL_MASTER_TICKS_PER_TSTATE: u64 = 1;
const DEFAULT_MAX_FRAME_INSTRUCTIONS: usize = 100_000;
const DEBUG_TRACE_CAPACITY: usize = 2_048;
const WORKER_COMMAND_QUEUE_CAPACITY: usize = 64;
// Running workers accept control commands at complete-frame boundaries; debug builds may need
// substantially longer than one wall-clock frame to reach that safe point.
const WORKER_REPLY_TIMEOUT: Duration = Duration::from_secs(2);
// A complete frame step can execute tens of thousands of instructions, especially in debug builds.
const WORKER_FRAME_STEP_TIMEOUT: Duration = Duration::from_secs(5);
const WORKER_STARTUP_TIMEOUT: Duration = Duration::from_secs(5);
const WORKER_IDLE_POLL_INTERVAL: Duration = Duration::from_millis(50);

/// Absolute host-clock deadline for the next SMS frame.
///
/// This is presentation pacing only. It never advances or modifies virtual
/// machine time; non-pausing worker commands consume the existing budget
/// instead of restarting it.
#[derive(Debug, Default, Clone, Copy)]
struct FramePacingDeadline {
    deadline: Option<Instant>,
}

impl FramePacingDeadline {
    fn set(&mut self, deadline: Option<Instant>) {
        self.deadline = deadline;
    }

    fn remaining(&self, now: Instant) -> Option<Duration> {
        self.deadline
            .map(|deadline| deadline.saturating_duration_since(now))
    }

    fn clear(&mut self) {
        self.deadline = None;
    }
}

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

fn enqueue_worker_command(
    commands: &SyncSender<WorkerCommand>,
    command: WorkerCommand,
) -> Result<(), String> {
    match commands.try_send(command) {
        Ok(()) => Ok(()),
        Err(TrySendError::Full(_)) => {
            Err("SMS machine worker command queue is full; retry shortly".to_owned())
        }
        Err(TrySendError::Disconnected(_)) => Err("SMS machine worker is unavailable".to_owned()),
    }
}

fn receive_worker_reply_with_timeout<T>(
    receiver: Receiver<T>,
    operation: &str,
    timeout: Duration,
) -> Result<T, String> {
    receiver.recv_timeout(timeout).map_err(|error| match error {
        RecvTimeoutError::Timeout => {
            format!("SMS machine worker timed out while {operation}")
        }
        RecvTimeoutError::Disconnected => {
            format!("SMS machine worker stopped while {operation}")
        }
    })
}

fn receive_worker_reply<T>(receiver: Receiver<T>, operation: &str) -> Result<T, String> {
    receive_worker_reply_with_timeout(receiver, operation, WORKER_REPLY_TIMEOUT)
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

/// Host-clock measurements taken only when the SMS worker completes a full frame.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct SmsPerformanceMetrics {
    /// Completed SMS frames per wall-clock second, sampled over a rolling interval.
    pub emulation_fps: f32,
    /// Wall-clock milliseconds between consecutive completed SMS frames.
    pub frame_time_ms: Option<f32>,
    /// Milliseconds from host input detection to the first completed frame after it.
    pub input_to_frame_ms: Option<f32>,
    /// Monotonic SMS frame counter observed by this telemetry instance.
    pub completed_frames: u64,
}

#[derive(Debug, Default)]
struct FrameTimingState {
    last_completed: Option<Instant>,
    sample_started: Option<Instant>,
    sample_frames: u64,
    metrics: SmsPerformanceMetrics,
    pending_input: Option<Instant>,
}

impl FrameTimingState {
    fn record_input(&mut self, detected_at: Instant) {
        // Controller commands carry a complete button state. The latest accepted
        // state is the one that the next completed frame can incorporate.
        self.pending_input = Some(detected_at);
    }

    fn record_frame_completed(&mut self, completed_at: Instant) {
        self.metrics.completed_frames = self.metrics.completed_frames.saturating_add(1);
        if let Some(previous) = self.last_completed {
            self.metrics.frame_time_ms = Some(
                completed_at
                    .saturating_duration_since(previous)
                    .as_secs_f32()
                    * 1_000.0,
            );
            self.sample_frames = self.sample_frames.saturating_add(1);
        }
        self.last_completed = Some(completed_at);

        let sample_started = *self.sample_started.get_or_insert(completed_at);
        let elapsed = completed_at.saturating_duration_since(sample_started);
        if elapsed >= Duration::from_millis(500) {
            let sample_frames = u16::try_from(self.sample_frames).unwrap_or(u16::MAX);
            self.metrics.emulation_fps = f32::from(sample_frames) / elapsed.as_secs_f32();
            self.sample_frames = 0;
            self.sample_started = Some(completed_at);
        }

        if let Some(input_detected_at) = self.pending_input.take() {
            self.metrics.input_to_frame_ms = Some(
                completed_at
                    .saturating_duration_since(input_detected_at)
                    .as_secs_f32()
                    * 1_000.0,
            );
        }
    }

    fn metrics_at(&self, now: Instant) -> SmsPerformanceMetrics {
        let mut metrics = self.metrics;
        if self
            .last_completed
            .is_none_or(|last| now.saturating_duration_since(last) >= Duration::from_millis(500))
        {
            metrics.emulation_fps = 0.0;
        }
        metrics
    }

    fn reset(&mut self) {
        *self = Self::default();
    }
}

#[derive(Debug, Default)]
struct WorkerTelemetry {
    frame_timing: Mutex<FrameTimingState>,
    running: AtomicBool,
    shutdown: AtomicBool,
    diagnostics_enabled: AtomicBool,
    frame_number: AtomicU64,
    machine_time_ticks: AtomicU64,
    machine_clock_hz: AtomicU64,
    total_instructions: AtomicU64,
    audio_underruns: AtomicU64,
    audio_dropped_blocks: AtomicU64,
    audio_sample_rate_hz: AtomicU64,
    audio_queue_capacity_frames: AtomicU64,
    audio_queue_queued_frames: AtomicU64,
    audio_queue_high_water_frames: AtomicU64,
    audio_accepted_frames: AtomicU64,
    audio_consumed_frames: AtomicU64,
    audio_dropped_frames: AtomicU64,
    audio_underrun_frames: AtomicU64,
    audio_flushed_frames: AtomicU64,
    audio_endpoint_starvation_events: AtomicU64,
    audio_submission_failed_frames: AtomicU64,
    audio_output_error: Mutex<Option<String>>,
    video_output_error: Mutex<Option<String>>,
    video_last_published_sequence: AtomicU64,
    video_replaced_frames: AtomicU64,
    runtime_error: Mutex<Option<String>>,
    #[cfg(test)]
    frame_signal: (Mutex<u64>, Condvar),
}

impl WorkerTelemetry {
    fn record_input_detected(&self, detected_at: Instant) {
        if let Ok(mut timing) = self.frame_timing.lock() {
            timing.record_input(detected_at);
        }
    }

    fn record_frame_completed(&self, completed_at: Instant) {
        if let Ok(mut timing) = self.frame_timing.lock() {
            timing.record_frame_completed(completed_at);
        }
    }

    fn performance_metrics(&self) -> SmsPerformanceMetrics {
        self.frame_timing.lock().map_or_else(
            |_| SmsPerformanceMetrics::default(),
            |timing| timing.metrics_at(Instant::now()),
        )
    }

    fn reset_frame_timing(&self) {
        if let Ok(mut timing) = self.frame_timing.lock() {
            timing.reset();
        }
    }

    fn set_frame_number(&self, frame_number: u64) {
        self.frame_number.store(frame_number, Ordering::Release);
        #[cfg(test)]
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

    fn set_diagnostics_enabled(&self, enabled: bool) {
        self.diagnostics_enabled.store(enabled, Ordering::Release);
        if !enabled {
            self.total_instructions.store(0, Ordering::Release);
            self.video_replaced_frames.store(0, Ordering::Release);
            self.audio_underruns.store(0, Ordering::Release);
            self.audio_dropped_blocks.store(0, Ordering::Release);
            self.audio_sample_rate_hz.store(0, Ordering::Release);
            self.audio_queue_capacity_frames.store(0, Ordering::Release);
            self.audio_queue_queued_frames.store(0, Ordering::Release);
            self.audio_queue_high_water_frames
                .store(0, Ordering::Release);
            self.audio_accepted_frames.store(0, Ordering::Release);
            self.audio_consumed_frames.store(0, Ordering::Release);
            self.audio_dropped_frames.store(0, Ordering::Release);
            self.audio_underrun_frames.store(0, Ordering::Release);
            self.audio_flushed_frames.store(0, Ordering::Release);
            self.audio_endpoint_starvation_events
                .store(0, Ordering::Release);
            self.audio_submission_failed_frames
                .store(0, Ordering::Release);
        }
    }

    fn diagnostics_enabled(&self) -> bool {
        self.diagnostics_enabled.load(Ordering::Acquire)
    }

    fn set_audio_metrics(&self, metrics: AudioOutputMetrics) {
        if !self.diagnostics_enabled() {
            return;
        }
        self.audio_underruns
            .store(metrics.underrun_events, Ordering::Release);
        self.audio_dropped_blocks
            .store(metrics.dropped_blocks, Ordering::Release);
        self.audio_sample_rate_hz
            .store(u64::from(metrics.sample_rate_hz), Ordering::Release);
        self.audio_queue_capacity_frames
            .store(metrics.queue_capacity_frames, Ordering::Release);
        self.audio_queue_queued_frames
            .store(metrics.queue_queued_frames, Ordering::Release);
        self.audio_queue_high_water_frames
            .store(metrics.queue_high_water_frames, Ordering::Release);
        self.audio_accepted_frames
            .store(metrics.accepted_frames, Ordering::Release);
        self.audio_consumed_frames
            .store(metrics.consumed_frames, Ordering::Release);
        self.audio_dropped_frames
            .store(metrics.dropped_frames, Ordering::Release);
        self.audio_underrun_frames
            .store(metrics.underrun_frames, Ordering::Release);
        self.audio_flushed_frames
            .store(metrics.flushed_frames, Ordering::Release);
        self.audio_endpoint_starvation_events
            .store(metrics.endpoint_starvation_events, Ordering::Release);
        self.audio_submission_failed_frames
            .store(metrics.submission_failed_frames, Ordering::Release);
    }

    fn record_audio_session_metrics(&self, session: &dyn AudioSession) {
        if self.diagnostics_enabled() {
            self.set_audio_metrics(session.metrics());
        }
    }

    fn audio_output_metrics(&self) -> AudioOutputMetrics {
        AudioOutputMetrics {
            sample_rate_hz: self.audio_sample_rate_hz.load(Ordering::Acquire) as u32,
            queue_capacity_frames: self.audio_queue_capacity_frames.load(Ordering::Acquire),
            queue_queued_frames: self.audio_queue_queued_frames.load(Ordering::Acquire),
            queue_high_water_frames: self.audio_queue_high_water_frames.load(Ordering::Acquire),
            accepted_frames: self.audio_accepted_frames.load(Ordering::Acquire),
            consumed_frames: self.audio_consumed_frames.load(Ordering::Acquire),
            dropped_frames: self.audio_dropped_frames.load(Ordering::Acquire),
            underrun_frames: self.audio_underrun_frames.load(Ordering::Acquire),
            underrun_events: self.audio_underruns.load(Ordering::Acquire),
            dropped_blocks: self.audio_dropped_blocks.load(Ordering::Acquire),
            flushed_frames: self.audio_flushed_frames.load(Ordering::Acquire),
            endpoint_starvation_events: self
                .audio_endpoint_starvation_events
                .load(Ordering::Acquire),
            submission_failed_frames: self.audio_submission_failed_frames.load(Ordering::Acquire),
        }
    }

    fn reset_machine_counters(&self) {
        self.total_instructions.store(0, Ordering::Release);
        self.set_frame_number(0);
        self.machine_time_ticks.store(0, Ordering::Release);
        self.reset_frame_timing();
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
    StepInstruction(SyncSender<Result<(), String>>),
    DebugSnapshot(SyncSender<Result<SmsDebugSnapshot, String>>),
    ReadMemory(u16, usize, SyncSender<Result<Vec<u8>, String>>),
    SetTraceEnabled(bool, SyncSender<Result<(), String>>),
    SetSpriteLimitMode(SpriteLimitMode, SyncSender<Result<(), String>>),
    SetBreakpoint(u16, bool, SyncSender<Result<(), String>>),
    SetController(u8, ControllerButtons, Instant),
    RequestNmi,
    ConfigureAudio(AudioSessionFactory, SyncSender<Result<(), String>>),
    SetAudioVolume(u8, SyncSender<Result<(), String>>),
    CloseAudio(SyncSender<()>),
    #[cfg(any(windows, test))]
    SetVideoMailbox(Option<FrameMailbox>, SyncSender<Result<(), String>>),
    Shutdown,
}

/// Owned, immutable SMS/Z80 state captured on the machine worker thread.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmsDebugSnapshot {
    pub pc: u16,
    pub sp: u16,
    pub af: u16,
    pub bc: u16,
    pub de: u16,
    pub hl: u16,
    pub af_alt: u16,
    pub bc_alt: u16,
    pub de_alt: u16,
    pub hl_alt: u16,
    pub ix: u16,
    pub iy: u16,
    pub wz: u16,
    pub instruction_tstates: u32,
    pub i: u8,
    pub r: u8,
    pub iff1: bool,
    pub iff2: bool,
    pub interrupt_mode: u8,
    pub halted: bool,
    pub machine_time_ticks: u128,
    pub frame_number: u64,
    pub vdp_status: u8,
    pub vdp_scanline: u16,
    pub vdp_cycles_into_line: u16,
    pub sprite_limit_mode: SpriteLimitMode,
    pub mapper_banks: [u8; 3],
    pub mapper_control: u8,
    pub memory_control: u8,
    pub framebuffer_rgb: Vec<u8>,
    /// Most recent scheduler/bus events, oldest first.
    pub trace_events: Vec<SmsTemporalTraceEvent>,
    /// Number of trace events discarded after the bounded history filled.
    pub trace_dropped: u64,
    /// Whether runtime trace capture is currently enabled.
    pub trace_enabled: bool,
}

const DEBUG_MEMORY_READ_LIMIT: usize = 256;

/// Handle to the thread that exclusively owns the temporal SMS machine.
pub struct SmsTemporalWorker {
    commands: SyncSender<WorkerCommand>,
    worker: Option<JoinHandle<()>>,
    telemetry: Arc<WorkerTelemetry>,
    #[cfg(windows)]
    video_session: Mutex<Option<D3d12MailboxSession>>,
}

/// Cloneable, lightweight command handle for debugger viewports.
#[derive(Clone)]
pub struct SmsTemporalWorkerDebugHandle {
    commands: SyncSender<WorkerCommand>,
    telemetry: Arc<WorkerTelemetry>,
}

impl SmsTemporalWorkerDebugHandle {
    fn request_unit(
        &self,
        command: impl FnOnce(SyncSender<Result<(), String>>) -> WorkerCommand,
    ) -> Result<(), String> {
        let (reply_sender, reply_receiver) = mpsc::sync_channel(1);
        enqueue_worker_command(&self.commands, command(reply_sender))?;
        receive_worker_reply(reply_receiver, "acknowledging a command")?
    }

    /// Return whether the shared machine worker is currently running.
    pub fn is_running(&self) -> bool {
        self.telemetry.running.load(Ordering::Acquire)
    }

    /// Resume the machine through the shared worker command queue.
    pub fn start(&self) -> Result<(), String> {
        self.request_unit(WorkerCommand::Start)
    }

    /// Pause the machine at the worker's supported command boundary.
    pub fn pause(&self) -> Result<(), String> {
        self.request_unit(WorkerCommand::Pause)
    }

    /// Execute one complete instruction while the machine is paused.
    pub fn step_instruction(&self) -> Result<(), String> {
        self.request_unit(WorkerCommand::StepInstruction)
    }

    /// Add or remove a program-counter breakpoint.
    pub fn set_breakpoint(&self, address: u16, enabled: bool) -> Result<(), String> {
        self.request_unit(|reply| WorkerCommand::SetBreakpoint(address, enabled, reply))
    }

    /// Enable or disable bounded scheduler and bus trace capture.
    pub fn set_trace_enabled(&self, enabled: bool) -> Result<(), String> {
        self.request_unit(|reply| WorkerCommand::SetTraceEnabled(enabled, reply))
    }

    /// Capture an immutable, owned machine snapshot.
    pub fn debug_snapshot(&self) -> Result<SmsDebugSnapshot, String> {
        let (reply_sender, reply_receiver) = mpsc::sync_channel(1);
        enqueue_worker_command(&self.commands, WorkerCommand::DebugSnapshot(reply_sender))?;
        receive_worker_reply(reply_receiver, "returning a debugger snapshot")?
    }

    /// Read at most 256 bytes from the active mapper without advancing emulated time.
    pub fn inspect_memory(&self, start: u16, length: usize) -> Result<Vec<u8>, String> {
        if length > DEBUG_MEMORY_READ_LIMIT {
            return Err(format!(
                "memory inspection is limited to {DEBUG_MEMORY_READ_LIMIT} bytes per request"
            ));
        }
        let (reply_sender, reply_receiver) = mpsc::sync_channel(1);
        enqueue_worker_command(
            &self.commands,
            WorkerCommand::ReadMemory(start, length, reply_sender),
        )?;
        receive_worker_reply(reply_receiver, "returning memory inspection data")?
    }
}

impl SmsTemporalWorker {
    /// Create a cloneable debugger client without transferring machine ownership.
    pub fn debug_handle(&self) -> SmsTemporalWorkerDebugHandle {
        SmsTemporalWorkerDebugHandle {
            commands: self.commands.clone(),
            telemetry: Arc::clone(&self.telemetry),
        }
    }

    /// Start the native worker and wait until the first machine is constructed.
    pub fn new(rom: Vec<u8>, region: &str) -> Result<Self, String> {
        let region = region.to_owned();
        let telemetry = Arc::new(WorkerTelemetry::default());
        if let Ok((_, _, clock_hz)) = sms_temporal_region_profile(&region) {
            telemetry
                .machine_clock_hz
                .store(clock_hz, Ordering::Release);
        }
        let worker_telemetry = Arc::clone(&telemetry);
        let (commands, receiver) = mpsc::sync_channel(WORKER_COMMAND_QUEUE_CAPACITY);
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

        match ready_receiver.recv_timeout(WORKER_STARTUP_TIMEOUT) {
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
            Err(RecvTimeoutError::Timeout) => {
                // The command sender is dropped on return; the detached startup
                // thread then observes a disconnected command receiver and exits.
                drop(worker);
                Err("SMS machine worker startup timed out after 5 seconds".to_owned())
            }
            Err(RecvTimeoutError::Disconnected) => {
                let _ = worker.join();
                Err("SMS machine worker startup channel closed".to_owned())
            }
        }
    }

    fn request_unit(
        &self,
        command: impl FnOnce(SyncSender<Result<(), String>>) -> WorkerCommand,
    ) -> Result<(), String> {
        let (reply_sender, reply_receiver) = mpsc::sync_channel(1);
        enqueue_worker_command(&self.commands, command(reply_sender))?;
        receive_worker_reply(reply_receiver, "acknowledging a command")?
    }

    pub fn load_rom(&self, rom: Vec<u8>) -> Result<(), String> {
        self.request_unit(|reply| WorkerCommand::LoadRom(rom, reply))
    }

    /// Enable or disable optional diagnostic telemetry without changing emulation.
    pub fn set_diagnostics_enabled(&self, enabled: bool) {
        self.telemetry.set_diagnostics_enabled(enabled);
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

    /// Add or remove an instruction-boundary program-counter breakpoint.
    pub fn set_breakpoint(&self, address: u16, enabled: bool) -> Result<(), String> {
        self.request_unit(|reply| WorkerCommand::SetBreakpoint(address, enabled, reply))
    }

    /// Execute one complete CPU instruction while the worker is paused.
    ///
    /// CPU and Scheduler access remain exclusively on the machine-owning thread.
    pub fn step_instruction(&self) -> Result<(), String> {
        self.request_unit(WorkerCommand::StepInstruction)
    }

    /// Capture a coherent, owned debugger snapshot at a worker command boundary.
    pub fn debug_snapshot(&self) -> Result<SmsDebugSnapshot, String> {
        let (reply_sender, reply_receiver) = mpsc::sync_channel(1);
        enqueue_worker_command(&self.commands, WorkerCommand::DebugSnapshot(reply_sender))?;
        receive_worker_reply(reply_receiver, "returning a debugger snapshot")?
    }

    /// Enable or disable bounded scheduler/bus trace capture.
    pub fn set_trace_enabled(&self, enabled: bool) -> Result<(), String> {
        self.request_unit(|reply| WorkerCommand::SetTraceEnabled(enabled, reply))
    }

    /// Change the active SMS VDP sprite rendering limit through the machine worker.
    ///
    /// The setting changes rendering compatibility only; VDP timing, overflow
    /// reporting and Scheduler progression remain owned by the emulated machine.
    pub fn set_sprite_limit_mode(&self, mode: SpriteLimitMode) -> Result<(), String> {
        self.request_unit(|reply| WorkerCommand::SetSpriteLimitMode(mode, reply))
    }

    /// Read at most 256 bytes through the active mapper without advancing time.
    pub fn inspect_memory(&self, start: u16, length: usize) -> Result<Vec<u8>, String> {
        if length > DEBUG_MEMORY_READ_LIMIT {
            return Err(format!(
                "memory inspection is limited to {DEBUG_MEMORY_READ_LIMIT} bytes per request"
            ));
        }
        let (reply_sender, reply_receiver) = mpsc::sync_channel(1);
        enqueue_worker_command(
            &self.commands,
            WorkerCommand::ReadMemory(start, length, reply_sender),
        )?;
        receive_worker_reply(reply_receiver, "returning memory inspection data")?
    }

    pub fn step_frame(&self, max_instructions: usize) -> Result<usize, String> {
        let (reply_sender, reply_receiver) = mpsc::sync_channel(1);
        enqueue_worker_command(
            &self.commands,
            WorkerCommand::StepFrame(max_instructions, reply_sender),
        )?;
        receive_worker_reply_with_timeout(
            reply_receiver,
            "completing a frame step",
            WORKER_FRAME_STEP_TIMEOUT,
        )?
    }

    pub fn set_controller(&self, player: u8, buttons: ControllerButtons) -> Result<(), String> {
        self.set_controller_at(player, buttons, Instant::now())
    }

    /// Apply controller input with the host-detection timestamp used by latency telemetry.
    pub fn set_controller_at(
        &self,
        player: u8,
        buttons: ControllerButtons,
        detected_at: Instant,
    ) -> Result<(), String> {
        enqueue_worker_command(
            &self.commands,
            WorkerCommand::SetController(player, buttons, detected_at),
        )
    }

    /// Return frame-completion and input-to-frame measurements from the SMS worker.
    pub fn performance_metrics(&self) -> SmsPerformanceMetrics {
        self.telemetry.performance_metrics()
    }

    pub fn request_nmi(&self) -> Result<(), String> {
        enqueue_worker_command(&self.commands, WorkerCommand::RequestNmi)
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
        enqueue_worker_command(&self.commands, WorkerCommand::CloseAudio(reply_sender))?;
        receive_worker_reply(reply_receiver, "closing audio output")
    }

    #[cfg(any(windows, test))]
    fn set_video_mailbox(&self, mailbox: Option<FrameMailbox>) -> Result<(), String> {
        let (reply_sender, reply_receiver) = mpsc::sync_channel(1);
        enqueue_worker_command(
            &self.commands,
            WorkerCommand::SetVideoMailbox(mailbox, reply_sender),
        )?;
        receive_worker_reply(reply_receiver, "configuring video output")?
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

    /// Scheduler time in master ticks at the most recently completed worker step.
    pub fn machine_time_ticks(&self) -> u64 {
        self.telemetry.machine_time_ticks.load(Ordering::Acquire)
    }

    /// Configured SMS machine clock in ticks per second for the active region.
    pub fn machine_clock_hz(&self) -> u64 {
        self.telemetry.machine_clock_hz.load(Ordering::Acquire)
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

    pub fn audio_output_metrics(&self) -> AudioOutputMetrics {
        self.telemetry.audio_output_metrics()
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
        self.telemetry.shutdown.store(true, Ordering::Release);
        let _ = enqueue_worker_command(&self.commands, WorkerCommand::Shutdown);
        // The worker observes the shutdown flag at its next safe event-loop
        // boundary and performs audio/mailbox cleanup on its owning thread.
        // Detach here so dropping the host handle cannot stall the GUI on join.
        drop(self.worker.take());
    }
}

#[allow(clippy::too_many_arguments)]
fn execute_frame(
    machine: &mut SmsTemporalMachine,
    breakpoints: &BreakpointSet,
    max_instructions: usize,
    publish_audio: bool,
    audio_session: &mut Option<Box<dyn AudioSession>>,
    video_mailbox: &mut Option<FrameMailbox>,
    video_sequence: &mut u64,
    telemetry: &WorkerTelemetry,
) -> Result<(usize, Option<Instant>, bool), String> {
    let frame_started = Instant::now();
    let diagnostics_enabled = telemetry.diagnostics_enabled();
    let diagnostic_machine_start = diagnostics_enabled.then(|| machine.now().ticks());
    let diagnostic_step_start = diagnostics_enabled.then(Instant::now);
    let (instructions, breakpoint_hit) = machine
        .step_frame_until(max_instructions, |pc| breakpoints.contains(u32::from(pc)))
        .map_err(|error| error.to_string())?;
    let step_frame_elapsed =
        diagnostic_step_start.map_or(Duration::ZERO, |started| started.elapsed());
    let diagnostic_machine_end = diagnostic_machine_start.map(|_| machine.now().ticks());
    let frame_completed = !breakpoint_hit;

    if diagnostics_enabled {
        telemetry
            .total_instructions
            .fetch_add(instructions as u64, Ordering::AcqRel);
    }

    // A breakpoint may stop this call at an instruction boundary before the
    // VDP advances to the next frame. Such a partial execution is not a frame:
    // do not increment frame telemetry or publish its incomplete framebuffer.
    if frame_completed {
        telemetry.record_frame_completed(Instant::now());
    }
    telemetry.set_frame_number(machine.frame_number());
    telemetry.machine_time_ticks.store(
        u64::try_from(machine.now().ticks()).unwrap_or(u64::MAX),
        Ordering::Release,
    );

    if frame_completed {
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
                    if diagnostics_enabled {
                        match mailbox.replaced_frames() {
                            Ok(replaced) => telemetry
                                .video_replaced_frames
                                .store(replaced, Ordering::Release),
                            Err(error) => video_error = Some(error.to_string()),
                        }
                    }
                }
                Err(error) => video_error = Some(error),
            }
        }
        if let Some(error) = video_error {
            telemetry.set_video_error(Some(error));
            *video_mailbox = None;
        }
    }

    // Do not send audio generated by a partial frame stopped at a breakpoint.
    // Drain it through the discard path so paused single-step state stays bounded.
    let samples = machine.drain_audio_samples(None);
    let diagnostic_audio_start = diagnostics_enabled.then(Instant::now);
    let audio_result = audio_session.as_mut().map(|session| {
        if publish_audio && frame_completed {
            session.process_samples(&samples)
        } else {
            session.process_samples_discarding_output(&samples)
        }
    });
    let audio_submit_elapsed =
        diagnostic_audio_start.map_or(Duration::ZERO, |started| started.elapsed());
    match audio_result {
        Some(Ok(())) => {
            if let Some(session) = audio_session.as_ref() {
                telemetry.record_audio_session_metrics(session.as_ref());
            }
        }
        Some(Err(error)) => {
            if let Some(mut failed_session) = audio_session.take() {
                // Preserve the backend's terminal counters before close drops
                // its producer endpoint and makes queue occupancy unavailable.
                telemetry.record_audio_session_metrics(failed_session.as_ref());
                failed_session.close();
            }
            telemetry.set_audio_error(Some(error));
        }
        None => {}
    }

    let sample_count = samples.len();
    let pacing_sample_count = sample_count.max(1) as f64;
    let frame_duration =
        Duration::from_secs_f64(pacing_sample_count / SMS_TEMPORAL_SAMPLE_RATE as f64);
    // Anchor the pacing deadline to the beginning of this frame's host work.
    // Time spent publishing diagnostics after this point must consume the same
    // budget instead of shifting every subsequent frame later.
    let frame_deadline = frame_completed.then(|| frame_started + frame_duration);
    let frame_wait = frame_deadline
        .map(|deadline| deadline.saturating_duration_since(Instant::now()))
        .unwrap_or(Duration::ZERO);

    // Per-frame timing instrumentation is sampled and emitted only for a
    // completed SMS frame, never for a breakpoint-truncated execution.
    if frame_completed
        && let (Some(machine_time_start), Some(machine_time_end)) =
            (diagnostic_machine_start, diagnostic_machine_end)
    {
        sega_observability::tracing::trace!(
            target: "sega::audio::pacing",
            frame_number = machine.frame_number(),
            machine_time_start_ticks = machine_time_start,
            machine_time_end_ticks = machine_time_end,
            machine_time_delta_ticks = machine_time_end.saturating_sub(machine_time_start),
            instructions,
            psg_samples = sample_count,
            step_frame_elapsed_ns = step_frame_elapsed.as_nanos(),
            audio_submit_elapsed_ns = audio_submit_elapsed.as_nanos(),
            frame_processing_elapsed_ns = processing_elapsed.as_nanos(),
            pacing_budget_ns = frame_duration.as_nanos(),
            pacing_wait_ns = frame_wait.as_nanos(),
            publish_audio,
            "SMS frame pacing diagnostic"
        );
    }

    Ok((instructions, frame_deadline, breakpoint_hit))
}

fn worker_loop(
    mut machine: SmsTemporalMachine,
    mut rom: Vec<u8>,
    region: String,
    commands: Receiver<WorkerCommand>,
    telemetry: Arc<WorkerTelemetry>,
) {
    let mut running = false;
    let mut breakpoints = BreakpointSet::default();
    let mut trace_history = BoundedHistory::new(DEBUG_TRACE_CAPACITY);
    let mut trace_enabled = false;
    let mut audio_session: Option<Box<dyn AudioSession>> = None;
    let mut video_mailbox: Option<FrameMailbox> = None;
    let mut video_sequence = 0_u64;
    let mut frame_deadline = FramePacingDeadline::default();

    loop {
        if telemetry.shutdown.load(Ordering::Acquire) {
            break;
        }
        let command = if running {
            let timeout = frame_deadline
                .remaining(Instant::now())
                .unwrap_or(Duration::ZERO);
            match commands.recv_timeout(timeout) {
                Ok(command) => Some(command),
                Err(RecvTimeoutError::Timeout) => {
                    frame_deadline.clear();
                    None
                }
                Err(RecvTimeoutError::Disconnected) => break,
            }
        } else {
            frame_deadline.clear();
            match commands.recv_timeout(WORKER_IDLE_POLL_INTERVAL) {
                Ok(command) => Some(command),
                Err(RecvTimeoutError::Timeout) => continue,
                Err(RecvTimeoutError::Disconnected) => break,
            }
        };

        if let Some(command) = command {
            let was_running = running;
            if !handle_command(
                command,
                &mut machine,
                &mut breakpoints,
                &mut rom,
                &region,
                &mut running,
                &mut audio_session,
                &mut video_mailbox,
                &mut video_sequence,
                &telemetry,
                &mut trace_history,
                &mut trace_enabled,
            ) {
                break;
            }
            // Pause/reset/load/power transitions invalidate the previous
            // frame's wall-clock epoch. Non-pausing commands retain it.
            if was_running != running || !running {
                frame_deadline.clear();
            }
            drain_machine_trace(&mut machine, &mut trace_history);
            continue;
        }

        match execute_frame(
            &mut machine,
            &breakpoints,
            DEFAULT_MAX_FRAME_INSTRUCTIONS,
            true,
            &mut audio_session,
            &mut video_mailbox,
            &mut video_sequence,
            &telemetry,
        ) {
            Ok((_, next_frame_deadline, breakpoint_hit)) => {
                drain_machine_trace(&mut machine, &mut trace_history);
                if breakpoint_hit {
                    running = false;
                    telemetry.running.store(false, Ordering::Release);
                    frame_deadline.clear();
                    if let Some(session) = audio_session.as_mut() {
                        if let Err(error) = session.suspend_and_flush() {
                            telemetry.set_audio_error(Some(error));
                        }
                        if telemetry.diagnostics_enabled() {
                            telemetry.set_audio_metrics(session.metrics());
                        }
                    }
                }
                if running {
                    frame_deadline.set(next_frame_deadline);
                } else {
                    frame_deadline.clear();
                }
            }
            Err(error) => {
                running = false;
                telemetry.running.store(false, Ordering::Release);
                telemetry.set_runtime_error(Some(error));
                if let Some(mut session) = audio_session.take() {
                    session.close();
                }
                telemetry.set_audio_metrics(AudioOutputMetrics::default());
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

fn drain_machine_trace(
    machine: &mut SmsTemporalMachine,
    history: &mut BoundedHistory<SmsTemporalTraceEvent>,
) {
    for event in machine.take_trace() {
        history.push(event);
    }
}

/// Dispatch one coarse worker command over the worker-owned machine and outputs.
///
/// The explicit state references keep machine, audio, video and timing ownership
/// visible at this boundary instead of hiding them in shared mutable globals.
#[allow(clippy::too_many_arguments)]
fn handle_command(
    command: WorkerCommand,
    machine: &mut SmsTemporalMachine,
    breakpoints: &mut BreakpointSet,
    rom: &mut Vec<u8>,
    region: &str,
    running: &mut bool,
    audio_session: &mut Option<Box<dyn AudioSession>>,
    video_mailbox: &mut Option<FrameMailbox>,
    video_sequence: &mut u64,
    telemetry: &WorkerTelemetry,
    trace_history: &mut BoundedHistory<SmsTemporalTraceEvent>,
    trace_enabled: &mut bool,
) -> bool {
    match command {
        WorkerCommand::Start(reply) => {
            let result = audio_session
                .as_mut()
                .map_or(Ok(()), |session| session.resume());
            if let Some(session) = audio_session.as_ref() {
                telemetry.record_audio_session_metrics(session.as_ref());
            }
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
        WorkerCommand::Pause(reply) => {
            *running = false;
            telemetry.running.store(false, Ordering::Release);
            let result = audio_session
                .as_mut()
                .map_or(Ok(()), |session| session.suspend_and_flush());
            if let Some(session) = audio_session.as_ref() {
                telemetry.record_audio_session_metrics(session.as_ref());
            }
            if let Err(error) = &result {
                telemetry.set_audio_error(Some(error.clone()));
            }
            let _ = reply.send(result);
        }
        WorkerCommand::Stop(reply) => {
            // POWER-off is different from Pause: retain the ROM, but cold-reset the machine.
            *running = false;
            telemetry.running.store(false, Ordering::Release);
            let sprite_limit_mode = machine.vdp().sprite_limit_mode();
            let result =
                build_sms_temporal_machine(rom.clone(), region).and_then(|mut powered_off| {
                    powered_off.set_sprite_limit_mode(sprite_limit_mode);
                    audio_session
                        .as_mut()
                        .map_or(Ok::<(), String>(()), |session| {
                            session.suspend_and_flush()?;
                            session.reset_dsp();
                            Ok(())
                        })?;
                    if let Some(session) = audio_session.as_ref() {
                        telemetry.record_audio_session_metrics(session.as_ref());
                    }
                    *machine = powered_off;
                    telemetry.reset_machine_counters();
                    trace_history.clear();
                    Ok(())
                });
            if let Err(error) = &result {
                telemetry.set_runtime_error(Some(error.clone()));
            }
            let _ = reply.send(result);
        }
        WorkerCommand::Reset(reply) => match build_sms_temporal_machine(rom.clone(), region) {
            Ok(reset_machine) => {
                *running = false;
                telemetry.running.store(false, Ordering::Release);
                let result: Result<(), String> = audio_session.as_mut().map_or(Ok(()), |session| {
                    session.suspend_and_flush()?;
                    session.reset_dsp();
                    Ok(())
                });
                if let Some(session) = audio_session.as_ref() {
                    telemetry.record_audio_session_metrics(session.as_ref());
                }
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
                    let result: Result<(), String> =
                        audio_session.as_mut().map_or(Ok(()), |session| {
                            session.suspend_and_flush()?;
                            session.reset_dsp();
                            Ok(())
                        });
                    if let Some(session) = audio_session.as_ref() {
                        telemetry.record_audio_session_metrics(session.as_ref());
                    }
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
        WorkerCommand::StepInstruction(reply) => {
            if *running {
                let _ = reply.send(Err(
                    "step_instruction is available only while the native worker is paused"
                        .to_owned(),
                ));
            } else {
                match machine.step_complete_instruction() {
                    Ok(()) => {
                        if telemetry.diagnostics_enabled() {
                            telemetry.total_instructions.fetch_add(1, Ordering::AcqRel);
                        }
                        telemetry.set_frame_number(machine.frame_number());
                        telemetry.machine_time_ticks.store(
                            u64::try_from(machine.now().ticks()).unwrap_or(u64::MAX),
                            Ordering::Release,
                        );
                        let _ = reply.send(Ok(()));
                    }
                    Err(error) => {
                        let error = error.to_string();
                        telemetry.set_runtime_error(Some(error.clone()));
                        let _ = reply.send(Err(error));
                    }
                }
            }
        }
        WorkerCommand::DebugSnapshot(reply) => {
            let cpu = machine.cpu().export_architectural_state();
            let registers = cpu.registers;
            let alternate = cpu.alternate;
            let snapshot = SmsDebugSnapshot {
                pc: registers.pc,
                sp: registers.sp,
                af: u16::from_be_bytes([registers.a, registers.f]),
                bc: registers.bc(),
                de: registers.de(),
                hl: registers.hl(),
                af_alt: u16::from_be_bytes([alternate.a, alternate.f]),
                bc_alt: u16::from_be_bytes([alternate.b, alternate.c]),
                de_alt: u16::from_be_bytes([alternate.d, alternate.e]),
                hl_alt: u16::from_be_bytes([alternate.h, alternate.l]),
                ix: cpu.ix,
                iy: cpu.iy,
                wz: cpu.wz,
                instruction_tstates: machine.cpu().export_state().instruction_tstates,
                i: cpu.i,
                r: cpu.r,
                iff1: cpu.iff1,
                iff2: cpu.iff2,
                interrupt_mode: cpu.interrupt_mode,
                halted: cpu.halted,
                machine_time_ticks: machine.now().ticks(),
                frame_number: machine.frame_number(),
                vdp_status: machine.vdp().status(),
                vdp_scanline: machine.vdp().scanline(),
                vdp_cycles_into_line: machine.vdp().cycles_into_line(),
                sprite_limit_mode: machine.vdp().sprite_limit_mode(),
                mapper_banks: machine.selected_mapper_banks(),
                mapper_control: machine.mapper_control(),
                memory_control: machine.memory_control(),
                framebuffer_rgb: machine.framebuffer().to_vec(),
                trace_events: trace_history.entries().iter().copied().collect(),
                trace_dropped: trace_history.dropped(),
                trace_enabled: *trace_enabled,
            };
            let _ = reply.send(Ok(snapshot));
        }
        WorkerCommand::SetTraceEnabled(enabled, reply) => {
            machine.set_trace_enabled(enabled);
            *trace_enabled = enabled;
            if !enabled {
                trace_history.clear();
            }
            let _ = reply.send(Ok(()));
        }
        WorkerCommand::SetSpriteLimitMode(mode, reply) => {
            machine.set_sprite_limit_mode(mode);
            let _ = reply.send(Ok(()));
        }
        WorkerCommand::ReadMemory(start, length, reply) => {
            let bytes = (0..length)
                .map(|offset| machine.inspect_memory(start.wrapping_add(offset as u16)))
                .collect();
            let _ = reply.send(Ok(bytes));
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
                    breakpoints,
                    max_instructions,
                    false,
                    audio_session,
                    video_mailbox,
                    video_sequence,
                    telemetry,
                ) {
                    Ok((instructions, _, _)) => {
                        let _ = reply.send(Ok(instructions));
                    }
                    Err(error) => {
                        telemetry.set_runtime_error(Some(error.clone()));
                        let _ = reply.send(Err(error));
                    }
                }
            }
        }
        WorkerCommand::SetBreakpoint(address, enabled, reply) => {
            let result = if enabled {
                if breakpoints.insert(u32::from(address)) {
                    Ok(())
                } else {
                    Err("breakpoint already exists or the 64-address limit was reached".to_owned())
                }
            } else if breakpoints.remove(u32::from(address)) {
                Ok(())
            } else {
                Err("breakpoint address was not configured".to_owned())
            };
            let _ = reply.send(result);
        }
        WorkerCommand::SetController(player, buttons, detected_at) => {
            match machine.set_controller(player, buttons) {
                Ok(()) => telemetry.record_input_detected(detected_at),
                Err(error) => telemetry.set_runtime_error(Some(error.to_string())),
            }
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
                    telemetry.set_audio_metrics(AudioOutputMetrics::default());
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
                            telemetry.record_audio_session_metrics(session.as_ref());
                            *audio_session = Some(session);
                            telemetry.set_audio_error(None);
                            let _ = reply.send(Ok(()));
                        }
                        Err(error) => {
                            session.close();
                            telemetry.set_audio_metrics(AudioOutputMetrics::default());
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
            telemetry.set_audio_metrics(AudioOutputMetrics::default());
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
        BreakpointSet, DEFAULT_MAX_FRAME_INSTRUCTIONS, Ordering, SMS_TEMPORAL_SAMPLE_RATE,
        SmsTemporalWorker, SpriteLimitMode, WorkerTelemetry, build_sms_temporal_machine,
        execute_frame, FramePacingDeadline,
    };
    use crate::SmsTemporalWorkerDebugHandle;
    use sega_audio_runtime::{AudioOutputMetrics, NativeAudioSession, PcmSink};
    use sega_sms_core::ControllerButtons;
    use sega_video_runtime::{FrameMailbox, PixelFormat};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    #[test]
    fn frame_pacing_deadline_preserves_remaining_budget_across_commands() {
        use std::time::Instant;

        let frame_completed = Instant::now();
        let mut pacing = FramePacingDeadline::default();
        pacing.set(Some(frame_completed + Duration::from_millis(16)));

        assert_eq!(
            pacing.remaining(frame_completed),
            Some(Duration::from_millis(16))
        );

        // A command wakes the worker 5 ms into the wait. Its 3 ms handler
        // consumes time from the original budget; it must not restart 16 ms.
        let command_received = frame_completed + Duration::from_millis(5);
        assert_eq!(
            pacing.remaining(command_received),
            Some(Duration::from_millis(11))
        );
        let command_completed = command_received + Duration::from_millis(3);
        assert_eq!(
            pacing.remaining(command_completed),
            Some(Duration::from_millis(8))
        );

        // A second queued command also consumes the same absolute budget.
        let second_command_completed = command_completed + Duration::from_millis(2);
        assert_eq!(
            pacing.remaining(second_command_completed),
            Some(Duration::from_millis(6))
        );
        assert_eq!(
            pacing.remaining(frame_completed + Duration::from_millis(16)),
            Some(Duration::ZERO)
        );
    }

    #[test]
    fn frame_pacing_deadline_clears_for_worker_state_transitions() {
        use std::time::Instant;

        let now = Instant::now();
        let mut pacing = FramePacingDeadline::default();
        pacing.set(Some(now + Duration::from_millis(16)));
        assert!(pacing.remaining(now).is_some());

        // Pause, stop, reset and successful ROM replacement all leave the
        // worker outside its previous running epoch.
        pacing.clear();
        assert_eq!(pacing.remaining(now), None);

        // Starting a new running epoch must not inherit the old deadline.
        pacing.set(Some(now + Duration::from_millis(62)));
        assert_eq!(
            pacing.remaining(now + Duration::from_millis(50)),
            Some(Duration::from_millis(12))
        );
    }

    #[test]
    fn zero_frame_wait_does_not_create_a_deadline() {
        let mut pacing = FramePacingDeadline::default();
        pacing.set(None);
        assert_eq!(pacing.remaining(std::time::Instant::now()), None);
    }

    #[test]
    fn frame_timing_uses_worker_completion_timestamps_not_video_presentation() {
        use super::FrameTimingState;
        use std::time::Instant;

        let start = Instant::now();
        let mut timing = FrameTimingState::default();
        timing.record_frame_completed(start);
        timing.record_frame_completed(start + Duration::from_millis(20));
        assert_eq!(timing.metrics.completed_frames, 2);
        assert_eq!(timing.metrics.frame_time_ms, Some(20.0));
        assert_eq!(timing.metrics.emulation_fps, 0.0);
        timing.record_frame_completed(start + Duration::from_millis(520));
        assert!((timing.metrics.emulation_fps - 3.846154).abs() < 0.0001);
    }

    #[test]
    fn input_latency_ends_at_the_first_completed_sms_frame() {
        use super::FrameTimingState;
        use std::time::Instant;

        let start = Instant::now();
        let mut timing = FrameTimingState::default();
        timing.record_input(start + Duration::from_millis(3));
        timing.record_frame_completed(start + Duration::from_millis(19));
        assert_eq!(timing.metrics.input_to_frame_ms, Some(16.0));
        assert!(timing.pending_input.is_none());
    }

    #[test]
    fn bounded_command_queue_rejects_overflow_without_waiting() {
        let (commands, _receiver) = super::mpsc::sync_channel(1);
        super::enqueue_worker_command(&commands, super::WorkerCommand::RequestNmi).unwrap();
        let error = super::enqueue_worker_command(&commands, super::WorkerCommand::RequestNmi)
            .expect_err("a full queue must reject commands immediately");
        assert!(error.contains("queue is full"));
    }

    #[test]
    fn input_latency_uses_the_latest_controller_state_applied_before_frame() {
        use super::FrameTimingState;
        use std::time::Instant;

        let start = Instant::now();
        let mut timing = FrameTimingState::default();
        timing.record_input(start + Duration::from_millis(3));
        timing.record_input(start + Duration::from_millis(10));
        timing.record_frame_completed(start + Duration::from_millis(19));
        assert_eq!(timing.metrics.input_to_frame_ms, Some(9.0));
    }

    #[test]
    fn emulation_fps_expires_when_no_sms_frame_completes() {
        use super::FrameTimingState;
        use std::time::Instant;

        let start = Instant::now();
        let mut timing = FrameTimingState::default();
        timing.record_frame_completed(start);
        timing.record_frame_completed(start + Duration::from_millis(500));
        assert!(
            timing
                .metrics_at(start + Duration::from_millis(999))
                .emulation_fps
                > 0.0
        );
        assert_eq!(
            timing
                .metrics_at(start + Duration::from_secs(1))
                .emulation_fps,
            0.0
        );
    }

    #[test]
    fn breakpoint_before_frame_completion_does_not_count_or_publish_a_frame() {
        let mut machine = build_sms_temporal_machine(SYNTHETIC_ROM.to_vec(), "usa").unwrap();
        let mut breakpoints = BreakpointSet::default();
        breakpoints.insert(0);
        let mut video_sequence = 0;
        let telemetry = WorkerTelemetry::default();
        let mut audio_session = None;
        let mailbox = FrameMailbox::new();
        let mut video_mailbox = Some(mailbox.clone());

        let (instructions, _, breakpoint_hit) = execute_frame(
            &mut machine,
            &breakpoints,
            DEFAULT_MAX_FRAME_INSTRUCTIONS,
            false,
            &mut audio_session,
            &mut video_mailbox,
            &mut video_sequence,
            &telemetry,
        )
        .unwrap();

        assert_eq!(instructions, 0);
        assert!(breakpoint_hit);
        assert_eq!(machine.frame_number(), 0);
        assert_eq!(telemetry.performance_metrics().completed_frames, 0);
        assert_eq!(telemetry.frame_number.load(Ordering::Acquire), 0);
        assert_eq!(video_sequence, 0);
        assert!(mailbox.take_latest().unwrap().is_none());
    }

    #[test]
    fn input_latency_remains_pending_across_breakpoint_until_a_complete_frame() {
        use std::time::Instant;

        let mut machine = build_sms_temporal_machine(SYNTHETIC_ROM.to_vec(), "usa").unwrap();
        let telemetry = WorkerTelemetry::default();
        telemetry.record_input_detected(Instant::now());

        let mut breakpoint = BreakpointSet::default();
        breakpoint.insert(0);
        let mut video_sequence = 0;
        let mut audio_session = None;
        let mut video_mailbox = None;

        let (_, _, breakpoint_hit) = execute_frame(
            &mut machine,
            &breakpoint,
            DEFAULT_MAX_FRAME_INSTRUCTIONS,
            false,
            &mut audio_session,
            &mut video_mailbox,
            &mut video_sequence,
            &telemetry,
        )
        .unwrap();
        assert!(breakpoint_hit);
        assert_eq!(telemetry.performance_metrics().completed_frames, 0);
        assert!(
            telemetry
                .frame_timing
                .lock()
                .unwrap()
                .pending_input
                .is_some()
        );

        let no_breakpoints = BreakpointSet::default();
        let (_, _, breakpoint_hit) = execute_frame(
            &mut machine,
            &no_breakpoints,
            DEFAULT_MAX_FRAME_INSTRUCTIONS,
            false,
            &mut audio_session,
            &mut video_mailbox,
            &mut video_sequence,
            &telemetry,
        )
        .unwrap();

        assert!(!breakpoint_hit);
        let metrics = telemetry.performance_metrics();
        assert_eq!(metrics.completed_frames, 1);
        assert!(metrics.input_to_frame_ms.is_some());
        assert_eq!(video_sequence, 1);
    }

    #[test]
    fn worker_telemetry_round_trips_all_audio_output_metrics() {
        let telemetry = WorkerTelemetry::default();
        let expected = AudioOutputMetrics {
            sample_rate_hz: 48_000,
            queue_capacity_frames: 2_048,
            queue_queued_frames: 480,
            queue_high_water_frames: 960,
            accepted_frames: 12_000,
            consumed_frames: 11_513,
            dropped_frames: 7,
            underrun_frames: 32,
            underrun_events: 4,
            dropped_blocks: 3,
            flushed_frames: 9,
            endpoint_starvation_events: 2,
            submission_failed_frames: 7,
        };

        telemetry.set_diagnostics_enabled(true);
        telemetry.set_audio_metrics(expected);
        assert_eq!(telemetry.audio_output_metrics(), expected);
    }

    #[test]
    fn optional_diagnostic_telemetry_is_disabled_by_default() {
        let telemetry = WorkerTelemetry::default();
        assert!(!telemetry.diagnostics_enabled());
        telemetry.set_audio_metrics(AudioOutputMetrics {
            sample_rate_hz: 48_000,
            underrun_events: 3,
            ..AudioOutputMetrics::default()
        });
        assert_eq!(
            telemetry.audio_output_metrics(),
            AudioOutputMetrics::default()
        );
    }

    #[test]
    fn disabling_diagnostics_clears_optional_telemetry_counters() {
        let telemetry = WorkerTelemetry::default();
        telemetry.set_diagnostics_enabled(true);
        telemetry.set_audio_metrics(AudioOutputMetrics {
            sample_rate_hz: 48_000,
            underrun_events: 3,
            ..AudioOutputMetrics::default()
        });
        telemetry.total_instructions.store(99, Ordering::Release);
        telemetry.set_diagnostics_enabled(false);
        assert_eq!(
            telemetry.audio_output_metrics(),
            AudioOutputMetrics::default()
        );
        assert_eq!(telemetry.total_instructions.load(Ordering::Acquire), 0);
    }

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

        fn metrics(&self) -> AudioOutputMetrics {
            AudioOutputMetrics::default()
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

        fn metrics(&self) -> AudioOutputMetrics {
            AudioOutputMetrics::default()
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
    fn sprite_limit_mode_changes_only_through_worker_command_queue() {
        let worker = SmsTemporalWorker::new(vec![0; 0x4000], "usa").unwrap();
        assert_eq!(
            worker.debug_snapshot().unwrap().sprite_limit_mode,
            SpriteLimitMode::HardwareLimit
        );

        worker
            .set_sprite_limit_mode(SpriteLimitMode::UnlockedCompatibility)
            .unwrap();
        assert_eq!(
            worker.debug_snapshot().unwrap().sprite_limit_mode,
            SpriteLimitMode::UnlockedCompatibility
        );

        worker
            .set_sprite_limit_mode(SpriteLimitMode::HardwareLimit)
            .unwrap();
        assert_eq!(
            worker.debug_snapshot().unwrap().sprite_limit_mode,
            SpriteLimitMode::HardwareLimit
        );
    }

    #[test]
    fn stop_powers_off_machine_but_keeps_rom_available_for_restart() {
        let worker = SmsTemporalWorker::new(vec![0; 0x4000], "usa").unwrap();
        worker.step_frame(100_000).unwrap();
        assert!(worker.machine_time_ticks() > 0);

        worker.stop().unwrap();
        assert!(!worker.is_running());
        let powered_off = worker.debug_snapshot().unwrap();
        assert_eq!(powered_off.frame_number, 0);
        assert_eq!(powered_off.machine_time_ticks, 0);

        worker.start().unwrap();
        assert!(worker.is_running());
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
    fn single_instruction_step_advances_scheduler_on_paused_worker() {
        let worker = SmsTemporalWorker::new(SYNTHETIC_ROM.to_vec(), "usa").unwrap();
        worker.set_diagnostics_enabled(true);

        let initial_ticks = worker.machine_time_ticks();
        assert_eq!(worker.total_instructions(), 0);
        assert!(!worker.is_running());

        worker.step_instruction().unwrap();

        assert!(!worker.is_running());
        assert_eq!(worker.total_instructions(), 1);
        assert!(worker.machine_time_ticks() > initial_ticks);
    }

    #[test]
    fn single_instruction_step_is_rejected_while_worker_is_running() {
        let worker = SmsTemporalWorker::new(SYNTHETIC_ROM.to_vec(), "usa").unwrap();
        worker.start().unwrap();

        let result = worker.step_instruction();

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .contains("only while the native worker is paused")
        );
        assert!(worker.is_running());
    }

    #[test]
    fn debugger_snapshot_and_mapper_reads_are_owned_and_bounded() {
        let worker = SmsTemporalWorker::new(SYNTHETIC_ROM.to_vec(), "usa").unwrap();
        let initial_ticks = worker.machine_time_ticks();

        let snapshot = worker.debug_snapshot().unwrap();
        assert_eq!(snapshot.pc, 0);
        assert_eq!(snapshot.frame_number, 0);
        assert_eq!(snapshot.machine_time_ticks, 0);
        assert_eq!(snapshot.mapper_banks, [0, 1, 2]);
        assert_eq!(worker.machine_time_ticks(), initial_ticks);

        assert_eq!(worker.inspect_memory(0, 4).unwrap(), [0, 0, 0, 0]);
        assert!(worker.inspect_memory(0, 257).is_err());
        assert_eq!(worker.machine_time_ticks(), initial_ticks);
    }

    #[test]
    fn cloneable_debug_handle_controls_the_same_worker_instance() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<SmsTemporalWorkerDebugHandle>();

        let worker = SmsTemporalWorker::new(SYNTHETIC_ROM.to_vec(), "usa").unwrap();
        let handle = worker.debug_handle();

        assert_eq!(
            handle.debug_snapshot().unwrap(),
            worker.debug_snapshot().unwrap()
        );
        handle.set_breakpoint(0x0000, true).unwrap();
        handle.set_trace_enabled(true).unwrap();
        assert!(handle.debug_snapshot().unwrap().trace_enabled);

        handle.set_trace_enabled(false).unwrap();
        assert!(!worker.debug_snapshot().unwrap().trace_enabled);
    }

    #[test]
    fn trace_capture_is_opt_in_and_exposes_bounded_owned_events() {
        let worker = SmsTemporalWorker::new(SYNTHETIC_ROM.to_vec(), "usa").unwrap();
        let initial = worker.debug_snapshot().unwrap();
        assert!(!initial.trace_enabled);
        assert!(initial.trace_events.is_empty());

        worker.set_trace_enabled(true).unwrap();
        worker.step_instruction().unwrap();
        let captured = worker.debug_snapshot().unwrap();
        assert!(captured.trace_enabled);
        assert!(!captured.trace_events.is_empty());
        assert_eq!(captured.trace_dropped, 0);

        worker.set_trace_enabled(false).unwrap();
        let disabled = worker.debug_snapshot().unwrap();
        assert!(!disabled.trace_enabled);
        assert!(disabled.trace_events.is_empty());
        assert_eq!(disabled.trace_dropped, 0);
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
                let session = NativeAudioSession::new(SMS_TEMPORAL_SAMPLE_RATE as u32, 100, sink)?;
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

        let (instructions, _, _) = execute_frame(
            &mut machine,
            &BreakpointSet::default(),
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

        let (instructions, _, _) = execute_frame(
            &mut machine,
            &BreakpointSet::default(),
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
