//! Broad deterministic sweep for ED ADC/SBC HL,rr arithmetic and flags.
//!
//! Every possible HL value is exercised with both carry inputs against a set
//! of boundary operands for BC, DE and SP; ADC/SBC HL,HL sweeps every HL value
//! as both operands. The oracle uses widened integer arithmetic and independent
//! flag equations. This is semantic validation, not cycle-accurate bus timing.
use sega_sms_core::{CpuBus, Z80};

const FLAG_C: u8 = 0x01;
const FLAG_N: u8 = 0x02;
const FLAG_PV: u8 = 0x04;
const FLAG_H: u8 = 0x10;
const FLAG_Z: u8 = 0x40;
const FLAG_S: u8 = 0x80;
const FLAG_XY: u8 = 0x28;

const RHS_BOUNDARIES: [u16; 7] = [0x0000, 0x0001, 0x0FFF, 0x1000, 0x7FFF, 0x8000, 0xFFFF];

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

fn set_pair(cpu: &mut Z80, pair_code: u8, value: u16) {
    match pair_code {
        0 => cpu.registers.set_bc(value),
        1 => cpu.registers.set_de(value),
        2 => cpu.registers.set_hl(value),
        3 => cpu.registers.sp = value,
        _ => unreachable!(),
    }
}

fn expected_result_and_flags(lhs: u16, rhs: u16, carry: bool, subtract: bool) -> (u16, u8) {
    let carry_in = u32::from(carry);
    let lhs32 = u32::from(lhs);
    let rhs32 = u32::from(rhs);
    let wide = if subtract {
        lhs32.wrapping_sub(rhs32 + carry_in)
    } else {
        lhs32 + rhs32 + carry_in
    };
    let result = wide as u16;
    let mut flags = ((result >> 8) as u8) & (FLAG_S | FLAG_XY);

    if result == 0 {
        flags |= FLAG_Z;
    }

    if subtract {
        flags |= FLAG_N;
        if u32::from(lhs & 0x0FFF) < u32::from(rhs & 0x0FFF) + carry_in {
            flags |= FLAG_H;
        }
        if ((lhs ^ rhs) & (lhs ^ result) & 0x8000) != 0 {
            flags |= FLAG_PV;
        }
        if lhs32 < rhs32 + carry_in {
            flags |= FLAG_C;
        }
    } else {
        if u32::from(lhs & 0x0FFF) + u32::from(rhs & 0x0FFF) + carry_in > 0x0FFF {
            flags |= FLAG_H;
        }
        if (!(lhs ^ rhs) & (lhs ^ result) & 0x8000) != 0 {
            flags |= FLAG_PV;
        }
        if wide > u32::from(u16::MAX) {
            flags |= FLAG_C;
        }
    }

    (result, flags)
}

#[test]
fn ed_adc_sbc_hl_sweeps_every_lhs_and_boundary_rhs_with_both_carry_inputs() {
    let mut bus = FlatBus::default();
    let mut cpu = Z80::default();

    for subtract in [false, true] {
        for pair_code in 0_u8..4 {
            let opcode_base = if subtract { 0x42 } else { 0x4A };
            let opcode = opcode_base | (pair_code << 4);
            bus.memory[0] = 0xED;
            bus.memory[1] = opcode;

            let rhs_values: &[u16] = if pair_code == 2 {
                &[0]
            } else {
                &RHS_BOUNDARIES
            };

            for fixed_rhs in rhs_values.iter().copied() {
                for lhs in u16::MIN..=u16::MAX {
                    for carry in [false, true] {
                        let rhs = if pair_code == 2 { lhs } else { fixed_rhs };
                        cpu.registers.pc = 0;
                        cpu.registers.set_hl(lhs);
                        if pair_code != 2 {
                            set_pair(&mut cpu, pair_code, rhs);
                        }
                        cpu.registers.f = if carry { FLAG_C } else { 0 };
                        cpu.wz = 0xA55A;

                        let (expected_hl, expected_flags) =
                            expected_result_and_flags(lhs, rhs, carry, subtract);
                        assert_eq!(
                            cpu.step(&mut bus),
                            Ok(15),
                            "ED {opcode:02X}, HL={lhs:04X}, rhs={rhs:04X}, carry={carry}"
                        );
                        assert_eq!(
                            cpu.registers.hl(),
                            expected_hl,
                            "ED {opcode:02X}, HL={lhs:04X}, rhs={rhs:04X}, carry={carry}"
                        );
                        assert_eq!(
                            cpu.registers.f, expected_flags,
                            "ED {opcode:02X} flags, HL={lhs:04X}, rhs={rhs:04X}, carry={carry}"
                        );
                        assert_eq!(cpu.registers.pc, 2, "ED {opcode:02X} PC");
                        assert_eq!(cpu.wz, lhs.wrapping_add(1), "ED {opcode:02X} WZ");
                    }
                }
            }
        }
    }
}
