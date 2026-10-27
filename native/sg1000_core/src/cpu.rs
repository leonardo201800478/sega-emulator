//! First Python-independent Z80 execution slice.
//!
//! This module deliberately reports unsupported opcodes instead of silently
//! delegating execution to the Python reference. The implementation is
//! incremental and is not yet a complete Z80.

use crate::alu::{adc8, add8, and8, cp8, or8, sbc8, sub8, xor8};
use crate::bus::CpuBus;

const FLAG_C: u8 = 0x01;
const FLAG_N: u8 = 0x02;
const FLAG_PV: u8 = 0x04;
const FLAG_H: u8 = 0x10;
const FLAG_Z: u8 = 0x40;
const FLAG_S: u8 = 0x80;
const FLAG_XY: u8 = 0x28;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CpuError {
    UnsupportedOpcode { opcode: u8, address: u16 },
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Z80Registers {
    pub a: u8,
    pub f: u8,
    pub b: u8,
    pub c: u8,
    pub d: u8,
    pub e: u8,
    pub h: u8,
    pub l: u8,
    pub sp: u16,
    pub pc: u16,
}

impl Z80Registers {
    #[inline]
    pub fn bc(&self) -> u16 {
        u16::from_be_bytes([self.b, self.c])
    }

    #[inline]
    pub fn de(&self) -> u16 {
        u16::from_be_bytes([self.d, self.e])
    }

    #[inline]
    pub fn hl(&self) -> u16 {
        u16::from_be_bytes([self.h, self.l])
    }

    #[inline]
    pub fn set_bc(&mut self, value: u16) {
        [self.b, self.c] = value.to_be_bytes();
    }

    #[inline]
    pub fn set_de(&mut self, value: u16) {
        [self.d, self.e] = value.to_be_bytes();
    }

    #[inline]
    pub fn set_hl(&mut self, value: u16) {
        [self.h, self.l] = value.to_be_bytes();
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Z80AlternateRegisters {
    pub a: u8,
    pub f: u8,
    pub b: u8,
    pub c: u8,
    pub d: u8,
    pub e: u8,
    pub h: u8,
    pub l: u8,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Z80 {
    pub registers: Z80Registers,
    /// Alternate AF/BC/DE/HL bank selected by EX AF,AF' and EXX.
    pub alternate: Z80AlternateRegisters,
    /// Index registers used by DD/FD-prefixed instructions.
    pub ix: u16,
    pub iy: u16,
    /// Memory refresh and interrupt vector registers.
    pub i: u8,
    pub r: u8,
    pub iff1: bool,
    pub iff2: bool,
    /// Interrupt mode 0, 1 or 2.
    pub interrupt_mode: u8,
    /// Internal WZ/MEMPTR latch used by selected flag and control instructions.
    pub wz: u16,
    /// EI inhibits maskable interrupt acceptance for the following instruction.
    pub ei_delay: u8,
    /// Latched non-maskable interrupt request.
    pub nmi_pending: bool,
    /// Maskable INT request and the external data-bus byte for acknowledge.
    pub int_pending: Option<u8>,
    /// Opcode supplied by an IM 0 interrupt acknowledge; it does not consume PC.
    injected_opcode: Option<u8>,
    /// Last flags value written by an instruction (Z80 Q latch).
    pub q: u8,
    q_previous: u8,
    /// P/V latch used by the Z80 LD A,I/R interrupt edge case.
    pub p: bool,
    step_depth: bool,
    pub halted: bool,
    pub total_tstates: u64,
}

impl Z80 {
    pub fn reset(&mut self) {
        self.registers = Z80Registers::default();
        self.alternate = Z80AlternateRegisters::default();
        self.ix = 0;
        self.iy = 0;
        self.i = 0;
        self.r = 0;
        self.iff1 = false;
        self.iff2 = false;
        self.interrupt_mode = 0;
        self.wz = 0;
        self.ei_delay = 0;
        self.nmi_pending = false;
        self.int_pending = None;
        self.injected_opcode = None;
        self.q = 0;
        self.q_previous = 0;
        self.p = false;
        self.step_depth = false;
        self.halted = false;
        self.total_tstates = 0;
    }

    /// Latch a non-maskable interrupt; it is serviced at the next step boundary.
    pub fn request_nmi(&mut self) {
        self.nmi_pending = true;
    }

    /// Latch a maskable interrupt and the byte supplied by the external data bus.
    pub fn request_int(&mut self, data_bus: u8) {
        self.int_pending = Some(data_bus);
    }

    /// Clear a maskable request when the external INT line is deasserted.
    pub fn clear_int_request(&mut self) {
        self.int_pending = None;
    }

    fn service_nmi<B: CpuBus>(&mut self, bus: &mut B) -> Result<u32, CpuError> {
        self.nmi_pending = false;
        self.halted = false;
        self.r = (self.r & 0x80) | (self.r.wrapping_add(1) & 0x7F);
        self.iff2 = self.iff1;
        self.iff1 = false;
        self.push16(bus, self.registers.pc);
        self.registers.pc = 0x0066;
        self.wz = self.registers.pc;
        self.total_tstates += 11;
        Ok(11)
    }

    fn service_int<B: CpuBus>(&mut self, bus: &mut B) -> Result<u32, CpuError> {
        let data_bus = self.int_pending.take().unwrap_or(0xFF);
        let return_address = self.registers.pc;
        self.halted = false;
        self.r = (self.r & 0x80) | (self.r.wrapping_add(1) & 0x7F);
        self.iff1 = false;
        self.iff2 = false;

        let cycles = match self.interrupt_mode {
            0 => {
                // IM 0 executes the externally supplied byte as the first
                // opcode fetch. Operands, if any, are fetched from the current PC.
                self.injected_opcode = Some(data_bus);
                let instruction_cycles = self.step(bus)?;
                self.total_tstates += 2;
                return Ok(instruction_cycles + 2);
            }
            1 => {
                self.push16(bus, return_address);
                self.registers.pc = 0x0038;
                13
            }
            2 => {
                self.push16(bus, return_address);
                let vector = (u16::from(self.i) << 8) | u16::from(data_bus);
                let low = bus.read_memory(vector);
                let high = bus.read_memory(vector.wrapping_add(1));
                self.registers.pc = u16::from_le_bytes([low, high]);
                self.wz = self.registers.pc;
                19
            }
            _ => {
                return Err(CpuError::UnsupportedOpcode {
                    opcode: data_bus,
                    address: return_address,
                });
            }
        };
        self.total_tstates += u64::from(cycles);
        Ok(cycles)
    }

    /// Execute one instruction and return its documented base T-state count.
    ///
    /// This first extraction slice covers common base instructions only.
    /// Prefixes, interrupts, and all unimplemented opcodes return an explicit
    /// error until their native implementation is migrated.
    pub fn step<B: CpuBus>(&mut self, bus: &mut B) -> Result<u32, CpuError> {
        if self.step_depth {
            return self.step_inner(bus);
        }
        self.q_previous = self.q;
        self.q = 0;
        self.step_depth = true;
        let result = self.step_inner(bus);
        self.step_depth = false;
        result
    }

    fn step_inner<B: CpuBus>(&mut self, bus: &mut B) -> Result<u32, CpuError> {
        if self.nmi_pending {
            self.p = false;
            return self.service_nmi(bus);
        }
        if self.int_pending.is_some() && self.iff1 && self.ei_delay == 0 {
            if self.p {
                self.set_flags(self.registers.f & !FLAG_PV);
            }
            self.p = false;
            return self.service_int(bus);
        }
        self.p = false;
        if self.halted {
            self.r = (self.r & 0x80) | (self.r.wrapping_add(1) & 0x7F);
            self.total_tstates += 4;
            return Ok(4);
        }

        let address = self.registers.pc;
        let opcode = if let Some(injected) = self.injected_opcode.take() {
            self.r = (self.r & 0x80) | (self.r.wrapping_add(1) & 0x7F);
            injected
        } else {
            self.fetch_opcode(bus)
        };
        if opcode == 0xDD || opcode == 0xFD {
            return self.execute_indexed(bus, opcode);
        }
        let cycles = match opcode {
            0x00 => 4, // NOP
            0x08 => {
                std::mem::swap(&mut self.registers.a, &mut self.alternate.a);
                std::mem::swap(&mut self.registers.f, &mut self.alternate.f);
                self.q = self.registers.f;
                4
            }
            0xD9 => {
                std::mem::swap(&mut self.registers.b, &mut self.alternate.b);
                std::mem::swap(&mut self.registers.c, &mut self.alternate.c);
                std::mem::swap(&mut self.registers.d, &mut self.alternate.d);
                std::mem::swap(&mut self.registers.e, &mut self.alternate.e);
                std::mem::swap(&mut self.registers.h, &mut self.alternate.h);
                std::mem::swap(&mut self.registers.l, &mut self.alternate.l);
                4
            }
            0xE3 => {
                let low = bus.read_memory(self.registers.sp);
                let high = bus.read_memory(self.registers.sp.wrapping_add(1));
                let old_hl = self.registers.hl().to_le_bytes();
                bus.write_memory(self.registers.sp, old_hl[0]);
                bus.write_memory(self.registers.sp.wrapping_add(1), old_hl[1]);
                self.registers.set_hl(u16::from_le_bytes([low, high]));
                19
            }
            0xF3 => {
                self.iff1 = false;
                self.iff2 = false;
                self.ei_delay = 0;
                4
            }
            0xFB => {
                self.iff1 = true;
                self.iff2 = true;
                self.ei_delay = 1;
                4
            }
            0xCB => {
                let extended_opcode = self.fetch_opcode(bus);
                self.execute_cb(bus, extended_opcode)
            }
            0xED => {
                let extended_opcode = self.fetch_opcode(bus);
                self.execute_ed(bus, extended_opcode)
            }
            0xD3 => {
                let port_low = self.fetch8(bus);
                let port = (u16::from(self.registers.a) << 8) | u16::from(port_low);
                bus.write_port(port, self.registers.a);
                11
            }
            0xDB => {
                let port_low = self.fetch8(bus);
                let port = (u16::from(self.registers.a) << 8) | u16::from(port_low);
                self.registers.a = bus.read_port(port);
                11
            }
            0x07 | 0x0F | 0x17 | 0x1F => {
                let old_a = self.registers.a;
                let old_carry = self.registers.f & FLAG_C != 0;
                let (result, carry) = match opcode {
                    0x07 => (old_a.rotate_left(1), old_a & 0x80 != 0),
                    0x0F => (old_a.rotate_right(1), old_a & 0x01 != 0),
                    0x17 => ((old_a << 1) | u8::from(old_carry), old_a & 0x80 != 0),
                    _ => ((old_a >> 1) | (u8::from(old_carry) << 7), old_a & 0x01 != 0),
                };
                self.registers.a = result;
                self.set_flags(
                    (self.registers.f & (FLAG_S | FLAG_Z | FLAG_PV))
                        | (result & FLAG_XY)
                        | if carry { FLAG_C } else { 0 },
                );
                4
            }
            0x27 => {
                let a = self.registers.a;
                let old_flags = self.registers.f;
                let subtract = old_flags & FLAG_N != 0;
                let mut correction = 0u8;
                let mut carry = old_flags & FLAG_C != 0;
                if old_flags & FLAG_H != 0 || (!subtract && (a & 0x0F) > 9) {
                    correction |= 0x06;
                }
                if carry || (!subtract && a > 0x99) {
                    correction |= 0x60;
                    carry = true;
                }
                let result = if subtract {
                    a.wrapping_sub(correction)
                } else {
                    a.wrapping_add(correction)
                };
                let mut flags = (old_flags & FLAG_N) | (result & (FLAG_S | FLAG_XY));
                if result == 0 {
                    flags |= FLAG_Z;
                }
                if result.count_ones().is_multiple_of(2) {
                    flags |= FLAG_PV;
                }
                if (a ^ result) & 0x10 != 0 {
                    flags |= FLAG_H;
                }
                if carry {
                    flags |= FLAG_C;
                }
                self.registers.a = result;
                self.set_flags(flags);
                4
            }
            0x2F => {
                self.registers.a = !self.registers.a;
                self.set_flags(
                    (self.registers.f & (FLAG_S | FLAG_Z | FLAG_PV | FLAG_C))
                        | (self.registers.a & FLAG_XY)
                        | FLAG_H
                        | FLAG_N,
                );
                4
            }
            0x37 => {
                let undocumented =
                    (self.registers.a | (self.q_previous ^ self.registers.f)) & FLAG_XY;
                self.set_flags(
                    (self.registers.f & (FLAG_S | FLAG_Z | FLAG_PV)) | undocumented | FLAG_C,
                );
                4
            }
            0x3F => {
                let old_carry = self.registers.f & FLAG_C != 0;
                let undocumented =
                    (self.registers.a | (self.q_previous ^ self.registers.f)) & FLAG_XY;
                self.set_flags(
                    (self.registers.f & (FLAG_S | FLAG_Z | FLAG_PV))
                        | undocumented
                        | if old_carry { FLAG_H } else { FLAG_C },
                );
                4
            }
            0xE9 => {
                self.registers.pc = self.registers.hl();
                4
            }
            0xEB => {
                let de = self.registers.de();
                self.registers.set_de(self.registers.hl());
                self.registers.set_hl(de);
                4
            }
            0xF9 => {
                self.registers.sp = self.registers.hl();
                6
            }
            0x76 => {
                self.halted = true;
                4
            }
            0x01 | 0x11 | 0x21 | 0x31 => {
                let value = self.fetch16(bus);
                match opcode {
                    0x01 => self.registers.set_bc(value),
                    0x11 => self.registers.set_de(value),
                    0x21 => self.registers.set_hl(value),
                    _ => self.registers.sp = value,
                }
                10
            }
            0x02 | 0x12 => {
                let address = if opcode == 0x02 {
                    self.registers.bc()
                } else {
                    self.registers.de()
                };
                bus.write_memory(address, self.registers.a);
                7
            }
            0x0A | 0x1A => {
                let address = if opcode == 0x0A {
                    self.registers.bc()
                } else {
                    self.registers.de()
                };
                self.registers.a = bus.read_memory(address);
                7
            }
            0x22 | 0x32 => {
                let address = self.fetch16(bus);
                if opcode == 0x22 {
                    let [low, high] = self.registers.hl().to_le_bytes();
                    bus.write_memory(address, low);
                    bus.write_memory(address.wrapping_add(1), high);
                    16
                } else {
                    bus.write_memory(address, self.registers.a);
                    13
                }
            }
            0x2A | 0x3A => {
                let address = self.fetch16(bus);
                if opcode == 0x2A {
                    let low = bus.read_memory(address);
                    let high = bus.read_memory(address.wrapping_add(1));
                    self.registers.set_hl(u16::from_le_bytes([low, high]));
                    16
                } else {
                    self.registers.a = bus.read_memory(address);
                    13
                }
            }
            0x03 | 0x13 | 0x23 | 0x33 | 0x0B | 0x1B | 0x2B | 0x3B => {
                let pair = (opcode >> 4) & 0x03;
                let decrement = opcode & 0x08 != 0;
                let value = self.read_register_pair(pair);
                let value = if decrement {
                    value.wrapping_sub(1)
                } else {
                    value.wrapping_add(1)
                };
                self.write_register_pair(pair, value);
                6
            }
            0x09 | 0x19 | 0x29 | 0x39 => {
                let value = self.read_register_pair((opcode >> 4) & 0x03);
                let hl = self.registers.hl();
                let result = u32::from(hl) + u32::from(value);
                let result16 = result as u16;
                let preserved = self.registers.f & (FLAG_S | FLAG_Z | FLAG_PV);
                let mut flags = preserved | ((result16 >> 8) as u8 & FLAG_XY);
                if (hl & 0x0FFF) + (value & 0x0FFF) > 0x0FFF {
                    flags |= FLAG_H;
                }
                if result > u32::from(u16::MAX) {
                    flags |= FLAG_C;
                }
                self.registers.set_hl(result16);
                self.set_flags(flags);
                11
            }
            0x06 | 0x0E | 0x16 | 0x1E | 0x26 | 0x2E | 0x36 | 0x3E => {
                let value = self.fetch8(bus);
                let destination = (opcode >> 3) & 0x07;
                self.write_register(bus, destination, value);
                if destination == 6 { 10 } else { 7 }
            }
            0x40..=0x7F => {
                let destination = (opcode >> 3) & 0x07;
                let source = opcode & 0x07;
                let value = self.read_register(bus, source);
                self.write_register(bus, destination, value);
                if destination == 6 || source == 6 {
                    7
                } else {
                    4
                }
            }
            0x10 => {
                let displacement = self.fetch8(bus) as i8;
                self.registers.b = self.registers.b.wrapping_sub(1);
                if self.registers.b != 0 {
                    self.registers.pc = self
                        .registers
                        .pc
                        .wrapping_add_signed(i16::from(displacement));
                    13
                } else {
                    8
                }
            }
            0x18 | 0x20 | 0x28 | 0x30 | 0x38 => {
                let displacement = self.fetch8(bus) as i8;
                if opcode == 0x18 || self.condition((opcode >> 3) & 0x03) {
                    self.registers.pc = self
                        .registers
                        .pc
                        .wrapping_add_signed(i16::from(displacement));
                    12
                } else {
                    7
                }
            }
            0xC5 | 0xD5 | 0xE5 | 0xF5 => {
                let value = match (opcode >> 4) & 0x03 {
                    0 => self.registers.bc(),
                    1 => self.registers.de(),
                    2 => self.registers.hl(),
                    _ => u16::from_be_bytes([self.registers.a, self.registers.f]),
                };
                self.push16(bus, value);
                11
            }
            0xC1 | 0xD1 | 0xE1 | 0xF1 => {
                let value = self.pop16(bus);
                match (opcode >> 4) & 0x03 {
                    0 => self.registers.set_bc(value),
                    1 => self.registers.set_de(value),
                    2 => self.registers.set_hl(value),
                    _ => {
                        let [accumulator, flags] = value.to_be_bytes();
                        self.registers.a = accumulator;
                        self.set_flags(flags);
                    }
                }
                10
            }
            0xC4 | 0xCC | 0xD4 | 0xDC | 0xE4 | 0xEC | 0xF4 | 0xFC => {
                let target = self.fetch16(bus);
                if self.condition((opcode >> 3) & 0x07) {
                    self.push16(bus, self.registers.pc);
                    self.registers.pc = target;
                    17
                } else {
                    10
                }
            }
            0xC0 | 0xC8 | 0xD0 | 0xD8 | 0xE0 | 0xE8 | 0xF0 | 0xF8 => {
                if self.condition((opcode >> 3) & 0x07) {
                    self.registers.pc = self.pop16(bus);
                    11
                } else {
                    5
                }
            }
            0xC9 => {
                self.registers.pc = self.pop16(bus);
                10
            }
            0xCD => {
                let target = self.fetch16(bus);
                self.push16(bus, self.registers.pc);
                self.registers.pc = target;
                17
            }
            0xC2 | 0xCA | 0xD2 | 0xDA | 0xE2 | 0xEA | 0xF2 | 0xFA => {
                let target = self.fetch16(bus);
                if self.condition((opcode >> 3) & 0x07) {
                    self.registers.pc = target;
                }
                10
            }
            0x04 | 0x0C | 0x14 | 0x1C | 0x24 | 0x2C | 0x34 | 0x3C | 0x05 | 0x0D | 0x15 | 0x1D
            | 0x25 | 0x2D | 0x35 | 0x3D => {
                let register = (opcode >> 3) & 0x07;
                let decrement = opcode & 0x01 != 0;
                let old = self.read_register(bus, register);
                let value = if decrement {
                    old.wrapping_sub(1)
                } else {
                    old.wrapping_add(1)
                };
                let carry = self.registers.f & FLAG_C;
                let mut flags = (value & (FLAG_S | FLAG_XY)) | carry;
                if value == 0 {
                    flags |= FLAG_Z;
                }
                if if decrement {
                    old & 0x0F == 0
                } else {
                    old & 0x0F == 0x0F
                } {
                    flags |= FLAG_H;
                }
                if if decrement { old == 0x80 } else { old == 0x7F } {
                    flags |= FLAG_PV;
                }
                if decrement {
                    flags |= 0x02; // N
                }
                self.write_register(bus, register, value);
                self.set_flags(flags);
                if register == 6 { 11 } else { 4 }
            }
            0xAF => {
                self.registers.a = 0;
                self.set_flags(FLAG_Z | FLAG_PV);
                4
            }
            0x80..=0xBF => {
                let source = opcode & 0x07;
                let value = self.read_register(bus, source);
                self.apply_alu_operation((opcode >> 3) & 0x07, value);
                if source == 6 { 7 } else { 4 }
            }
            0xC6 | 0xCE | 0xD6 | 0xDE | 0xE6 | 0xEE | 0xF6 | 0xFE => {
                let value = self.fetch8(bus);
                let operation = (opcode >> 3) & 0x07;
                self.apply_alu_operation(operation, value);
                7
            }
            0xC3 => {
                self.registers.pc = self.fetch16(bus);
                10
            }
            opcode if opcode & 0xC7 == 0xC7 => {
                self.push16(bus, self.registers.pc);
                self.registers.pc = u16::from(opcode & 0x38);
                11
            }
            _ => return Err(CpuError::UnsupportedOpcode { opcode, address }),
        };
        self.total_tstates += u64::from(cycles);
        if self.ei_delay > 0 && opcode != 0xFB {
            self.ei_delay -= 1;
        }
        Ok(cycles)
    }

    /// Execute DD/FD-prefixed instructions for IX/IY. Prefixes that do not
    /// affect an instruction are ignored while retaining their extra M1 cost.
    fn execute_indexed<B: CpuBus>(&mut self, bus: &mut B, prefix: u8) -> Result<u32, CpuError> {
        let mut active_prefix = prefix;
        let mut opcode = self.fetch_opcode(bus);
        let mut extra_prefix_cycles = 0_u32;
        while opcode == 0xDD || opcode == 0xFD {
            active_prefix = opcode;
            extra_prefix_cycles += 4;
            opcode = self.fetch_opcode(bus);
        }
        let prefix = active_prefix;
        let index = if prefix == 0xDD { self.ix } else { self.iy };
        let cycles = match opcode {
            0xCB => {
                let displacement = self.fetch8(bus) as i8;
                let cb_opcode = self.fetch8(bus);
                let address = index.wrapping_add_signed(i16::from(displacement));
                self.wz = address;
                self.execute_indexed_cb(bus, address, cb_opcode)
            }
            0x21 => {
                let value = self.fetch16(bus);
                self.set_index_value(prefix, value);
                14
            }
            0x22 | 0x2A => {
                let address = self.fetch16(bus);
                self.wz = address.wrapping_add(1);
                if opcode == 0x22 {
                    let [low, high] = index.to_le_bytes();
                    bus.write_memory(address, low);
                    bus.write_memory(address.wrapping_add(1), high);
                } else {
                    let low = bus.read_memory(address);
                    let high = bus.read_memory(address.wrapping_add(1));
                    self.set_index_value(prefix, u16::from_le_bytes([low, high]));
                }
                20
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
                10
            }
            0x09 | 0x19 | 0x29 | 0x39 => {
                let pair_code = (opcode >> 4) & 3;
                let operand = if pair_code == 2 {
                    index
                } else {
                    self.read_register_pair(pair_code)
                };
                let result = u32::from(index) + u32::from(operand);
                let result16 = result as u16;
                let mut flags = self.registers.f & (FLAG_S | FLAG_Z | FLAG_PV);
                flags |= ((result16 >> 8) as u8) & FLAG_XY;
                if u32::from(index & 0x0FFF) + u32::from(operand & 0x0FFF) > 0x0FFF {
                    flags |= FLAG_H;
                }
                if result > u32::from(u16::MAX) {
                    flags |= FLAG_C;
                }
                self.set_index_value(prefix, result16);
                self.set_flags(flags);
                self.wz = index.wrapping_add(1);
                15
            }
            0xE1 => {
                let value = self.pop16(bus);
                self.set_index_value(prefix, value);
                14
            }
            0xE5 => {
                self.push16(bus, index);
                15
            }
            0xE3 => {
                let low = bus.read_memory(self.registers.sp);
                let high_address = self.registers.sp.wrapping_add(1);
                let high = bus.read_memory(high_address);
                let [old_low, old_high] = index.to_le_bytes();
                bus.write_memory(high_address, old_high);
                bus.write_memory(self.registers.sp, old_low);
                self.set_index_value(prefix, u16::from_le_bytes([low, high]));
                self.wz = u16::from_le_bytes([low, high]);
                23
            }
            0xE9 => {
                self.registers.pc = index;
                8
            }
            0xF9 => {
                self.registers.sp = index;
                10
            }
            0x24 | 0x25 | 0x26 | 0x2C | 0x2D | 0x2E => {
                let code = if matches!(opcode, 0x24..=0x26) { 4 } else { 5 };
                if opcode == 0x26 || opcode == 0x2E {
                    let value = self.fetch8(bus);
                    self.write_index_register(prefix, code, value, true);
                    11
                } else {
                    let old = self.read_index_register(bus, prefix, code, true);
                    let value =
                        self.increment_decrement_value(old, opcode == 0x25 || opcode == 0x2D);
                    self.write_index_register(prefix, code, value, true);
                    8
                }
            }
            0x34 | 0x35 => {
                let displacement = self.fetch8(bus) as i8;
                let address = index.wrapping_add_signed(i16::from(displacement));
                self.wz = address;
                let old = bus.read_memory(address);
                let value = self.increment_decrement_value(old, opcode == 0x35);
                bus.write_memory(address, value);
                23
            }
            0x36 => {
                let displacement = self.fetch8(bus) as i8;
                let value = self.fetch8(bus);
                let address = index.wrapping_add_signed(i16::from(displacement));
                self.wz = address;
                bus.write_memory(address, value);
                19
            }
            0x40..=0x7F if opcode != 0x76 => {
                let destination = (opcode >> 3) & 7;
                let source = opcode & 7;
                if destination == 6 || source == 6 {
                    let displacement = self.fetch8(bus) as i8;
                    let address = index.wrapping_add_signed(i16::from(displacement));
                    self.wz = address;
                    if source == 6 {
                        let value = bus.read_memory(address);
                        self.write_index_register(prefix, destination, value, false);
                    } else {
                        let value = self.read_index_register(bus, prefix, source, false);
                        bus.write_memory(address, value);
                    }
                    19
                } else {
                    let value = self.read_index_register(bus, prefix, source, true);
                    self.write_index_register(prefix, destination, value, true);
                    8
                }
            }
            0x80..=0xBF => {
                let source = opcode & 7;
                let value = if source == 6 {
                    let displacement = self.fetch8(bus) as i8;
                    let address = index.wrapping_add_signed(i16::from(displacement));
                    self.wz = address;
                    bus.read_memory(address)
                } else {
                    self.read_index_register(bus, prefix, source, true)
                };
                self.apply_alu_operation((opcode >> 3) & 7, value);
                if source == 6 { 19 } else { 8 }
            }
            _ => {
                // Rewind the already-read opcode and its refresh increment so
                // the base decoder handles this unaffected instruction once.
                self.registers.pc = self.registers.pc.wrapping_sub(1);
                self.r = (self.r & 0x80) | (self.r.wrapping_sub(1) & 0x7F);
                let base_cycles = self.step(bus)?;
                let prefix_cycles = 4 + extra_prefix_cycles;
                self.total_tstates += u64::from(prefix_cycles);
                return Ok(base_cycles + prefix_cycles);
            }
        };

        let total_cycles = cycles + extra_prefix_cycles;
        self.total_tstates += u64::from(total_cycles);
        if self.ei_delay > 0 {
            self.ei_delay -= 1;
        }
        Ok(total_cycles)
    }

    fn execute_indexed_cb<B: CpuBus>(&mut self, bus: &mut B, address: u16, opcode: u8) -> u32 {
        let register = opcode & 7;
        let operation = opcode >> 6;
        let bit = (opcode >> 3) & 7;
        let value = bus.read_memory(address);
        match operation {
            0 => {
                let carry_in = self.registers.f & FLAG_C != 0;
                let (result, carry) = match bit {
                    0 => (value.rotate_left(1), value & 0x80 != 0),
                    1 => (value.rotate_right(1), value & 1 != 0),
                    2 => ((value << 1) | u8::from(carry_in), value & 0x80 != 0),
                    3 => ((value >> 1) | (u8::from(carry_in) << 7), value & 1 != 0),
                    4 => (value << 1, value & 0x80 != 0),
                    5 => ((value >> 1) | (value & 0x80), value & 1 != 0),
                    6 => ((value << 1) | 1, value & 0x80 != 0),
                    _ => (value >> 1, value & 1 != 0),
                };
                bus.write_memory(address, result);
                if register != 6 {
                    self.write_register(bus, register, result);
                }
                let mut flags = result & (FLAG_S | FLAG_XY);
                if result == 0 {
                    flags |= FLAG_Z;
                }
                if result.count_ones().is_multiple_of(2) {
                    flags |= FLAG_PV;
                }
                if carry {
                    flags |= FLAG_C;
                }
                self.set_flags(flags);
                23
            }
            1 => {
                let bit_clear = value & (1 << bit) == 0;
                let mut flags =
                    (self.registers.f & FLAG_C) | FLAG_H | ((address >> 8) as u8 & FLAG_XY);
                if bit_clear {
                    flags |= FLAG_Z | FLAG_PV;
                }
                if bit == 7 && !bit_clear {
                    flags |= FLAG_S;
                }
                self.set_flags(flags);
                20
            }
            2 | 3 => {
                let result = if operation == 2 {
                    value & !(1 << bit)
                } else {
                    value | (1 << bit)
                };
                bus.write_memory(address, result);
                if register != 6 {
                    self.write_register(bus, register, result);
                }
                23
            }
            _ => unreachable!(),
        }
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

    fn read_index_register<B: CpuBus>(
        &mut self,
        bus: &mut B,
        prefix: u8,
        code: u8,
        halves: bool,
    ) -> u8 {
        let index = self.index_value(prefix);
        match (halves, code) {
            (true, 4) => (index >> 8) as u8,
            (true, 5) => index as u8,
            _ => self.read_register(bus, code),
        }
    }

    fn write_index_register(&mut self, prefix: u8, code: u8, value: u8, halves: bool) {
        let index = self.index_value(prefix);
        match (halves, code) {
            (true, 4) => self.set_index_value(prefix, (u16::from(value) << 8) | (index & 0x00FF)),
            (true, 5) => self.set_index_value(prefix, (index & 0xFF00) | u16::from(value)),
            _ => {
                // The only non-index destinations reaching this path are H/L;
                // memory operands are handled separately by the indexed decoder.
                if code == 4 {
                    self.registers.h = value;
                } else if code == 5 {
                    self.registers.l = value;
                } else if code == 7 {
                    self.registers.a = value;
                } else if code == 0 {
                    self.registers.b = value;
                } else if code == 1 {
                    self.registers.c = value;
                } else if code == 2 {
                    self.registers.d = value;
                } else if code == 3 {
                    self.registers.e = value;
                }
            }
        }
    }

    fn increment_decrement_value(&mut self, old: u8, decrement: bool) -> u8 {
        let value = if decrement {
            old.wrapping_sub(1)
        } else {
            old.wrapping_add(1)
        };
        let carry = self.registers.f & FLAG_C;
        let mut flags = (value & (FLAG_S | FLAG_XY)) | carry;
        if value == 0 {
            flags |= FLAG_Z;
        }
        if if decrement {
            old & 0x0F == 0
        } else {
            old & 0x0F == 0x0F
        } {
            flags |= FLAG_H;
        }
        if if decrement { old == 0x80 } else { old == 0x7F } {
            flags |= FLAG_PV;
        }
        if decrement {
            flags |= FLAG_N;
        }
        self.set_flags(flags);
        value
    }

    #[inline]
    fn set_flags(&mut self, flags: u8) {
        self.registers.f = flags;
        self.q = flags;
    }

    /// Execute ED-prefixed Z80 instructions. Unassigned ED encodings are
    /// documented as two-byte NOPs on the classic Z80.
    fn execute_ed<B: CpuBus>(&mut self, bus: &mut B, opcode: u8) -> u32 {
        match opcode {
            0x47 => {
                self.i = self.registers.a;
                9
            }
            0x4F => {
                self.r = self.registers.a;
                9
            }
            0x57 | 0x5F => {
                self.registers.a = if opcode == 0x57 { self.i } else { self.r };
                let mut flags =
                    (self.registers.f & FLAG_C) | (self.registers.a & (FLAG_S | FLAG_XY));
                if self.registers.a == 0 {
                    flags |= FLAG_Z;
                }
                if self.iff2 {
                    flags |= FLAG_PV;
                }
                self.set_flags(flags);
                self.p = true;
                9
            }
            op if op & 0xCF == 0x43 => {
                let address = self.fetch16(bus);
                let [low, high] = self.read_register_pair((op >> 4) & 3).to_le_bytes();
                bus.write_memory(address, low);
                bus.write_memory(address.wrapping_add(1), high);
                self.wz = address.wrapping_add(1);
                20
            }
            op if op & 0xCF == 0x4B => {
                let address = self.fetch16(bus);
                let low = bus.read_memory(address);
                let high = bus.read_memory(address.wrapping_add(1));
                self.write_register_pair((op >> 4) & 3, u16::from_le_bytes([low, high]));
                self.wz = address.wrapping_add(1);
                20
            }
            op if op & 0xCF == 0x4A => {
                self.adc_sbc_hl(self.read_register_pair((op >> 4) & 3), false);
                15
            }
            op if op & 0xCF == 0x42 => {
                self.adc_sbc_hl(self.read_register_pair((op >> 4) & 3), true);
                15
            }
            op if op & 0xC7 == 0x40 => {
                let register = (op >> 3) & 7;
                let port = self.registers.bc();
                let value = bus.read_port(port);
                if register != 6 {
                    self.write_register(bus, register, value);
                }
                let mut flags = (self.registers.f & FLAG_C) | (value & (FLAG_S | FLAG_XY));
                if value == 0 {
                    flags |= FLAG_Z;
                }
                if value.count_ones().is_multiple_of(2) {
                    flags |= FLAG_PV;
                }
                self.set_flags(flags);
                self.wz = port.wrapping_add(1);
                12
            }
            op if op & 0xC7 == 0x41 => {
                let register = (op >> 3) & 7;
                let port = self.registers.bc();
                let value = if register == 6 {
                    0
                } else {
                    self.read_register(bus, register)
                };
                bus.write_port(port, value);
                self.wz = port.wrapping_add(1);
                12
            }
            op if op & 0xC7 == 0x44 => {
                let value = self.registers.a;
                let result = sub8(0, value);
                self.registers.a = result.value;
                self.set_flags(result.flags);
                8
            }
            op if op & 0xC7 == 0x45 => {
                self.registers.pc = self.pop16(bus);
                self.iff1 = self.iff2;
                self.wz = self.registers.pc;
                14
            }
            0x46 | 0x4E | 0x66 | 0x6E => {
                self.interrupt_mode = 0;
                8
            }
            0x56 | 0x76 => {
                self.interrupt_mode = 1;
                8
            }
            0x5E | 0x7E => {
                self.interrupt_mode = 2;
                8
            }
            0x67 | 0x6F => {
                let address = self.registers.hl();
                let memory_value = bus.read_memory(address);
                let accumulator = self.registers.a;
                let new_memory;
                if opcode == 0x67 {
                    new_memory = ((accumulator & 0x0F) << 4) | (memory_value >> 4);
                    self.registers.a = (accumulator & 0xF0) | (memory_value & 0x0F);
                } else {
                    new_memory = (memory_value << 4) | (accumulator & 0x0F);
                    self.registers.a = (accumulator & 0xF0) | (memory_value >> 4);
                }
                bus.write_memory(address, new_memory);
                self.wz = address.wrapping_add(1);
                self.set_szp_xy_flags_preserving_carry(self.registers.a);
                18
            }
            0xA0 | 0xA8 | 0xB0 | 0xB8 => self.execute_block_transfer(bus, opcode),
            0xA1 | 0xA9 | 0xB1 | 0xB9 => self.execute_block_compare(bus, opcode),
            0xA2 | 0xAA | 0xB2 | 0xBA | 0xA3 | 0xAB | 0xB3 | 0xBB => {
                self.execute_block_io(bus, opcode)
            }
            _ => 8,
        }
    }

    fn adc_sbc_hl(&mut self, operand: u16, subtract: bool) {
        let lhs = self.registers.hl();
        let carry = u32::from(self.registers.f & FLAG_C != 0);
        let rhs = u32::from(operand);
        let lhs32 = u32::from(lhs);
        let result32 = if subtract {
            lhs32.wrapping_sub(rhs + carry)
        } else {
            lhs32 + rhs + carry
        };
        let result = result32 as u16;
        let mut flags = ((result >> 8) as u8) & (FLAG_S | FLAG_XY);
        if result == 0 {
            flags |= FLAG_Z;
        }
        if subtract {
            flags |= FLAG_N;
            if (lhs & 0x0FFF) < ((operand & 0x0FFF) + carry as u16) {
                flags |= FLAG_H;
            }
            if ((lhs ^ operand) & (lhs ^ result) & 0x8000) != 0 {
                flags |= FLAG_PV;
            }
            if lhs32 < rhs + carry {
                flags |= FLAG_C;
            }
        } else {
            if u32::from(lhs & 0x0FFF) + u32::from(operand & 0x0FFF) + carry > 0x0FFF {
                flags |= FLAG_H;
            }
            if (!(lhs ^ operand) & (lhs ^ result) & 0x8000) != 0 {
                flags |= FLAG_PV;
            }
            if result32 > u32::from(u16::MAX) {
                flags |= FLAG_C;
            }
        }
        self.registers.set_hl(result);
        self.set_flags(flags);
    }

    fn set_szp_xy_flags_preserving_carry(&mut self, value: u8) {
        let mut flags = (self.registers.f & FLAG_C) | (value & (FLAG_S | FLAG_XY));
        if value == 0 {
            flags |= FLAG_Z;
        }
        if value.count_ones().is_multiple_of(2) {
            flags |= FLAG_PV;
        }
        self.set_flags(flags);
    }

    fn execute_block_transfer<B: CpuBus>(&mut self, bus: &mut B, opcode: u8) -> u32 {
        let direction = if opcode == 0xA8 || opcode == 0xB8 {
            -1_i16
        } else {
            1_i16
        };
        let address = self.registers.hl();
        let value = bus.read_memory(address);
        bus.write_memory(self.registers.de(), value);
        self.registers
            .set_hl(address.wrapping_add_signed(direction));
        self.registers
            .set_de(self.registers.de().wrapping_add_signed(direction));
        let count = self.registers.bc().wrapping_sub(1);
        self.registers.set_bc(count);
        let sum = self.registers.a.wrapping_add(value);
        let mut flags = self.registers.f & (FLAG_S | FLAG_Z | FLAG_C);
        if count != 0 {
            flags |= FLAG_PV;
        }
        flags |= sum & 0x08;
        flags |= (sum & 0x02) << 4;
        self.set_flags(flags);
        if (opcode == 0xB0 || opcode == 0xB8) && count != 0 {
            self.registers.pc = self.registers.pc.wrapping_sub(2);
            self.wz = self.registers.pc.wrapping_add(1);
            21
        } else {
            16
        }
    }

    fn execute_block_compare<B: CpuBus>(&mut self, bus: &mut B, opcode: u8) -> u32 {
        let direction = if opcode == 0xA9 || opcode == 0xB9 {
            -1_i16
        } else {
            1_i16
        };
        let value = bus.read_memory(self.registers.hl());
        let accumulator = self.registers.a;
        let result = accumulator.wrapping_sub(value);
        let half = (accumulator & 0x0F) < (value & 0x0F);
        self.registers
            .set_hl(self.registers.hl().wrapping_add_signed(direction));
        let count = self.registers.bc().wrapping_sub(1);
        self.registers.set_bc(count);
        let adjusted = result.wrapping_sub(u8::from(half));
        let mut flags = (self.registers.f & FLAG_C) | FLAG_N | (result & FLAG_S);
        if result == 0 {
            flags |= FLAG_Z;
        }
        if half {
            flags |= FLAG_H;
        }
        if count != 0 {
            flags |= FLAG_PV;
        }
        flags |= adjusted & 0x08;
        flags |= (adjusted & 0x02) << 4;
        self.set_flags(flags);
        self.wz = self.wz.wrapping_add_signed(direction);
        if (opcode == 0xB1 || opcode == 0xB9) && count != 0 && result != 0 {
            self.registers.pc = self.registers.pc.wrapping_sub(2);
            self.wz = self.registers.pc.wrapping_add(1);
            21
        } else {
            16
        }
    }

    fn execute_block_io<B: CpuBus>(&mut self, bus: &mut B, opcode: u8) -> u32 {
        let input = matches!(opcode, 0xA2 | 0xAA | 0xB2 | 0xBA);
        let repeat = matches!(opcode, 0xB2 | 0xBA | 0xB3 | 0xBB);
        let direction = if matches!(opcode, 0xAA | 0xBA | 0xAB | 0xBB) {
            -1_i16
        } else {
            1_i16
        };
        let old_bc = self.registers.bc();
        let value = if input {
            let value = bus.read_port(old_bc);
            bus.write_memory(self.registers.hl(), value);
            value
        } else {
            let value = bus.read_memory(self.registers.hl());
            let output_port =
                (u16::from(self.registers.b.wrapping_sub(1)) << 8) | u16::from(self.registers.c);
            bus.write_port(output_port, value);
            value
        };
        self.registers
            .set_hl(self.registers.hl().wrapping_add_signed(direction));
        self.registers.b = self.registers.b.wrapping_sub(1);
        let low_term = if input {
            self.registers.c.wrapping_add_signed(direction as i8)
        } else {
            self.registers.l
        };
        let sum = u16::from(value) + u16::from(low_term);
        let mut flags = self.registers.b & (FLAG_S | FLAG_XY);
        if self.registers.b == 0 {
            flags |= FLAG_Z;
        }
        if value & 0x80 != 0 {
            flags |= FLAG_N;
        }
        if sum > 0xFF {
            flags |= FLAG_H | FLAG_C;
        }
        let parity_arg = (sum as u8 & 0x07) ^ self.registers.b;
        if parity_arg.count_ones().is_multiple_of(2) {
            flags |= FLAG_PV;
        }
        self.set_flags(flags);
        if repeat && self.registers.b != 0 {
            self.registers.pc = self.registers.pc.wrapping_sub(2);
            self.wz = self.registers.pc.wrapping_add(1);
            21
        } else {
            let wz_base = if input { old_bc } else { self.registers.bc() };
            self.wz = wz_base.wrapping_add_signed(direction);
            16
        }
    }

    /// Execute the complete CB-prefixed rotate/shift, BIT, RES and SET matrix.
    /// T-state totals are instruction-level totals, not bus-phase timestamps.
    fn execute_cb<B: CpuBus>(&mut self, bus: &mut B, opcode: u8) -> u32 {
        let target = opcode & 0x07;
        let group = opcode >> 6;
        let operation = (opcode >> 3) & 0x07;
        let value = self.read_register(bus, target);

        match group {
            0 => {
                let old_carry = self.registers.f & FLAG_C != 0;
                let (result, carry) = match operation {
                    0 => (value.rotate_left(1), value & 0x80 != 0), // RLC
                    1 => (value.rotate_right(1), value & 0x01 != 0), // RRC
                    2 => ((value << 1) | u8::from(old_carry), value & 0x80 != 0), // RL
                    3 => ((value >> 1) | (u8::from(old_carry) << 7), value & 1 != 0), // RR
                    4 => (value << 1, value & 0x80 != 0),           // SLA
                    5 => ((value >> 1) | (value & 0x80), value & 1 != 0), // SRA
                    6 => ((value << 1) | 1, value & 0x80 != 0),     // undocumented SLL
                    _ => (value >> 1, value & 1 != 0),              // SRL
                };
                let mut flags = result & (FLAG_S | FLAG_XY);
                if result == 0 {
                    flags |= FLAG_Z;
                }
                if result.count_ones().is_multiple_of(2) {
                    flags |= FLAG_PV;
                }
                if carry {
                    flags |= FLAG_C;
                }
                self.set_flags(flags);
                self.write_register(bus, target, result);
            }
            1 => {
                let mask = 1_u8 << operation;
                let mut flags = (self.registers.f & FLAG_C) | FLAG_H;
                let bit_clear = value & mask == 0;
                if bit_clear {
                    flags |= FLAG_Z | FLAG_PV;
                }
                if operation == 7 && !bit_clear {
                    flags |= FLAG_S;
                }
                let undocumented = if target == 6 {
                    (self.registers.hl() >> 8) as u8 & FLAG_XY
                } else {
                    value & FLAG_XY
                };
                flags |= undocumented;
                self.set_flags(flags);
            }
            2 => self.write_register(bus, target, value & !(1_u8 << operation)),
            _ => self.write_register(bus, target, value | (1_u8 << operation)),
        }

        if target == 6 {
            if group == 1 { 12 } else { 15 }
        } else {
            8
        }
    }

    #[inline]
    fn read_register_pair(&self, code: u8) -> u16 {
        match code & 0x03 {
            0 => self.registers.bc(),
            1 => self.registers.de(),
            2 => self.registers.hl(),
            _ => self.registers.sp,
        }
    }

    #[inline]
    fn write_register_pair(&mut self, code: u8, value: u16) {
        match code & 0x03 {
            0 => self.registers.set_bc(value),
            1 => self.registers.set_de(value),
            2 => self.registers.set_hl(value),
            _ => self.registers.sp = value,
        }
    }

    #[inline]
    fn apply_alu_operation(&mut self, operation: u8, value: u8) {
        let accumulator = self.registers.a;
        let carry = self.registers.f & FLAG_C != 0;
        let result = match operation {
            0 => add8(accumulator, value),
            1 => adc8(accumulator, value, carry),
            2 => sub8(accumulator, value),
            3 => sbc8(accumulator, value, carry),
            4 => and8(accumulator, value),
            5 => xor8(accumulator, value),
            6 => or8(accumulator, value),
            _ => cp8(accumulator, value),
        };
        if operation != 7 {
            self.registers.a = result.value;
        }
        self.set_flags(result.flags);
    }

    #[inline]
    fn condition(&self, code: u8) -> bool {
        let flags = self.registers.f;
        match code & 0x07 {
            0 => flags & FLAG_Z == 0,  // NZ
            1 => flags & FLAG_Z != 0,  // Z
            2 => flags & FLAG_C == 0,  // NC
            3 => flags & FLAG_C != 0,  // C
            4 => flags & FLAG_PV == 0, // PO
            5 => flags & FLAG_PV != 0, // PE
            6 => flags & FLAG_S == 0,  // P
            _ => flags & FLAG_S != 0,  // M
        }
    }

    #[inline]
    fn push16<B: CpuBus>(&mut self, bus: &mut B, value: u16) {
        let [high, low] = value.to_be_bytes();
        self.registers.sp = self.registers.sp.wrapping_sub(1);
        bus.write_memory(self.registers.sp, high);
        self.registers.sp = self.registers.sp.wrapping_sub(1);
        bus.write_memory(self.registers.sp, low);
    }

    #[inline]
    fn pop16<B: CpuBus>(&mut self, bus: &mut B) -> u16 {
        let low = bus.read_memory(self.registers.sp);
        self.registers.sp = self.registers.sp.wrapping_add(1);
        let high = bus.read_memory(self.registers.sp);
        self.registers.sp = self.registers.sp.wrapping_add(1);
        u16::from_le_bytes([low, high])
    }

    #[inline]
    fn fetch8<B: CpuBus>(&mut self, bus: &mut B) -> u8 {
        let value = bus.read_memory(self.registers.pc);
        self.registers.pc = self.registers.pc.wrapping_add(1);
        value
    }

    #[inline]
    fn fetch_opcode<B: CpuBus>(&mut self, bus: &mut B) -> u8 {
        let opcode = self.fetch8(bus);
        self.r = (self.r & 0x80) | (self.r.wrapping_add(1) & 0x7F);
        opcode
    }

    #[inline]
    fn fetch16<B: CpuBus>(&mut self, bus: &mut B) -> u16 {
        let low = self.fetch8(bus);
        let high = self.fetch8(bus);
        u16::from_le_bytes([low, high])
    }

    #[inline]
    fn read_register<B: CpuBus>(&mut self, bus: &mut B, code: u8) -> u8 {
        match code {
            0 => self.registers.b,
            1 => self.registers.c,
            2 => self.registers.d,
            3 => self.registers.e,
            4 => self.registers.h,
            5 => self.registers.l,
            6 => bus.read_memory(self.registers.hl()),
            _ => self.registers.a,
        }
    }

    #[inline]
    fn write_register<B: CpuBus>(&mut self, bus: &mut B, code: u8, value: u8) {
        match code {
            0 => self.registers.b = value,
            1 => self.registers.c = value,
            2 => self.registers.d = value,
            3 => self.registers.e = value,
            4 => self.registers.h = value,
            5 => self.registers.l = value,
            6 => bus.write_memory(self.registers.hl(), value),
            _ => self.registers.a = value,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{FLAG_C, FLAG_H, FLAG_N, FLAG_PV, FLAG_Z, Z80};
    use crate::bus::CpuBus;

    struct TestBus([u8; 0x10000]);

    impl CpuBus for TestBus {
        fn read_memory(&mut self, address: u16) -> u8 {
            self.0[usize::from(address)]
        }

        fn write_memory(&mut self, address: u16, value: u8) {
            self.0[usize::from(address)] = value;
        }
    }

    #[test]
    fn accumulator_rotate_and_flag_control_opcodes_are_supported() {
        let mut bus = TestBus([0; 0x10000]);
        bus.0[..6].copy_from_slice(&[0x3E, 0x81, 0x07, 0x2F, 0x37, 0x3F]);
        let mut cpu = Z80::default();
        assert_eq!(cpu.step(&mut bus), Ok(7));
        assert_eq!(cpu.step(&mut bus), Ok(4));
        assert_eq!(cpu.registers.a, 0x03);
        assert_ne!(cpu.registers.f & FLAG_C, 0);
        assert_eq!(cpu.step(&mut bus), Ok(4));
        assert_eq!(cpu.registers.a, 0xFC);
        assert_eq!(cpu.registers.f & (FLAG_H | FLAG_N), FLAG_H | FLAG_N);
        assert_eq!(cpu.step(&mut bus), Ok(4));
        assert_eq!(cpu.registers.f & (FLAG_H | FLAG_N | FLAG_C), FLAG_C);
        assert_eq!(cpu.step(&mut bus), Ok(4));
        assert_eq!(cpu.registers.f & (FLAG_H | FLAG_N | FLAG_C), FLAG_H);
    }

    #[test]
    fn daa_corrects_bcd_addition() {
        let mut bus = TestBus([0; 0x10000]);
        bus.0[..5].copy_from_slice(&[0x3E, 0x09, 0xC6, 0x01, 0x27]);
        let mut cpu = Z80::default();
        assert_eq!(cpu.step(&mut bus), Ok(7));
        assert_eq!(cpu.step(&mut bus), Ok(7));
        assert_eq!(cpu.registers.a, 0x0A);
        assert_eq!(cpu.step(&mut bus), Ok(4));
        assert_eq!(cpu.registers.a, 0x10);
        assert_ne!(cpu.registers.f & FLAG_H, 0);
        assert_eq!(cpu.registers.f & FLAG_C, 0);
    }

    #[test]
    fn djnz_uses_taken_and_not_taken_timing() {
        let mut bus = TestBus([0; 0x10000]);
        bus.0[..4].copy_from_slice(&[0x10, 0xFE, 0x10, 0x00]);
        let mut cpu = Z80::default();
        cpu.registers.b = 2;
        assert_eq!(cpu.step(&mut bus), Ok(13));
        assert_eq!(cpu.registers.b, 1);
        assert_eq!(cpu.registers.pc, 0);
        assert_eq!(cpu.step(&mut bus), Ok(8));
        assert_eq!(cpu.registers.b, 0);
        assert_eq!(cpu.registers.pc, 2);
        assert_eq!(cpu.step(&mut bus), Ok(13));
        assert_eq!(cpu.registers.b, 0xFF);
        assert_eq!(cpu.registers.pc, 4);
    }

    #[test]
    fn rst_pushes_return_address_and_jumps_to_vector() {
        let mut bus = TestBus([0; 0x10000]);
        bus.0[0] = 0xFF;
        let mut cpu = Z80::default();
        cpu.registers.sp = 0xD000;
        assert_eq!(cpu.step(&mut bus), Ok(11));
        assert_eq!(cpu.registers.pc, 0x0038);
        assert_eq!(cpu.registers.sp, 0xCFFE);
        assert_eq!(bus.0[0xCFFE], 0x01);
        assert_eq!(bus.0[0xCFFF], 0x00);
    }

    #[test]
    fn executes_load_jump_and_halt_without_python() {
        let mut bus = TestBus([0; 0x10000]);
        bus.0[..7].copy_from_slice(&[0x3E, 0x42, 0x21, 0x00, 0xC0, 0x76, 0x00]);
        let mut cpu = Z80::default();

        assert_eq!(cpu.step(&mut bus), Ok(7));
        assert_eq!(cpu.registers.a, 0x42);
        assert_eq!(cpu.step(&mut bus), Ok(10));
        assert_eq!(cpu.registers.hl(), 0xC000);
        assert_eq!(cpu.step(&mut bus), Ok(4));
        assert!(cpu.halted);
        assert_eq!(cpu.total_tstates, 21);
        assert_eq!(cpu.r, 3);
        assert_eq!(cpu.step(&mut bus), Ok(4));
        assert_eq!(cpu.r, 4);
        assert_eq!(cpu.total_tstates, 25);
    }

    #[test]
    fn scf_uses_q_latch_for_undocumented_flag_bits() {
        let mut bus = TestBus([0; 0x10000]);
        bus.0[..2].copy_from_slice(&[0x00, 0x37]); // NOP; SCF
        let mut cpu = Z80::default();
        cpu.registers.a = 0;
        cpu.registers.f = FLAG_XY & 0x20;

        assert_eq!(cpu.step(&mut bus), Ok(4));
        assert_eq!(cpu.q, 0); // NOP does not write flags.
        assert_eq!(cpu.step(&mut bus), Ok(4));
        assert_eq!(cpu.registers.f, 0x21); // old F contributes bit 5 through Q.
        assert_eq!(cpu.q, 0x21);
    }

    #[test]
    fn ld_a_i_interrupt_edge_clears_pv_before_maskable_interrupt_service() {
        let mut bus = TestBus([0; 0x10000]);
        bus.0[..3].copy_from_slice(&[0xED, 0x57, 0x00]); // LD A,I; NOP
        let mut cpu = Z80 {
            interrupt_mode: 1,
            i: 0x80,
            ..Z80::default()
        };
        cpu.registers.sp = 0xC100;
        cpu.iff1 = true;
        cpu.iff2 = true;

        assert_eq!(cpu.step(&mut bus), Ok(9));
        assert_eq!(cpu.registers.a, 0x80);
        assert_ne!(cpu.registers.f & FLAG_PV, 0);
        assert!(cpu.p);

        cpu.request_int(0xFF);
        assert_eq!(cpu.step(&mut bus), Ok(13));
        assert_eq!(cpu.registers.pc, 0x0038);
        assert_eq!(cpu.registers.f & FLAG_PV, 0);
        assert!(!cpu.p);
    }

    #[test]
    fn nmi_pushes_pc_vectors_and_copies_iff1_to_iff2() {
        let mut bus = TestBus([0; 0x10000]);
        let mut cpu = Z80::default();
        cpu.registers.pc = 0x1234;
        cpu.registers.sp = 0xC100;
        cpu.iff1 = true;
        cpu.iff2 = false;
        cpu.request_nmi();

        assert_eq!(cpu.step(&mut bus), Ok(11));
        assert_eq!(cpu.registers.pc, 0x0066);
        assert_eq!(cpu.registers.sp, 0xC0FE);
        assert_eq!((bus.0[0xC0FE], bus.0[0xC0FF]), (0x34, 0x12));
        assert!(!cpu.iff1);
        assert!(cpu.iff2);
        assert!(!cpu.nmi_pending);
    }

    #[test]
    fn maskable_interrupt_modes_zero_one_and_two_use_their_vectors() {
        let mut bus = TestBus([0; 0x10000]);

        let mut im0 = Z80::default();
        im0.registers.pc = 0x1234;
        im0.registers.sp = 0xC100;
        im0.iff1 = true;
        im0.iff2 = true;
        im0.request_int(0xCF); // external RST 08h
        assert_eq!(im0.step(&mut bus), Ok(13));
        assert_eq!(im0.registers.pc, 0x0008);
        assert_eq!((bus.0[0xC0FE], bus.0[0xC0FF]), (0x34, 0x12));

        let mut im1 = Z80 {
            interrupt_mode: 1,
            ..Z80::default()
        };
        im1.registers.pc = 0x5678;
        im1.registers.sp = 0xC200;
        im1.iff1 = true;
        im1.iff2 = true;
        im1.request_int(0xFF);
        assert_eq!(im1.step(&mut bus), Ok(13));
        assert_eq!(im1.registers.pc, 0x0038);
        assert_eq!((bus.0[0xC1FE], bus.0[0xC1FF]), (0x78, 0x56));

        let mut im2 = Z80 {
            interrupt_mode: 2,
            i: 0x20,
            ..Z80::default()
        };
        im2.registers.pc = 0x9ABC;
        im2.registers.sp = 0xC300;
        im2.iff1 = true;
        im2.iff2 = true;
        im2.request_int(0xFF);
        bus.0[0x20FF] = 0x78;
        bus.0[0x2100] = 0x56;
        assert_eq!(im2.step(&mut bus), Ok(19));
        assert_eq!(im2.registers.pc, 0x5678);
        assert_eq!((bus.0[0xC2FE], bus.0[0xC2FF]), (0xBC, 0x9A));
    }

    #[test]
    fn im0_executes_an_external_call_opcode_and_fetches_operands_from_pc() {
        let mut bus = TestBus([0; 0x10000]);
        bus.0[0x1234] = 0x78;
        bus.0[0x1235] = 0x56;
        let mut cpu = Z80::default();
        cpu.registers.pc = 0x1234;
        cpu.registers.sp = 0xC100;
        cpu.iff1 = true;
        cpu.iff2 = true;
        cpu.request_int(0xCD); // CALL nn supplied by the interrupting device

        assert_eq!(cpu.step(&mut bus), Ok(19));
        assert_eq!(cpu.registers.pc, 0x5678);
        assert_eq!(cpu.registers.sp, 0xC0FE);
        assert_eq!((bus.0[0xC0FE], bus.0[0xC0FF]), (0x36, 0x12));
        assert_eq!(cpu.total_tstates, 19);
    }

    #[test]
    fn ei_delay_defers_maskable_interrupt_until_one_following_instruction() {
        let mut bus = TestBus([0; 0x10000]);
        bus.0[..3].copy_from_slice(&[0xFB, 0x00, 0x76]);
        let mut cpu = Z80::default();
        cpu.request_int(0xFF);

        assert_eq!(cpu.step(&mut bus), Ok(4)); // EI
        assert_eq!(cpu.registers.pc, 1);
        assert_eq!(cpu.step(&mut bus), Ok(4)); // one instruction after EI
        assert_eq!(cpu.registers.pc, 2);
        assert_eq!(cpu.step(&mut bus), Ok(13)); // INT accepted before HALT
        assert_eq!(cpu.registers.pc, 0x0038);
        assert!(!cpu.iff1);
        assert!(cpu.int_pending.is_none());
    }

    #[test]
    fn every_primary_and_prefixed_encoding_is_classified_without_fallback_errors() {
        for opcode in 0_u16..=255 {
            let mut bus = TestBus([0; 0x10000]);
            bus.0[0] = opcode as u8;
            let mut cpu = Z80::default();
            assert!(
                cpu.step(&mut bus).is_ok(),
                "primary opcode 0x{opcode:02X} returned an error"
            );
        }

        for prefix in [0xCB, 0xED, 0xDD, 0xFD] {
            for opcode in 0_u16..=255 {
                let mut bus = TestBus([0; 0x10000]);
                bus.0[0] = prefix;
                bus.0[1] = opcode as u8;
                let mut cpu = Z80::default();
                assert!(
                    cpu.step(&mut bus).is_ok(),
                    "prefix 0x{prefix:02X} opcode 0x{opcode:02X} returned an error"
                );
            }
        }
    }

    #[test]
    fn indexed_prefix_supports_ix_iy_displacements_and_indexed_cb() {
        let mut bus = TestBus([0; 0x10000]);
        bus.0[..28].copy_from_slice(&[
            0xDD, 0x21, 0x00, 0xC0, // LD IX,0xC000
            0xDD, 0x26, 0x12, // LD IXH,0x12
            0xDD, 0x2E, 0x34, // LD IXL,0x34
            0xDD, 0x23, // INC IX -> 0x1235
            0xFD, 0x21, 0x00, 0xC1, // LD IY,0xC100
            0xFD, 0x36, 0x02, 0x5A, // LD (IY+2),0x5A
            0xFD, 0xCB, 0x02, 0x46, // BIT 0,(IY+2)
            0xDD, 0xCB, 0x02, 0xC6, // SET 0,(IX+2)
        ]);
        let mut cpu = Z80::default();

        for expected in [14, 11, 11, 10, 14, 19, 20, 23] {
            assert_eq!(cpu.step(&mut bus), Ok(expected));
        }
        assert_eq!(cpu.ix, 0x1235);
        assert_eq!(cpu.iy, 0xC100);
        assert_eq!(bus.0[0xC102], 0x5A);
        assert_eq!(
            cpu.registers.f & (FLAG_H | FLAG_Z | FLAG_PV),
            FLAG_H | FLAG_Z | FLAG_PV
        );
        assert_eq!(bus.0[0x1237], 1);
        assert_eq!(cpu.r, 16);
    }

    #[test]
    fn repeated_index_prefixes_are_counted_without_recursive_overflow() {
        let mut bus = TestBus([0; 0x10000]);
        bus.0[..3].copy_from_slice(&[0xDD, 0xFD, 0x00]);
        let mut cpu = Z80::default();
        assert_eq!(cpu.step(&mut bus), Ok(12));
        assert_eq!(cpu.registers.pc, 3);
        assert_eq!(cpu.r, 3);
        assert_eq!(cpu.total_tstates, 12);
    }

    #[test]
    fn ignored_index_prefix_executes_base_instruction_with_prefix_cycles() {
        let mut bus = TestBus([0; 0x10000]);
        bus.0[..2].copy_from_slice(&[0xDD, 0x00]);
        let mut cpu = Z80::default();
        assert_eq!(cpu.step(&mut bus), Ok(8));
        assert_eq!(cpu.registers.pc, 2);
        assert_eq!(cpu.r, 2);
    }

    #[test]
    fn arithmetic_and_logic_instructions_update_accumulator_and_flags() {
        let mut bus = TestBus([0; 0x10000]);
        bus.0[..8].copy_from_slice(&[
            0x3E, 0x7F, // LD A,0x7F
            0xC6, 0x01, // ADD A,1 -> 0x80, signed overflow
            0xCE, 0x00, // ADC A,0 (carry is clear) -> 0x80
            0xFE, 0x80, // CP 0x80 -> Z set, A preserved
        ]);
        let mut cpu = Z80::default();

        assert_eq!(cpu.step(&mut bus), Ok(7));
        assert_eq!(cpu.step(&mut bus), Ok(7));
        assert_eq!(cpu.registers.a, 0x80);
        assert_ne!(cpu.registers.f & FLAG_PV, 0);
        assert_eq!(cpu.step(&mut bus), Ok(7));
        assert_eq!(cpu.registers.a, 0x80);
        assert_eq!(cpu.step(&mut bus), Ok(7));
        assert_eq!(cpu.registers.a, 0x80);
        assert_ne!(cpu.registers.f & FLAG_Z, 0);
    }

    #[test]
    fn register_alu_operations_support_hl_memory_operand() {
        let mut bus = TestBus([0; 0x10000]);
        bus.0[..7].copy_from_slice(&[
            0x21, 0x00, 0xC0, // LD HL,0xC000
            0x36, 0x0F, // LD (HL),0x0F
            0x3E, 0x01, // LD A,1
        ]);
        bus.0[7] = 0x86; // ADD A,(HL)
        let mut cpu = Z80::default();

        assert_eq!(cpu.step(&mut bus), Ok(10));
        assert_eq!(cpu.step(&mut bus), Ok(10));
        assert_eq!(cpu.step(&mut bus), Ok(7));
        assert_eq!(cpu.step(&mut bus), Ok(7));
        assert_eq!(cpu.registers.a, 0x10);
        assert_ne!(cpu.registers.f & FLAG_H, 0);
    }

    #[test]
    fn conditional_jumps_and_stack_calls_preserve_return_address() {
        let mut bus = TestBus([0; 0x10000]);
        // LD SP,0xD000; LD A,1; CP 1; JR NZ,+2 (not taken);
        // CALL 0x0010; HALT; RET at 0x0010.
        bus.0[..11].copy_from_slice(&[
            0x31, 0x00, 0xD0, 0x3E, 0x01, 0xFE, 0x01, 0x20, 0x02, 0xCD, 0x10,
        ]);
        bus.0[11] = 0x00; // high byte of CALL target is fetched here
        bus.0[12] = 0x76; // HALT return target
        bus.0[0x10] = 0xC9; // RET
        let mut cpu = Z80::default();

        assert_eq!(cpu.step(&mut bus), Ok(10)); // LD SP
        assert_eq!(cpu.step(&mut bus), Ok(7)); // LD A
        assert_eq!(cpu.step(&mut bus), Ok(7)); // CP
        assert_eq!(cpu.step(&mut bus), Ok(7)); // JR NZ not taken
        assert_eq!(cpu.registers.pc, 9);
        assert_eq!(cpu.step(&mut bus), Ok(17)); // CALL
        assert_eq!(cpu.registers.pc, 0x0010);
        assert_eq!(cpu.step(&mut bus), Ok(10)); // RET
        assert_eq!(cpu.registers.pc, 12);
        assert_eq!(cpu.registers.sp, 0xD000);
        assert_eq!(cpu.step(&mut bus), Ok(4)); // HALT
        assert!(cpu.halted);
    }

    #[test]
    fn push_and_pop_register_pairs_round_trip() {
        let mut bus = TestBus([0; 0x10000]);
        bus.0[..8].copy_from_slice(&[
            0x31, 0x00, 0xD0, // LD SP,0xD000
            0x01, 0x34, 0x12, // LD BC,0x1234
            0xC5, 0xC1, // PUSH BC; POP BC
        ]);
        let mut cpu = Z80::default();
        for expected in [10, 10, 11, 10] {
            assert_eq!(cpu.step(&mut bus), Ok(expected));
        }
        assert_eq!(cpu.registers.bc(), 0x1234);
        assert_eq!(cpu.registers.sp, 0xD000);
    }

    #[test]
    fn eight_bit_increment_and_decrement_preserve_carry_and_set_edge_flags() {
        let mut bus = TestBus([0; 0x10000]);
        bus.0[..8].copy_from_slice(&[
            0x3E, 0x7F, // LD A,0x7F
            0x3C, // INC A -> 0x80, overflow and half-carry
            0x3D, // DEC A -> 0x7F, overflow and half-carry
            0x21, 0x00, 0xC0, // LD HL,0xC000
            0x34, // INC (HL)
        ]);
        let mut cpu = Z80::default();
        cpu.registers.f = FLAG_C;

        assert_eq!(cpu.step(&mut bus), Ok(7));
        assert_eq!(cpu.registers.a, 0x7F);
        assert_eq!(cpu.step(&mut bus), Ok(4));
        assert_eq!(cpu.registers.a, 0x80);
        assert_ne!(cpu.registers.f & FLAG_PV, 0);
        assert_ne!(cpu.registers.f & FLAG_H, 0);
        assert_ne!(cpu.registers.f & FLAG_C, 0);
        assert_eq!(cpu.step(&mut bus), Ok(4));
        assert_eq!(cpu.registers.a, 0x7F);
        assert_ne!(cpu.registers.f & FLAG_PV, 0);
        assert_ne!(cpu.registers.f & FLAG_H, 0);
        assert_ne!(cpu.registers.f & FLAG_C, 0);
        assert_eq!(cpu.step(&mut bus), Ok(10));
        assert_eq!(cpu.step(&mut bus), Ok(11));
        assert_eq!(bus.0[0xC000], 1);
    }

    #[test]
    fn absolute_and_indirect_loads_transfer_bytes_in_little_endian_order() {
        let mut bus = TestBus([0; 0x10000]);
        bus.0[..19].copy_from_slice(&[
            0x01, 0x00, 0xC0, // LD BC,0xC000
            0x3E, 0x5A, // LD A,0x5A
            0x02, // LD (BC),A
            0x0A, // LD A,(BC)
            0x21, 0x34, 0x12, // LD HL,0x1234
            0x22, 0x00, 0xC1, // LD (0xC100),HL
            0x2A, 0x00, 0xC1, // LD HL,(0xC100)
            0x32, 0x02, 0xC1, // LD (0xC102),A
        ]);
        let mut cpu = Z80::default();

        for expected in [10, 7, 7, 7, 10, 16, 16, 13] {
            assert_eq!(cpu.step(&mut bus), Ok(expected));
        }
        assert_eq!(bus.0[0xC000], 0x5A);
        assert_eq!(cpu.registers.a, 0x5A);
        assert_eq!(bus.0[0xC100], 0x34);
        assert_eq!(bus.0[0xC101], 0x12);
        assert_eq!(cpu.registers.hl(), 0x1234);
        assert_eq!(bus.0[0xC102], 0x5A);
    }

    #[test]
    fn sixteen_bit_increment_decrement_and_add_hl_update_registers_and_flags() {
        let mut bus = TestBus([0; 0x10000]);
        bus.0[..14].copy_from_slice(&[
            0x21, 0xFF, 0x0F, // LD HL,0x0FFF
            0x11, 0x01, 0x00, // LD DE,1
            0x19, // ADD HL,DE -> 0x1000, H set
            0x23, // INC HL -> 0x1001
            0x2B, // DEC HL -> 0x1000
            0x01, 0xFF, 0xFF, // LD BC,0xFFFF
            0x03, // INC BC -> 0
            0x0B, // DEC BC -> 0xFFFF
        ]);
        let mut cpu = Z80::default();

        for expected in [10, 10, 11, 6, 6, 10, 6, 6] {
            assert_eq!(cpu.step(&mut bus), Ok(expected));
        }
        assert_eq!(cpu.registers.hl(), 0x1000);
        assert_eq!(cpu.registers.bc(), 0xFFFF);
        assert_ne!(cpu.registers.f & FLAG_H, 0);
        assert_eq!(cpu.registers.f & FLAG_C, 0);
    }

    #[test]
    fn exchange_de_hl_load_sp_hl_and_jump_hl_execute_with_base_cycles() {
        let mut bus = TestBus([0; 0x10000]);
        bus.0[..5].copy_from_slice(&[
            0xEB, // EX DE,HL
            0xF9, // LD SP,HL
            0xE9, // JP (HL)
            0x00, // target: NOP
            0x76, // HALT
        ]);
        let mut cpu = Z80::default();
        cpu.registers.set_de(0x1234);
        cpu.registers.set_hl(0x4567);
        cpu.registers.f = FLAG_C | FLAG_Z;

        assert_eq!(cpu.step(&mut bus), Ok(4));
        assert_eq!(cpu.registers.de(), 0x4567);
        assert_eq!(cpu.registers.hl(), 0x1234);
        assert_eq!(cpu.registers.f, FLAG_C | FLAG_Z);

        assert_eq!(cpu.step(&mut bus), Ok(6));
        assert_eq!(cpu.registers.sp, 0x1234);

        assert_eq!(cpu.step(&mut bus), Ok(4));
        assert_eq!(cpu.registers.pc, 0x1234);
        // Jump to a mapped NOP at the new target to prove control flow.
        bus.0[0x1234] = 0x00;
        assert_eq!(cpu.step(&mut bus), Ok(4));
        assert_eq!(cpu.registers.pc, 0x1235);
    }

    #[test]
    fn exchange_alternate_register_banks_and_af_preserves_values() {
        let mut bus = TestBus([0; 0x10000]);
        bus.0[..2].copy_from_slice(&[0x08, 0xD9]);
        let mut cpu = Z80::default();
        cpu.registers.a = 0x12;
        cpu.registers.f = 0xA5;
        cpu.alternate.a = 0x34;
        cpu.alternate.f = 0x56;
        cpu.registers.set_bc(0x1234);
        cpu.alternate.b = 0xAB;
        cpu.alternate.c = 0xCD;

        assert_eq!(cpu.step(&mut bus), Ok(4)); // EX AF,AF'
        assert_eq!((cpu.registers.a, cpu.registers.f), (0x34, 0x56));
        assert_eq!((cpu.alternate.a, cpu.alternate.f), (0x12, 0xA5));
        assert_eq!(cpu.step(&mut bus), Ok(4)); // EXX
        assert_eq!(cpu.registers.bc(), 0xABCD);
        assert_eq!((cpu.alternate.b, cpu.alternate.c), (0x12, 0x34));
    }

    #[test]
    fn exchange_stack_hl_and_di_ei_update_state() {
        let mut bus = TestBus([0; 0x10000]);
        bus.0[..3].copy_from_slice(&[0xE3, 0xF3, 0xFB]);
        let mut cpu = Z80::default();
        cpu.registers.sp = 0xC000;
        cpu.registers.set_hl(0x1234);
        bus.0[0xC000] = 0xCD;
        bus.0[0xC001] = 0xAB;

        assert_eq!(cpu.step(&mut bus), Ok(19));
        assert_eq!(cpu.registers.hl(), 0xABCD);
        assert_eq!((bus.0[0xC000], bus.0[0xC001]), (0x34, 0x12));
        assert_eq!(cpu.step(&mut bus), Ok(4));
        assert!(!cpu.iff1 && !cpu.iff2);
        assert_eq!(cpu.step(&mut bus), Ok(4));
        assert!(cpu.iff1 && cpu.iff2);
        assert_eq!(cpu.ei_delay, 1);
    }

    #[test]
    fn ed_prefix_loads_special_registers_arithmetic_and_block_transfer() {
        let mut bus = TestBus([0; 0x10000]);
        bus.0[..16].copy_from_slice(&[
            0xED, 0x47, // LD I,A
            0xED, 0x57, // LD A,I
            0xED, 0x44, // NEG
            0xED, 0x43, 0x00, 0xC0, // LD (0xC000),BC
            0xED, 0x4B, 0x00, 0xC0, // LD BC,(0xC000)
            0xED, 0xA0, // LDI
        ]);
        let mut cpu = Z80::default();
        cpu.registers.a = 0x80;
        cpu.iff2 = true;
        cpu.registers.set_bc(0x1234);

        assert_eq!(cpu.step(&mut bus), Ok(9));
        assert_eq!(cpu.i, 0x80);
        assert_eq!(cpu.step(&mut bus), Ok(9));
        assert_eq!(cpu.registers.a, 0x80);
        assert_ne!(cpu.registers.f & FLAG_PV, 0);
        assert_eq!(cpu.step(&mut bus), Ok(8));
        assert_eq!(cpu.registers.a, 0x80);
        assert_ne!(cpu.registers.f & FLAG_N, 0);

        assert_eq!(cpu.step(&mut bus), Ok(20));
        assert_eq!((bus.0[0xC000], bus.0[0xC001]), (0x34, 0x12));
        assert_eq!(cpu.step(&mut bus), Ok(20));
        assert_eq!(cpu.registers.bc(), 0x1234);

        cpu.registers.set_hl(0xC100);
        cpu.registers.set_de(0xC200);
        cpu.registers.set_bc(1);
        bus.0[0xC100] = 0x5A;
        assert_eq!(cpu.step(&mut bus), Ok(16));
        assert_eq!(bus.0[0xC200], 0x5A);
        assert_eq!(cpu.registers.bc(), 0);
        assert_eq!(cpu.registers.hl(), 0xC101);
        assert_eq!(cpu.registers.de(), 0xC201);
    }

    #[test]
    fn refresh_register_increments_for_m1_fetches_and_preserves_bit_seven() {
        let mut bus = TestBus([0; 0x10000]);
        bus.0[..4].copy_from_slice(&[0xCB, 0x00, 0x00, 0x00]);
        let mut cpu = Z80 {
            r: 0xFE,
            ..Z80::default()
        };

        assert_eq!(cpu.step(&mut bus), Ok(8));
        assert_eq!(cpu.r, 0x80);
        assert_eq!(cpu.step(&mut bus), Ok(4));
        assert_eq!(cpu.r, 0x81);
        cpu.reset();
        assert_eq!(cpu.r, 0);
    }

    #[test]
    fn cb_prefix_executes_rotate_bit_res_set_and_memory_forms() {
        let mut bus = TestBus([0; 0x10000]);
        bus.0[..7].copy_from_slice(&[
            0x06, 0x81, // LD B,0x81
            0xCB, 0x00, // RLC B -> 0x03, carry
            0xCB, 0x40, // BIT 0,B -> set
            0x21, // next opcode is LD HL,nn
        ]);
        let mut cpu = Z80::default();
        assert_eq!(cpu.step(&mut bus), Ok(7));
        assert_eq!(cpu.step(&mut bus), Ok(8));
        assert_eq!(cpu.registers.b, 0x03);
        assert_ne!(cpu.registers.f & FLAG_C, 0);
        assert_eq!(cpu.step(&mut bus), Ok(8));
        assert_eq!(cpu.registers.f & (FLAG_H | FLAG_Z), FLAG_H);
        assert_eq!(cpu.step(&mut bus), Ok(10));
        assert_eq!(cpu.registers.hl(), 0x0000);

        let mut memory_bus = TestBus([0; 0x10000]);
        memory_bus.0[..5].copy_from_slice(&[0x21, 0x00, 0xC0, 0xCB, 0xC6]); // SET 0,(HL)
        let mut memory_cpu = Z80::default();
        assert_eq!(memory_cpu.step(&mut memory_bus), Ok(10));
        assert_eq!(memory_cpu.step(&mut memory_bus), Ok(15));
        assert_eq!(memory_bus.0[0xC000], 1);
    }

    #[test]
    fn relative_jump_uses_signed_displacement() {
        let mut bus = TestBus([0; 0x10000]);
        bus.0[0] = 0x18;
        bus.0[1] = 0xFC;
        let mut cpu = Z80::default();
        assert_eq!(cpu.step(&mut bus), Ok(12));
        assert_eq!(cpu.registers.pc, 0xFFFE);
    }
}
