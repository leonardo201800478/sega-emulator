//! PyO3 adapter for the Scheduler-driven SMS temporal machine.

use crate::audio::{MachineAudioSession, open_audio_session};
use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use sega_sms_core::{
    ControllerButtons, SmsTemporalMachine, SmsVdpProfile, SmsVdpRevision, SpriteLimitMode,
    Territory, VideoStandard,
};

const SMS_TEMPORAL_SAMPLE_RATE: u64 = 44_100;
const SMS_TEMPORAL_MASTER_TICKS_PER_TSTATE: u64 = 1;

fn sms_temporal_region_profile(
    region: &str,
) -> Result<(Territory, SmsVdpProfile, u64), &'static str> {
    let sms1 = SmsVdpRevision::Sega3155124;
    let sprite_limit = SpriteLimitMode::HardwareLimit;
    match region {
        "usa" => Ok((
            Territory::Export,
            SmsVdpProfile::sms1_t0_ntsc(),
            3_579_545,
        )),
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
) -> Result<SmsTemporalMachine, SmsTemporalMachineBuildError> {
    let (territory, vdp_profile, psg_clock_hz) =
        sms_temporal_region_profile(region).map_err(SmsTemporalMachineBuildError::InvalidRegion)?;
    SmsTemporalMachine::new_with_hardware_profile(
        rom,
        SMS_TEMPORAL_MASTER_TICKS_PER_TSTATE,
        psg_clock_hz,
        SMS_TEMPORAL_SAMPLE_RATE,
        vdp_profile,
        territory,
    )
    .map_err(|error| SmsTemporalMachineBuildError::Machine(error.to_string()))
}

#[derive(Debug)]
enum SmsTemporalMachineBuildError {
    InvalidRegion(&'static str),
    Machine(String),
}

/// Thin PyO3 boundary for the temporal SMS runtime.
#[pyclass(unsendable, module = "sega_z80_core")]
pub struct NativeSMSTemporalMachine {
    machine: SmsTemporalMachine,
    rom: Vec<u8>,
    region: String,
    total_instructions: u64,
    running: bool,
    audio_session: Option<MachineAudioSession>,
    audio_output_error: Option<String>,
}

impl NativeSMSTemporalMachine {
    fn pump_audio_output(&mut self) {
        if self.audio_session.is_some() {
            let samples = self.machine.drain_audio_samples(None);
            let result = match self.audio_session.as_mut() {
                Some(session) => session.process_samples(&samples),
                None => return,
            };
            if let Err(error) = result {
                self.audio_session = None;
                self.audio_output_error = Some(error);
            }
        }
    }

    fn replace_machine(&mut self, rom: Vec<u8>) -> PyResult<()> {
        let machine = build_sms_temporal_machine(rom.clone(), &self.region).map_err(|error| {
            match error {
                SmsTemporalMachineBuildError::InvalidRegion(message) => {
                    PyValueError::new_err(message)
                }
                SmsTemporalMachineBuildError::Machine(message) => {
                    PyRuntimeError::new_err(message)
                }
            }
        })?;
        self.close_audio_output();
        self.machine = machine;
        self.rom = rom;
        self.total_instructions = 0;
        self.running = false;
        Ok(())
    }
}

#[pymethods]
impl NativeSMSTemporalMachine {
    #[new]
    #[pyo3(signature = (rom, region="usa"))]
    fn new(rom: Vec<u8>, region: &str) -> PyResult<Self> {
        let machine = build_sms_temporal_machine(rom.clone(), region).map_err(|error| match error {
            SmsTemporalMachineBuildError::InvalidRegion(message) => PyValueError::new_err(message),
            SmsTemporalMachineBuildError::Machine(message) => PyRuntimeError::new_err(message),
        })?;
        Ok(Self {
            machine,
            rom,
            region: region.to_owned(),
            total_instructions: 0,
            running: false,
            audio_session: None,
            audio_output_error: None,
        })
    }

    /// Load a new ROM and replace the current temporal machine.
    fn load_rom(&mut self, rom: Vec<u8>) -> PyResult<()> {
        self.replace_machine(rom)
    }

    /// Reset the current ROM to its power-on machine state.
    fn reset(&mut self) -> PyResult<()> {
        self.replace_machine(self.rom.clone())
    }

    /// Mark the session as running; frame execution remains explicit/coarse-grained.
    fn start(&mut self) {
        self.running = true;
    }

    /// Pause frame execution without changing machine time or chip state.
    fn pause(&mut self) {
        self.running = false;
    }

    /// Stop frame execution while retaining the loaded ROM.
    fn stop(&mut self) {
        self.running = false;
    }

    #[getter]
    fn is_running(&self) -> bool {
        self.running
    }

    #[getter]
    fn region(&self) -> String {
        self.region.clone()
    }

    #[getter]
    fn total_instructions(&self) -> u64 {
        self.total_instructions
    }

    #[getter]
    fn frame_number(&self) -> u64 {
        self.machine.frame_number()
    }

    /// Execute exactly one complete VDP frame through the Rust Scheduler.
    #[pyo3(signature = (max_instructions=100_000))]
    fn step_frame(&mut self, max_instructions: usize) -> PyResult<usize> {
        if max_instructions == 0 {
            return Err(PyValueError::new_err("max_instructions deve ser positivo"));
        }
        let instructions = self
            .machine
            .step_frame(max_instructions)
            .map_err(|error| PyRuntimeError::new_err(error.to_string()))?;
        self.total_instructions = self
            .total_instructions
            .saturating_add(instructions as u64);
        self.pump_audio_output();
        Ok(instructions)
    }

    /// Drive controller levels; this updates inputs without advancing hardware time.
    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (player, up, down, left, right, button_1, button_2))]
    fn set_controller(
        &mut self,
        player: u8,
        up: bool,
        down: bool,
        left: bool,
        right: bool,
        button_1: bool,
        button_2: bool,
    ) -> PyResult<()> {
        if !matches!(player, 1 | 2) {
            return Err(PyValueError::new_err("player deve ser 1 ou 2"));
        }
        self.machine
            .set_controller(
                player,
                ControllerButtons {
                    up,
                    down,
                    left,
                    right,
                    button_1,
                    button_2,
                },
            )
            .map_err(|error| PyRuntimeError::new_err(error.to_string()))
    }

    /// Assert the SMS Pause switch through the Z80 NMI input.
    fn request_nmi(&mut self) {
        self.machine.request_nmi();
    }

    /// Configure the Rust-owned audio session; PCM never crosses into Python.
    #[pyo3(signature = (backend, sample_rate=48000, buffer_size=512, queue_blocks=4, volume=100, balance=0))]
    fn configure_audio_output(
        &mut self,
        backend: &str,
        sample_rate: u32,
        buffer_size: u32,
        queue_blocks: usize,
        volume: u8,
        balance: i8,
    ) -> PyResult<()> {
        let session = open_audio_session(
            backend,
            sample_rate,
            buffer_size,
            queue_blocks,
            volume,
            balance,
        )
        .map_err(PyRuntimeError::new_err)?;
        self.audio_session = Some(session);
        self.audio_output_error = None;
        Ok(())
    }

    fn close_audio_output(&mut self) {
        if let Some(mut session) = self.audio_session.take() {
            session.close();
        }
        self.audio_output_error = None;
    }

    #[getter]
    fn audio_output_error(&self) -> Option<String> {
        self.audio_output_error.clone()
    }

    #[getter]
    fn audio_output_metrics(&self) -> (u64, u64) {
        self.audio_session
            .as_ref()
            .map(MachineAudioSession::metrics)
            .unwrap_or((0, 0))
    }
}

#[cfg(test)]
mod tests {
    use super::sms_temporal_region_profile;
    use sega_sms_core::{SmsVdpRevision, Territory, VideoStandard};

    #[test]
    fn brazil_uses_pal_m_without_changing_the_sms1_asic() {
        let (territory, profile, clock_hz) = sms_temporal_region_profile("brazil").unwrap();
        assert_eq!(territory, Territory::Export);
        assert_eq!(profile.revision, SmsVdpRevision::Sega3155124);
        assert_eq!(profile.video_standard, VideoStandard::PalM);
        assert_eq!(clock_hz, 3_579_545);
    }

    #[test]
    fn europe_uses_pal_clock_and_japan_uses_japanese_territory() {
        let (_, europe, europe_clock) = sms_temporal_region_profile("europe").unwrap();
        assert_eq!(europe.video_standard, VideoStandard::Pal);
        assert_eq!(europe_clock, 3_546_895);

        let (japan_territory, japan, japan_clock) =
            sms_temporal_region_profile("japan").unwrap();
        assert_eq!(japan_territory, Territory::Japan);
        assert_eq!(japan.video_standard, VideoStandard::Ntsc);
        assert_eq!(japan_clock, 3_579_545);
    }

    #[test]
    fn unsupported_temporal_region_is_rejected() {
        assert!(sms_temporal_region_profile("unknown").is_err());
    }
}
