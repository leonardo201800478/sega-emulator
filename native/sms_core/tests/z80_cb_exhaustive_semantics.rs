//! Exhaustive semantic regressions for register and (HL) CB instruction targets.
//!
//! The expected values below are computed from the documented Z80 operation
//! definitions, not from the production ALU helpers. They cover every byte
//! value and carry input for rotate/shift operations, every BIT position and
//! operand, and exhaustive RES/SET results with flag-preservation sentinels.
//! This does not validate undocumented silicon differences or bus timing.
use sega_sms_core::{CpuBus, Z80};

const FLAG_C: u8 = 0x01;
const FLAG_PV: u8 = 0x04;
const FLAG_H: u8 = 0x10;
const FLAG_Z: u8 = 0x40;
const FLAG_S: u8 = 0x80;
const FLAG_XY: u8 = 0x28;

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

fn set_register_target(cpu: &mut Z80, target: u8, value: u8) {
    match target & 7 {
        0 => cpu.registers.b = value,
        1 => cpu.registers.c = value,
        2 => cpu.registers.d = value,
        3 => cpu.registers.e = value,
        4 => cpu.registers.h = value,
        5 => cpu.registers.l = value,
        6 => unreachable!("memory target uses run_cb_memory"),
        _ => cpu.registers.a = value,
    }
}

fn get_register_target(cpu: &Z80, target: u8) -> u8 {
    match target & 7 {
        0 => cpu.registers.b,
        1 => cpu.registers.c,
        2 => cpu.registers.d,
        3 => cpu.registers.e,
        4 => cpu.registers.h,
        5 => cpu.registers.l,
        6 => unreachable!("memory target uses run_cb_memory"),
        _ => cpu.registers.a,
    }
}

fn run_cb(opcode: u8, value: u8, flags: u8) -> (Z80, FlatBus, u32) {
    let mut bus = FlatBus::default();
    bus.memory[0] = 0xCB;
    bus.memory[1] = opcode;

    let mut cpu = Z80::default();
    set_register_target(&mut cpu, opcode & 7, value);
    cpu.registers.f = flags;
    let cycles = cpu
        .step(&mut bus)
        .unwrap_or_else(|error| panic!("CB {opcode:02X} failed: {error:?}"));
    (cpu, bus, cycles)
}

fn run_cb_memory(opcode: u8, value: u8, flags: u8) -> (Z80, FlatBus, u32) {
    let mut bus = FlatBus::default();
    bus.memory[0] = 0xCB;
    bus.memory[1] = opcode;
    bus.memory[0x8000] = value;

    let mut cpu = Z80::default();
    cpu.registers.set_hl(0x8000);
    cpu.registers.f = flags;
    // BIT (HL) takes undocumented X/Y flags from the existing WZ high byte
    // on the target NMOS-compatible behavior modeled by this core.
    cpu.wz = 0xA800;
    let cycles = cpu
        .step(&mut bus)
        .unwrap_or_else(|error| panic!("CB {opcode:02X} failed: {error:?}"));
    (cpu, bus, cycles)
}

fn even_parity(value: u8) -> bool {
    value.count_ones().is_multiple_of(2)
}

fn rotate_shift_reference(operation: u8, value: u8, carry_in: bool) -> (u8, bool) {
    match operation {
        0 => (value.rotate_left(1), value & 0x80 != 0), // RLC
        1 => (value.rotate_right(1), value & 0x01 != 0), // RRC
        2 => (
            value.wrapping_shl(1) | u8::from(carry_in),
            value & 0x80 != 0,
        ), // RL
        3 => ((value >> 1) | (u8::from(carry_in) << 7), value & 0x01 != 0), // RR
        4 => (value.wrapping_shl(1), value & 0x80 != 0), // SLA
        5 => ((value >> 1) | (value & 0x80), value & 0x01 != 0), // SRA
        6 => (value.wrapping_shl(1) | 1, value & 0x80 != 0), // SLL, undocumented
        _ => (value >> 1, value & 0x01 != 0),           // SRL
    }
}

#[test]
fn cb_rotate_and_shift_all_register_targets_are_exhaustive_for_all_values_and_carry_inputs() {
    for operation in 0_u8..8 {
        for target in [0_u8, 1, 2, 3, 4, 5, 7] {
        for value in u8::MIN..=u8::MAX {
            for carry_in in [false, true] {
                let initial_flags = if carry_in { FLAG_C } else { 0 };
                let opcode = (operation << 3) | target;
                let (cpu, _, cycles) = run_cb(opcode, value, initial_flags);
                let (expected_value, carry_out) =
                    rotate_shift_reference(operation, value, carry_in);

                let mut expected_flags = expected_value & (FLAG_S | FLAG_XY);
                if expected_value == 0 {
                    expected_flags |= FLAG_Z;
                }
                if even_parity(expected_value) {
                    expected_flags |= FLAG_PV;
                }
                if carry_out {
                    expected_flags |= FLAG_C;
                }

                assert_eq!(
                    get_register_target(&cpu, target), expected_value,
                    "CB {opcode:02X}, target={target}, value={value:02X}, carry_in={carry_in}"
                );
                assert_eq!(
                    cpu.registers.f, expected_flags,
                    "CB {opcode:02X} flags, target={target}, value={value:02X}, carry_in={carry_in}"
                );
                assert_eq!(cycles, 8, "CB {opcode:02X} register timing");
                assert_eq!(cpu.registers.pc, 2, "CB {opcode:02X} PC");
            }
        }
        }
    }
}

#[test]
fn cb_bit_all_register_targets_are_exhaustive_for_every_bit_value_and_carry_input() {
    for bit in 0_u8..8 {
        for target in [0_u8, 1, 2, 3, 4, 5, 7] {
        for value in u8::MIN..=u8::MAX {
            for carry_in in [false, true] {
                let opcode = 0x40 | (bit << 3) | target; // BIT bit,r
                let initial_flags = if carry_in { FLAG_C } else { 0 };
                let (cpu, _, cycles) = run_cb(opcode, value, initial_flags);
                let bit_clear = value & (1_u8 << bit) == 0;

                let mut expected_flags = FLAG_H | (value & FLAG_XY);
                if carry_in {
                    expected_flags |= FLAG_C;
                }
                if bit_clear {
                    expected_flags |= FLAG_Z | FLAG_PV;
                }
                if bit == 7 && !bit_clear {
                    expected_flags |= FLAG_S;
                }

                assert_eq!(
                    get_register_target(&cpu, target), value,
                    "CB {opcode:02X} must not modify target={target}, value={value:02X}"
                );
                assert_eq!(
                    cpu.registers.f, expected_flags,
                    "CB {opcode:02X}, target={target}, value={value:02X}, carry_in={carry_in}"
                );
                assert_eq!(cycles, 8, "CB {opcode:02X} register timing");
                assert_eq!(cpu.registers.pc, 2, "CB {opcode:02X} PC");
            }
        }
        }
    }
}

#[test]
fn cb_res_and_set_all_register_targets_are_exhaustive_and_preserve_all_flags() {
    // Distinctive flag patterns include all bits set, all bits clear, and
    // alternating patterns, catching accidental flag writes in RES/SET.
    for group in [0x80_u8, 0xC0_u8] {
        for bit in 0_u8..8 {
            for target in [0_u8, 1, 2, 3, 4, 5, 7] {
                for value in u8::MIN..=u8::MAX {
                    for initial_flags in [0x00, 0xFF, 0xA5, 0x5A] {
                    let opcode = group | (bit << 3) | target; // RES/SET bit,r
                    let (cpu, _, cycles) = run_cb(opcode, value, initial_flags);
                    let mask = 1_u8 << bit;
                    let expected_value = if group == 0x80 {
                        value & !mask
                    } else {
                        value | mask
                    };

                    assert_eq!(
                        get_register_target(&cpu, target), expected_value,
                        "CB {opcode:02X}, target={target}, value={value:02X}"
                    );
                    assert_eq!(
                        cpu.registers.f, initial_flags,
                        "CB {opcode:02X} must preserve F={initial_flags:02X}"
                    );
                    assert_eq!(cycles, 8, "CB {opcode:02X} register timing");
                    assert_eq!(cpu.registers.pc, 2, "CB {opcode:02X} PC");
                    }
                }
            }
        }
    }
}

#[test]
fn cb_rotate_and_shift_hl_memory_are_exhaustive_for_all_values_and_carry_inputs() {
    for operation in 0_u8..8 {
        for value in u8::MIN..=u8::MAX {
            for carry_in in [false, true] {
                let initial_flags = if carry_in { FLAG_C } else { 0 };
                let opcode = (operation << 3) | 6; // target (HL)
                let (cpu, bus, cycles) = run_cb_memory(opcode, value, initial_flags);
                let (expected_value, carry_out) =
                    rotate_shift_reference(operation, value, carry_in);

                let mut expected_flags = expected_value & (FLAG_S | FLAG_XY);
                if expected_value == 0 {
                    expected_flags |= FLAG_Z;
                }
                if even_parity(expected_value) {
                    expected_flags |= FLAG_PV;
                }
                if carry_out {
                    expected_flags |= FLAG_C;
                }

                assert_eq!(
                    bus.memory[0x8000], expected_value,
                    "CB {opcode:02X}, (HL)={value:02X}, carry_in={carry_in}"
                );
                assert_eq!(
                    cpu.registers.f, expected_flags,
                    "CB {opcode:02X} flags, (HL)={value:02X}, carry_in={carry_in}"
                );
                assert_eq!(cycles, 15, "CB {opcode:02X} memory timing");
                assert_eq!(cpu.registers.pc, 2, "CB {opcode:02X} PC");
                assert_eq!(cpu.wz, 0xA800, "CB {opcode:02X} must preserve WZ");
            }
        }
    }
}

#[test]
fn cb_bit_hl_memory_is_exhaustive_for_every_bit_value_and_carry_input() {
    for bit in 0_u8..8 {
        for value in u8::MIN..=u8::MAX {
            for carry_in in [false, true] {
                let opcode = 0x40 | (bit << 3) | 6; // BIT bit,(HL)
                let initial_flags = if carry_in { FLAG_C } else { 0 };
                let (cpu, bus, cycles) = run_cb_memory(opcode, value, initial_flags);
                let bit_clear = value & (1_u8 << bit) == 0;

                let mut expected_flags = FLAG_H | (0xA8 & FLAG_XY);
                if carry_in {
                    expected_flags |= FLAG_C;
                }
                if bit_clear {
                    expected_flags |= FLAG_Z | FLAG_PV;
                }
                if bit == 7 && !bit_clear {
                    expected_flags |= FLAG_S;
                }

                assert_eq!(
                    bus.memory[0x8000], value,
                    "CB {opcode:02X} must not modify (HL)={value:02X}"
                );
                assert_eq!(
                    cpu.registers.f, expected_flags,
                    "CB {opcode:02X}, (HL)={value:02X}, carry_in={carry_in}"
                );
                assert_eq!(cycles, 12, "CB {opcode:02X} BIT memory timing");
                assert_eq!(cpu.registers.pc, 2, "CB {opcode:02X} PC");
                assert_eq!(cpu.wz, 0xA800, "CB {opcode:02X} must preserve WZ");
            }
        }
    }
}

#[test]
fn cb_res_and_set_hl_memory_are_exhaustive_and_preserve_all_flags() {
    for group in [0x80_u8, 0xC0_u8] {
        for bit in 0_u8..8 {
            for value in u8::MIN..=u8::MAX {
                for initial_flags in [0x00, 0xFF, 0xA5, 0x5A] {
                    let opcode = group | (bit << 3) | 6; // RES/SET bit,(HL)
                    let (cpu, bus, cycles) = run_cb_memory(opcode, value, initial_flags);
                    let mask = 1_u8 << bit;
                    let expected_value = if group == 0x80 {
                        value & !mask
                    } else {
                        value | mask
                    };

                    assert_eq!(
                        bus.memory[0x8000], expected_value,
                        "CB {opcode:02X}, (HL)={value:02X}"
                    );
                    assert_eq!(
                        cpu.registers.f, initial_flags,
                        "CB {opcode:02X} must preserve F={initial_flags:02X}"
                    );
                    assert_eq!(cycles, 15, "CB {opcode:02X} memory timing");
                    assert_eq!(cpu.registers.pc, 2, "CB {opcode:02X} PC");
                    assert_eq!(cpu.wz, 0xA800, "CB {opcode:02X} must preserve WZ");
                }
            }
        }
    }
}
