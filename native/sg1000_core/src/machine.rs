//! Deterministic headless SG-1000 machine assembled from pure Rust components.

use std::fmt;

use crate::{CpuError, MemoryConfig, MemoryError, Sg1000Memory, Z80};

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

/// Minimal headless machine used to develop and validate the standalone core.
///
/// VDP, PSG, I/O and interrupt scheduling are not integrated yet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sg1000Machine {
    pub cpu: Z80,
    pub memory: Sg1000Memory,
}

impl Sg1000Machine {
    pub fn new(rom: &[u8], memory_config: MemoryConfig) -> Result<Self, MachineError> {
        Ok(Self {
            cpu: Z80::default(),
            memory: Sg1000Memory::new(rom, memory_config)?,
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
}

#[cfg(test)]
mod tests {
    use super::Sg1000Machine;
    use crate::MemoryConfig;

    #[test]
    fn headless_machine_runs_rom_and_writes_ram_deterministically() {
        let rom = [0x3E, 0x42, 0x21, 0x00, 0xC0, 0x36, 0x99, 0x76];
        let mut first = Sg1000Machine::new(&rom, MemoryConfig::default()).unwrap();
        let mut second = first.clone();

        assert_eq!(first.run_instructions(4), Ok(31));
        assert_eq!(second.run_instructions(4), Ok(31));
        assert_eq!(first.cpu, second.cpu);
        assert_eq!(first.memory, second.memory);
        assert_eq!(first.cpu.registers.a, 0x42);
        assert_eq!(first.read_memory(0xC000), 0x99);
        assert!(first.cpu.halted);
    }

    #[test]
    fn unsupported_instruction_is_reported_to_caller() {
        let mut machine = Sg1000Machine::new(&[0xDD], MemoryConfig::default()).unwrap();
        assert!(machine.step().is_err());
    }
}
