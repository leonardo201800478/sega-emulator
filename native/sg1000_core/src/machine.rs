//! Deterministic headless SG-1000 machine assembled from pure Rust components.
//!
//! CPU, memory, VDP raster timing, PSG sample generation and the first machine
//! bus are integrated. Timing is still instruction-boundary based, not cycle-exact.

use std::fmt;

use crate::CpuBus;
use crate::{
    CpuError, IoError, MemoryConfig, MemoryError, PsgError, PsgWriteEvent, Sg1000Io, Sg1000Memory,
    Sn76489, Tms9918A, Z80,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MachineError {
    Memory(MemoryError),
    Cpu(CpuError),
    Audio(PsgError),
}

impl fmt::Display for MachineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Memory(error) => write!(f, "SG-1000 memory initialization failed: {error}"),
            Self::Cpu(error) => write!(f, "Z80 execution failed: {error:?}"),
            Self::Audio(error) => write!(f, "SG-1000 PSG initialization failed: {error}"),
        }
    }
}

impl std::error::Error for MachineError {}

impl From<MemoryError> for MachineError {
    fn from(error: MemoryError) -> Self {
        Self::Memory(error)
    }
}

impl From<CpuError> for MachineError {
    fn from(error: CpuError) -> Self {
        Self::Cpu(error)
    }
}

impl From<PsgError> for MachineError {
    fn from(error: PsgError) -> Self {
        Self::Audio(error)
    }
}

/// Incremental headless machine used to develop and validate the standalone core.
///
/// The CPU's immediate IN/OUT instructions route through a split-borrow bus
/// adapter. The VDP and PSG advance from nominal instruction T-states.

/// Split-borrow adapter that exposes all SG-1000 devices to one Z80 step.
struct Sg1000MachineBus<'a> {
    memory: &'a mut Sg1000Memory,
    io: &'a mut Sg1000Io,
    vdp: &'a mut Tms9918A,
    psg: &'a mut Sn76489,
    psg_tstate: &'a mut u64,
    tstate: u64,
}

impl CpuBus for Sg1000MachineBus<'_> {
    fn read_memory(&mut self, address: u16) -> u8 {
        self.memory.read(address)
    }

    fn write_memory(&mut self, address: u16, value: u8) {
        self.memory.write(address, value);
    }

    fn read_port(&mut self, port: u16) -> u8 {
        match (port as u8) & 0xC1 {
            0x80 | 0x81 => self.vdp.read_port(port),
            _ => self.io.read_port(port),
        }
    }

    fn write_port(&mut self, port: u16, value: u8) {
        match (port as u8) & 0xC1 {
            0x80 | 0x81 => self.vdp.write_port(port, value),
            _ => {
                self.io.write_port_at(port, value, self.tstate);
                if (0x40..=0x7F).contains(&(port as u8)) {
                    if self.tstate > *self.psg_tstate {
                        self.psg.advance_clocks(self.tstate - *self.psg_tstate);
                        *self.psg_tstate = self.tstate;
                    }
                    self.psg.write(value);
                }
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Sg1000Machine {
    pub cpu: Z80,
    pub memory: Sg1000Memory,
    pub io: Sg1000Io,
    pub vdp: Tms9918A,
    pub psg: Sn76489,
    psg_tstate: u64,
    pending_psg_writes: Vec<PsgWriteEvent>,
}

impl Sg1000Machine {
    pub fn new(rom: &[u8], memory_config: MemoryConfig) -> Result<Self, MachineError> {
        Self::new_with_audio(rom, memory_config, 3_579_545, 44_100)
    }

    /// Construct a machine with an explicit PSG master clock and output rate.
    /// The default clock profile is a provisional SG-1000 nominal clock; verify
    /// against board-level documentation before treating timing as hardware-exact.
    pub fn new_with_audio(
        rom: &[u8],
        memory_config: MemoryConfig,
        psg_clock_hz: u64,
        sample_rate: u64,
    ) -> Result<Self, MachineError> {
        Ok(Self {
            cpu: Z80::default(),
            memory: Sg1000Memory::new(rom, memory_config)?,
            io: Sg1000Io::default(),
            vdp: Tms9918A::new(),
            psg: Sn76489::new(psg_clock_hz, sample_rate)?,
            psg_tstate: 0,
            pending_psg_writes: Vec::new(),
        })
    }

    /// Execute one CPU instruction and return its nominal base T-state count.
    pub fn step(&mut self) -> Result<u32, MachineError> {
        let tstate = self.cpu.total_tstates;
        let cycles = {
            let mut bus = Sg1000MachineBus {
                memory: &mut self.memory,
                io: &mut self.io,
                vdp: &mut self.vdp,
                psg: &mut self.psg,
                psg_tstate: &mut self.psg_tstate,
                tstate,
            };
            self.cpu.step(&mut bus)?
        };
        self.vdp.advance(u64::from(cycles));
        let end_tstate = self.cpu.total_tstates;
        loop {
            let next = self
                .pending_psg_writes
                .iter()
                .enumerate()
                .filter(|(_, event)| event.tstate <= end_tstate)
                .min_by_key(|(_, event)| event.tstate)
                .map(|(index, _)| index);
            let Some(index) = next else {
                break;
            };
            let event = self.pending_psg_writes.remove(index);
            if event.tstate > self.psg_tstate {
                self.psg.advance_clocks(event.tstate - self.psg_tstate);
                self.psg_tstate = event.tstate;
            }
            self.psg.write(event.value);
        }
        if end_tstate > self.psg_tstate {
            self.psg.advance_clocks(end_tstate - self.psg_tstate);
        }
        self.psg_tstate = end_tstate;
        Ok(cycles)
    }

    /// Execute a fixed number of instructions and return elapsed nominal T-states.
    pub fn run_instructions(&mut self, count: usize) -> Result<u64, MachineError> {
        let initial_tstates = self.cpu.total_tstates;
        for _ in 0..count {
            self.step()?;
        }
        Ok(self.cpu.total_tstates - initial_tstates)
    }

    pub fn read_memory(&self, address: u16) -> u8 {
        self.memory.read(address)
    }

    /// Read the currently modeled controller/input ports.
    pub fn read_port(&mut self, address: u16) -> u8 {
        match (address as u8) & 0xC1 {
            0x80 | 0x81 => self.vdp.read_port(address),
            _ => self.io.read_port(address),
        }
    }

    /// Capture a port write at an explicit nominal machine T-state.
    pub fn write_port_at(&mut self, address: u16, value: u8, tstate: u64) {
        match (address as u8) & 0xC1 {
            0x80 | 0x81 => self.vdp.write_port(address, value),
            _ => {
                self.io.write_port_at(address, value, tstate);
                if (0x40..=0x7F).contains(&(address as u8)) {
                    if tstate > self.cpu.total_tstates {
                        self.pending_psg_writes
                            .push(PsgWriteEvent { tstate, value });
                    } else {
                        if tstate > self.psg_tstate {
                            self.psg.advance_clocks(tstate - self.psg_tstate);
                            self.psg_tstate = tstate;
                        }
                        self.psg.write(value);
                    }
                }
            }
        }
    }

    /// Capture a port write at the CPU's current accumulated nominal T-state.
    ///
    /// This is a coarse boundary helper, not a claim about the exact I/O bus
    /// phase within an instruction.
    pub fn write_port(&mut self, address: u16, value: u8) {
        self.write_port_at(address, value, self.cpu.total_tstates);
    }

    pub fn set_joystick(&mut self, player: u8, value: u8) -> Result<(), IoError> {
        self.io.set_joystick(player, value)
    }

    pub fn drain_psg_write_events(&mut self) -> Vec<PsgWriteEvent> {
        self.io.drain_psg_write_events()
    }

    /// Drain generated mono audio samples for the host audio backend.
    pub fn drain_audio_samples(&mut self, max_samples: Option<usize>) -> Vec<f64> {
        self.psg.drain_samples(max_samples)
    }
}

#[cfg(test)]
mod tests {
    use super::Sg1000Machine;
    use crate::{IoError, MemoryConfig, PsgWriteEvent};

    #[test]
    fn headless_machine_runs_rom_and_writes_ram_deterministically() {
        let rom = [0x3E, 0x42, 0x21, 0x00, 0xC0, 0x36, 0x99, 0x76];
        let mut first = Sg1000Machine::new(&rom, MemoryConfig::default()).unwrap();
        let mut second = first.clone();

        assert_eq!(first.run_instructions(4), Ok(31));
        assert_eq!(second.run_instructions(4), Ok(31));
        assert_eq!(first.cpu, second.cpu);
        assert_eq!(first.memory, second.memory);
        assert_eq!(first.io, second.io);
        assert_eq!(first.vdp, second.vdp);
        assert_eq!(first.psg, second.psg);
        assert_eq!(first.cpu.registers.a, 0x42);
        assert_eq!(first.read_memory(0xC000), 0x99);
        assert!(first.cpu.halted);
    }

    #[test]
    fn machine_exposes_controller_input_and_rejects_invalid_player() {
        let mut machine = Sg1000Machine::new(&[0x76], MemoryConfig::default()).unwrap();
        assert_eq!(machine.read_port(0x00), 0xFF);
        assert_eq!(machine.set_joystick(1, 0xEF), Ok(()));
        assert_eq!(machine.read_port(0x00), 0xEF);
        assert_eq!(machine.read_port(0xDC), 0xEF);
        assert_eq!(machine.set_joystick(3, 0), Err(IoError::InvalidPlayer(3)));
    }

    #[test]
    fn machine_drains_psg_events_without_losing_timestamp_or_order() {
        let mut machine = Sg1000Machine::new(&[0x76], MemoryConfig::default()).unwrap();
        machine.write_port_at(0x7F, 0x90, 8);
        machine.write_port_at(0x40, 0x23, 8);
        machine.write_port_at(0x80, 0x55, 9);
        assert_eq!(
            machine.drain_psg_write_events(),
            vec![
                PsgWriteEvent {
                    tstate: 8,
                    value: 0x90
                },
                PsgWriteEvent {
                    tstate: 8,
                    value: 0x23
                },
            ]
        );
        assert!(machine.drain_psg_write_events().is_empty());
    }

    #[test]
    fn vdp_ports_are_routed_and_machine_step_advances_video_timing() {
        let mut machine = Sg1000Machine::new(&[0x00, 0x76], MemoryConfig::default()).unwrap();
        assert_eq!(machine.vdp.scanline(), 0);
        assert_eq!(machine.step(), Ok(4));
        assert_eq!(machine.vdp.cycles_into_line(), 4);
        machine.write_port_at(0x81, 0x00, 4);
        machine.write_port_at(0x81, 0x40, 4);
        machine.write_port_at(0x80, 0xA5, 4);
        assert_eq!(machine.vdp.vram()[0], 0xA5);
        machine.write_port_at(0x40, 0x85, machine.cpu.total_tstates);
        machine.write_port_at(0x40, 0x02, machine.cpu.total_tstates);
        assert_eq!(machine.psg.tone_periods()[0], 0x25);
        assert_eq!(machine.drain_psg_write_events().len(), 2);
    }

    #[test]
    fn z80_in_reads_controller_and_out_updates_native_psg() {
        let mut input =
            Sg1000Machine::new(&[0x3E, 0x00, 0xDB, 0xDC, 0x76], MemoryConfig::default()).unwrap();
        input.set_joystick(1, 0xFE).unwrap();
        assert_eq!(input.run_instructions(2), Ok(18));
        assert_eq!(input.cpu.registers.a, 0xFE);

        let rom = [0x3E, 0x85, 0xD3, 0x40, 0x3E, 0x02, 0xD3, 0x40, 0x76];
        let mut output = Sg1000Machine::new(&rom, MemoryConfig::default()).unwrap();
        assert_eq!(output.run_instructions(4), Ok(36));
        assert_eq!(output.psg.tone_periods()[0], 0x25);
        assert_eq!(output.drain_psg_write_events().len(), 2);
    }

    #[test]
    fn unsupported_instruction_is_reported_to_caller() {
        let mut machine = Sg1000Machine::new(&[0xDD], MemoryConfig::default()).unwrap();
        assert!(machine.step().is_err());
    }
}
