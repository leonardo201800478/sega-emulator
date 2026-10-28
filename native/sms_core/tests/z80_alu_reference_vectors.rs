//! Exhaustive arithmetic/logic vectors against a separately expressed reference model.
//!
//! The reference equations use widened unsigned/signed arithmetic rather than the
//! implementation's bitwise overflow formulas. This is an internal cross-check,
//! not a substitute for an external Z80 conformance suite or hardware evidence.

use sega_sms_core::alu::{adc8, add8, and8, cp8, or8, sbc8, sub8, xor8};

const C: u8 = 0x01;
const N: u8 = 0x02;
const PV: u8 = 0x04;
const H: u8 = 0x10;
const XY: u8 = 0x28;
const Z: u8 = 0x40;
const S: u8 = 0x80;

fn szxy(value: u8) -> u8 {
    (value & (S | XY)) | if value == 0 { Z } else { 0 }
}

fn parity(value: u8) -> bool {
    value.count_ones().is_multiple_of(2)
}

fn reference_add(lhs: u8, rhs: u8, carry: bool) -> (u8, u8) {
    let carry_in = i32::from(carry);
    let wide = u16::from(lhs) + u16::from(rhs) + u16::from(carry);
    let value = wide as u8;
    let signed = i32::from(lhs as i8) + i32::from(rhs as i8) + carry_in;
    let mut flags = szxy(value);
    if u16::from(lhs & 0x0f) + u16::from(rhs & 0x0f) + u16::from(carry) > 0x0f {
        flags |= H;
    }
    if !(-128..=127).contains(&signed) {
        flags |= PV;
    }
    if wide > 0xff {
        flags |= C;
    }
    (value, flags)
}

fn reference_sub(lhs: u8, rhs: u8, borrow: bool, compare: bool) -> (u8, u8) {
    let borrow_in = i32::from(borrow);
    let value = lhs.wrapping_sub(rhs).wrapping_sub(u8::from(borrow));
    let signed = i32::from(lhs as i8) - i32::from(rhs as i8) - borrow_in;
    let mut flags = (if compare { rhs & XY } else { value & XY }) | N;
    if value == 0 {
        flags |= Z;
    }
    if value & S != 0 {
        flags |= S;
    }
    if u16::from(lhs & 0x0f) < u16::from(rhs & 0x0f) + u16::from(borrow) {
        flags |= H;
    }
    if !(-128..=127).contains(&signed) {
        flags |= PV;
    }
    if u16::from(lhs) < u16::from(rhs) + u16::from(borrow) {
        flags |= C;
    }
    (if compare { lhs } else { value }, flags)
}

fn reference_logic(value: u8, half_carry: bool) -> (u8, u8) {
    let mut flags = szxy(value);
    if half_carry {
        flags |= H;
    }
    if parity(value) {
        flags |= PV;
    }
    (value, flags)
}

#[test]
fn exhaustive_add_adc_sub_sbc_and_cp_match_reference_equations() {
    for lhs in 0_u16..=255 {
        for rhs in 0_u16..=255 {
            let (lhs, rhs) = (lhs as u8, rhs as u8);
            let expected_add = reference_add(lhs, rhs, false);
            let actual_add = add8(lhs, rhs);
            assert_eq!(
                (actual_add.value, actual_add.flags),
                expected_add,
                "ADD {lhs:02X},{rhs:02X}"
            );

            let expected_sub = reference_sub(lhs, rhs, false, false);
            let actual_sub = sub8(lhs, rhs);
            assert_eq!(
                (actual_sub.value, actual_sub.flags),
                expected_sub,
                "SUB {lhs:02X},{rhs:02X}"
            );

            let expected_cp = reference_sub(lhs, rhs, false, true);
            let actual_cp = cp8(lhs, rhs);
            assert_eq!(
                (actual_cp.value, actual_cp.flags),
                expected_cp,
                "CP {lhs:02X},{rhs:02X}"
            );

            for carry in [false, true] {
                let expected_adc = reference_add(lhs, rhs, carry);
                let actual_adc = adc8(lhs, rhs, carry);
                assert_eq!(
                    (actual_adc.value, actual_adc.flags),
                    expected_adc,
                    "ADC {lhs:02X},{rhs:02X}, carry={carry}"
                );

                let expected_sbc = reference_sub(lhs, rhs, carry, false);
                let actual_sbc = sbc8(lhs, rhs, carry);
                assert_eq!(
                    (actual_sbc.value, actual_sbc.flags),
                    expected_sbc,
                    "SBC {lhs:02X},{rhs:02X}, borrow={carry}"
                );
            }

            let expected_and = reference_logic(lhs & rhs, true);
            let actual_and = and8(lhs, rhs);
            assert_eq!(
                (actual_and.value, actual_and.flags),
                expected_and,
                "AND {lhs:02X},{rhs:02X}"
            );

            let expected_or = reference_logic(lhs | rhs, false);
            let actual_or = or8(lhs, rhs);
            assert_eq!(
                (actual_or.value, actual_or.flags),
                expected_or,
                "OR {lhs:02X},{rhs:02X}"
            );

            let expected_xor = reference_logic(lhs ^ rhs, false);
            let actual_xor = xor8(lhs, rhs);
            assert_eq!(
                (actual_xor.value, actual_xor.flags),
                expected_xor,
                "XOR {lhs:02X},{rhs:02X}"
            );
        }
    }
}
