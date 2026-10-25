//! First Python-independent Z80 execution slice.
//!
//! This module deliberately reports unsupported opcodes instead of silently
//! delegating execution to the Python reference. The implementation is
//! incremental and is not yet a complete Z80.

use crate::bus::CpuBus;
use sega_z80_alu::{adc8, add8, and8, cp8, or8, sbc8, sub8, xor8};

const FLAG_C: u8 = 0x01;
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
            0x3C => {
                let old = self.registers.a;
                let value = old.wrapping_add(1);
                let carry = self.registers.f & FLAG_C;
                let mut flags = value & (FLAG_S | FLAG_XY);
                if value == 0 {
                    flags |= FLAG_Z;
                }
                if old & 0x0F == 0x0F {
                    flags |= FLAG_H;
                }
                if old == 0x7F {
                    flags |= FLAG_PV;
                }
                self.registers.a = value;
                self.registers.f = flags | carry;
                4
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
            _ => return Err(CpuError::UnsupportedOpcode { opcode, address }),
        };
        self.total_tstates += u64::from(cycles);
        Ok(cycles)
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
    use super::{CpuError, Z80, FLAG_H, FLAG_PV, FLAG_Z};
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
    fn relative_jump_uses_signed_displacement() {
        let mut bus = TestBus([0; 0x10000]);
        bus.0[0] = 0x18;
        bus.0[1] = 0xFC;
        let mut cpu = Z80::default();
        assert_eq!(cpu.step(&mut bus), Ok(12));
        assert_eq!(cpu.registers.pc, 0xFFFE);
    }
}
