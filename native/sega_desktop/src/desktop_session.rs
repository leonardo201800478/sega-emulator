//! Multi-system session boundary for the native desktop host.
//!
//! This module adapts the SMS and SG-1000 workers to the small set of
//! coarse-grained operations shared by the desktop. Hardware execution,
//! scheduler time and device ownership remain inside each runtime worker.

use std::time::Instant;

use sega_sg1000_runtime::AudioOutputMetrics;
use sega_sg1000_runtime::ControllerButtons as Sg1000ControllerButtons;
use sega_sg1000_runtime::Sg1000TemporalWorker;
use sega_sms_core::ControllerButtons as SmsControllerButtons;
use sega_sms_runtime::AudioSession;
use sega_sms_runtime::{
    AnalogOutputConfig, SmsFramePacingMetrics, SmsTemporalWorker, SpriteLimitMode,
};
use sega_video_runtime::FrameMailbox;

/// Explicit system selection used by the desktop ROM-routing boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DesktopSystem {
    /// Sega Master System, first-generation VDP.
    Sms1,
    /// Sega Master System, second-generation VDP.
    Sms2,
    /// Sega SG-1000.
    Sg1000,
}

impl DesktopSystem {
    /// Parse only stable UI identifiers; unknown values never fall back to SMS.
    pub fn from_id(value: &str) -> Result<Self, String> {
        match value {
            "sms1" => Ok(Self::Sms1),
            "sms2" => Ok(Self::Sms2),
            "sg1000" => Ok(Self::Sg1000),
            _ => Err(format!("unsupported desktop system identifier: {value}")),
        }
    }

    /// Return the stable identifier persisted by the Slint desktop.
    pub const fn id(self) -> &'static str {
        match self {
            Self::Sms1 => "sms1",
            Self::Sms2 => "sms2",
            Self::Sg1000 => "sg1000",
        }
    }
}

/// Owns exactly one machine worker, selected explicitly by the host.
pub enum DesktopSession {
    /// Existing SMS runtime and all of its SMS-only capabilities.
    Sms {
        worker: SmsTemporalWorker,
        system: DesktopSystem,
    },
    /// Native temporal SG-1000 runtime.
    Sg1000(Sg1000TemporalWorker),
}

impl DesktopSession {
    /// Identify the runtime currently owned by this session.
    /// The SMS hardware variant is retained explicitly for SMS1/SMS2 identity.
    pub const fn system(&self) -> DesktopSystem {
        match self {
            Self::Sms { system, .. } => *system,
            Self::Sg1000(_) => DesktopSystem::Sg1000,
        }
    }

    /// Whether continuous emulation is active.
    pub fn is_running(&self) -> bool {
        match self {
            Self::Sms { worker, .. } => worker.is_running(),
            Self::Sg1000(worker) => worker.is_running(),
        }
    }

    /// Latest completed video frame number published by the runtime.
    pub fn frame_number(&self) -> u64 {
        match self {
            Self::Sms { worker, .. } => worker.frame_number(),
            Self::Sg1000(worker) => worker.frame_number(),
        }
    }

    /// Start continuous execution at the worker's safe command boundary.
    pub fn start(&self) -> Result<(), String> {
        match self {
            Self::Sms { worker, .. } => worker.start(),
            Self::Sg1000(worker) => worker.start(),
        }
    }

    /// Pause continuous execution without advancing machine time from the host.
    pub fn pause(&self) -> Result<(), String> {
        match self {
            Self::Sms { worker, .. } => worker.pause(),
            Self::Sg1000(worker) => worker.pause(),
        }
    }

    /// Stop execution while retaining the loaded machine and ROM.
    pub fn stop(&self) -> Result<(), String> {
        match self {
            Self::Sms { worker, .. } => worker.stop(),
            Self::Sg1000(worker) => worker.stop(),
        }
    }

    /// Reset the selected machine using its native runtime implementation.
    pub fn reset(&self) -> Result<(), String> {
        match self {
            Self::Sms { worker, .. } => worker.reset(),
            Self::Sg1000(worker) => worker.reset(),
        }
    }

    /// Replace the ROM through the selected runtime's validated worker command.
    pub fn load_rom(&self, rom: Vec<u8>) -> Result<(), String> {
        match self {
            Self::Sms { worker, .. } => worker.load_rom(rom),
            Self::Sg1000(worker) => worker.load_rom(rom),
        }
    }

    /// Execute one complete frame while paused.
    pub fn step_frame(&self, max_instructions: usize) -> Result<usize, String> {
        match self {
            Self::Sms { worker, .. } => worker.step_frame(max_instructions),
            Self::Sg1000(worker) => worker.step_frame(max_instructions),
        }
    }

    /// Apply a normalized host controller snapshot.
    ///
    /// SMS retains its input-detection timestamp for existing latency
    /// telemetry. SG-1000 receives the same logical snapshot through its
    /// worker command queue; no host callback enters the emulated CPU.
    pub fn set_controller_at(
        &self,
        player: u8,
        buttons: SmsControllerButtons,
        detected_at: Instant,
    ) -> Result<(), String> {
        match self {
            Self::Sms { worker, .. } => worker.set_controller_at(player, buttons, detected_at),
            Self::Sg1000(worker) => worker.set_controller(
                player,
                Sg1000ControllerButtons {
                    up: buttons.up,
                    down: buttons.down,
                    left: buttons.left,
                    right: buttons.right,
                    button_1: buttons.button_1,
                    button_2: buttons.button_2,
                },
            ),
        }
    }

    /// Latch the host pause/NMI action in the selected runtime.
    pub fn request_nmi(&self) -> Result<(), String> {
        match self {
            Self::Sms { worker, .. } => worker.request_nmi(),
            Self::Sg1000(worker) => worker.request_nmi(),
        }
    }

    /// Attach the shared, externally-owned presentation mailbox.
    pub fn attach_video_mailbox(&self, mailbox: FrameMailbox) -> Result<(), String> {
        match self {
            Self::Sms { worker, .. } => worker.attach_video_mailbox(mailbox),
            Self::Sg1000(worker) => worker.attach_video_mailbox(mailbox),
        }
    }

    /// Detach the shared mailbox before the host releases its presentation owner.
    pub fn detach_video_mailbox(&self) -> Result<(), String> {
        match self {
            Self::Sms { worker, .. } => worker.detach_video_mailbox(),
            Self::Sg1000(worker) => worker.detach_video_mailbox(),
        }
    }

    /// Configure the shared PCM/DSP contract on the machine-owning worker thread.
    pub fn configure_audio_output<F>(&self, factory: F) -> Result<(), String>
    where
        F: FnOnce() -> Result<Box<dyn AudioSession>, String> + Send + 'static,
    {
        match self {
            Self::Sms { worker, .. } => worker.configure_audio_output(factory),
            Self::Sg1000(worker) => worker.configure_audio_output(factory),
        }
    }

    /// Change output volume without transferring the audio session to the GUI.
    pub fn set_audio_volume(&self, volume: u8) -> Result<(), String> {
        match self {
            Self::Sms { worker, .. } => worker.set_audio_volume(volume),
            Self::Sg1000(worker) => worker.set_audio_volume(volume),
        }
    }

    /// Apply the shared analog output filter configuration.
    pub fn set_audio_dsp_config(&self, config: AnalogOutputConfig) -> Result<(), String> {
        match self {
            Self::Sms { worker, .. } => worker.set_audio_dsp_config(config),
            Self::Sg1000(worker) => worker.set_audio_dsp_config(config),
        }
    }

    /// Close the physical audio session on the owning runtime thread.
    pub fn close_audio_output(&self) -> Result<(), String> {
        match self {
            Self::Sms { worker, .. } => worker.close_audio_output(),
            Self::Sg1000(worker) => worker.close_audio_output(),
        }
    }

    /// Apply the desktop sprite-limit preference using each system's native contract.
    pub fn set_sprite_limit_enabled(&self, enabled: bool) -> Result<(), String> {
        match self {
            Self::Sms { worker, .. } => worker.set_sprite_limit_mode(if enabled {
                SpriteLimitMode::HardwareLimit
            } else {
                SpriteLimitMode::UnlockedCompatibility
            }),
            Self::Sg1000(worker) => worker.set_sprite_limit_enabled(enabled),
        }
    }

    /// Return SMS frame-pacing telemetry when supported by the selected runtime.
    ///
    /// SG-1000 does not currently publish the SMS-specific pacing structure.
    pub fn sms_frame_pacing_metrics(&self) -> SmsFramePacingMetrics {
        match self {
            Self::Sms { worker, .. } => worker.frame_pacing_metrics(),
            Self::Sg1000(_) => SmsFramePacingMetrics::default(),
        }
    }

    /// Return the number of presentation frames replaced before host consumption.
    pub fn video_replaced_frames(&self) -> u64 {
        match self {
            Self::Sms { worker, .. } => worker.video_replaced_frames(),
            Self::Sg1000(_) => 0,
        }
    }

    /// Return the configured machine clock when exposed by the selected runtime.
    pub fn machine_clock_hz(&self) -> u64 {
        match self {
            Self::Sms { worker, .. } => worker.machine_clock_hz(),
            Self::Sg1000(_) => 0,
        }
    }

    /// Return physical audio queue metrics from the selected worker.
    pub fn audio_output_metrics(&self) -> AudioOutputMetrics {
        match self {
            Self::Sms { worker, .. } => worker.audio_output_metrics(),
            Self::Sg1000(worker) => worker.audio_output_metrics(),
        }
    }

    /// Enable SMS diagnostics using the same explicit consent capability as the logger.
    pub fn enable_sms_diagnostics(&self, consent: &DiagnosticsConsent) -> Result<(), String> {
        if let Self::Sms { worker, .. } = self {
            worker.enable_diagnostics(consent);
        }
        Ok(())
    }

    /// Revoke SMS diagnostics and discard all retained trace data.
    pub fn disable_sms_diagnostics(&self) -> Result<(), String> {
        if let Self::Sms { worker, .. } = self {
            worker.disable_diagnostics()?;
        }
        Ok(())
    }

    /// Return the runtime's latest reported audio output error, if any.
    pub fn audio_output_error(&self) -> Option<String> {
        match self {
            Self::Sms { worker, .. } => worker.audio_output_error(),
            Self::Sg1000(worker) => worker.audio_output_error(),
        }
    }

    /// Return the runtime's latest machine execution error, if any.
    pub fn runtime_error(&self) -> Option<String> {
        match self {
            Self::Sms { worker, .. } => worker.runtime_error(),
            Self::Sg1000(worker) => worker.runtime_error(),
        }
    }

    /// Capture a concise system-neutral debugger summary only while central diagnostics consent is active.
    pub fn debug_summary(&self, diagnostics_enabled: bool) -> Result<String, String> {
        if !diagnostics_enabled {
            return Err(
                "Ative Coleta de logs no Debugger antes de capturar um snapshot".to_owned(),
            );
        }
        match self {
            Self::Sms { worker, .. } => worker.debug_snapshot().map(|snapshot| {
                format!(
                    "PC={:04X} SP={:04X} AF={:04X} BC={:04X} DE={:04X} HL={:04X} | frame={} ticks={} T-states={}",
                    snapshot.pc,
                    snapshot.sp,
                    snapshot.af,
                    snapshot.bc,
                    snapshot.de,
                    snapshot.hl,
                    snapshot.frame_number,
                    snapshot.machine_time_ticks,
                    snapshot.instruction_tstates
                )
            }),
            Self::Sg1000(worker) => worker.debug_snapshot().map(|snapshot| {
                format!(
                    "PC={:04X} A={:02X} F={:02X} R={:02X} HALT={} | frame={} T-states={}",
                    snapshot.pc,
                    snapshot.a,
                    snapshot.f,
                    snapshot.refresh,
                    snapshot.halted,
                    snapshot.frame_number,
                    snapshot.total_tstates
                )
            }),
        }
    }

    /// Configure the SMS-only PSG reconstruction mode.
    ///
    /// SG-1000 deliberately rejects this operation rather than silently
    /// accepting a setting that has no corresponding SG-1000 contract.
    pub fn set_sms_audio_renderer(
        &self,
        mode: sega_sms_runtime::AudioRendererMode,
    ) -> Result<(), String> {
        match self {
            Self::Sms { worker, .. } => worker.set_audio_renderer(mode),
            Self::Sg1000(_) => {
                Err("SMS audio renderer settings are unavailable for SG-1000".to_owned())
            }
        }
    }

    /// Configure SMS-only instruction trace capture.
    pub fn set_sms_trace_enabled(&self, enabled: bool) -> Result<(), String> {
        match self {
            Self::Sms { worker, .. } => worker.set_trace_enabled(enabled),
            Self::Sg1000(_) => Err("SMS instruction trace is unavailable for SG-1000".to_owned()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{DesktopSession, DesktopSystem};
    use sega_observability::DiagnosticsConsent;
    use sega_sg1000_runtime::Sg1000TemporalWorker;
    use sega_sms_runtime::SmsTemporalWorker;

    #[test]
    fn system_identifiers_are_explicit_and_unknown_values_do_not_fall_back() {
        assert_eq!(DesktopSystem::from_id("sms1"), Ok(DesktopSystem::Sms1));
        assert_eq!(DesktopSystem::from_id("sms2"), Ok(DesktopSystem::Sms2));
        assert_eq!(DesktopSystem::from_id("sg1000"), Ok(DesktopSystem::Sg1000));
        assert!(DesktopSystem::from_id("unknown").is_err());
    }

    #[test]
    fn session_dispatches_lifecycle_to_the_selected_runtime() {
        let sms = DesktopSession::Sms {
            worker: SmsTemporalWorker::new(vec![0; 0x4000], "usa").unwrap(),
            system: DesktopSystem::Sms1,
        };
        let sg1000 = DesktopSession::Sg1000(Sg1000TemporalWorker::new(vec![0x76]).unwrap());

        for session in [&sms, &sg1000] {
            assert!(!session.is_running());
            session.start().unwrap();
            assert!(session.is_running());
            session.pause().unwrap();
            assert!(!session.is_running());
            session.reset().unwrap();
            session.stop().unwrap();
            assert!(!session.is_running());
        }
    }

    #[test]
    fn sms_hardware_variant_is_preserved_by_the_session_adapter() {
        let sms2 = DesktopSession::Sms {
            worker: SmsTemporalWorker::new(vec![0; 0x4000], "usa").unwrap(),
            system: DesktopSystem::Sms2,
        };

        assert_eq!(sms2.system(), DesktopSystem::Sms2);
    }

    #[test]
    fn controller_snapshot_is_translated_at_the_sg1000_host_boundary() {
        use std::time::Instant;

        use sega_sms_core::ControllerButtons;

        let sg1000 =
            DesktopSession::Sg1000(Sg1000TemporalWorker::new(vec![0xDB, 0xDC, 0x76]).unwrap());
        sg1000
            .set_controller_at(
                1,
                ControllerButtons {
                    up: true,
                    ..ControllerButtons::default()
                },
                Instant::now(),
            )
            .unwrap();
        sg1000.step_frame(20_000).unwrap();

        let DesktopSession::Sg1000(worker) = &sg1000 else {
            unreachable!("test constructed an SG-1000 session");
        };
        assert_eq!(worker.debug_snapshot().unwrap().a, 0xFE);
    }

    #[test]
    fn both_runtimes_publish_native_frames_through_the_shared_mailbox_contract() {
        use sega_video_runtime::PixelFormat;

        let sms = DesktopSession::Sms {
            worker: SmsTemporalWorker::new(vec![0; 0x4000], "usa").unwrap(),
            system: DesktopSystem::Sms1,
        };
        let sg1000 = DesktopSession::Sg1000(Sg1000TemporalWorker::new(vec![0x76]).unwrap());

        for session in [&sms, &sg1000] {
            let mailbox = sega_video_runtime::FrameMailbox::new();
            session.attach_video_mailbox(mailbox.clone()).unwrap();
            session.step_frame(100_000).unwrap();
            let frame = mailbox.take_latest().unwrap().unwrap();

            assert_eq!(frame.width(), 256);
            assert_eq!(frame.height(), 192);
            assert_eq!(frame.pixel_format(), PixelFormat::Rgb888);
            assert_eq!(frame.pixels().len(), 256 * 192 * 3);

            session.detach_video_mailbox().unwrap();
        }
    }

    #[test]
    fn sms_only_operations_fail_explicitly_for_sg1000() {
        let sg1000 = DesktopSession::Sg1000(Sg1000TemporalWorker::new(vec![0x76]).unwrap());

        assert!(
            sg1000
                .set_sms_trace_enabled(true)
                .unwrap_err()
                .contains("unavailable for SG-1000")
        );
    }
}
