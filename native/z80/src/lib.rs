use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

const MEMORY_SIZE: usize = 0x1_0000;
const FLAG_S: u8 = 0x80;
const FLAG_Z: u8 = 0x40;
const FLAG_Y: u8 = 0x20;
const FLAG_H: u8 = 0x10;
const FLAG_X: u8 = 0x08;
const FLAG_PV: u8 = 0x04;
const FLAG_C: u8 = 0x01;

/// Deliberately small native execution slice used to validate the migration path.
///
/// This is not yet a drop-in replacement for the Python Z80 core. The subset
/// grows only alongside differential tests against the reference implementation.
#[pyclass(module = "sega_z80_native")]
struct Z80Subset {
    a: u8,
    f: u8,
    pc: u16,
    halted: bool,
    memory: Vec<u8>,
    total_tstates: u64,
}

impl Z80Subset {
    fn step_inner(&mut self) -> Result<u8, &'static str> {
        if self.halted {
            self.total_tstates += 4;
            return Ok(4);
        }

        let opcode = self.fetch_byte();
        let cycles = match opcode {
            0x00 => 4, // NOP
            0x3E => {
                self.a = self.fetch_byte();
                7 // LD A,n
            }
            0xAF => {
                self.a = 0;
                self.f = FLAG_Z | FLAG_PV;
                4 // XOR A
            }
            0x3C => {
                let old = self.a;
                let result = old.wrapping_add(1);
                let mut flags = self.f & FLAG_C;
                flags |= result & (FLAG_S | FLAG_Y | FLAG_X);
                if result == 0 {
                    flags |= FLAG_Z;
                }
                if old & 0x0F == 0x0F {
                    flags |= FLAG_H;
                }
                if old == 0x7F {
                    flags |= FLAG_PV;
                }
                self.a = result;
                self.f = flags;
                4 // INC A
            }
            0x18 => {
                let displacement = self.fetch_byte() as i8;
                self.pc = self.pc.wrapping_add_signed(i16::from(displacement));
                12 // JR e
            }
            0x76 => {
                self.halted = true;
                4 // HALT
            }
            0xC3 => {
                let low = self.fetch_byte();
                let high = self.fetch_byte();
                self.pc = u16::from_le_bytes([low, high]);
                10 // JP nn
            }
            _ => return Err("opcode is outside the current native Z80 subset"),
        };
        self.total_tstates += u64::from(cycles);
        Ok(cycles)
    }

    fn fetch_byte(&mut self) -> u8 {
        let value = self.memory[usize::from(self.pc)];
        self.pc = self.pc.wrapping_add(1);
        value
    }
}

#[pymethods]
impl Z80Subset {
    #[new]
    #[pyo3(signature = (memory=None))]
    fn new(memory: Option<Vec<u8>>) -> PyResult<Self> {
        let memory = match memory {
            Some(memory) if memory.len() == MEMORY_SIZE => memory,
            Some(_) => {
                return Err(PyValueError::new_err(
                    "memory must contain exactly 65536 bytes",
                ));
            }
            None => vec![0; MEMORY_SIZE],
        };
        Ok(Self {
            a: 0,
            f: 0,
            pc: 0,
            halted: false,
            memory,
            total_tstates: 0,
        })
    }

    #[getter]
    fn a(&self) -> u8 {
        self.a
    }

    #[getter]
    fn f(&self) -> u8 {
        self.f
    }

    #[getter]
    fn pc(&self) -> u16 {
        self.pc
    }

    #[getter]
    fn halted(&self) -> bool {
        self.halted
    }

    #[getter]
    fn total_tstates(&self) -> u64 {
        self.total_tstates
    }

    fn load_program(&mut self, address: u16, data: &[u8]) -> PyResult<()> {
        let start = usize::from(address);
        let end = start + data.len();
        if end > MEMORY_SIZE {
            return Err(PyValueError::new_err(
                "program must fit in the 64 KiB address space without wrapping",
            ));
        }
        self.memory[start..end].copy_from_slice(data);
        self.pc = address;
        self.halted = false;
        Ok(())
    }

    fn read_memory(&self, address: u16) -> u8 {
        self.memory[usize::from(address)]
    }

    fn step(&mut self) -> PyResult<u8> {
        self.step_inner()
            .map_err(|message| PyValueError::new_err(message))
    }

    /// Execute multiple instructions in Rust so Python does not cross the FFI
    /// boundary for each emulated opcode.
    fn run_steps(&mut self, count: usize) -> PyResult<usize> {
        for _ in 0..count {
            self.step_inner()
                .map_err(|message| PyValueError::new_err(message))?;
        }
        Ok(count)
    }

    fn state(&self) -> (u8, u8, u16, bool, u64) {
        (self.a, self.f, self.pc, self.halted, self.total_tstates)
    }
}

#[pymodule]
fn sega_z80_native(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<Z80Subset>()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cpu_with(program: &[u8]) -> Z80Subset {
        let mut cpu = Z80Subset {
            a: 0,
            f: 0,
            pc: 0,
            halted: false,
            memory: vec![0; MEMORY_SIZE],
            total_tstates: 0,
        };
        cpu.memory[..program.len()].copy_from_slice(program);
        cpu
    }

    #[test]
    fn ld_a_immediate_and_inc_a_update_flags() {
        let mut cpu = cpu_with(&[0x3E, 0x7F, 0x3C]);
        assert_eq!(cpu.step_inner(), Ok(7));
        assert_eq!(cpu.a, 0x7F);
        assert_eq!(cpu.step_inner(), Ok(4));
        assert_eq!(cpu.a, 0x80);
        assert_eq!(cpu.f & (FLAG_S | FLAG_H | FLAG_PV), FLAG_S | FLAG_H | FLAG_PV);
        assert_eq!(cpu.total_tstates, 11);
    }

    #[test]
    fn relative_jump_wraps_program_counter() {
        let mut cpu = cpu_with(&[0x18, 0xFE]);
        assert_eq!(cpu.step_inner(), Ok(12));
        assert_eq!(cpu.pc, 0);
    }

    #[test]
    fn halt_keeps_program_counter_and_advances_four_tstates() {
        let mut cpu = cpu_with(&[0x76]);
        assert_eq!(cpu.step_inner(), Ok(4));
        assert!(cpu.halted);
        assert_eq!(cpu.pc, 1);
        assert_eq!(cpu.step_inner(), Ok(4));
        assert_eq!(cpu.pc, 1);
        assert_eq!(cpu.total_tstates, 8);
    }

    #[test]
    fn jump_reads_little_endian_target() {
        let mut cpu = cpu_with(&[0xC3, 0x34, 0x12]);
        assert_eq!(cpu.step_inner(), Ok(10));
        assert_eq!(cpu.pc, 0x1234);
    }
}
