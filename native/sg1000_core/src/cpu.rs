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

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Z80 {
    pub registers: Z80Registers,
    pub halted: bool,
    pub total_tstates: u64,
}

impl Z80 {
    pub fn reset(&mut self) {
        self.registers = Z80Registers::default();
        self.halted = false;
        self.total_tstates = 0;
    }

    /// Execute one instruction and return its documented base T-state count.
    ///
    /// This first extraction slice covers common base instructions only.
    /// Prefixes, interrupts, and all unimplemented opcodes return an explicit
    /// error until their native implementation is migrated.
    pub fn step<B: CpuBus>(&mut self, bus: &mut B) -> Result<u32, CpuError> {
        if self.halted {
            self.total_tstates += 4;
            return Ok(4);
        }

        let address = self.registers.pc;
        let opcode = self.fetch8(bus);
        let cycles = match opcode {
            0x00 => 4, // NOP
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
                self.registers.f = (self.registers.f & (FLAG_S | FLAG_Z | FLAG_PV))
                    | (result & FLAG_XY)
                    | if carry { FLAG_C } else { 0 };
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
                self.registers.f = flags;
                4
            }
            0x2F => {
                self.registers.a = !self.registers.a;
                self.registers.f = (self.registers.f & (FLAG_S | FLAG_Z | FLAG_PV | FLAG_C))
                    | (self.registers.a & FLAG_XY)
                    | FLAG_H
                    | FLAG_N;
                4
            }
            0x37 => {
                self.registers.f = (self.registers.f & (FLAG_S | FLAG_Z | FLAG_PV))
                    | (self.registers.a & FLAG_XY)
                    | FLAG_C;
                4
            }
            0x3F => {
                let old_carry = self.registers.f & FLAG_C != 0;
                self.registers.f = (self.registers.f & (FLAG_S | FLAG_Z | FLAG_PV))
                    | (self.registers.a & FLAG_XY)
                    | if old_carry { FLAG_H } else { FLAG_C };
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
                self.registers.f = flags;
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
                    _ => [self.registers.a, self.registers.f] = value.to_be_bytes(),
                }
                10
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
                self.registers.f = flags;
                if register == 6 { 11 } else { 4 }
            }
            0xAF => {
                self.registers.a = 0;
                self.registers.f = FLAG_Z | FLAG_PV;
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
        Ok(cycles)
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
        self.registers.f = result.flags;
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
    use super::{CpuError, FLAG_C, FLAG_H, FLAG_N, FLAG_PV, FLAG_Z, Z80};
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
    }

    #[test]
    fn unsupported_opcodes_fail_explicitly_without_fallback() {
        let mut bus = TestBus([0xDD; 0x10000]);
        let mut cpu = Z80::default();
        assert_eq!(
            cpu.step(&mut bus),
            Err(CpuError::UnsupportedOpcode {
                opcode: 0xDD,
                address: 0
            })
        );
        assert_eq!(cpu.registers.pc, 1);
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
    fn relative_jump_uses_signed_displacement() {
        let mut bus = TestBus([0; 0x10000]);
        bus.0[0] = 0x18;
        bus.0[1] = 0xFC;
        let mut cpu = Z80::default();
        assert_eq!(cpu.step(&mut bus), Ok(12));
        assert_eq!(cpu.registers.pc, 0xFFFE);
    }
}
