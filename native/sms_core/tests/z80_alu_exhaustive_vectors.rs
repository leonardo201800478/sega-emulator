//! Exhaustive 8-bit Z80 ALU and flag regression tests for the SMS-owned Rust core.
//!
//! Expected values are computed from byte arithmetic, signed-range overflow,
//! nibble carry/borrow, and Z80 flag definitions. This is an internal
//! mathematical oracle, not a hardware capture or an independent emulator.
//! DAA is checked for every accumulator value and every N/H/C input combination.

use sega_sms_core::{CpuBus, Z80};

const FLAG_C: u8 = 0x01;
const FLAG_N: u8 = 0x02;
const FLAG_PV: u8 = 0x04;
const FLAG_X: u8 = 0x08;
const FLAG_H: u8 = 0x10;
const FLAG_Y: u8 = 0x20;
const FLAG_Z: u8 = 0x40;
const FLAG_S: u8 = 0x80;
const FLAG_XY: u8 = FLAG_X | FLAG_Y;
const FLAG_SZPV: u8 = FLAG_S | FLAG_Z | FLAG_PV;

struct TestBus([u8; 0x1_0000]);

impl Default for TestBus {
    fn default() -> Self {
        Self([0; 0x1_0000])
    }
}

impl CpuBus for TestBus {
    fn read_memory(&mut self, address: u16) -> u8 {
        self.0[usize::from(address)]
    }

    fn write_memory(&mut self, address: u16, value: u8) {
        self.0[usize::from(address)] = value;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ArithmeticOp {
    Add,
    Adc,
    Sub,
    Sbc,
    Cp,
}

fn signed_byte(value: u8) -> i16 {
    if value & FLAG_S != 0 {
        i16::from(value) - 256
    } else {
        i16::from(value)
    }
}

fn expected_arithmetic(operation: ArithmeticOp, lhs: u8, rhs: u8, carry_in: bool) -> (u8, u8) {
    let has_carry_in = matches!(operation, ArithmeticOp::Adc | ArithmeticOp::Sbc);
    let carry = if has_carry_in && carry_in {
        1_i16
    } else {
        0_i16
    };

    if matches!(operation, ArithmeticOp::Add | ArithmeticOp::Adc) {
        let unsigned_sum = u16::from(lhs) + u16::from(rhs) + u16::from(carry as u8);
        let result = (unsigned_sum & 0x00FF) as u8;
        let signed_sum = signed_byte(lhs) + signed_byte(rhs) + carry;
        let mut flags = result & (FLAG_S | FLAG_XY);

        if result == 0 {
            flags |= FLAG_Z;
        }
        if u16::from(lhs & 0x0F) + u16::from(rhs & 0x0F) + u16::from(carry as u8) > 0x0F {
            flags |= FLAG_H;
        }
        if !(-128..=127).contains(&signed_sum) {
            flags |= FLAG_PV;
        }
        if unsigned_sum > u16::from(u8::MAX) {
            flags |= FLAG_C;
        }
        return (result, flags);
    }

    let unsigned_subtrahend = u16::from(rhs) + u16::from(carry as u8);
    let signed_difference = signed_byte(lhs) - signed_byte(rhs) - carry;
    let result = u8::try_from(signed_difference.rem_euclid(256))
        .expect("modulo-256 subtraction result must fit in a byte");
    let mut flags = (result & FLAG_S) | FLAG_N;

    let xy_source = if operation == ArithmeticOp::Cp {
        rhs
    } else {
        result
    };
    flags |= xy_source & FLAG_XY;
    if result == 0 {
        flags |= FLAG_Z;
    }
    if u16::from(lhs & 0x0F) < u16::from(rhs & 0x0F) + u16::from(carry as u8) {
        flags |= FLAG_H;
    }
    if !(-128..=127).contains(&signed_difference) {
        flags |= FLAG_PV;
    }
    if u16::from(lhs) < unsigned_subtrahend {
        flags |= FLAG_C;
    }

    let value = if operation == ArithmeticOp::Cp {
        lhs
    } else {
        result
    };
    (value, flags)
}

fn expected_logic(operation: u8, lhs: u8, rhs: u8) -> (u8, u8) {
    let result = match operation {
        0xA0 => lhs & rhs,
        0xA8 => lhs ^ rhs,
        0xB0 => lhs | rhs,
        _ => unreachable!("only AND, XOR and OR are included"),
    };
    let mut flags = result & (FLAG_S | FLAG_XY);

    if result == 0 {
        flags |= FLAG_Z;
    }
    if result.count_ones().is_multiple_of(2) {
        flags |= FLAG_PV;
    }
    if operation == 0xA0 {
        flags |= FLAG_H;
    }
    (result, flags)
}

#[test]
fn arithmetic_register_operations_match_reference_for_all_byte_pairs() {
    let operations = [
        (0x80, ArithmeticOp::Add),
        (0x88, ArithmeticOp::Adc),
        (0x90, ArithmeticOp::Sub),
        (0x98, ArithmeticOp::Sbc),
        (0xB8, ArithmeticOp::Cp),
    ];
    let mut bus = TestBus::default();
    let mut cpu = Z80::default();

    for (opcode, operation) in operations {
        bus.0[0] = opcode;
        for lhs in 0_u8..=u8::MAX {
            for rhs in 0_u8..=u8::MAX {
                for carry_in in [false, true] {
                    if !matches!(operation, ArithmeticOp::Adc | ArithmeticOp::Sbc) && carry_in {
                        continue;
                    }
                    let (expected_a, expected_f) =
                        expected_arithmetic(operation, lhs, rhs, carry_in);
                    cpu.registers.a = lhs;
                    cpu.registers.b = rhs;
                    cpu.registers.f = if carry_in { FLAG_C } else { 0 };
                    cpu.registers.pc = 0;

                    assert_eq!(
                        cpu.step(&mut bus),
                        Ok(4),
                        "{operation:?} opcode={opcode:02X} A={lhs:02X} B={rhs:02X} C={carry_in}"
                    );
                    assert_eq!(
                        cpu.registers.a, expected_a,
                        "{operation:?} A={lhs:02X} B={rhs:02X} C={carry_in}: accumulator"
                    );
                    assert_eq!(
                        cpu.registers.f, expected_f,
                        "{operation:?} A={lhs:02X} B={rhs:02X} C={carry_in}: flags"
                    );
                }
            }
        }
    }
}

#[test]
fn logical_register_operations_match_reference_for_all_byte_pairs() {
    let operations = [0xA0, 0xA8, 0xB0];
    let mut bus = TestBus::default();
    let mut cpu = Z80::default();

    for opcode in operations {
        bus.0[0] = opcode;
        for lhs in 0_u8..=u8::MAX {
            for rhs in 0_u8..=u8::MAX {
                let (expected_a, expected_f) = expected_logic(opcode, lhs, rhs);
                cpu.registers.a = lhs;
                cpu.registers.b = rhs;
                cpu.registers.f = 0xFF;
                cpu.registers.pc = 0;

                assert_eq!(cpu.step(&mut bus), Ok(4), "opcode={opcode:02X}");
                assert_eq!(
                    cpu.registers.a, expected_a,
                    "opcode={opcode:02X} A={lhs:02X} B={rhs:02X}: accumulator"
                );
                assert_eq!(
                    cpu.registers.f, expected_f,
                    "opcode={opcode:02X} A={lhs:02X} B={rhs:02X}: flags"
                );
            }
        }
    }
}

fn expected_inc_dec(value: u8, decrement: bool, carry_in: bool) -> (u8, u8) {
    let result = if decrement {
        value.wrapping_sub(1)
    } else {
        value.wrapping_add(1)
    };
    let mut flags = (result & (FLAG_S | FLAG_XY)) | if carry_in { FLAG_C } else { 0 };

    if result == 0 {
        flags |= FLAG_Z;
    }
    if if decrement {
        value & 0x0F == 0
    } else {
        value & 0x0F == 0x0F
    } {
        flags |= FLAG_H;
    }
    let overflow = if decrement {
        value == 0x80
    } else {
        value == 0x7F
    };
    if overflow {
        flags |= FLAG_PV;
    }
    if decrement {
        flags |= FLAG_N;
    }
    (result, flags)
}

#[test]
fn inc_dec_register_and_memory_forms_match_every_byte_and_carry_state() {
    let mut bus = TestBus::default();
    let mut cpu = Z80::default();

    for (base_opcode, decrement) in [(0x04_u8, false), (0x05_u8, true)] {
        for register in 0_u8..=7 {
            let opcode = base_opcode | (register << 3);
            bus.0[0] = opcode;
            for value in 0_u8..=u8::MAX {
                for carry_in in [false, true] {
                    let (expected_value, expected_flags) =
                        expected_inc_dec(value, decrement, carry_in);
                    match register {
                        0 => cpu.registers.b = value,
                        1 => cpu.registers.c = value,
                        2 => cpu.registers.d = value,
                        3 => cpu.registers.e = value,
                        4 => cpu.registers.h = value,
                        5 => cpu.registers.l = value,
                        6 => {
                            cpu.registers.set_hl(0xC000);
                            bus.0[0xC000] = value;
                        }
                        7 => cpu.registers.a = value,
                        _ => unreachable!("register code is three bits"),
                    }
                    cpu.registers.f = 0xFE | if carry_in { FLAG_C } else { 0 };
                    cpu.registers.pc = 0;

                    assert_eq!(
                        cpu.step(&mut bus),
                        Ok(if register == 6 { 11 } else { 4 }),
                        "opcode={opcode:02X} register={register} value={value:02X}"
                    );
                    let actual_value = match register {
                        0 => cpu.registers.b,
                        1 => cpu.registers.c,
                        2 => cpu.registers.d,
                        3 => cpu.registers.e,
                        4 => cpu.registers.h,
                        5 => cpu.registers.l,
                        6 => bus.0[0xC000],
                        7 => cpu.registers.a,
                        _ => unreachable!("register code is three bits"),
                    };
                    assert_eq!(
                        actual_value, expected_value,
                        "opcode={opcode:02X} register={register} value={value:02X}: value"
                    );
                    assert_eq!(
                        cpu.registers.f, expected_flags,
                        "opcode={opcode:02X} register={register} value={value:02X} C={carry_in}: flags"
                    );
                }
            }
        }
    }
}

fn expected_daa(accumulator: u8, old_flags: u8) -> (u8, u8) {
    let subtract = old_flags & FLAG_N != 0;
    let half_carry = old_flags & FLAG_H != 0;
    let carry_in = old_flags & FLAG_C != 0;
    let mut correction = 0_u8;
    let mut carry_out = carry_in;

    if half_carry || (accumulator & 0x0F) > 9 {
        correction |= 0x06;
    }
    if carry_in || accumulator > 0x99 {
        correction |= 0x60;
        carry_out = true;
    }

    let result = if subtract {
        accumulator.wrapping_sub(correction)
    } else {
        accumulator.wrapping_add(correction)
    };
    let mut flags = (result & (FLAG_S | FLAG_XY)) | if subtract { FLAG_N } else { 0 };

    if result == 0 {
        flags |= FLAG_Z;
    }
    if ((accumulator ^ result) & 0x10) != 0 {
        flags |= FLAG_H;
    }
    if result.count_ones().is_multiple_of(2) {
        flags |= FLAG_PV;
    }
    if carry_out {
        flags |= FLAG_C;
    }
    (result, flags)
}

#[test]
fn daa_matches_reference_for_every_accumulator_and_nhc_combination() {
    let mut bus = TestBus::default();
    bus.0[0] = 0x27; // DAA
    let mut cpu = Z80::default();

    for accumulator in 0_u8..=u8::MAX {
        for subtract in [false, true] {
            for half_carry in [false, true] {
                for carry_in in [false, true] {
                    // Seed unrelated flags to ensure DAA recomputes S/Z/PV/X/Y.
                    let old_flags = 0xEC
                        | if subtract { FLAG_N } else { 0 }
                        | if half_carry { FLAG_H } else { 0 }
                        | if carry_in { FLAG_C } else { 0 };
                    let (expected_a, expected_f) = expected_daa(accumulator, old_flags);
                    cpu.registers.a = accumulator;
                    cpu.registers.f = old_flags;
                    cpu.registers.pc = 0;

                    assert_eq!(
                        cpu.step(&mut bus),
                        Ok(4),
                        "DAA A={accumulator:02X} N={subtract} H={half_carry} C={carry_in}"
                    );
                    assert_eq!(
                        cpu.registers.a, expected_a,
                        "DAA A={accumulator:02X} N={subtract} H={half_carry} C={carry_in}: A"
                    );
                    assert_eq!(
                        cpu.registers.f, expected_f,
                        "DAA A={accumulator:02X} N={subtract} H={half_carry} C={carry_in}: F"
                    );
                }
            }
        }
    }
}

fn expected_accumulator_rotate(opcode: u8, accumulator: u8, old_flags: u8) -> (u8, u8) {
    let carry_in = old_flags & FLAG_C != 0;
    let (result, carry_out) = match opcode {
        0x07 => (
            accumulator.wrapping_shl(1) | (accumulator >> 7),
            accumulator & FLAG_S != 0,
        ),
        0x0F => (
            (accumulator >> 1) | accumulator.wrapping_shl(7),
            accumulator & FLAG_C != 0,
        ),
        0x17 => (
            accumulator.wrapping_shl(1) | u8::from(carry_in),
            accumulator & FLAG_S != 0,
        ),
        0x1F => (
            (accumulator >> 1) | (u8::from(carry_in) << 7),
            accumulator & 0x01 != 0,
        ),
        _ => unreachable!("only accumulator rotates are included"),
    };
    let flags = (old_flags & FLAG_SZPV) | (result & FLAG_XY) | if carry_out { FLAG_C } else { 0 };
    (result, flags)
}

#[test]
fn accumulator_rotates_match_all_accumulators_and_initial_flag_states() {
    let mut bus = TestBus::default();
    let mut cpu = Z80::default();

    for opcode in [0x07, 0x0F, 0x17, 0x1F] {
        bus.0[0] = opcode;
        for accumulator in 0_u8..=u8::MAX {
            for old_flags in 0_u8..=u8::MAX {
                let (expected_a, expected_f) =
                    expected_accumulator_rotate(opcode, accumulator, old_flags);
                cpu.registers.a = accumulator;
                cpu.registers.f = old_flags;
                cpu.registers.pc = 0;

                assert_eq!(cpu.step(&mut bus), Ok(4), "opcode={opcode:02X}");
                assert_eq!(
                    cpu.registers.a, expected_a,
                    "opcode={opcode:02X} A={accumulator:02X} F={old_flags:02X}: A"
                );
                assert_eq!(
                    cpu.registers.f, expected_f,
                    "opcode={opcode:02X} A={accumulator:02X} F={old_flags:02X}: F"
                );
            }
        }
    }
}

#[test]
fn cpl_scf_and_ccf_flags_match_all_accumulator_flag_and_q_xy_states() {
    let mut bus = TestBus::default();
    let mut cpu = Z80::default();

    // CPL does not depend on the Q latch. Exhaust every A/F combination.
    bus.0[0] = 0x2F;
    for accumulator in 0_u8..=u8::MAX {
        for old_flags in 0_u8..=u8::MAX {
            let expected_a = !accumulator;
            let expected_f = (old_flags & (FLAG_S | FLAG_Z | FLAG_PV | FLAG_C))
                | (expected_a & FLAG_XY)
                | FLAG_H
                | FLAG_N;
            cpu.registers.a = accumulator;
            cpu.registers.f = old_flags;
            cpu.registers.pc = 0;
            cpu.q = 0;

            assert_eq!(
                cpu.step(&mut bus),
                Ok(4),
                "CPL A={accumulator:02X} F={old_flags:02X}"
            );
            assert_eq!(cpu.registers.a, expected_a, "CPL accumulator");
            assert_eq!(
                cpu.registers.f, expected_f,
                "CPL flags A={accumulator:02X} F={old_flags:02X}"
            );
        }
    }

    // SCF/CCF expose the Q-latch-dependent undocumented X/Y behavior.
    // Q's X/Y projection has four states; its other bits cannot affect these formulas.
    for (opcode, complement_carry) in [(0x37_u8, false), (0x3F_u8, true)] {
        bus.0[0] = opcode;
        for accumulator in 0_u8..=u8::MAX {
            for old_flags in 0_u8..=u8::MAX {
                for q_xy in [0_u8, FLAG_X, FLAG_Y, FLAG_XY] {
                    let undocumented = (accumulator | (q_xy ^ old_flags)) & FLAG_XY;
                    let old_carry = old_flags & FLAG_C != 0;
                    let carry_or_half = if complement_carry && old_carry {
                        FLAG_H
                    } else {
                        FLAG_C
                    };
                    let expected_f = (old_flags & FLAG_SZPV) | undocumented | carry_or_half;
                    cpu.registers.a = accumulator;
                    cpu.registers.f = old_flags;
                    cpu.registers.pc = 0;
                    cpu.q = q_xy;

                    assert_eq!(
                        cpu.step(&mut bus),
                        Ok(4),
                        "opcode={opcode:02X} A={accumulator:02X} F={old_flags:02X} Qxy={q_xy:02X}"
                    );
                    assert_eq!(
                        cpu.registers.a, accumulator,
                        "opcode={opcode:02X}: accumulator must be preserved"
                    );
                    assert_eq!(
                        cpu.registers.f, expected_f,
                        "opcode={opcode:02X} A={accumulator:02X} F={old_flags:02X} Qxy={q_xy:02X}"
                    );
                }
            }
        }
    }
}

#[test]
fn neg_matches_all_accumulator_and_initial_flag_states() {
    let mut bus = TestBus::default();
    bus.0[..2].copy_from_slice(&[0xED, 0x44]); // NEG
    let mut cpu = Z80::default();

    for accumulator in 0_u8..=u8::MAX {
        let expected_a = 0_u8.wrapping_sub(accumulator);
        let mut expected_f = (expected_a & (FLAG_S | FLAG_XY)) | FLAG_N;
        if expected_a == 0 {
            expected_f |= FLAG_Z;
        }
        if accumulator & 0x0F != 0 {
            expected_f |= FLAG_H;
        }
        if accumulator == 0x80 {
            expected_f |= FLAG_PV;
        }
        if accumulator != 0 {
            expected_f |= FLAG_C;
        }

        for old_flags in 0_u8..=u8::MAX {
            cpu.registers.a = accumulator;
            cpu.registers.f = old_flags;
            cpu.registers.pc = 0;

            assert_eq!(
                cpu.step(&mut bus),
                Ok(8),
                "NEG A={accumulator:02X} F={old_flags:02X}"
            );
            assert_eq!(
                cpu.registers.a, expected_a,
                "NEG A={accumulator:02X} F={old_flags:02X}: accumulator"
            );
            assert_eq!(
                cpu.registers.f, expected_f,
                "NEG A={accumulator:02X} F={old_flags:02X}: flags"
            );
        }
    }
}
