//! Thin PyO3 adapter over the consolidated, Python-independent SG-1000 core.
//!
//! This module owns no CPU, memory, VDP or PSG implementation. It exposes
//! snapshots and machine operations from sega_sg1000_core.

use crate::audio::MachineAudioSession;
use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::PyBytes;
use sega_sg1000_core::{MemoryConfig, Sg1000Machine};

#[pyclass(module = "sega_z80_core")]
pub struct NativeCpuSnapshot {
    #[pyo3(get)]
    pub a: u8,
    #[pyo3(get)]
    pub f: u8,
    #[pyo3(get)]
    pub b: u8,
    #[pyo3(get)]
    pub c: u8,
    #[pyo3(get)]
    pub d: u8,
    #[pyo3(get)]
    pub e: u8,
    #[pyo3(get)]
    pub h: u8,
    #[pyo3(get)]
    pub l: u8,
    #[pyo3(get)]
    pub pc: u16,
    #[pyo3(get)]
    pub sp: u16,
    #[pyo3(get)]
    pub ix: u16,
    #[pyo3(get)]
    pub iy: u16,
    #[pyo3(get)]
    pub i: u8,
    #[pyo3(get)]
    pub r: u8,
    #[pyo3(get)]
    pub iff1: bool,
    #[pyo3(get)]
    pub iff2: bool,
    #[pyo3(get)]
    pub interrupt_mode: u8,
    #[pyo3(get)]
    pub halted: bool,
    #[pyo3(get)]
    pub wz: u16,
    #[pyo3(get)]
    pub q: u8,
    #[pyo3(get)]
    pub p: bool,
    #[pyo3(get)]
    pub total_tstates: u64,
    #[pyo3(get)]
    pub total_cycles: u64,
    #[pyo3(get)]
    pub af_alt: u16,
    #[pyo3(get)]
    pub bc_alt: u16,
    #[pyo3(get)]
    pub de_alt: u16,
    #[pyo3(get)]
    pub hl_alt: u16,
}

impl NativeCpuSnapshot {
    fn from_machine(machine: &Sg1000Machine) -> Self {
        let cpu = &machine.cpu;
        let regs = cpu.registers;
        let alt = cpu.alternate;
        Self {
            a: regs.a,
            f: regs.f,
            b: regs.b,
            c: regs.c,
            d: regs.d,
            e: regs.e,
            h: regs.h,
            l: regs.l,
            pc: regs.pc,
            sp: regs.sp,
            ix: cpu.ix,
            iy: cpu.iy,
            i: cpu.i,
            r: cpu.r,
            iff1: cpu.iff1,
            iff2: cpu.iff2,
            interrupt_mode: cpu.interrupt_mode,
            halted: cpu.halted,
            wz: cpu.wz,
            q: cpu.q,
            p: cpu.p,
            total_tstates: cpu.total_tstates,
            total_cycles: cpu.total_tstates,
            af_alt: u16::from_be_bytes([alt.a, alt.f]),
            bc_alt: u16::from_be_bytes([alt.b, alt.c]),
            de_alt: u16::from_be_bytes([alt.d, alt.e]),
            hl_alt: u16::from_be_bytes([alt.h, alt.l]),
        }
    }
}

#[pymethods]
impl NativeCpuSnapshot {
    #[getter]
    fn af(&self) -> u16 {
        u16::from_be_bytes([self.a, self.f])
    }

    #[getter]
    fn bc(&self) -> u16 {
        u16::from_be_bytes([self.b, self.c])
    }

    #[getter]
    fn de(&self) -> u16 {
        u16::from_be_bytes([self.d, self.e])
    }

    #[getter]
    fn hl(&self) -> u16 {
        u16::from_be_bytes([self.h, self.l])
    }
}

#[pyclass(module = "sega_z80_core")]
pub struct NativeVdpSnapshot {
    status: u8,
    scanline: u16,
    cycles_into_line: u16,
    frame_number: u64,
    display_mode: u8,
    interrupt_pending: bool,
    sprite_limit_enabled: bool,
    registers: [u8; 8],
    vram: Vec<u8>,
    framebuffer: Vec<u8>,
}

#[pymethods]
impl NativeVdpSnapshot {
    #[getter]
    fn status(&self) -> u8 {
        self.status
    }

    #[getter]
    fn scanline(&self) -> u16 {
        self.scanline
    }

    #[getter(_cycles_into_line)]
    fn cycles_into_line(&self) -> u16 {
        self.cycles_into_line
    }

    #[getter]
    fn frame_number(&self) -> u64 {
        self.frame_number
    }

    #[getter]
    fn display_mode(&self) -> u8 {
        self.display_mode
    }

    #[getter(_display_mode)]
    fn private_display_mode(&self) -> u8 {
        self.display_mode
    }

    #[getter]
    fn interrupt_pending(&self) -> bool {
        self.interrupt_pending
    }

    #[getter]
    fn sprite_limit_enabled(&self) -> bool {
        self.sprite_limit_enabled
    }

    #[getter]
    fn framebuffer<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        PyBytes::new(py, &self.framebuffer)
    }

    #[getter]
    fn vram<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        PyBytes::new(py, &self.vram)
    }

    #[getter]
    fn registers<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        PyBytes::new(py, &self.registers)
    }

    fn _sprite_line_active(&self, line: u16) -> sega_sg1000_core::vdp::SpriteLineInspection {
        sega_sg1000_core::vdp::inspect_sprite_line(
            &self.vram,
            &self.registers,
            line,
            self.sprite_limit_enabled,
        )
    }
}

#[pyclass(module = "sega_z80_core")]
pub struct NativePsgSnapshot {
    tone_periods: [u16; 3],
    attenuations: [u8; 4],
    tone_outputs: [u8; 3],
}

#[pymethods]
impl NativePsgSnapshot {
    #[getter]
    fn tone_periods(&self) -> (u16, u16, u16) {
        (
            self.tone_periods[0],
            self.tone_periods[1],
            self.tone_periods[2],
        )
    }

    #[getter]
    fn attenuations(&self) -> (u8, u8, u8, u8) {
        (
            self.attenuations[0],
            self.attenuations[1],
            self.attenuations[2],
            self.attenuations[3],
        )
    }

    #[getter(_tone_outputs)]
    fn tone_outputs(&self) -> (u8, u8, u8) {
        (
            self.tone_outputs[0],
            self.tone_outputs[1],
            self.tone_outputs[2],
        )
    }
}

/// Thread-affine PyO3 adapter; the Qt runtime creates and accesses it on one thread.
#[pyclass(unsendable, module = "sega_z80_core")]
pub struct NativeSG1000Machine {
    machine: Sg1000Machine,
    total_instructions: u64,
    audio_session: Option<MachineAudioSession>,
    audio_output_error: Option<String>,
}

impl NativeSG1000Machine {
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
}

#[pymethods]
impl NativeSG1000Machine {
    #[new]
    fn new(
        rom: Vec<u8>,
        internal_ram_size: usize,
        cartridge_ram_size: usize,
        cartridge_ram_start: u16,
        cartridge_ram_end: u16,
    ) -> PyResult<Self> {
        let config = MemoryConfig {
            internal_ram_size,
            cartridge_ram_size,
            cartridge_ram_start,
            cartridge_ram_end,
        };
        let machine = Sg1000Machine::new(&rom, config)
            .map_err(|error| PyRuntimeError::new_err(error.to_string()))?;
        Ok(Self {
            machine,
            total_instructions: 0,
            audio_session: None,
            audio_output_error: None,
        })
    }

    /// Configure the Rust-owned audio session. No PCM is returned to Python.
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
        let session = MachineAudioSession::open(
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

    #[getter]
    fn cpu(&self) -> NativeCpuSnapshot {
        NativeCpuSnapshot::from_machine(&self.machine)
    }

    #[getter]
    fn vdp(&self) -> NativeVdpSnapshot {
        let vdp = &self.machine.vdp;
        NativeVdpSnapshot {
            status: vdp.status(),
            scanline: vdp.scanline(),
            cycles_into_line: vdp.cycles_into_line(),
            frame_number: vdp.frame_number(),
            display_mode: vdp.display_mode(),
            interrupt_pending: vdp.interrupt_pending(),
            sprite_limit_enabled: vdp.sprite_limit_enabled(),
            registers: *vdp.registers(),
            vram: vdp.vram().to_vec(),
            framebuffer: vdp.framebuffer().to_vec(),
        }
    }

    #[getter]
    fn psg(&self) -> NativePsgSnapshot {
        let psg = &self.machine.psg;
        NativePsgSnapshot {
            tone_periods: psg.tone_periods(),
            attenuations: psg.attenuations(),
            tone_outputs: psg.tone_outputs(),
        }
    }

    #[getter]
    fn total_instructions(&self) -> u64 {
        self.total_instructions
    }

    #[getter]
    fn total_tstates(&self) -> u64 {
        self.machine.cpu.total_tstates
    }

    #[getter]
    fn sprite_limit_enabled(&self) -> bool {
        self.machine.vdp.sprite_limit_enabled()
    }

    fn set_sprite_limit_enabled(&mut self, enabled: bool) {
        self.machine.vdp.set_sprite_limit_enabled(enabled);
    }

    fn request_nmi(&mut self) {
        self.machine.cpu.request_nmi();
    }

    fn read_memory(&self, address: u16) -> u8 {
        self.machine.read_memory(address)
    }

    fn read_port(&mut self, address: u16) -> u8 {
        self.machine.read_port(address)
    }

    fn write_port(&mut self, address: u16, value: u8) {
        self.machine.write_port(address, value);
    }

    fn write_port_at(&mut self, address: u16, value: u8, tstate: u64) {
        self.machine.write_port_at(address, value, tstate);
    }

    fn set_joystick(&mut self, player: u8, value: u8) -> PyResult<()> {
        self.machine
            .set_joystick(player, value)
            .map_err(|error| PyValueError::new_err(error.to_string()))
    }

    fn drain_psg_write_events(&mut self) -> Vec<(u64, u8)> {
        self.machine
            .drain_psg_write_events()
            .into_iter()
            .map(|event| (event.tstate, event.value))
            .collect()
    }

    fn drain_audio_pcm<'py>(
        &mut self,
        py: Python<'py>,
        mut resampler: PyRefMut<'_, NativePcmResampler>,
    ) -> Bound<'py, PyBytes> {
        let samples = self.machine.drain_audio_samples(None);
        PyBytes::new(py, &resampler.process_native(&samples))
    }

    fn step(&mut self) -> PyResult<u32> {
        let cycles = self
            .machine
            .step()
            .map_err(|error| PyRuntimeError::new_err(error.to_string()))?;
        self.total_instructions = self.total_instructions.saturating_add(1);
        Ok(cycles)
    }

    fn run_frame(&mut self, max_steps: usize) -> PyResult<usize> {
        if max_steps == 0 {
            return Err(PyValueError::new_err("max_steps must be positive"));
        }
        let start_frame = self.machine.vdp.frame_number();
        for steps in 1..=max_steps {
            self.step()?;
            if self.machine.vdp.frame_number() != start_frame {
                self.pump_audio_output();
                return Ok(steps);
            }
        }
        Err(PyRuntimeError::new_err(
            "frame did not complete within max_steps",
        ))
    }

    fn run_frames(&mut self, count: usize) -> PyResult<usize> {
        let mut total_steps = 0usize;
        for _ in 0..count {
            total_steps = total_steps.saturating_add(self.run_frame(100_000)?);
        }
        Ok(total_steps)
    }
}
