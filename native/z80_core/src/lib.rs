//! Experimental native Z80 execution core.
//!
//! This first selectable increment intentionally supports a subset of the
//! Python reference core. Unsupported opcodes fail explicitly; they never
//! fall back silently to Python.

use pyo3::exceptions::PyNotImplementedError;
use pyo3::prelude::*;
use sega_z80_alu::{adc8, add8, and8, cp8, or8, sbc8, sub8, xor8};

const FLAG_C: u8 = 0x01;
const FLAG_N: u8 = 0x02;
const FLAG_PV: u8 = 0x04;
const FLAG_H: u8 = 0x10;
const FLAG_Z: u8 = 0x40;
const FLAG_S: u8 = 0x80;

#[pyclass(module = "sega_z80_core")]
pub struct RustZ80Cpu {
    bus: Py<PyAny>,
    io_bus: Option<Py<PyAny>>,
    #[pyo3(get, set)]
    pub a: u8,
    #[pyo3(get, set)]
    pub f: u8,
    #[pyo3(get, set)]
    pub b: u8,
    #[pyo3(get, set)]
    pub c: u8,
    #[pyo3(get, set)]
    pub d: u8,
    #[pyo3(get, set)]
    pub e: u8,
    #[pyo3(get, set)]
    pub h: u8,
    #[pyo3(get, set)]
    pub l: u8,
    #[pyo3(get, set)]
    pub sp: u16,
    #[pyo3(get, set)]
    pub pc: u16,
    #[pyo3(get, set)]
    pub ix: u16,
    #[pyo3(get, set)]
    pub iy: u16,
    #[pyo3(get, set)]
    pub i: u8,
    #[pyo3(get, set)]
    pub r: u8,
    #[pyo3(get, set)]
    pub iff1: bool,
    #[pyo3(get, set)]
    pub iff2: bool,
    #[pyo3(get, set)]
    pub interrupt_mode: u8,
    #[pyo3(get)]
    pub halted: bool,
    #[pyo3(get, set)]
    pub wz: u16,
    #[pyo3(get)]
    total_cycles: u64,
    int_pending: Option<u8>,
    nmi_pending: bool,
    ei_delay: u8,
}

#[pymethods]
impl RustZ80Cpu {
    #[new]
    fn new(bus: Py<PyAny>, io_bus: Option<Py<PyAny>>) -> Self {
        Self {
            bus,
            io_bus,
            a: 0,
            f: 0,
            b: 0,
            c: 0,
            d: 0,
            e: 0,
            h: 0,
            l: 0,
            sp: 0,
            pc: 0,
            ix: 0,
            iy: 0,
            i: 0,
            r: 0,
            iff1: false,
            iff2: false,
            interrupt_mode: 0,
            halted: false,
            wz: 0,
            total_cycles: 0,
            int_pending: None,
            nmi_pending: false,
            ei_delay: 0,
        }
    }

    #[getter]
    fn af(&self) -> u16 {
        u16::from_be_bytes([self.a, self.f])
    }

    #[setter]
    fn set_af(&mut self, value: u16) {
        [self.a, self.f] = value.to_be_bytes();
    }

    #[getter]
    fn bc(&self) -> u16 {
        u16::from_be_bytes([self.b, self.c])
    }

    #[setter]
    fn set_bc(&mut self, value: u16) {
        [self.b, self.c] = value.to_be_bytes();
    }

    #[getter]
    fn de(&self) -> u16 {
        u16::from_be_bytes([self.d, self.e])
    }

    #[setter]
    fn set_de(&mut self, value: u16) {
        [self.d, self.e] = value.to_be_bytes();
    }

    #[getter]
    fn hl(&self) -> u16 {
        u16::from_be_bytes([self.h, self.l])
    }

    #[setter]
    fn set_hl(&mut self, value: u16) {
        [self.h, self.l] = value.to_be_bytes();
    }

    fn reset(&mut self) {
        self.a = 0;
        self.f = 0;
        self.b = 0;
        self.c = 0;
        self.d = 0;
        self.e = 0;
        self.h = 0;
        self.l = 0;
        self.sp = 0;
        self.pc = 0;
        self.ix = 0;
        self.iy = 0;
        self.i = 0;
        self.r = 0;
        self.iff1 = false;
        self.iff2 = false;
        self.interrupt_mode = 0;
        self.halted = false;
        self.wz = 0;
        self.total_cycles = 0;
        self.int_pending = None;
        self.nmi_pending = false;
        self.ei_delay = 0;
    }

    #[pyo3(signature = (data_bus=255))]
    fn request_int(&mut self, data_bus: u8) {
        self.int_pending = Some(data_bus);
    }

    fn clear_int_request(&mut self) {
        self.int_pending = None;
    }

    fn request_nmi(&mut self) {
        self.nmi_pending = true;
    }

    fn step(&mut self, py: Python<'_>) -> PyResult<u32> {
        if self.nmi_pending {
            self.nmi_pending = false;
            self.halted = false;
            self.iff2 = self.iff1;
            self.iff1 = false;
            self.increment_r();
            self.push_word(py, self.pc)?;
            self.pc = 0x0066;
            self.total_cycles += 11;
            return Ok(11);
        }
        if let Some(data_bus) = self.int_pending
            && self.iff1
            && self.ei_delay == 0
        {
            self.int_pending = None;
            self.halted = false;
            self.iff1 = false;
            self.iff2 = false;
            self.increment_r();
            let cycles: u32 = match self.interrupt_mode {
                0 if data_bus & 0xC7 == 0xC7 => {
                    self.push_word(py, self.pc)?;
                    self.pc = u16::from(data_bus & 0x38);
                    13
                }
                0 => {
                    return Err(PyNotImplementedError::new_err(format!(
                        "Rust Z80 core does not support IM 0 acknowledge byte 0x{data_bus:02X}"
                    )));
                }
                1 => {
                    self.push_word(py, self.pc)?;
                    self.pc = 0x0038;
                    13
                }
                2 => {
                    self.push_word(py, self.pc)?;
                    let vector = (u16::from(self.i) << 8) | u16::from(data_bus);
                    let low = self.read8(py, vector)?;
                    let high = self.read8(py, vector.wrapping_add(1))?;
                    self.pc = u16::from_le_bytes([low, high]);
                    19
                }
                mode => {
                    return Err(PyNotImplementedError::new_err(format!(
                        "Rust Z80 core does not support interrupt mode {mode}"
                    )));
                }
            };
            self.total_cycles += u64::from(cycles);
            return Ok(cycles);
        }
        if self.halted {
            self.increment_r();
            self.total_cycles += 4;
            return Ok(4);
        }

        let opcode_pc = self.pc;
        let opcode = self.fetch_opcode(py)?;
        let cycles = self.execute_opcode(py, opcode, opcode_pc)?;
        self.total_cycles += u64::from(cycles);
        if self.ei_delay > 0 {
            self.ei_delay -= 1;
        }
        Ok(cycles)
    }
}

impl RustZ80Cpu {
    fn read8(&self, py: Python<'_>, address: u16) -> PyResult<u8> {
        self.bus
            .bind(py)
            .call_method1("read", (address,))?
            .extract()
    }

    fn write8(&self, py: Python<'_>, address: u16, value: u8) -> PyResult<()> {
        self.bus.bind(py).call_method1("write", (address, value))?;
        Ok(())
    }

    fn read_port(&self, py: Python<'_>, address: u16) -> PyResult<u8> {
        let io = self
            .io_bus
            .as_ref()
            .ok_or_else(|| PyNotImplementedError::new_err("IN requires a configured I/O bus"))?;
        io.bind(py).call_method1("read_port", (address,))?.extract()
    }

    fn write_port(&self, py: Python<'_>, address: u16, value: u8) -> PyResult<()> {
        let io = self
            .io_bus
            .as_ref()
            .ok_or_else(|| PyNotImplementedError::new_err("OUT requires a configured I/O bus"))?;
        io.bind(py).call_method1("write_port", (address, value))?;
        Ok(())
    }

    fn fetch8(&mut self, py: Python<'_>) -> PyResult<u8> {
        let value = self.read8(py, self.pc)?;
        self.pc = self.pc.wrapping_add(1);
        Ok(value)
    }

    fn fetch_opcode(&mut self, py: Python<'_>) -> PyResult<u8> {
        let value = self.fetch8(py)?;
        self.increment_r();
        Ok(value)
    }

    fn increment_r(&mut self) {
        self.r = (self.r & 0x80) | (self.r.wrapping_add(1) & 0x7F);
    }

    fn read_reg(&self, py: Python<'_>, code: u8) -> PyResult<u8> {
        match code {
            0 => Ok(self.b),
            1 => Ok(self.c),
            2 => Ok(self.d),
            3 => Ok(self.e),
            4 => Ok(self.h),
            5 => Ok(self.l),
            6 => self.read8(py, self.hl()),
            _ => Ok(self.a),
        }
    }

    fn write_reg(&mut self, py: Python<'_>, code: u8, value: u8) -> PyResult<()> {
        match code {
            0 => self.b = value,
            1 => self.c = value,
            2 => self.d = value,
            3 => self.e = value,
            4 => self.h = value,
            5 => self.l = value,
            6 => {
                let address = self.hl();
                self.write8(py, address, value)?;
            }
            _ => self.a = value,
        }
        Ok(())
    }

    fn read_pair(&self, code: u8) -> u16 {
        match code {
            0 => self.bc(),
            1 => self.de(),
            2 => self.hl(),
            _ => self.sp,
        }
    }

    fn write_pair(&mut self, code: u8, value: u16) {
        match code {
            0 => self.set_bc(value),
            1 => self.set_de(value),
            2 => self.set_hl(value),
            _ => self.sp = value,
        }
    }

    fn read_stack_pair(&self, code: u8) -> u16 {
        if code == 3 {
            self.af()
        } else {
            self.read_pair(code)
        }
    }

    fn write_stack_pair(&mut self, code: u8, value: u16) {
        if code == 3 {
            self.set_af(value)
        } else {
            self.write_pair(code, value)
        }
    }

    fn push_word(&mut self, py: Python<'_>, value: u16) -> PyResult<()> {
        self.sp = self.sp.wrapping_sub(1);
        self.write8(py, self.sp, (value >> 8) as u8)?;
        self.sp = self.sp.wrapping_sub(1);
        self.write8(py, self.sp, value as u8)
    }

    fn pop_word(&mut self, py: Python<'_>) -> PyResult<u16> {
        let low = self.read8(py, self.sp)?;
        self.sp = self.sp.wrapping_add(1);
        let high = self.read8(py, self.sp)?;
        self.sp = self.sp.wrapping_add(1);
        Ok(u16::from_le_bytes([low, high]))
    }

    fn condition(&self, code: u8) -> bool {
        match code & 7 {
            0 => self.f & FLAG_Z == 0,
            1 => self.f & FLAG_Z != 0,
            2 => self.f & FLAG_C == 0,
            3 => self.f & FLAG_C != 0,
            4 => self.f & FLAG_PV == 0,
            5 => self.f & FLAG_PV != 0,
            6 => self.f & FLAG_S == 0,
            _ => self.f & FLAG_S != 0,
        }
    }

    fn alu(&mut self, operation: u8, value: u8) {
        let result = match operation & 7 {
            0 => add8(self.a, value),
            1 => adc8(self.a, value, self.f & FLAG_C != 0),
            2 => sub8(self.a, value),
            3 => sbc8(self.a, value, self.f & FLAG_C != 0),
            4 => and8(self.a, value),
            5 => xor8(self.a, value),
            6 => or8(self.a, value),
            _ => cp8(self.a, value),
        };
        if operation & 7 != 7 {
            self.a = result.value;
        }
        self.f = result.flags;
    }

    fn inc8(&mut self, value: u8) -> u8 {
        let result = value.wrapping_add(1);
        self.f = (self.f & FLAG_C) | (result & 0xA8);
        if result == 0 {
            self.f |= FLAG_Z;
        }
        if value & 0x0F == 0x0F {
            self.f |= FLAG_H;
        }
        if value == 0x7F {
            self.f |= FLAG_PV;
        }
        result
    }

    fn dec8(&mut self, value: u8) -> u8 {
        let result = value.wrapping_sub(1);
        self.f = (self.f & FLAG_C) | (result & 0xA8) | FLAG_N;
        if result == 0 {
            self.f |= FLAG_Z;
        }
        if value & 0x0F == 0 {
            self.f |= FLAG_H;
        }
        if value == 0x80 {
            self.f |= FLAG_PV;
        }
        result
    }

    fn add_hl(&mut self, value: u16) {
        let old = self.hl();
        let result = u32::from(old) + u32::from(value);
        let word = result as u16;
        self.f = (self.f & 0xC4) | ((word >> 8) as u8 & 0x28);
        if (old & 0x0FFF) + (value & 0x0FFF) > 0x0FFF {
            self.f |= FLAG_H;
        }
        if result > 0xFFFF {
            self.f |= FLAG_C;
        }
        self.wz = old.wrapping_add(1);
        self.set_hl(word);
    }

    fn execute_opcode(&mut self, py: Python<'_>, opcode: u8, opcode_pc: u16) -> PyResult<u32> {
        if opcode == 0x00 {
            return Ok(4);
        }
        if (0x40..=0x7F).contains(&opcode) {
            if opcode == 0x76 {
                self.halted = true;
                return Ok(4);
            }
            let destination = (opcode >> 3) & 7;
            let source = opcode & 7;
            let value = self.read_reg(py, source)?;
            self.write_reg(py, destination, value)?;
            return Ok(if destination == 6 || source == 6 {
                7
            } else {
                4
            });
        }
        if opcode & 0xC7 == 0x06 {
            let register = (opcode >> 3) & 7;
            let value = self.fetch8(py)?;
            self.write_reg(py, register, value)?;
            return Ok(if register == 6 { 10 } else { 7 });
        }
        if opcode & 0xCF == 0x01 {
            let low = self.fetch8(py)?;
            let high = self.fetch8(py)?;
            self.write_pair((opcode >> 4) & 3, u16::from_le_bytes([low, high]));
            return Ok(10);
        }
        if opcode & 0xC7 == 0x04 {
            let register = (opcode >> 3) & 7;
            let value = self.read_reg(py, register)?;
            let result = self.inc8(value);
            self.write_reg(py, register, result)?;
            return Ok(if register == 6 { 11 } else { 4 });
        }
        if opcode & 0xC7 == 0x05 {
            let register = (opcode >> 3) & 7;
            let value = self.read_reg(py, register)?;
            let result = self.dec8(value);
            self.write_reg(py, register, result)?;
            return Ok(if register == 6 { 11 } else { 4 });
        }
        if (0x80..=0xBF).contains(&opcode) {
            let operation = (opcode >> 3) & 7;
            let register = opcode & 7;
            let value = self.read_reg(py, register)?;
            self.alu(operation, value);
            return Ok(if register == 6 { 7 } else { 4 });
        }
        if opcode & 0xC7 == 0xC6 {
            let value = self.fetch8(py)?;
            self.alu((opcode >> 3) & 7, value);
            return Ok(7);
        }

        match opcode {
            0x02 | 0x12 => {
                let address = if opcode == 0x02 { self.bc() } else { self.de() };
                self.write8(py, address, self.a)?;
                self.wz = (u16::from(self.a) << 8) | (address.wrapping_add(1) & 0x00FF);
                Ok(7)
            }
            0x0A | 0x1A => {
                let address = if opcode == 0x0A { self.bc() } else { self.de() };
                self.a = self.read8(py, address)?;
                self.wz = address.wrapping_add(1);
                Ok(7)
            }
            0x03 | 0x13 | 0x23 | 0x33 => {
                let pair = (opcode >> 4) & 3;
                self.write_pair(pair, self.read_pair(pair).wrapping_add(1));
                Ok(6)
            }
            0x0B | 0x1B | 0x2B | 0x3B => {
                let pair = (opcode >> 4) & 3;
                self.write_pair(pair, self.read_pair(pair).wrapping_sub(1));
                Ok(6)
            }
            0x09 | 0x19 | 0x29 | 0x39 => {
                self.add_hl(self.read_pair((opcode >> 4) & 3));
                Ok(11)
            }
            0x10 => {
                let displacement = self.fetch8(py)? as i8;
                self.b = self.b.wrapping_sub(1);
                if self.b != 0 {
                    self.pc = self.pc.wrapping_add_signed(i16::from(displacement));
                    self.wz = self.pc;
                    Ok(13)
                } else {
                    Ok(8)
                }
            }
            0x18 | 0x20 | 0x28 | 0x30 | 0x38 => {
                let displacement = self.fetch8(py)? as i8;
                let take = opcode == 0x18 || self.condition((opcode >> 3) & 3);
                if take {
                    self.pc = self.pc.wrapping_add_signed(i16::from(displacement));
                    self.wz = self.pc;
                    Ok(12)
                } else {
                    Ok(7)
                }
            }
            0x22 => {
                let low = self.fetch8(py)?;
                let high = self.fetch8(py)?;
                let address = u16::from_le_bytes([low, high]);
                let [lo, hi] = self.hl().to_le_bytes();
                self.write8(py, address, lo)?;
                self.write8(py, address.wrapping_add(1), hi)?;
                self.wz = address.wrapping_add(1);
                Ok(16)
            }
            0x2A => {
                let low = self.fetch8(py)?;
                let high = self.fetch8(py)?;
                let address = u16::from_le_bytes([low, high]);
                let lo = self.read8(py, address)?;
                let hi = self.read8(py, address.wrapping_add(1))?;
                self.set_hl(u16::from_le_bytes([lo, hi]));
                self.wz = address.wrapping_add(1);
                Ok(16)
            }
            0x32 => {
                let low = self.fetch8(py)?;
                let high = self.fetch8(py)?;
                let address = u16::from_le_bytes([low, high]);
                self.write8(py, address, self.a)?;
                self.wz = (u16::from(self.a) << 8) | (address.wrapping_add(1) & 0x00FF);
                Ok(13)
            }
            0x3A => {
                let low = self.fetch8(py)?;
                let high = self.fetch8(py)?;
                let address = u16::from_le_bytes([low, high]);
                self.a = self.read8(py, address)?;
                self.wz = address.wrapping_add(1);
                Ok(13)
            }
            0xC3 => {
                let low = self.fetch8(py)?;
                let high = self.fetch8(py)?;
                self.pc = u16::from_le_bytes([low, high]);
                self.wz = self.pc;
                Ok(10)
            }
            0xE9 => {
                self.pc = self.hl();
                Ok(4)
            }
            0xF9 => {
                self.sp = self.hl();
                Ok(6)
            }
            0xC9 => {
                self.pc = self.pop_word(py)?;
                self.wz = self.pc;
                Ok(10)
            }
            0xCD => {
                let low = self.fetch8(py)?;
                let high = self.fetch8(py)?;
                let target = u16::from_le_bytes([low, high]);
                self.push_word(py, self.pc)?;
                self.pc = target;
                self.wz = target;
                Ok(17)
            }
            0xC5 | 0xD5 | 0xE5 | 0xF5 => {
                self.push_word(py, self.read_stack_pair((opcode >> 4) & 3))?;
                Ok(11)
            }
            0xC1 | 0xD1 | 0xE1 | 0xF1 => {
                let value = self.pop_word(py)?;
                self.write_stack_pair((opcode >> 4) & 3, value);
                Ok(10)
            }
            0xD3 => {
                let port = self.fetch8(py)?;
                let address = (u16::from(self.a) << 8) | u16::from(port);
                self.write_port(py, address, self.a)?;
                self.wz = address.wrapping_add(1);
                Ok(11)
            }
            0xDB => {
                let port = self.fetch8(py)?;
                let address = (u16::from(self.a) << 8) | u16::from(port);
                self.a = self.read_port(py, address)?;
                self.wz = address.wrapping_add(1);
                Ok(11)
            }
            0xF3 => {
                self.iff1 = false;
                self.iff2 = false;
                self.ei_delay = 0;
                Ok(4)
            }
            0xFB => {
                self.iff1 = true;
                self.iff2 = true;
                self.ei_delay = 2;
                Ok(4)
            }
            0xED => {
                let sub = self.fetch_opcode(py)?;
                match sub {
                    0x46 | 0x4E | 0x66 | 0x6E => {
                        self.interrupt_mode = 0;
                        Ok(8)
                    }
                    0x56 | 0x76 => {
                        self.interrupt_mode = 1;
                        Ok(8)
                    }
                    0x5E | 0x7E => {
                        self.interrupt_mode = 2;
                        Ok(8)
                    }
                    0x45 | 0x4D | 0x55 | 0x5D | 0x65 | 0x6D | 0x75 | 0x7D => {
                        self.pc = self.pop_word(py)?;
                        self.wz = self.pc;
                        self.iff1 = self.iff2;
                        Ok(14)
                    }
                    // LDI/LDD/LDIR/LDDR: copy one byte, move both pointers,
                    // decrement BC, and update only the documented transfer flags.
                    0xA0 | 0xA8 | 0xB0 | 0xB8 => {
                        let direction: i16 = if matches!(sub, 0xA8 | 0xB8) { -1 } else { 1 };
                        let value = self.read8(py, self.hl())?;
                        self.write8(py, self.de(), value)?;
                        self.set_hl(self.hl().wrapping_add_signed(direction));
                        self.set_de(self.de().wrapping_add_signed(direction));
                        self.set_bc(self.bc().wrapping_sub(1));

                        let sum = self.a.wrapping_add(value);
                        self.f = (self.f & 0xC1)
                            | (if self.bc() != 0 { FLAG_PV } else { 0 })
                            | (sum & 0x08)
                            | ((sum & 0x02) << 4);

                        if matches!(sub, 0xB0 | 0xB8) && self.bc() != 0 {
                            self.pc = self.pc.wrapping_sub(2);
                            Ok(21)
                        } else {
                            Ok(16)
                        }
                    }
                    // Only genuinely unassigned ED bytes are NOPs. Assigned but
                    // unsupported instructions must remain explicit errors.
                    _ if is_unassigned_ed_opcode(sub) => Ok(8),
                    _ => Err(PyNotImplementedError::new_err(format!(
                        "Rust Z80 core does not support ED opcode 0x{sub:02X} at PC 0x{opcode_pc:04X}"
                    ))),
                }
            }
            _ if opcode & 0xC7 == 0xC7 => {
                let target = u16::from(opcode & 0x38);
                self.push_word(py, self.pc)?;
                self.pc = target;
                self.wz = target;
                Ok(11)
            }
            _ if opcode & 0xC7 == 0xC2 => {
                let low = self.fetch8(py)?;
                let high = self.fetch8(py)?;
                let target = u16::from_le_bytes([low, high]);
                self.wz = target;
                if self.condition((opcode >> 3) & 7) {
                    self.pc = target;
                }
                Ok(10)
            }
            _ if opcode & 0xC7 == 0xC4 => {
                let low = self.fetch8(py)?;
                let high = self.fetch8(py)?;
                let target = u16::from_le_bytes([low, high]);
                if self.condition((opcode >> 3) & 7) {
                    self.push_word(py, self.pc)?;
                    self.pc = target;
                    self.wz = target;
                    Ok(17)
                } else {
                    Ok(10)
                }
            }
            _ if opcode & 0xC7 == 0xC0 => {
                if self.condition((opcode >> 3) & 7) {
                    self.pc = self.pop_word(py)?;
                    self.wz = self.pc;
                    Ok(11)
                } else {
                    Ok(5)
                }
            }
            _ => Err(PyNotImplementedError::new_err(format!(
                "Rust Z80 core does not support opcode 0x{opcode:02X} at PC 0x{opcode_pc:04X}"
            ))),
        }
    }
}

fn is_unassigned_ed_opcode(opcode: u8) -> bool {
    matches!(
        opcode,
        0x00..=0x3F
            | 0x77
            | 0x7F
            | 0x80..=0x9F
            | 0xA4..=0xA7
            | 0xAC..=0xAF
            | 0xB4..=0xB7
            | 0xBC..=0xBF
            | 0xC0..=0xFF
    )
}

#[pymodule]
fn sega_z80_core(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<RustZ80Cpu>()?;
    Ok(())
}
