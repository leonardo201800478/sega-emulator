//! PyO3 adapter for the independent SMS core and native PSG audio path.

use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use pyo3::types::PyBytes;
use sega_sms_core::{ControllerButtons, SmsMachine, SmsMachineConfig, VideoStandard};

use crate::audio::NativePcmResampler;

/// Resolve the territory and video standard without constructing Python errors.
///
/// Keeping this policy independent of PyO3 also makes it safe to unit-test in
/// Rust's parallel test harness, where no Python interpreter is guaranteed.
fn sms_region_profile(
    region: Option<&str>,
    pal: bool,
) -> Result<(sega_sms_core::Territory, VideoStandard), &'static str> {
    let region = region.unwrap_or(if pal { "europe" } else { "usa" });
    match region {
        "usa" => Ok((sega_sms_core::Territory::Export, VideoStandard::Ntsc)),
        "brazil" => Ok((sega_sms_core::Territory::Export, VideoStandard::PalM)),
        "europe" => Ok((sega_sms_core::Territory::Export, VideoStandard::Pal)),
        "japan" => Ok((sega_sms_core::Territory::Japan, VideoStandard::Ntsc)),
        _ => Err("region deve ser usa, brazil, europe ou japan"),
    }
}

/// Construct the SMS hardware without invoking the Python interpreter.
///
/// This is shared by the PyO3 constructor and native Rust tests so the test
/// harness never creates Python exceptions or requires interpreter lifecycle.
fn build_sms_machine(
    rom: Vec<u8>,
    pal: bool,
    region: Option<&str>,
    bios: Option<Vec<u8>>,
) -> Result<SmsMachine, SmsMachineBuildError> {
    let (territory, video_standard) =
        sms_region_profile(region, pal).map_err(SmsMachineBuildError::InvalidRegion)?;
    let config = SmsMachineConfig {
        territory,
        video_standard,
        ..SmsMachineConfig::default()
    };
    SmsMachine::with_bios_audio(rom, bios, config, 44_100)
        .map_err(|error| SmsMachineBuildError::Machine(error.to_string()))
}

#[derive(Debug)]
enum SmsMachineBuildError {
    InvalidRegion(&'static str),
    Machine(String),
}

/// Thin Python boundary; the SMS machine, PSG and sample generation stay in Rust.
#[pyclass(module = "sega_z80_core")]
pub struct NativeSMSMachine {
    machine: SmsMachine,
    total_instructions: u64,
}

#[pymethods]
impl NativeSMSMachine {
    #[new]
    #[pyo3(signature = (rom, pal=false, region=None, bios=None))]
    fn new(rom: Vec<u8>, pal: bool, region: Option<&str>, bios: Option<Vec<u8>>) -> PyResult<Self> {
        let machine = build_sms_machine(rom, pal, region, bios).map_err(|error| match error {
            SmsMachineBuildError::InvalidRegion(message) => {
                pyo3::exceptions::PyValueError::new_err(message)
            }
            SmsMachineBuildError::Machine(message) => PyRuntimeError::new_err(message),
        })?;
        Ok(Self {
            machine,
            total_instructions: 0,
        })
    }

    #[getter]
    fn bios_enabled(&self) -> bool {
        self.machine.memory().bios_enabled()
    }

    #[getter]
    fn total_instructions(&self) -> u64 {
        self.total_instructions
    }

    #[getter]
    fn total_tstates(&self) -> u64 {
        self.machine.cpu().total_tstates
    }

    #[getter]
    fn pc(&self) -> u16 {
        self.machine.cpu().registers.pc
    }

    #[getter]
    fn frame_number(&self) -> u64 {
        self.machine.vdp().frame_number()
    }

    #[getter]
    fn visible_height(&self) -> u16 {
        self.machine.vdp().visible_height()
    }

    #[getter]
    fn scanline(&self) -> u16 {
        self.machine.vdp().scanline()
    }

    #[getter]
    fn vdp_status(&self) -> u8 {
        self.machine.vdp().status()
    }

    #[getter]
    fn framebuffer<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        PyBytes::new(py, self.machine.vdp().framebuffer())
    }

    #[getter]
    fn psg_clock_hz(&self) -> u64 {
        self.machine.psg().clock_hz()
    }

    #[getter]
    fn psg_sample_rate(&self) -> u64 {
        self.machine.psg().sample_rate()
    }

    #[getter]
    fn psg_tone_periods(&self) -> (u16, u16, u16) {
        let periods = self.machine.psg().tone_periods();
        (periods[0], periods[1], periods[2])
    }

    #[getter]
    fn psg_attenuations(&self) -> (u8, u8, u8, u8) {
        let levels = self.machine.psg().attenuations();
        (levels[0], levels[1], levels[2], levels[3])
    }

    /// Enable bounded architectural tracing for the next SMS CPU instructions.
    #[pyo3(signature = (max_instructions=256))]
    fn enable_instruction_trace(&mut self, max_instructions: usize) -> PyResult<usize> {
        if max_instructions == 0 {
            return Err(pyo3::exceptions::PyValueError::new_err(
                "max_instructions deve ser positivo",
            ));
        }
        Ok(self.machine.enable_instruction_trace(max_instructions))
    }

    /// Drain captured instruction traces as JSON Lines. Records include executed ROM bytes.
    fn drain_instruction_trace_json(&mut self) -> String {
        let traces = self.machine.drain_instruction_traces();
        let mut output = String::new();
        for trace in traces {
            trace
                .write_json_line(&mut output)
                .expect("writing instruction trace to String cannot fail");
            output.push('\n');
        }
        output
    }

    fn step(&mut self) -> PyResult<u32> {
        let cycles = self
            .machine
            .step()
            .map_err(|error| PyRuntimeError::new_err(format!("SMS CPU step failed: {error:?}")))?;
        self.total_instructions = self.total_instructions.saturating_add(1);
        Ok(cycles)
    }

    /// Advance instruction-stepped SMS hardware until the VDP frame counter changes.
    ///
    /// This is nominal instruction timing, not cycle-exact bus scheduling.
    #[pyo3(signature = (max_steps=100_000))]
    fn run_frame(&mut self, max_steps: usize) -> PyResult<usize> {
        if max_steps == 0 {
            return Err(pyo3::exceptions::PyValueError::new_err(
                "max_steps deve ser positivo",
            ));
        }
        let initial_frame = self.machine.vdp().frame_number();
        for steps in 1..=max_steps {
            self.step()?;
            if self.machine.vdp().frame_number() != initial_frame {
                return Ok(steps);
            }
        }
        Err(PyRuntimeError::new_err(
            "SMS não concluiu um quadro dentro do limite de instruções",
        ))
    }

    // Preserve a flat, explicit Python-facing controller API; the booleans map directly to SMS input lines.
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
            return Err(pyo3::exceptions::PyValueError::new_err(
                "player deve ser 1 ou 2",
            ));
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

    fn drain_audio_samples(&mut self, max_samples: Option<usize>) -> Vec<f64> {
        self.machine.drain_audio_samples(max_samples)
    }

    fn drain_audio_pcm<'py>(
        &mut self,
        py: Python<'py>,
        mut resampler: PyRefMut<'_, NativePcmResampler>,
    ) -> Bound<'py, PyBytes> {
        let samples = self.machine.drain_audio_samples(None);
        PyBytes::new(py, &resampler.process_native(&samples))
    }

    fn discard_audio_samples(&mut self) {
        let _ = self.machine.drain_audio_samples(None);
    }

    fn read_memory(&self, address: u16) -> u8 {
        self.machine.read_memory(address)
    }

    fn read_port(&self, address: u16) -> u8 {
        self.machine.read_port(address)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sms_adapter_uses_native_psg_and_reports_configured_clock() {
        let machine = NativeSMSMachine {
            machine: build_sms_machine(vec![0; 16], false, Some("usa"), None).unwrap(),
            total_instructions: 0,
        };
        assert_eq!(machine.psg_sample_rate(), 44_100);
        assert_eq!(machine.psg_clock_hz(), 3_579_545);
        assert_eq!(machine.total_tstates(), 0);
    }

    #[test]
    fn region_selects_pal_clock_and_japan_territory_profile() {
        let pal_machine = NativeSMSMachine {
            machine: build_sms_machine(vec![0; 16], false, Some("europe"), None).unwrap(),
            total_instructions: 0,
        };
        assert_eq!(pal_machine.psg_clock_hz(), 3_546_895);

        let japan_machine = NativeSMSMachine {
            machine: build_sms_machine(vec![0; 16], false, Some("japan"), None).unwrap(),
            total_instructions: 0,
        };
        assert_eq!(japan_machine.psg_clock_hz(), 3_579_545);
    }

    #[test]
    fn bios_is_forwarded_to_native_memory_and_executes() {
        let mut bios = vec![0; 0x2000];
        bios[..4].copy_from_slice(&[0x3E, 0x08, 0xD3, 0x3E]);
        let machine = NativeSMSMachine {
            machine: build_sms_machine(vec![0xC3; 0x4000], false, Some("usa"), Some(bios)).unwrap(),
            total_instructions: 0,
        };
        assert!(machine.bios_enabled());
        assert_eq!(machine.read_memory(0), 0x3E);
    }

    #[test]
    fn brazil_region_selects_export_territory_with_pal_m_timing() {
        let machine = build_sms_machine(vec![0; 16], false, Some("brazil"), None).unwrap();
        assert_eq!(machine.vdp().system_clock_hz(), 3_579_545);
        assert_eq!(machine.vdp().video_standard(), VideoStandard::PalM);
        assert_eq!(machine.vdp().total_lines(), 262);
    }

    #[test]
    fn unsupported_region_is_rejected_without_initializing_python() {
        assert!(matches!(
            build_sms_machine(vec![0; 16], false, Some("automatic"), None),
            Err(SmsMachineBuildError::InvalidRegion(_))
        ));
    }

    #[test]
    fn draining_audio_does_not_change_cpu_timing() {
        let mut machine = NativeSMSMachine::new(vec![0; 16], false, Some("usa"), None).unwrap();
        for _ in 0..100 {
            machine.step().unwrap();
        }
        let before = machine.total_tstates();
        let _ = machine.drain_audio_samples(None);
        assert_eq!(machine.total_tstates(), before);
    }

    #[test]
    fn frame_runner_advances_native_vdp_and_exposes_frame_state() {
        let mut machine = NativeSMSMachine::new(vec![0; 16], false, Some("usa"), None).unwrap();
        assert_eq!(machine.frame_number(), 0);
        let steps = machine.run_frame(100_000).unwrap();
        assert!(steps > 0 && steps <= 100_000);
        assert_eq!(machine.frame_number(), 1);
        assert_eq!(machine.visible_height(), 192);
        assert!(machine.total_tstates() > 0);
    }

    #[test]
    fn controller_rejects_unknown_player() {
        let mut machine = NativeSMSMachine::new(vec![0; 16], false, Some("usa"), None).unwrap();
        assert!(
            machine
                .set_controller(3, false, false, false, false, false, false)
                .is_err()
        );
        assert!(
            machine
                .set_controller(1, true, false, false, false, true, false)
                .is_ok()
        );
    }
}
