//! Rust-owned continuous execution worker for the temporal SMS machine.
//!
//! The worker constructs and owns the complete hardware machine on its own
//! thread. Python sends only coarse-grained commands; CPU, Scheduler, VDP,
//! PSG, audio pacing and state transitions remain inside Rust.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender, SyncSender};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crate::audio::{MachineAudioSession, open_audio_session};
use sega_sms_core::{
    ControllerButtons, SmsTemporalMachine, SmsVdpProfile, SmsVdpRevision, SpriteLimitMode,
    Territory, VideoStandard,
};

const SMS_TEMPORAL_SAMPLE_RATE: u64 = 44_100;
const SMS_TEMPORAL_MASTER_TICKS_PER_TSTATE: u64 = 1;
const DEFAULT_MAX_FRAME_INSTRUCTIONS: usize = 100_000;

fn sms_temporal_region_profile(
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

fn build_sms_temporal_machine(
    rom: Vec<u8>,
    region: &str,
) -> Result<SmsTemporalMachine, String> {
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

    fn set_runtime_error(&self, error: Option<String>) {
        if let Ok(mut current) = self.runtime_error.lock() {
            *current = error;
        }
    }

    fn set_audio_metrics(&self, metrics: (u64, u64)) {
        self.audio_underruns.store(metrics.0, Ordering::Release);
        self.audio_dropped_blocks.store(metrics.1, Ordering::Release);
    }

    fn reset_machine_counters(&self) {
        self.total_instructions.store(0, Ordering::Release);
        self.set_frame_number(0);
        self.set_runtime_error(None);
    }
}

struct AudioOutputConfig {
    backend: String,
    sample_rate: u32,
    buffer_size: u32,
    queue_blocks: usize,
    volume: u8,
    balance: i8,
}

enum WorkerCommand {
    Start(SyncSender<Result<(), String>>),
    Pause(SyncSender<Result<(), String>>),
    Stop(SyncSender<Result<(), String>>),
    Reset(SyncSender<Result<(), String>>),
    LoadRom(Vec<u8>, SyncSender<Result<(), String>>),
    StepFrame(usize, SyncSender<Result<usize, String>>),
    SetController(u8, ControllerButtons, SyncSender<Result<(), String>>),
    RequestNmi,
    ConfigureAudio(AudioOutputConfig, SyncSender<Result<(), String>>),
    CloseAudio(SyncSender<()>),
    Shutdown,
}

/// Handle to the thread that exclusively owns the temporal SMS machine.
pub(super) struct SmsTemporalWorker {
    commands: Sender<WorkerCommand>,
    worker: Option<JoinHandle<()>>,
    telemetry: Arc<WorkerTelemetry>,
}

impl SmsTemporalWorker {
    /// Start the native worker and wait until the first machine is constructed.
    pub(super) fn new(rom: Vec<u8>, region: &str) -> Result<Self, String> {
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
                worker_loop(
                    machine,
                    rom,
                    worker_region,
                    receiver,
                    worker_telemetry,
                );
            })
            .map_err(|error| format!("could not start SMS machine worker: {error}"))?;

        match ready_receiver.recv() {
            Ok(Ok(())) => Ok(Self {
                commands,
                worker: Some(worker),
                telemetry,
            }),
            Ok(Err(error)) => {
                let _ = worker.join();
                Err(error)
            }
            Err(error) => {
                let _ = worker.join();
                Err(format!("SMS machine worker startup channel closed: {error}"))
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

    pub(super) fn load_rom(&self, rom: Vec<u8>) -> Result<(), String> {
        self.request_unit(|reply| WorkerCommand::LoadRom(rom, reply))
    }

    pub(super) fn reset(&self) -> Result<(), String> {
        self.request_unit(WorkerCommand::Reset)
    }

    pub(super) fn start(&self) -> Result<(), String> {
        self.request_unit(WorkerCommand::Start)
    }

    pub(super) fn pause(&self) -> Result<(), String> {
        self.request_unit(WorkerCommand::Pause)
    }

    pub(super) fn stop(&self) -> Result<(), String> {
        self.request_unit(WorkerCommand::Stop)
    }

    pub(super) fn step_frame(&self, max_instructions: usize) -> Result<usize, String> {
        let (reply_sender, reply_receiver) = mpsc::sync_channel(1);
        self.commands
            .send(WorkerCommand::StepFrame(max_instructions, reply_sender))
            .map_err(|error| format!("SMS machine worker is unavailable: {error}"))?;
        reply_receiver
            .recv()
            .map_err(|error| format!("SMS machine worker did not return a frame: {error}"))?
    }

    pub(super) fn set_controller(
        &self,
        player: u8,
        buttons: ControllerButtons,
    ) -> Result<(), String> {
        self.request_unit(|reply| WorkerCommand::SetController(player, buttons, reply))
    }

    pub(super) fn request_nmi(&self) -> Result<(), String> {
        self.commands
            .send(WorkerCommand::RequestNmi)
            .map_err(|error| format!("SMS machine worker is unavailable: {error}"))
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn configure_audio_output(
        &self,
        backend: &str,
        sample_rate: u32,
        buffer_size: u32,
        queue_blocks: usize,
        volume: u8,
        balance: i8,
    ) -> Result<(), String> {
        let config = AudioOutputConfig {
            backend: backend.to_owned(),
            sample_rate,
            buffer_size,
            queue_blocks,
            volume,
            balance,
        };
        self.request_unit(|reply| WorkerCommand::ConfigureAudio(config, reply))
    }

    pub(super) fn close_audio_output(&self) -> Result<(), String> {
        let (reply_sender, reply_receiver) = mpsc::sync_channel(1);
        self.commands
            .send(WorkerCommand::CloseAudio(reply_sender))
            .map_err(|error| format!("SMS machine worker is unavailable: {error}"))?;
        reply_receiver
            .recv()
            .map_err(|error| format!("SMS machine worker did not close audio: {error}"))
    }

    pub(super) fn is_running(&self) -> bool {
        self.telemetry.running.load(Ordering::Acquire)
    }

    pub(super) fn frame_number(&self) -> u64 {
        self.telemetry.frame_number.load(Ordering::Acquire)
    }

    pub(super) fn total_instructions(&self) -> u64 {
        self.telemetry.total_instructions.load(Ordering::Acquire)
    }

    pub(super) fn audio_output_error(&self) -> Option<String> {
        self.telemetry
            .audio_output_error
            .lock()
            .ok()
            .and_then(|error| error.clone())
    }

    pub(super) fn runtime_error(&self) -> Option<String> {
        self.telemetry
            .runtime_error
            .lock()
            .ok()
            .and_then(|error| error.clone())
    }

    pub(super) fn audio_output_metrics(&self) -> (u64, u64) {
        (
            self.telemetry.audio_underruns.load(Ordering::Acquire),
            self.telemetry
                .audio_dropped_blocks
                .load(Ordering::Acquire),
        )
    }

    #[cfg(test)]
    fn wait_for_frame_after(&self, frame_number: u64, timeout: Duration) -> bool {
        let Ok(observed) = self.telemetry.frame_signal.0.lock() else {
            return false;
        };
        let Ok((observed, timeout_result)) = self
            .telemetry
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
        let _ = self.commands.send(WorkerCommand::Shutdown);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn execute_frame(
    machine: &mut SmsTemporalMachine,
    max_instructions: usize,
    audio_session: &mut Option<MachineAudioSession>,
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

    let samples = machine.drain_audio_samples(None);
    if let Some(session) = audio_session.as_mut()
        && let Err(error) = session.process_samples(&samples)
    {
        if let Some(mut failed_session) = audio_session.take() {
            failed_session.close();
        }
        telemetry.set_audio_error(Some(error));
        telemetry.set_audio_metrics((0, 0));
    } else if let Some(session) = audio_session.as_ref() {
        telemetry.set_audio_metrics(session.metrics());
    }

    let sample_count = samples.len().max(1) as f64;
    let frame_duration = Duration::from_secs_f64(
        sample_count / SMS_TEMPORAL_SAMPLE_RATE as f64,
    );
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
    let mut audio_session: Option<MachineAudioSession> = None;

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
                &telemetry,
            ) {
                break;
            }
            continue;
        }

        match execute_frame(
            &mut machine,
            DEFAULT_MAX_FRAME_INSTRUCTIONS,
            &mut audio_session,
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
}

fn handle_command(
    command: WorkerCommand,
    machine: &mut SmsTemporalMachine,
    rom: &mut Vec<u8>,
    region: &str,
    running: &mut bool,
    audio_session: &mut Option<MachineAudioSession>,
    telemetry: &WorkerTelemetry,
) -> bool {
    match command {
        WorkerCommand::Start(reply) => {
            *running = true;
            telemetry.running.store(true, Ordering::Release);
            let _ = reply.send(Ok(()));
        }
        WorkerCommand::Pause(reply) | WorkerCommand::Stop(reply) => {
            *running = false;
            telemetry.running.store(false, Ordering::Release);
            let _ = reply.send(Ok(()));
        }
        WorkerCommand::Reset(reply) => match build_sms_temporal_machine(rom.clone(), region) {
            Ok(reset_machine) => {
                *machine = reset_machine;
                telemetry.reset_machine_counters();
                let _ = reply.send(Ok(()));
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
                    *machine = new_machine;
                    *rom = new_rom;
                    telemetry.reset_machine_counters();
                    let _ = reply.send(Ok(()));
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
                match execute_frame(machine, max_instructions, audio_session, telemetry) {
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
        WorkerCommand::ConfigureAudio(config, reply) => {
            match open_audio_session(
                &config.backend,
                config.sample_rate,
                config.buffer_size,
                config.queue_blocks,
                config.volume,
                config.balance,
            ) {
                Ok(session) => {
                    if let Some(mut previous) = audio_session.replace(session) {
                        previous.close();
                    }
                    telemetry.set_audio_error(None);
                    telemetry.set_audio_metrics((0, 0));
                    let _ = reply.send(Ok(()));
                }
                Err(error) => {
                    telemetry.set_audio_error(Some(error.clone()));
                    let _ = reply.send(Err(error));
                }
            }
        }
        WorkerCommand::CloseAudio(reply) => {
            if let Some(mut session) = audio_session.take() {
                session.close();
            }
            telemetry.set_audio_error(None);
            telemetry.set_audio_metrics((0, 0));
            let _ = reply.send(());
        }
        WorkerCommand::Shutdown => {
            *running = false;
            telemetry.running.store(false, Ordering::Release);
            if let Some(mut session) = audio_session.take() {
                session.close();
            }
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::SmsTemporalWorker;
    use sega_sms_core::ControllerButtons;
    use std::time::Duration;

    const SYNTHETIC_ROM: [u8; 0x4000] = [0; 0x4000];

    #[test]
    fn native_worker_runs_pauses_and_steps_without_python_callbacks() {
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
    fn unknown_audio_backend_is_reported_without_stopping_the_machine() {
        let worker = SmsTemporalWorker::new(SYNTHETIC_ROM.to_vec(), "usa").unwrap();
        assert!(
            worker
                .configure_audio_output("unsupported", 48_000, 512, 4, 100, 0)
                .is_err()
        );
        assert!(!worker.is_running());
    }
}
