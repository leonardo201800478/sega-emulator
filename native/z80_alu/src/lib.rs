//! Isolated Z80 8-bit ALU prototype.
//!
//! This crate is not connected to the emulator runtime. The Python Z80
//! implementation remains the reference until differential validation passes.

/// Result of an 8-bit arithmetic operation and the Z80 flag register.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AluResult {
    pub value: u8,
    pub flags: u8,
}

const FLAG_C: u8 = 0x01;
const FLAG_N: u8 = 0x02;
const FLAG_PV: u8 = 0x04;
const FLAG_H: u8 = 0x10;
const FLAG_Z: u8 = 0x40;
const FLAG_S: u8 = 0x80;
const FLAG_XY: u8 = 0x28;

fn sign_zero_xy(value: u8) -> u8 {
    (value & (FLAG_S | FLAG_XY)) | if value == 0 { FLAG_Z } else { 0 }
}

/// Executes Z80 ADD A,r-style 8-bit addition without carry-in.
pub fn add8(lhs: u8, rhs: u8) -> AluResult {
    let wide = u16::from(lhs) + u16::from(rhs);
    let value = wide as u8;
    let mut flags = sign_zero_xy(value);

    if (lhs & 0x0f) + (rhs & 0x0f) > 0x0f {
        flags |= FLAG_H;
    }
    if (!(lhs ^ rhs) & (lhs ^ value) & FLAG_S) != 0 {
        flags |= FLAG_PV;
    }
    if wide > u16::from(u8::MAX) {
        flags |= FLAG_C;
    }

    AluResult { value, flags }
}

/// Executes Z80 ADC A,r-style addition with carry-in.
pub fn adc8(lhs: u8, rhs: u8, carry_in: bool) -> AluResult {
    let carry = u16::from(carry_in);
    let wide = u16::from(lhs) + u16::from(rhs) + carry;
    let value = wide as u8;
    let mut flags = sign_zero_xy(value);
    if u16::from(lhs & 0x0f) + u16::from(rhs & 0x0f) + carry > 0x0f {
        flags |= FLAG_H;
    }
    if (!(lhs ^ rhs) & (lhs ^ value) & FLAG_S) != 0 {
        flags |= FLAG_PV;
    }
    if wide > u16::from(u8::MAX) {
        flags |= FLAG_C;
    }
    AluResult { value, flags }
}

/// Executes Z80 SUB r-style 8-bit subtraction.
pub fn sub8(lhs: u8, rhs: u8) -> AluResult {
    let value = lhs.wrapping_sub(rhs);
    let mut flags = sign_zero_xy(value) | FLAG_N;

    if (lhs & 0x0f) < (rhs & 0x0f) {
        flags |= FLAG_H;
    }
    if ((lhs ^ rhs) & (lhs ^ value) & FLAG_S) != 0 {
        flags |= FLAG_PV;
    }
    if lhs < rhs {
        flags |= FLAG_C;
    }

    AluResult { value, flags }
}

/// Executes Z80 SBC A,r-style subtraction with borrow-in.
pub fn sbc8(lhs: u8, rhs: u8, carry_in: bool) -> AluResult {
    let borrow = u16::from(carry_in);
    let subtrahend = u16::from(rhs) + borrow;
    let value = lhs.wrapping_sub(rhs).wrapping_sub(u8::from(carry_in));
    let mut flags = sign_zero_xy(value) | FLAG_N;
    if u16::from(lhs & 0x0f) < u16::from(rhs & 0x0f) + borrow {
        flags |= FLAG_H;
    }
    if ((lhs ^ rhs) & (lhs ^ value) & FLAG_S) != 0 {
        flags |= FLAG_PV;
    }
    if u16::from(lhs) < subtrahend {
        flags |= FLAG_C;
    }
    AluResult { value, flags }
}

#[cfg(test)]
mod tests {
    use super::{adc8, add8, sbc8, sub8};

    #[test]
    fn arithmetic_wraps_at_eight_bits() {
        assert_eq!(add8(0xff, 1).value, 0x00);
        assert_eq!(sub8(0x00, 1).value, 0xff);
    }

    #[test]
    fn addition_sets_half_carry_overflow_and_carry() {
        assert_eq!(add8(0x0f, 1).flags & 0x10, 0x10);
        assert_eq!(add8(0x7f, 1).flags & 0x04, 0x04);
        assert_eq!(add8(0xff, 1).flags & 0x01, 0x01);
    }

    #[test]
    fn subtraction_sets_subtract_half_borrow_overflow_and_carry() {
        assert_eq!(sub8(0x00, 1).flags & 0x02, 0x02);
        assert_eq!(sub8(0x10, 1).flags & 0x10, 0x10);
        assert_eq!(sub8(0x80, 1).flags & 0x04, 0x04);
        assert_eq!(sub8(0x00, 1).flags & 0x01, 0x01);
    }

    #[test]
    fn carry_in_is_included_in_adc_and_sbc() {
        assert_eq!(adc8(0xff, 0x00, true).value, 0x00);
        assert_eq!(adc8(0x7f, 0x00, true).flags & 0x04, 0x04);
        assert_eq!(sbc8(0x00, 0x00, true).value, 0xff);
        assert_eq!(sbc8(0x80, 0x00, true).flags & 0x04, 0x04);
    }

    #[test]
    fn undocumented_xy_flags_follow_the_result_for_add_and_sub() {
        assert_eq!(add8(0x00, 0x08).flags & 0x28, 0x08);
        assert_eq!(sub8(0x00, 0x10).flags & 0x28, 0x20);
    }
}
