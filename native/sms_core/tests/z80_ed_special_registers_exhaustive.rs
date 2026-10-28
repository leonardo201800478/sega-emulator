//! Exhaustive ED special-register transfer and flag regressions.
//!
//! Covers LD I,A, LD R,A, LD A,I and LD A,R across byte values, carry states,
//! IFF2 states and varied initial flags. R's two opcode-fetch increments are
//! modeled separately from the LD A,R data transfer.
use sega_sms_core::{CpuBus, Z80};

const FLAG_C: u8 = 0x01;
const FLAG_PV: u8 = 0x04;
const FLAG_Z: u8 = 0x40;
const FLAG_S: u8 = 0x80;
const FLAG_XY: u8 = 0x28;
const FLAG_PATTERNS: [u8; 4] = [0x00, 0xFF, 0xA5, 0x5A];

struct FlatBus {
    memory: Box<[u8]>,
}

impl Default for FlatBus {
    fn default() -> Self {
        Self {
            memory: vec![0; 0x1_0000].into_boxed_slice(),
        }
    }
}

impl CpuBus for FlatBus {
    fn read_memory(&mut self, address: u16) -> u8 {
        self.memory[usize::from(address)]
    }

    fn write_memory(&mut self, address: u16, value: u8) {
        self.memory[usize::from(address)] = value;
    }
}

fn increment_r(value: u8) -> u8 {
    (value & 0x80) | (value.wrapping_add(1) & 0x7F)
}

fn flags_for_load_a(value: u8, carry: bool, iff2: bool) -> u8 {
    let mut flags = value & (FLAG_S | FLAG_XY);
    if value == 0 {
        flags |= FLAG_Z;
    }
    if iff2 {
        flags |= FLAG_PV;
    }
    if carry {
        flags |= FLAG_C;
    }
    flags
}

#[test]
fn ld_i_a_and_ld_r_a_cover_every_accumulator_value_and_preserve_flags() {
    let mut bus = FlatBus::default();
    let mut cpu = Z80::default();

    for (opcode, load_r) in [(0x47_u8, false), (0x4F, true)] {
        bus.memory[0] = 0xED;
        bus.memory[1] = opcode;

        for accumulator in u8::MIN..=u8::MAX {
            for flags in FLAG_PATTERNS {
                cpu.registers.pc = 0;
                cpu.registers.a = accumulator;
                cpu.registers.f = flags;
                cpu.i = 0x34;
                cpu.r = 0x7E;
                cpu.iff2 = false;
                cpu.wz = 0xABCD;

                assert_eq!(cpu.step(&mut bus), Ok(9), "ED {opcode:02X}");
                if load_r {
                    assert_eq!(cpu.r, accumulator, "ED 4F R=A={accumulator:02X}");
                    assert_eq!(cpu.i, 0x34, "ED 4F must preserve I");
                } else {
                    assert_eq!(cpu.i, accumulator, "ED 47 I=A={accumulator:02X}");
                    assert_eq!(cpu.r, increment_r(increment_r(0x7E)), "ED 47 R");
                }
                assert_eq!(cpu.registers.f, flags, "ED {opcode:02X} must preserve F");
                assert_eq!(cpu.registers.pc, 2, "ED {opcode:02X} PC");
                assert_eq!(cpu.wz, 0xABCD, "ED {opcode:02X} WZ");
            }
        }
    }
}

#[test]
fn ld_a_i_covers_every_i_value_carry_iff2_and_initial_flag_pattern() {
    let mut bus = FlatBus::default();
    bus.memory[0..2].copy_from_slice(&[0xED, 0x57]);
    let mut cpu = Z80::default();

    for value in u8::MIN..=u8::MAX {
        for carry in [false, true] {
            for iff2 in [false, true] {
                for initial_flags in FLAG_PATTERNS {
                    cpu.registers.pc = 0;
                    cpu.registers.a = 0xC3;
                    cpu.registers.f = (initial_flags & !FLAG_C) | if carry { FLAG_C } else { 0 };
                    cpu.i = value;
                    cpu.r = 0x7E;
                    cpu.iff2 = iff2;
                    cpu.wz = 0xABCD;

                    assert_eq!(cpu.step(&mut bus), Ok(9), "ED 57");
                    assert_eq!(cpu.registers.a, value, "LD A,I value={value:02X}");
                    assert_eq!(
                        cpu.registers.f,
                        flags_for_load_a(value, carry, iff2),
                        "LD A,I value={value:02X}, carry={carry}, IFF2={iff2}, F={initial_flags:02X}"
                    );
                    assert_eq!(cpu.r, increment_r(increment_r(0x7E)), "LD A,I R");
                    assert_eq!(cpu.registers.pc, 2, "LD A,I PC");
                    assert_eq!(cpu.wz, 0xABCD, "LD A,I WZ");
                }
            }
        }
    }
}

#[test]
fn ld_a_r_covers_every_initial_r_carry_iff2_and_initial_flag_pattern() {
    let mut bus = FlatBus::default();
    bus.memory[0..2].copy_from_slice(&[0xED, 0x5F]);
    let mut cpu = Z80::default();

    for initial_r in u8::MIN..=u8::MAX {
        for carry in [false, true] {
            for iff2 in [false, true] {
                for initial_flags in FLAG_PATTERNS {
                    let expected_r = increment_r(increment_r(initial_r));
                    cpu.registers.pc = 0;
                    cpu.registers.a = 0xC3;
                    cpu.registers.f = (initial_flags & !FLAG_C) | if carry { FLAG_C } else { 0 };
                    cpu.i = 0x34;
                    cpu.r = initial_r;
                    cpu.iff2 = iff2;
                    cpu.wz = 0xABCD;

                    assert_eq!(cpu.step(&mut bus), Ok(9), "ED 5F");
                    assert_eq!(cpu.registers.a, expected_r, "LD A,R initial R={initial_r:02X}");
                    assert_eq!(
                        cpu.registers.f,
                        flags_for_load_a(expected_r, carry, iff2),
                        "LD A,R initial R={initial_r:02X}, carry={carry}, IFF2={iff2}, F={initial_flags:02X}"
                    );
                    assert_eq!(cpu.r, expected_r, "LD A,R final R");
                    assert_eq!(cpu.registers.pc, 2, "LD A,R PC");
                    assert_eq!(cpu.wz, 0xABCD, "LD A,R WZ");
                }
            }
        }
    }
}
