//! PyO3 command boundary for the Rust-owned temporal SMS worker.

use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use sega_sms_core::ControllerButtons;

use crate::sms_temporal_worker::{SmsTemporalWorker, sms_temporal_region_profile};

/// Coarse-grained Python boundary; the hardware machine lives on a Rust worker.
#[pyclass(unsendable, module = "sega_z80_core")]
pub struct NativeSMSTemporalMachine {
    worker: SmsTemporalWorker,
    region: String,
}

#[pymethods]
impl NativeSMSTemporalMachine {
    #[new]
    #[pyo3(signature = (rom, region="usa"))]
    fn new(rom: Vec<u8>, region: &str) -> PyResult<Self> {
        sms_temporal_region_profile(region).map_err(PyValueError::new_err)?;
        let worker = SmsTemporalWorker::new(rom, region).map_err(PyRuntimeError::new_err)?;
        Ok(Self {
            worker,
            region: region.to_owned(),
        })
    }

    /// Load a new cartridge on the Rust execution thread.
    fn load_rom(&mut self, rom: Vec<u8>) -> PyResult<()> {
        self.worker.load_rom(rom).map_err(PyRuntimeError::new_err)
    }

    /// Rebuild the loaded cartridge at its power-on state on the Rust worker.
    fn reset(&mut self) -> PyResult<()> {
        self.worker.reset().map_err(PyRuntimeError::new_err)
    }

    /// Start continuous execution on the dedicated Rust worker thread.
    fn start(&mut self) -> PyResult<()> {
        self.worker.start().map_err(PyRuntimeError::new_err)
    }

    /// Pause at a completed frame boundary without advancing emulated time.
    fn pause(&mut self) -> PyResult<()> {
        self.worker.pause().map_err(PyRuntimeError::new_err)
    }

    /// Stop execution while retaining the loaded cartridge and worker.
    fn stop(&mut self) -> PyResult<()> {
        self.worker.stop().map_err(PyRuntimeError::new_err)
    }

    #[getter]
    fn is_running(&self) -> bool {
        self.worker.is_running()
    }

    #[getter]
    fn region(&self) -> String {
        self.region.clone()
    }

    #[getter]
    fn total_instructions(&self) -> u64 {
        self.worker.total_instructions()
    }

    #[getter]
    fn frame_number(&self) -> u64 {
        self.worker.frame_number()
    }

    /// Execute one complete frame synchronously, only while the worker is paused.
    #[pyo3(signature = (max_instructions=100_000))]
    fn step_frame(&mut self, max_instructions: usize) -> PyResult<usize> {
        if max_instructions == 0 {
            return Err(PyValueError::new_err("max_instructions deve ser positivo"));
        }
        self.worker
            .step_frame(max_instructions)
            .map_err(PyRuntimeError::new_err)
    }

    /// Send controller levels to the worker; no Python-side clocking occurs.
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
        self.worker
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
            .map_err(PyRuntimeError::new_err)
    }

    /// Assert the SMS Pause switch through the Z80 NMI input.
    fn request_nmi(&mut self) -> PyResult<()> {
        self.worker.request_nmi().map_err(PyRuntimeError::new_err)
    }

    /// Configure audio output on the Rust worker; PCM never crosses into Python.
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
        self.worker
            .configure_audio_output(
                backend,
                sample_rate,
                buffer_size,
                queue_blocks,
                volume,
                balance,
            )
            .map_err(PyRuntimeError::new_err)
    }

    fn close_audio_output(&mut self) -> PyResult<()> {
        self.worker
            .close_audio_output()
            .map_err(PyRuntimeError::new_err)
    }

    /// Open the standalone native D3D12 presentation window.
    fn open_video_output(&mut self) -> PyResult<()> {
        self.worker
            .open_video_output()
            .map_err(PyRuntimeError::new_err)
    }

    /// Attach native D3D12 presentation to an existing GUI-owned HWND.
    ///
    /// The caller must invoke this on the HWND's owning thread and keep the
    /// widget alive until close_video_output() has returned.
    fn open_video_output_in_viewport(&mut self, window_handle: usize) -> PyResult<()> {
        self.worker
            .open_video_output_in_viewport(window_handle)
            .map_err(PyRuntimeError::new_err)
    }

    /// Close the native video window and detach its completed-frame mailbox.
    fn close_video_output(&mut self) -> PyResult<()> {
        self.worker
            .close_video_output()
            .map_err(PyRuntimeError::new_err)
    }

    #[getter]
    fn video_output_is_running(&self) -> bool {
        self.worker.video_output_is_running()
    }

    #[getter]
    fn video_output_error(&self) -> Option<String> {
        self.worker.video_output_error()
    }

    #[getter]
    fn video_last_published_sequence(&self) -> u64 {
        self.worker.video_last_published_sequence()
    }

    #[getter]
    fn video_last_presented_sequence(&self) -> u64 {
        self.worker.video_last_presented_sequence()
    }

    #[getter]
    fn video_replaced_frames(&self) -> u64 {
        self.worker.video_replaced_frames()
    }

    #[getter]
    fn video_device_recovery_count(&self) -> u64 {
        self.worker.video_device_recovery_count()
    }

    #[getter]
    fn audio_output_error(&self) -> Option<String> {
        self.worker.audio_output_error()
    }

    #[getter]
    fn runtime_error(&self) -> Option<String> {
        self.worker.runtime_error()
    }

    #[getter]
    fn audio_output_metrics(&self) -> (u64, u64) {
        self.worker.audio_output_metrics()
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

        let (japan_territory, japan, japan_clock) = sms_temporal_region_profile("japan").unwrap();
        assert_eq!(japan_territory, Territory::Japan);
        assert_eq!(japan.video_standard, VideoStandard::Ntsc);
        assert_eq!(japan_clock, 3_579_545);
    }

    #[test]
    fn unsupported_temporal_region_is_rejected() {
        assert!(sms_temporal_region_profile("unknown").is_err());
    }
}
