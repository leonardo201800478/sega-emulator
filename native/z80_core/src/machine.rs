//! Native SG-1000 machine orchestration and I/O routing.
use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::*;

use super::{NativeAudioSynchronizer, NativeMemoryBus, NativeSn76489, NativeTms9918A, RustZ80Cpu};

#[pyclass(module = "sega_z80_core")]
pub(crate) struct NativeSG1000Io {
    vdp: Py<NativeTms9918A>,
    audio_sync: Py<NativeAudioSynchronizer>,
    joystick1: u8,
    joystick2: u8,
    current_tstate: u64,
}

impl NativeSG1000Io {
    fn new(vdp: Py<NativeTms9918A>, audio_sync: Py<NativeAudioSynchronizer>) -> Self {
        Self {
            vdp,
            audio_sync,
            joystick1: 0xFF,
            joystick2: 0xFF,
            current_tstate: 0,
        }
    }

    pub(crate) fn read_port_native(&mut self, py: Python<'_>, address: u16) -> PyResult<u8> {
        let port = address as u8;
        let vdp_port = port & 0xC1;
        if vdp_port == 0x80 || vdp_port == 0x81 {
            return Ok(self
                .vdp
                .bind(py)
                .borrow_mut()
                .read_port_native(u16::from(vdp_port)));
        }
        match port {
            0x00 | 0xDC => Ok(self.joystick1),
            0x01 | 0xDD => Ok(self.joystick2),
            _ => Ok(0xFF),
        }
    }

    pub(crate) fn write_port_at_native(
        &mut self,
        py: Python<'_>,
        address: u16,
        value: u8,
        tstate: u64,
    ) -> PyResult<()> {
        let port = address as u8;
        let vdp_port = port & 0xC1;
        if vdp_port == 0x80 || vdp_port == 0x81 {
            self.vdp
                .bind(py)
                .borrow_mut()
                .write_port_native(u16::from(vdp_port), value);
            return Ok(());
        }
        if (0x40..=0x7F).contains(&port) {
            self.audio_sync
                .bind(py)
                .borrow_mut()
                .schedule_write_native(tstate, value)?;
        }
        Ok(())
    }

    fn write_port_current_native(
        &mut self,
        py: Python<'_>,
        address: u16,
        value: u8,
    ) -> PyResult<()> {
        let port = address as u8;
        self.write_port_at_native(py, address, value, self.current_tstate)?;
        if (0x40..=0x7F).contains(&port) {
            self.audio_sync
                .bind(py)
                .borrow_mut()
                .advance_to_native(py, self.current_tstate)?;
        }
        Ok(())
    }

    fn set_current_tstate(&mut self, tstate: u64) {
        self.current_tstate = tstate;
    }

    fn set_joystick(&mut self, player: u8, value: u8) {
        if player == 1 {
            self.joystick1 = value;
        } else {
            self.joystick2 = value;
        }
    }
}

#[pyclass(module = "sega_z80_core")]
pub(crate) struct NativeSG1000Machine {
    cpu: Py<RustZ80Cpu>,
    memory_bus: Py<NativeMemoryBus>,
    vdp: Py<NativeTms9918A>,
    psg: Py<NativeSn76489>,
    audio_sync: Py<NativeAudioSynchronizer>,
    io_bus: Py<NativeSG1000Io>,
    total_instructions: u64,
    total_tstates: u64,
}

#[pymethods]
impl NativeSG1000Machine {
    #[new]
    #[pyo3(signature = (cartridge_rom, internal_ram_size=1024, cartridge_ram_size=0, cartridge_ram_start=0, cartridge_ram_end=-1))]
    fn new(
        py: Python<'_>,
        cartridge_rom: Vec<u8>,
        internal_ram_size: usize,
        cartridge_ram_size: usize,
        cartridge_ram_start: u16,
        cartridge_ram_end: i32,
    ) -> PyResult<Self> {
        let memory_bus = Py::new(
            py,
            NativeMemoryBus::new_native(
                cartridge_rom,
                internal_ram_size,
                cartridge_ram_size,
                cartridge_ram_start,
                cartridge_ram_end,
            )?,
        )?;
        let vdp = Py::new(py, NativeTms9918A::new_native())?;
        let psg = Py::new(py, NativeSn76489::new_native(3_579_545, 44_100)?)?;
        let audio_sync = Py::new(py, NativeAudioSynchronizer::new_native(psg.clone_ref(py)))?;
        let io_bus = Py::new(
            py,
            NativeSG1000Io::new(vdp.clone_ref(py), audio_sync.clone_ref(py)),
        )?;
        let cpu = Py::new(
            py,
            RustZ80Cpu::new_with_native_buses(memory_bus.clone_ref(py), io_bus.clone_ref(py)),
        )?;
        Ok(Self {
            cpu,
            memory_bus,
            vdp,
            psg,
            audio_sync,
            io_bus,
            total_instructions: 0,
            total_tstates: 0,
        })
    }

    #[getter]
    fn cpu(&self, py: Python<'_>) -> Py<RustZ80Cpu> {
        self.cpu.clone_ref(py)
    }

    #[getter]
    fn memory_bus(&self, py: Python<'_>) -> Py<NativeMemoryBus> {
        self.memory_bus.clone_ref(py)
    }

    #[getter]
    fn vdp(&self, py: Python<'_>) -> Py<NativeTms9918A> {
        self.vdp.clone_ref(py)
    }

    #[getter]
    fn psg(&self, py: Python<'_>) -> Py<NativeSn76489> {
        self.psg.clone_ref(py)
    }

    #[getter]
    fn audio_sync(&self, py: Python<'_>) -> Py<NativeAudioSynchronizer> {
        self.audio_sync.clone_ref(py)
    }

    #[getter]
    fn total_instructions(&self) -> u64 {
        self.total_instructions
    }

    #[getter]
    fn total_tstates(&self) -> u64 {
        self.total_tstates
    }

    #[getter]
    fn joystick1(&self, py: Python<'_>) -> PyResult<u8> {
        Ok(self.io_bus.bind(py).borrow().joystick1)
    }

    #[getter]
    fn joystick2(&self, py: Python<'_>) -> PyResult<u8> {
        Ok(self.io_bus.bind(py).borrow().joystick2)
    }

    fn read_port(&mut self, py: Python<'_>, address: i64) -> PyResult<u8> {
        self.io_bus
            .bind(py)
            .borrow_mut()
            .read_port_native(py, address as u16)
    }

    fn write_port(&mut self, py: Python<'_>, address: i64, value: i64) -> PyResult<()> {
        let port = address as u8;
        if (0x40..=0x7F).contains(&port) && !(0..=0xFF).contains(&value) {
            return Err(PyValueError::new_err("value deve ser um byte inteiro."));
        }
        self.io_bus
            .bind(py)
            .borrow_mut()
            .write_port_current_native(py, address as u16, value as u8)
    }

    fn write_port_at(
        &mut self,
        py: Python<'_>,
        address: i64,
        value: i64,
        tstate: i64,
    ) -> PyResult<()> {
        if tstate < 0 {
            return Err(PyValueError::new_err(
                "tstate deve ser um inteiro não negativo.",
            ));
        }
        let port = address as u8;
        if (0x40..=0x7F).contains(&port) && !(0..=0xFF).contains(&value) {
            return Err(PyValueError::new_err("value deve ser um byte inteiro."));
        }
        self.io_bus.bind(py).borrow_mut().write_port_at_native(
            py,
            address as u16,
            value as u8,
            tstate as u64,
        )
    }

    fn set_joystick(&mut self, py: Python<'_>, player: i64, value: i64) -> PyResult<()> {
        if !(1..=2).contains(&player) {
            return Err(PyValueError::new_err("player deve ser 1 ou 2."));
        }
        if !(0..=0xFF).contains(&value) {
            return Err(PyValueError::new_err("value deve estar entre 00h e FFh."));
        }
        self.io_bus
            .bind(py)
            .borrow_mut()
            .set_joystick(player as u8, value as u8);
        Ok(())
    }

    fn step(&mut self, py: Python<'_>) -> PyResult<u32> {
        self.step_inner(py)
    }

    #[pyo3(signature = (max_steps=100000))]
    fn run_frame(&mut self, py: Python<'_>, max_steps: i64) -> PyResult<usize> {
        if max_steps < 1 {
            return Err(PyValueError::new_err(
                "max_steps deve ser um inteiro positivo.",
            ));
        }
        self.run_frame_inner(py, max_steps as usize)
    }

    fn run_frames(&mut self, py: Python<'_>, count: i64) -> PyResult<usize> {
        if count < 0 {
            return Err(PyValueError::new_err(
                "count deve ser um inteiro não negativo.",
            ));
        }
        let mut total_steps = 0usize;
        for _ in 0..count {
            total_steps = total_steps.saturating_add(self.run_frame_inner(py, 100_000)?);
        }
        Ok(total_steps)
    }
}

impl NativeSG1000Machine {
    fn step_inner(&mut self, py: Python<'_>) -> PyResult<u32> {
        let cycles = self.cpu.bind(py).borrow_mut().step(py)?;
        self.total_instructions += 1;
        self.total_tstates += u64::from(cycles);
        self.io_bus
            .bind(py)
            .borrow_mut()
            .set_current_tstate(self.total_tstates);
        self.vdp.bind(py).borrow_mut().advance_native(cycles)?;
        self.audio_sync
            .bind(py)
            .borrow_mut()
            .advance_to_native(py, self.total_tstates)?;
        if self.vdp.bind(py).borrow().interrupt_pending_native() {
            self.cpu.bind(py).borrow_mut().request_int(0xFF);
        } else {
            self.cpu.bind(py).borrow_mut().clear_int_request();
        }
        Ok(cycles)
    }

    fn run_frame_inner(&mut self, py: Python<'_>, max_steps: usize) -> PyResult<usize> {
        let start_frame = self.vdp.bind(py).borrow().frame_number_native();
        for steps in 1..=max_steps {
            self.step_inner(py)?;
            if self.vdp.bind(py).borrow().frame_number_native() != start_frame {
                return Ok(steps);
            }
        }
        Err(PyRuntimeError::new_err(
            "Limite de instruções excedido antes do próximo quadro.",
        ))
    }
}
