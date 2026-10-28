//! Exhaustive indexed-CB rotate/shift semantics over every input byte and carry-in.
//!
//! The expected results are computed from the Z80 operation definitions rather
//! than the production ALU helpers. This validates semantic state and nominal
//! instruction T-states, not an ordered per-T-state bus trace.
use sega_sms_core::{CpuBus, Z80};

const FLAG_C: u8 = 0x01;
const FLAG_PV: u8 = 0x04;
const FLAG_XY: u8 = 0x28;
const FLAG_Z: u8 = 0x40;
const FLAG_S: u8 = 0x80;

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

fn even_parity(value: u8) -> bool {
    value.count_ones().is_multiple_of(2)
}

fn reference_rotate_shift(operation: u8, value: u8, carry_in: bool) -> (u8, bool) {
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
fn indexed_cb_rotate_shift_memory_is_exhaustive_for_all_values_and_carry_inputs() {
    for prefix in [0xDD_u8, 0xFD] {
        let index = if prefix == 0xDD {
            0xC100_u16
        } else {
            0xD200_u16
        };
        let address = index.wrapping_add(1);

        for operation in 0_u8..8 {
            for value in u8::MIN..=u8::MAX {
                for carry_in in [false, true] {
                    let opcode = operation << 3 | 6; // destination field 6: memory only
                    let mut bus = FlatBus::default();
                    bus.memory[..4].copy_from_slice(&[prefix, 0xCB, 0x01, opcode]);
                    bus.memory[usize::from(address)] = value;

                    let mut cpu = Z80::default();
                    cpu.ix = if prefix == 0xDD { index } else { 0x1357 };
                    cpu.iy = if prefix == 0xFD { index } else { 0x2468 };
                    cpu.registers.f = u8::from(carry_in);
                    let context = format!(
                        "prefix={prefix:02X} operation={operation} value={value:02X} carry_in={carry_in}"
                    );

                    let (expected_value, carry_out) =
                        reference_rotate_shift(operation, value, carry_in);
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

                    assert_eq!(cpu.step(&mut bus), Ok(23), "{context}");
                    assert_eq!(
                        bus.memory[usize::from(address)],
                        expected_value,
                        "{context}"
                    );
                    assert_eq!(cpu.registers.f, expected_flags, "{context}: flags");
                    assert_eq!(cpu.registers.pc, 4, "{context}: PC");
                    assert_eq!(cpu.r, 2, "{context}: R");
                    assert_eq!(cpu.total_tstates, 23, "{context}: T-states");
                    assert_eq!(cpu.wz, address, "{context}: WZ");
                    assert_eq!(
                        cpu.ix,
                        if prefix == 0xDD { index } else { 0x1357 },
                        "{context}: IX"
                    );
                    assert_eq!(
                        cpu.iy,
                        if prefix == 0xFD { index } else { 0x2468 },
                        "{context}: IY"
                    );
                }
            }
        }
    }
}
