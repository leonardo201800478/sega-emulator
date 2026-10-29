//! PyO3 adapter for the independent SMS core and native PSG audio path.

use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use pyo3::types::PyBytes;
use sega_sms_core::{ControllerButtons, SmsMachine, SmsMachineConfig, VideoStandard};

use crate::audio::NativePcmResampler;

/// Thin Python boundary; the SMS machine, PSG and sample generation stay in Rust.
#[pyclass(module = "sega_z80_core")]
pub struct NativeSMSMachine {
    machine: SmsMachine,
    total_instructions: u64,
}

#[pymethods]
impl NativeSMSMachine {
    #[new]
    #[pyo3(signature = (rom, region="usa"))]
    fn new(rom: Vec<u8>, region: &str) -> PyResult<Self> {
        let (territory, video_standard) = match region {
            "usa" => (sega_sms_core::Territory::Export, VideoStandard::Ntsc),
            "europe" => (sega_sms_core::Territory::Export, VideoStandard::Pal),
            "japan" => (sega_sms_core::Territory::Japan, VideoStandard::Ntsc),
            _ => return Err(pyo3::exceptions::PyValueError::new_err(
                "region deve ser usa, europe ou japan",
            )),
        };
        let config = SmsMachineConfig {
            territory,
            video_standard,
            ..SmsMachineConfig::default()
        };
        // The shared native resampler consumes a deterministic 44.1 kHz PSG stream.
        let machine = SmsMachine::with_audio(rom, config, 44_100)
            .map_err(|error| PyRuntimeError::new_err(error.to_string()))?;
        Ok(Self {
            machine,
            total_instructions: 0,
        })
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
        let machine = NativeSMSMachine::new(vec![0; 16], "usa").unwrap();
        assert_eq!(machine.psg_sample_rate(), 44_100);
        assert_eq!(machine.psg_clock_hz(), 3_579_545);
        assert_eq!(machine.total_tstates(), 0);
    }

    #[test]
    fn region_selects_pal_clock_and_japan_territory_profile() {
        let pal_machine = NativeSMSMachine::new(vec![0; 16], "europe").unwrap();
        assert_eq!(pal_machine.psg_clock_hz(), 3_546_893);

        let japan_machine = NativeSMSMachine::new(vec![0; 16], "japan").unwrap();
        assert_eq!(japan_machine.psg_clock_hz(), 3_579_545);
    }

    #[test]
    fn unsupported_region_is_rejected() {
        assert!(NativeSMSMachine::new(vec![0; 16], "automatic").is_err());
    }

    #[test]
    fn draining_audio_does_not_change_cpu_timing() {
        let mut machine = NativeSMSMachine::new(vec![0; 16], false).unwrap();
        for _ in 0..100 {
            machine.step().unwrap();
        }
        let before = machine.total_tstates();
        let _ = machine.drain_audio_samples(None);
        assert_eq!(machine.total_tstates(), before);
    }

    #[test]
    fn frame_runner_advances_native_vdp_and_exposes_frame_state() {
        let mut machine = NativeSMSMachine::new(vec![0; 16], false).unwrap();
        assert_eq!(machine.frame_number(), 0);
        let steps = machine.run_frame(100_000).unwrap();
        assert!(steps > 0 && steps <= 100_000);
        assert_eq!(machine.frame_number(), 1);
        assert_eq!(machine.visible_height(), 192);
        assert!(machine.total_tstates() > 0);
    }

    #[test]
    fn controller_rejects_unknown_player() {
        let mut machine = NativeSMSMachine::new(vec![0; 16], false).unwrap();
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
