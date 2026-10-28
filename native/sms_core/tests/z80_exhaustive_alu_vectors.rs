use sega_sms_core::alu::{adc8, add8, and8, cp8, or8, sbc8, sub8, xor8};

const C: u8 = 0x01;
const N: u8 = 0x02;
const PV: u8 = 0x04;
const H: u8 = 0x10;
const Z: u8 = 0x40;
const S: u8 = 0x80;
const XY: u8 = 0x28;

fn szxy(value: u8) -> u8 {
    (value & (S | XY)) | if value == 0 { Z } else { 0 }
}

fn even_parity(value: u8) -> bool {
    value.count_ones() % 2 == 0
}

fn reference_add(lhs: u8, rhs: u8, carry: bool) -> (u8, u8) {
    let carry = i32::from(carry);
    let wide = u16::from(lhs) + u16::from(rhs) + carry as u16;
    let result = wide as u8;
    let signed = i32::from(lhs as i8) + i32::from(rhs as i8) + carry;
    let mut flags = szxy(result);
    if u16::from(lhs & 0x0f) + u16::from(rhs & 0x0f) + carry as u16 > 0x0f {
        flags |= H;
    }
    if !(-128..=127).contains(&signed) {
        flags |= PV;
    }
    if wide > 0xff {
        flags |= C;
    }
    (result, flags)
}

fn reference_sub(lhs: u8, rhs: u8, borrow: bool, compare: bool) -> (u8, u8) {
    let borrow_i = i32::from(borrow);
    let result = lhs.wrapping_sub(rhs).wrapping_sub(u8::from(borrow));
    let signed = i32::from(lhs as i8) - i32::from(rhs as i8) - borrow_i;
    let mut flags = (if compare { rhs & XY } else { result & XY })
        | (result & S)
        | if result == 0 { Z } else { 0 }
        | N;
    if u16::from(lhs & 0x0f) < u16::from(rhs & 0x0f) + borrow_i as u16 {
        flags |= H;
    }
    if !(-128..=127).contains(&signed) {
        flags |= PV;
    }
    if u16::from(lhs) < u16::from(rhs) + borrow_i as u16 {
        flags |= C;
    }
    (if compare { lhs } else { result }, flags)
}

fn reference_logic(lhs: u8, rhs: u8, operation: u8) -> (u8, u8) {
    let result = match operation {
        0 => lhs & rhs,
        1 => lhs ^ rhs,
        2 => lhs | rhs,
        _ => unreachable!("test operation is one of AND/XOR/OR"),
    };
    let mut flags = szxy(result);
    if operation == 0 {
        flags |= H;
    }
    if even_parity(result) {
        flags |= PV;
    }
    (result, flags)
}

#[test]
fn exhaustive_alu_results_and_flags_match_independent_arithmetic_model() {
    // Enumerates all 256×256 operand pairs. ADC/SBC are checked with both
    // carry/borrow inputs. The reference model uses signed-range checks and
    // widened arithmetic rather than the production ALU's bitwise formulas.
    for lhs in 0_u8..=u8::MAX {
        for rhs in 0_u8..=u8::MAX {
            let expected = reference_add(lhs, rhs, false);
            let actual = add8(lhs, rhs);
            assert_eq!((actual.value, actual.flags), expected, "ADD lhs={lhs:#04x} rhs={rhs:#04x}");

            for carry in [false, true] {
                let expected = reference_add(lhs, rhs, carry);
                let actual = adc8(lhs, rhs, carry);
                assert_eq!((actual.value, actual.flags), expected, "ADC lhs={lhs:#04x} rhs={rhs:#04x} carry={carry}");

                let expected = reference_sub(lhs, rhs, carry, false);
                let actual = sbc8(lhs, rhs, carry);
                assert_eq!((actual.value, actual.flags), expected, "SBC lhs={lhs:#04x} rhs={rhs:#04x} borrow={carry}");
            }

            let expected = reference_sub(lhs, rhs, false, false);
            let actual = sub8(lhs, rhs);
            assert_eq!((actual.value, actual.flags), expected, "SUB lhs={lhs:#04x} rhs={rhs:#04x}");

            let expected = reference_sub(lhs, rhs, false, true);
            let actual = cp8(lhs, rhs);
            assert_eq!((actual.value, actual.flags), expected, "CP lhs={lhs:#04x} rhs={rhs:#04x}");

            for operation in 0..3 {
                let expected = reference_logic(lhs, rhs, operation);
                let actual = match operation {
                    0 => and8(lhs, rhs),
                    1 => xor8(lhs, rhs),
                    2 => or8(lhs, rhs),
                    _ => unreachable!(),
                };
                assert_eq!((actual.value, actual.flags), expected, "logic op={operation} lhs={lhs:#04x} rhs={rhs:#04x}");
            }
        }
    }
}
