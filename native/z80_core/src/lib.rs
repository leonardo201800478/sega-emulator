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
    pub af_alt: u16,
    #[pyo3(get, set)]
    pub bc_alt: u16,
    #[pyo3(get, set)]
    pub de_alt: u16,
    #[pyo3(get, set)]
    pub hl_alt: u16,
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
    #[pyo3(get, set)]
    pub q: u8,
    #[pyo3(get, set)]
    pub _q_previous: u8,
    #[pyo3(get, set)]
    pub p: u8,
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
            af_alt: 0,
            bc_alt: 0,
            de_alt: 0,
            hl_alt: 0,
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
            q: 0,
            _q_previous: 0,
            p: 0,
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
        self.q = self.f;
        self.b = 0;
        self.c = 0;
        self.d = 0;
        self.e = 0;
        self.h = 0;
        self.l = 0;
        self.af_alt = 0;
        self.bc_alt = 0;
        self.de_alt = 0;
        self.hl_alt = 0;
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
        self.q = 0;
        self._q_previous = 0;
        self.p = 0;
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
        self._q_previous = self.q;
        self.q = 0;
        let previous_p = self.p;
        self.p = 0;
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
            if previous_p != 0 {
                self.f &= !FLAG_PV;
                self.q = self.f;
            }
            self.int_pending = None;
            self.halted = false;
            self.iff1 = false;
            self.iff2 = false;
            self.increment_r();
            if self.interrupt_mode == 0 {
                // In IM 0, the interrupting device supplies the first opcode;
                // any following operands or prefix bytes are fetched from PC.
                // The acknowledge replaces M1 and costs two extra T-states.
                let instruction_cycles =
                    self.execute_opcode(py, data_bus, self.pc.wrapping_sub(1))?;
                let cycles = instruction_cycles + 2;
                self.total_cycles += u64::from(cycles);
                if self.ei_delay > 0 {
                    self.ei_delay -= 1;
                }
                return Ok(cycles);
            }
            let cycles: u32 = match self.interrupt_mode {
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

    fn write_port(
        &self,
        py: Python<'_>,
        address: u16,
        value: u8,
        cycle_offset: Option<u64>,
    ) -> PyResult<()> {
        let io = self
            .io_bus
            .as_ref()
            .ok_or_else(|| PyNotImplementedError::new_err("OUT requires a configured I/O bus"))?;
        let io = io.bind(py);
        if let Some(offset) = cycle_offset
            && let Ok(timed_write) = io.getattr("write_port_at")
            && timed_write.is_callable()
        {
            timed_write.call1((address, value, self.total_cycles.wrapping_add(offset)))?;
            return Ok(());
        }
        io.call_method1("write_port", (address, value))?;
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
            self.set_af(value);
            self.q = self.f;
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

    fn set_repeat_block_xy_flags(&mut self) {
        self.f = (self.f & !0x28) | (((self.pc >> 8) as u8) & 0x28);
        self.q = self.f;
    }

    fn set_repeat_block_io_flags(&mut self, value: u8) {
        self.f = (self.f & !0x28) | (((self.pc >> 8) as u8) & 0x28);
        let carry = self.f & FLAG_C != 0;
        let mut parity = self.f & FLAG_PV != 0;
        let half = if carry {
            if value & 0x80 != 0 {
                let even = (self.b.wrapping_sub(1) & 0x07).count_ones() & 1 == 0;
                parity ^= !even;
                self.b & 0x0F == 0
            } else {
                let even = (self.b.wrapping_add(1) & 0x07).count_ones() & 1 == 0;
                parity ^= !even;
                self.b & 0x0F == 0x0F
            }
        } else {
            let even = (self.b & 0x07).count_ones() & 1 == 0;
            parity ^= !even;
            false
        };
        self.f = (self.f & !(FLAG_H | FLAG_PV))
            | if half { FLAG_H } else { 0 }
            | if parity { FLAG_PV } else { 0 };
        self.q = self.f;
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
        self.q = self.f;
    }

    fn inc8(&mut self, value: u8) -> u8 {
        let result = value.wrapping_add(1);
        self.f = (self.f & FLAG_C) | (result & 0xA8);
        self.q = self.f;
        if result == 0 {
            self.f |= FLAG_Z;
            self.q = self.f;
        }
        if value & 0x0F == 0x0F {
            self.f |= FLAG_H;
            self.q = self.f;
        }
        if value == 0x7F {
            self.f |= FLAG_PV;
            self.q = self.f;
        }
        result
    }

    fn dec8(&mut self, value: u8) -> u8 {
        let result = value.wrapping_sub(1);
        self.f = (self.f & FLAG_C) | (result & 0xA8) | FLAG_N;
        self.q = self.f;
        if result == 0 {
            self.f |= FLAG_Z;
            self.q = self.f;
        }
        if value & 0x0F == 0 {
            self.f |= FLAG_H;
            self.q = self.f;
        }
        if value == 0x80 {
            self.f |= FLAG_PV;
            self.q = self.f;
        }
        result
    }

    fn add_hl(&mut self, value: u16) {
        let old = self.hl();
        let result = u32::from(old) + u32::from(value);
        let word = result as u16;
        self.f = (self.f & 0xC4) | ((word >> 8) as u8 & 0x28);
        self.q = self.f;
        if (old & 0x0FFF) + (value & 0x0FFF) > 0x0FFF {
            self.f |= FLAG_H;
            self.q = self.f;
        }
        if result > 0xFFFF {
            self.f |= FLAG_C;
            self.q = self.f;
        }
        self.wz = old.wrapping_add(1);
        self.set_hl(word);
    }

    fn add_sub_hl(&mut self, value: u16, subtract: bool) {
        let old = self.hl();
        self.wz = old.wrapping_add(1);
        let carry = if self.f & FLAG_C != 0 { 1 } else { 0 };
        let raw = if subtract {
            i32::from(old) - i32::from(value) - carry as i32
        } else {
            i32::from(old) + i32::from(value) + carry as i32
        };
        let result = raw as u16;
        let half = if subtract {
            (old & 0x0FFF) < (value & 0x0FFF) + carry as u16
        } else {
            u32::from(old & 0x0FFF) + u32::from(value & 0x0FFF) + carry > 0x0FFF
        };
        let overflow = if subtract {
            ((old ^ value) & (old ^ result) & 0x8000) != 0
        } else {
            (!(old ^ value) & (old ^ result) & 0x8000) != 0
        };
        let mut flags = ((result >> 8) as u8) & 0xA8;
        if result == 0 {
            flags |= FLAG_Z;
        }
        if half {
            flags |= FLAG_H;
        }
        if overflow {
            flags |= FLAG_PV;
        }
        if subtract {
            flags |= FLAG_N;
            if raw < 0 {
                flags |= FLAG_C;
            }
        } else if raw > 0xFFFF {
            flags |= FLAG_C;
        }
        self.set_hl(result);
        self.f = flags;
        self.q = self.f;
    }

    fn execute_cb(&mut self, py: Python<'_>, opcode: u8) -> PyResult<u32> {
        let register = opcode & 7;
        let operation = opcode >> 6;
        let bit_index = (opcode >> 3) & 7;
        let value = self.read_reg(py, register)?;
        let memory = register == 6;

        match operation {
            0 => {
                let carry_in = self.f & FLAG_C;
                let (result, carry) = match bit_index {
                    0 => (value.rotate_left(1), value >> 7),          // RLC
                    1 => (value.rotate_right(1), value & 1),          // RRC
                    2 => ((value << 1) | carry_in, value >> 7),       // RL
                    3 => ((value >> 1) | (carry_in << 7), value & 1), // RR
                    4 => (value << 1, value >> 7),                    // SLA
                    5 => ((value >> 1) | (value & 0x80), value & 1),  // SRA
                    6 => ((value << 1) | 1, value >> 7),              // SLL (undocumented)
                    _ => (value >> 1, value & 1),                     // SRL
                };
                self.write_reg(py, register, result)?;
                self.f = (result & 0xA8) | carry;
                self.q = self.f;
                if result == 0 {
                    self.f |= FLAG_Z;
                    self.q = self.f;
                }
                if result.count_ones() & 1 == 0 {
                    self.f |= FLAG_PV;
                    self.q = self.f;
                }
            }
            1 => {
                let zero = value & (1 << bit_index) == 0;
                let xy = if memory {
                    (self.wz >> 8) as u8 & 0x28
                } else {
                    value & 0x28
                };
                self.f = (self.f & FLAG_C) | FLAG_H | xy;
                self.q = self.f;
                if zero {
                    self.f |= FLAG_Z | FLAG_PV;
                    self.q = self.f;
                }
                if bit_index == 7 && !zero {
                    self.f |= FLAG_S;
                    self.q = self.f;
                }
            }
            2 => self.write_reg(py, register, value & !(1 << bit_index))?,
            _ => self.write_reg(py, register, value | (1 << bit_index))?,
        }

        Ok(if memory {
            if operation == 1 { 12 } else { 15 }
        } else {
            8
        })
    }

    fn index_value(&self, prefix: u8) -> u16 {
        if prefix == 0xDD { self.ix } else { self.iy }
    }

    fn set_index_value(&mut self, prefix: u8, value: u16) {
        if prefix == 0xDD {
            self.ix = value;
        } else {
            self.iy = value;
        }
    }

    fn read_index_reg(&self, py: Python<'_>, prefix: u8, code: u8, halves: bool) -> PyResult<u8> {
        let index = self.index_value(prefix);
        match (halves, code) {
            (true, 4) => Ok((index >> 8) as u8),
            (true, 5) => Ok(index as u8),
            _ => self.read_reg(py, code),
        }
    }

    fn write_index_reg(
        &mut self,
        py: Python<'_>,
        prefix: u8,
        code: u8,
        value: u8,
        halves: bool,
    ) -> PyResult<()> {
        let index = self.index_value(prefix);
        match (halves, code) {
            (true, 4) => self.set_index_value(prefix, (u16::from(value) << 8) | (index & 0x00FF)),
            (true, 5) => self.set_index_value(prefix, (index & 0xFF00) | u16::from(value)),
            _ => self.write_reg(py, code, value)?,
        }
        Ok(())
    }

    fn execute_indexed(&mut self, py: Python<'_>, prefix: u8, prefix_pc: u16) -> PyResult<u32> {
        let opcode = self.fetch_opcode(py)?;
        if opcode == 0xDD || opcode == 0xFD {
            return Ok(self.execute_indexed(py, opcode, self.pc.wrapping_sub(1))? + 4);
        }
        let index = self.index_value(prefix);

        if opcode == 0xCB {
            let displacement = self.fetch8(py)? as i8;
            let cb_opcode = self.fetch8(py)?;
            let address = index.wrapping_add_signed(i16::from(displacement));
            self.wz = address;
            let value = self.read8(py, address)?;
            let operation = cb_opcode >> 6;
            let bit = (cb_opcode >> 3) & 7;
            let register = cb_opcode & 7;
            if operation == 0 {
                let carry_in = self.f & FLAG_C;
                let (result, carry) = match bit {
                    0 => (value.rotate_left(1), value >> 7),
                    1 => (value.rotate_right(1), value & 1),
                    2 => ((value << 1) | carry_in, value >> 7),
                    3 => ((value >> 1) | (carry_in << 7), value & 1),
                    4 => (value << 1, value >> 7),
                    5 => ((value >> 1) | (value & 0x80), value & 1),
                    6 => ((value << 1) | 1, value >> 7),
                    _ => (value >> 1, value & 1),
                };
                self.write8(py, address, result)?;
                if register != 6 {
                    self.write_reg(py, register, result)?;
                }
                self.f = (result & 0xA8) | carry;
                self.q = self.f;
                if result == 0 {
                    self.f |= FLAG_Z;
                    self.q = self.f;
                }
                if result.count_ones() & 1 == 0 {
                    self.f |= FLAG_PV;
                    self.q = self.f;
                }
                return Ok(23);
            }
            if operation == 1 {
                let zero = value & (1 << bit) == 0;
                self.f = (self.f & FLAG_C) | FLAG_H | (((address >> 8) as u8) & 0x28);
                self.q = self.f;
                if zero {
                    self.f |= FLAG_Z | FLAG_PV;
                    self.q = self.f;
                }
                if bit == 7 && !zero {
                    self.f |= FLAG_S;
                    self.q = self.f;
                }
                return Ok(20);
            }
            let result = if operation == 2 {
                value & !(1 << bit)
            } else {
                value | (1 << bit)
            };
            self.write8(py, address, result)?;
            if register != 6 {
                self.write_reg(py, register, result)?;
            }
            return Ok(23);
        }

        match opcode {
            0x21 => {
                let low = self.fetch8(py)?;
                let high = self.fetch8(py)?;
                self.set_index_value(prefix, u16::from_le_bytes([low, high]));
                Ok(14)
            }
            0x22 | 0x2A => {
                let low = self.fetch8(py)?;
                let high = self.fetch8(py)?;
                let address = u16::from_le_bytes([low, high]);
                self.wz = address.wrapping_add(1);
                if opcode == 0x22 {
                    self.write8(py, address, index as u8)?;
                    self.write8(py, address.wrapping_add(1), (index >> 8) as u8)?;
                } else {
                    let lo = self.read8(py, address)?;
                    let hi = self.read8(py, address.wrapping_add(1))?;
                    self.set_index_value(prefix, u16::from_le_bytes([lo, hi]));
                }
                Ok(20)
            }
            0x23 | 0x2B => {
                self.set_index_value(
                    prefix,
                    if opcode == 0x23 {
                        index.wrapping_add(1)
                    } else {
                        index.wrapping_sub(1)
                    },
                );
                Ok(10)
            }
            0x09 | 0x19 | 0x29 | 0x39 => {
                let code = (opcode >> 4) & 3;
                let operand = match code {
                    0 => self.bc(),
                    1 => self.de(),
                    2 => index,
                    _ => self.sp,
                };
                let sum = u32::from(index) + u32::from(operand);
                let result = sum as u16;
                let mut flags = self.f & 0xC4;
                flags |= ((result >> 8) as u8) & 0x28;
                if (index & 0x0FFF) + (operand & 0x0FFF) > 0x0FFF {
                    flags |= FLAG_H;
                }
                if sum > 0xFFFF {
                    flags |= FLAG_C;
                }
                self.set_index_value(prefix, result);
                self.f = flags;
                self.q = self.f;
                self.wz = index.wrapping_add(1);
                Ok(15)
            }
            0x24 | 0x25 | 0x26 | 0x2C | 0x2D | 0x2E => {
                let code = if matches!(opcode, 0x24..=0x26) { 4 } else { 5 };
                if matches!(opcode, 0x26 | 0x2E) {
                    let value = self.fetch8(py)?;
                    self.write_index_reg(py, prefix, code, value, true)?;
                    Ok(11)
                } else {
                    let old = self.read_index_reg(py, prefix, code, true)?;
                    let value = if matches!(opcode, 0x24 | 0x2C) {
                        self.inc8(old)
                    } else {
                        self.dec8(old)
                    };
                    self.write_index_reg(py, prefix, code, value, true)?;
                    Ok(8)
                }
            }
            0x34 | 0x35 => {
                let displacement = self.fetch8(py)? as i8;
                let address = index.wrapping_add_signed(i16::from(displacement));
                self.wz = address;
                let old = self.read8(py, address)?;
                let value = if opcode == 0x34 {
                    self.inc8(old)
                } else {
                    self.dec8(old)
                };
                self.write8(py, address, value)?;
                Ok(23)
            }
            0x36 => {
                let displacement = self.fetch8(py)? as i8;
                let value = self.fetch8(py)?;
                let address = index.wrapping_add_signed(i16::from(displacement));
                self.wz = address;
                self.write8(py, address, value)?;
                Ok(19)
            }
            0x40..=0x7F if opcode != 0x76 => {
                let destination = (opcode >> 3) & 7;
                let source = opcode & 7;
                if destination == 6 || source == 6 {
                    let displacement = self.fetch8(py)? as i8;
                    let address = index.wrapping_add_signed(i16::from(displacement));
                    self.wz = address;
                    if source == 6 {
                        let value = self.read8(py, address)?;
                        self.write_index_reg(py, prefix, destination, value, false)?;
                    } else {
                        let value = self.read_index_reg(py, prefix, source, false)?;
                        self.write8(py, address, value)?;
                    }
                    Ok(19)
                } else {
                    let value = self.read_index_reg(py, prefix, source, true)?;
                    self.write_index_reg(py, prefix, destination, value, true)?;
                    Ok(8)
                }
            }
            0x80..=0xBF => {
                let source = opcode & 7;
                let value = if source == 6 {
                    let displacement = self.fetch8(py)? as i8;
                    let address = index.wrapping_add_signed(i16::from(displacement));
                    self.wz = address;
                    self.read8(py, address)?
                } else {
                    self.read_index_reg(py, prefix, source, true)?
                };
                self.alu((opcode >> 3) & 7, value);
                Ok(if source == 6 { 19 } else { 8 })
            }
            0xE1 => {
                let value = self.pop_word(py)?;
                self.set_index_value(prefix, value);
                Ok(14)
            }
            0xE3 => {
                let low = self.read8(py, self.sp)?;
                let high_address = self.sp.wrapping_add(1);
                let high = self.read8(py, high_address)?;
                self.write8(py, high_address, (index >> 8) as u8)?;
                self.write8(py, self.sp, index as u8)?;
                self.set_index_value(prefix, u16::from_le_bytes([low, high]));
                self.wz = u16::from_le_bytes([low, high]);
                Ok(23)
            }
            0xE5 => {
                self.push_word(py, index)?;
                Ok(15)
            }
            0xE9 => {
                self.pc = index;
                Ok(8)
            }
            0xF9 => {
                self.sp = index;
                Ok(10)
            }
            _ => Ok(self.execute_opcode(py, opcode, prefix_pc.wrapping_add(1))? + 4),
        }
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
            0x07 | 0x17 | 0x0F | 0x1F => {
                let old_a = self.a;
                let old_carry = self.f & FLAG_C;
                let (result, carry) = match opcode {
                    0x07 => (old_a.rotate_left(1), old_a >> 7),
                    0x17 => ((old_a << 1) | old_carry, old_a >> 7),
                    0x0F => (old_a.rotate_right(1), old_a & 1),
                    _ => ((old_a >> 1) | (old_carry << 7), old_a & 1),
                };
                self.a = result;
                self.f = (self.f & 0xC4) | (result & 0x28) | carry;
                self.q = self.f;
                Ok(4)
            }
            0x08 => {
                let current = self.af();
                self.set_af(self.af_alt);
                self.q = self.f;
                self.af_alt = current;
                Ok(4)
            }
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
            0x27 => {
                let old_a = self.a;
                let old_f = self.f;
                let subtract = old_f & FLAG_N != 0;
                let old_half = old_f & FLAG_H != 0;
                let old_carry = old_f & FLAG_C != 0;
                let mut correction = 0u8;
                if old_half || (!subtract && (old_a & 0x0F) > 9) {
                    correction |= 0x06;
                }
                if old_carry || (!subtract && old_a > 0x99) {
                    correction |= 0x60;
                }
                let result = if subtract {
                    old_a.wrapping_sub(correction)
                } else {
                    old_a.wrapping_add(correction)
                };
                let mut flags = (result & 0xA8) | (old_f & FLAG_N);
                if result == 0 {
                    flags |= FLAG_Z;
                }
                if result.count_ones() & 1 == 0 {
                    flags |= FLAG_PV;
                }
                if (old_a ^ result) & 0x10 != 0 {
                    flags |= FLAG_H;
                }
                if old_carry || (!subtract && old_a > 0x99) {
                    flags |= FLAG_C;
                }
                self.a = result;
                self.f = flags;
                self.q = self.f;
                Ok(4)
            }
            0x2F => {
                self.a = !self.a;
                self.f = (self.f & 0xC5) | FLAG_H | FLAG_N | (self.a & 0x28);
                self.q = self.f;
                Ok(4)
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
            0x37 => {
                let undocumented = (self.a | (self._q_previous ^ self.f)) & 0x28;
                self.f = (self.f & 0xC4) | undocumented | FLAG_C;
                self.q = self.f;
                Ok(4)
            }
            0x3F => {
                let old_carry = self.f & FLAG_C;
                let undocumented = (self.a | (self._q_previous ^ self.f)) & 0x28;
                self.f = (self.f & 0xC4) | undocumented;
                self.q = self.f;
                if old_carry != 0 {
                    self.f |= FLAG_H;
                    self.q = self.f;
                } else {
                    self.f |= FLAG_C;
                    self.q = self.f;
                }
                Ok(4)
            }
            0xC3 => {
                let low = self.fetch8(py)?;
                let high = self.fetch8(py)?;
                self.pc = u16::from_le_bytes([low, high]);
                self.wz = self.pc;
                Ok(10)
            }
            0xD9 => {
                let bc = self.bc();
                let de = self.de();
                let hl = self.hl();
                self.set_bc(self.bc_alt);
                self.set_de(self.de_alt);
                self.set_hl(self.hl_alt);
                self.bc_alt = bc;
                self.de_alt = de;
                self.hl_alt = hl;
                Ok(4)
            }
            0xE3 => {
                let low = self.read8(py, self.sp)?;
                let high_address = self.sp.wrapping_add(1);
                let high = self.read8(py, high_address)?;
                self.write8(py, high_address, self.h)?;
                self.write8(py, self.sp, self.l)?;
                self.h = high;
                self.l = low;
                self.wz = u16::from_le_bytes([low, high]);
                Ok(19)
            }
            0xE9 => {
                self.pc = self.hl();
                Ok(4)
            }
            0xEB => {
                let de = self.de();
                self.set_de(self.hl());
                self.set_hl(de);
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
                self.write_port(py, address, self.a, Some(11))?;
                self.wz = (address & 0xFF00) | u16::from(port.wrapping_add(1));
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
            0xCB => {
                let sub = self.fetch_opcode(py)?;
                self.execute_cb(py, sub)
            }
            0xDD | 0xFD => self.execute_indexed(py, opcode, opcode_pc),
            0xED => {
                let sub = self.fetch_opcode(py)?;
                match sub {
                    0x43 | 0x53 | 0x63 | 0x73 => {
                        let low = self.fetch8(py)?;
                        let high = self.fetch8(py)?;
                        let address = u16::from_le_bytes([low, high]);
                        let value = self.read_stack_pair((sub >> 4) & 3);
                        self.write8(py, address, value as u8)?;
                        self.write8(py, address.wrapping_add(1), (value >> 8) as u8)?;
                        self.wz = address.wrapping_add(1);
                        Ok(20)
                    }
                    0x4B | 0x5B | 0x6B | 0x7B => {
                        let low = self.fetch8(py)?;
                        let high = self.fetch8(py)?;
                        let address = u16::from_le_bytes([low, high]);
                        let lo = self.read8(py, address)?;
                        let hi = self.read8(py, address.wrapping_add(1))?;
                        self.write_stack_pair((sub >> 4) & 3, u16::from_le_bytes([lo, hi]));
                        self.wz = address.wrapping_add(1);
                        Ok(20)
                    }
                    0x42 | 0x52 | 0x62 | 0x72 => {
                        self.add_sub_hl(self.read_pair((sub >> 4) & 3), true);
                        Ok(15)
                    }
                    0x4A | 0x5A | 0x6A | 0x7A => {
                        self.add_sub_hl(self.read_pair((sub >> 4) & 3), false);
                        Ok(15)
                    }
                    0x47 | 0x4F | 0x57 | 0x5F => {
                        match sub {
                            0x47 => self.i = self.a,
                            0x4F => self.r = self.a,
                            0x57 | 0x5F => {
                                self.a = if sub == 0x57 { self.i } else { self.r };
                                self.p = 1;
                                self.f = (self.f & FLAG_C) | (self.a & 0xA8);
                                self.q = self.f;
                                if self.a == 0 {
                                    self.f |= FLAG_Z;
                                    self.q = self.f;
                                }
                                if self.iff2 {
                                    self.f |= FLAG_PV;
                                    self.q = self.f;
                                }
                            }
                            _ => unreachable!(),
                        }
                        Ok(9)
                    }
                    0x40 | 0x48 | 0x50 | 0x58 | 0x60 | 0x68 | 0x70 | 0x78 => {
                        let register = (sub >> 3) & 7;
                        let port = self.bc();
                        let value = self.read_port(py, port)?;
                        self.wz = port.wrapping_add(1);
                        if register != 6 {
                            self.write_reg(py, register, value)?;
                        }
                        self.f = (self.f & FLAG_C) | (value & 0xA8);
                        self.q = self.f;
                        if value == 0 {
                            self.f |= FLAG_Z;
                            self.q = self.f;
                        }
                        if value.count_ones() & 1 == 0 {
                            self.f |= FLAG_PV;
                            self.q = self.f;
                        }
                        Ok(12)
                    }
                    0x41 | 0x49 | 0x51 | 0x59 | 0x61 | 0x69 | 0x71 | 0x79 => {
                        let register = (sub >> 3) & 7;
                        let port = self.bc();
                        let value = if register == 6 {
                            0
                        } else {
                            self.read_reg(py, register)?
                        };
                        self.write_port(py, port, value, Some(12))?;
                        self.wz = port.wrapping_add(1);
                        Ok(12)
                    }
                    0x44 | 0x4C | 0x54 | 0x5C | 0x64 | 0x6C | 0x74 | 0x7C => {
                        let result = sub8(0, self.a);
                        self.a = result.value;
                        self.f = result.flags;
                        self.q = self.f;
                        Ok(8)
                    }
                    0x67 | 0x6F => {
                        let address = self.hl();
                        let memory_value = self.read8(py, address)?;
                        let new_memory;
                        if sub == 0x67 {
                            new_memory = ((self.a & 0x0F) << 4) | (memory_value >> 4);
                            self.a = (self.a & 0xF0) | (memory_value & 0x0F);
                        } else {
                            new_memory = (memory_value << 4) | (self.a & 0x0F);
                            self.a = (self.a & 0xF0) | (memory_value >> 4);
                        }
                        self.write8(py, address, new_memory)?;
                        self.wz = address.wrapping_add(1);
                        self.f = (self.f & FLAG_C) | (self.a & 0xA8);
                        self.q = self.f;
                        if self.a == 0 {
                            self.f |= FLAG_Z;
                            self.q = self.f;
                        }
                        if self.a.count_ones() & 1 == 0 {
                            self.f |= FLAG_PV;
                            self.q = self.f;
                        }
                        Ok(18)
                    }
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
                    // CPI/CPD/CPIR/CPDR: compare (HL) against A without changing A.
                    0xA1 | 0xA9 | 0xB1 | 0xB9 => {
                        let direction: i16 = if matches!(sub, 0xA9 | 0xB9) { -1 } else { 1 };
                        let value = self.read8(py, self.hl())?;
                        let result = self.a.wrapping_sub(value);
                        let half = (self.a & 0x0F) < (value & 0x0F);
                        self.set_hl(self.hl().wrapping_add_signed(direction));
                        self.set_bc(self.bc().wrapping_sub(1));
                        let adjusted = result.wrapping_sub(u8::from(half));
                        self.f = (self.f & FLAG_C)
                            | FLAG_N
                            | (result & FLAG_S)
                            | if result == 0 { FLAG_Z } else { 0 }
                            | if half { FLAG_H } else { 0 }
                            | if self.bc() != 0 { FLAG_PV } else { 0 }
                            | (adjusted & 0x08)
                            | ((adjusted & 0x02) << 4);
                        self.q = self.f;
                        self.wz = self.wz.wrapping_add_signed(direction);
                        if matches!(sub, 0xB1 | 0xB9) && self.bc() != 0 && result != 0 {
                            self.pc = self.pc.wrapping_sub(2);
                            self.wz = self.pc.wrapping_add(1);
                            self.set_repeat_block_xy_flags();
                            Ok(21)
                        } else {
                            Ok(16)
                        }
                    }
                    // INI/IND/INIR/INDR and OUTI/OUTD/OTIR/OTDR.
                    0xA2 | 0xAA | 0xB2 | 0xBA | 0xA3 | 0xAB | 0xB3 | 0xBB => {
                        let input = matches!(sub, 0xA2 | 0xAA | 0xB2 | 0xBA);
                        let repeat = matches!(sub, 0xB2 | 0xBA | 0xB3 | 0xBB);
                        let direction: i16 = if matches!(sub, 0xAA | 0xBA | 0xAB | 0xBB) {
                            -1
                        } else {
                            1
                        };
                        let old_bc = self.bc();
                        let value = if input {
                            let value = self.read_port(py, old_bc)?;
                            self.write8(py, self.hl(), value)?;
                            value
                        } else {
                            let value = self.read8(py, self.hl())?;
                            let output_bc =
                                (u16::from(self.b.wrapping_sub(1)) << 8) | u16::from(self.c);
                            self.write_port(py, output_bc, value, Some(16))?;
                            value
                        };
                        self.set_hl(self.hl().wrapping_add_signed(direction));
                        self.b = self.b.wrapping_sub(1);
                        let addend = if input {
                            self.c.wrapping_add_signed(direction as i8)
                        } else {
                            self.l
                        };
                        let sum = u16::from(value) + u16::from(addend);
                        self.f = self.b & 0xA8;
                        if self.b == 0 {
                            self.f |= FLAG_Z;
                        }
                        if value & 0x80 != 0 {
                            self.f |= FLAG_N;
                        }
                        if sum > 0xFF {
                            self.f |= FLAG_H | FLAG_C;
                        }
                        if ((sum as u8 & 7) ^ self.b).count_ones() & 1 == 0 {
                            self.f |= FLAG_PV;
                        }
                        self.q = self.f;
                        if repeat && self.b != 0 {
                            self.pc = self.pc.wrapping_sub(2);
                            self.wz = self.pc.wrapping_add(1);
                            self.set_repeat_block_io_flags(value);
                            Ok(21)
                        } else {
                            self.wz = if input {
                                old_bc.wrapping_add_signed(direction)
                            } else {
                                self.bc().wrapping_add_signed(direction)
                            };
                            Ok(16)
                        }
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
                        self.q = self.f;

                        if matches!(sub, 0xB0 | 0xB8) && self.bc() != 0 {
                            self.pc = self.pc.wrapping_sub(2);
                            self.wz = self.pc.wrapping_add(1);
                            self.set_repeat_block_xy_flags();
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
                let return_address = self.pc;
                self.push_word(py, return_address)?;
                self.pc = target;
                self.wz = return_address;
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
                self.wz = target;
                if self.condition((opcode >> 3) & 7) {
                    self.push_word(py, self.pc)?;
                    self.pc = target;
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
