//! Deterministic headless SG-1000 machine assembled from pure Rust components.
//!
//! CPU, memory and the first I/O/input boundary are integrated. VDP integration,
//! PSG event consumption and a machine-wide device scheduler remain pending.

use std::fmt;

use crate::{CpuError, IoError, MemoryConfig, MemoryError, PsgWriteEvent, Sg1000Io, Sg1000Memory, Z80};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MachineError {
    Memory(MemoryError),
    Cpu(CpuError),
}

impl fmt::Display for MachineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Memory(error) => write!(f, "SG-1000 memory initialization failed: {error}"),
            Self::Cpu(error) => write!(f, "Z80 execution failed: {error:?}"),
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

/// Incremental headless machine used to develop and validate the standalone core.
///
/// The I/O boundary currently models controller aliases and captures PSG writes.
/// It does not yet connect the CPU's IN/OUT instructions, a native VDP, or a
/// machine-wide scheduler.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sg1000Machine {
    pub cpu: Z80,
    pub memory: Sg1000Memory,
    pub io: Sg1000Io,
}

impl Sg1000Machine {
    pub fn new(rom: &[u8], memory_config: MemoryConfig) -> Result<Self, MachineError> {
        Ok(Self {
            cpu: Z80::default(),
            memory: Sg1000Memory::new(rom, memory_config)?,
            io: Sg1000Io::default(),
        })
    }

    /// Execute one CPU instruction and return its nominal base T-state count.
    pub fn step(&mut self) -> Result<u32, MachineError> {
        Ok(self.cpu.step(&mut self.memory)?)
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
    pub fn read_port(&self, address: u16) -> u8 {
        self.io.read_port(address)
    }

    /// Capture a port write at an explicit nominal machine T-state.
    pub fn write_port_at(&mut self, address: u16, value: u8, tstate: u64) {
        self.io.write_port_at(address, value, tstate);
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
                PsgWriteEvent { tstate: 8, value: 0x90 },
                PsgWriteEvent { tstate: 8, value: 0x23 },
            ]
        );
        assert!(machine.drain_psg_write_events().is_empty());
    }

    #[test]
    fn unsupported_instruction_is_reported_to_caller() {
        let mut machine = Sg1000Machine::new(&[0xDD], MemoryConfig::default()).unwrap();
        assert!(machine.step().is_err());
    }
}
