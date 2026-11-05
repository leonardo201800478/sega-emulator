//! Research-derived regression matrix for all eight ED block-I/O opcodes.
//!
//! The reference rules are transcribed from David Banks, "Undocumented Z80
//! Flags" rev. 1.0 (2018), and cross-checked against the documented real-NMOS
//! hardware test by Peter Helcmanovsky:
//! https://github.com/MrKWatkins/ZXSpectrumNextTests/tree/8adcbc8384b2311fb6d36d424478aef583d5c307/Tests/ZX48_ZX128/Z80BlockInstructionFlags
//!
//! This is an independently written reference model, not a silicon oracle.
//! The hardware test is a limited ZX Spectrum test and explicitly warns that
//! CMOS/clone behavior may differ. SingleStepTests vectors remain Ares-derived.
//! Keep this provenance distinction when reporting E7 evidence.

use sega_sms_core::{CpuBus, Z80};

const FLAG_S: u8 = 0x80;
const FLAG_Z: u8 = 0x40;
const FLAG_YX: u8 = 0x28;
const FLAG_H: u8 = 0x10;
const FLAG_PV: u8 = 0x04;
const FLAG_N: u8 = 0x02;
const FLAG_C: u8 = 0x01;

struct FlatBus {
    memory: Box<[u8]>,
    ports: Box<[u8]>,
    last_port_read: Option<u16>,
    last_port_write: Option<(u16, u8)>,
}

impl Default for FlatBus {
    fn default() -> Self {
        Self {
            memory: vec![0; 0x1_0000].into_boxed_slice(),
            ports: vec![0; 0x1_0000].into_boxed_slice(),
            last_port_read: None,
            last_port_write: None,
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

    fn read_port(&mut self, address: u16) -> u8 {
        self.last_port_read = Some(address);
        self.ports[usize::from(address)]
    }

    fn write_port(&mut self, address: u16, value: u8) {
        self.last_port_write = Some((address, value));
        self.ports[usize::from(address)] = value;
    }
}

fn even_parity(value: u8) -> bool {
    value.count_ones().is_multiple_of(2)
}

fn add_direction_u8(value: u8, direction: i8) -> u8 {
    if direction > 0 {
        value.wrapping_add(1)
    } else {
        value.wrapping_sub(1)
    }
}

fn add_direction_u16(value: u16, direction: i8) -> u16 {
    if direction > 0 {
        value.wrapping_add(1)
    } else {
        value.wrapping_sub(1)
    }
}

/// Computes the published NMOS flag equations without calling production helpers.
fn reference_flags(
    opcode: u8,
    initial_b: u8,
    c: u8,
    hl: u16,
    value: u8,
    instruction_pc: u16,
) -> u8 {
    let input = matches!(opcode, 0xA2 | 0xAA | 0xB2 | 0xBA);
    let repeat = matches!(opcode, 0xB2 | 0xBA | 0xB3 | 0xBB);
    let direction = if matches!(opcode, 0xAA | 0xBA | 0xAB | 0xBB) {
        -1
    } else {
        1
    };
    let low_term = if input {
        add_direction_u8(c, direction)
    } else {
        add_direction_u8(hl as u8, direction)
    };
    let sum = u16::from(value) + u16::from(low_term);
    let b = initial_b.wrapping_sub(1);
    let mut flags = b & (FLAG_S | FLAG_YX);

    if b == 0 {
        flags |= FLAG_Z;
    }
    if value & 0x80 != 0 {
        flags |= FLAG_N;
    }
    if sum > 0xFF {
        flags |= FLAG_H | FLAG_C;
    }

    let parity_base = ((sum as u8) & 0x07) ^ b;
    if even_parity(parity_base) {
        flags |= FLAG_PV;
    }

    // The extra five-T-state repeat cycle exposes these flags if an interrupt
    // is accepted before the repeated instruction begins again.
    if repeat && b != 0 {
        flags = (flags & !FLAG_YX) | (((instruction_pc >> 8) as u8) & FLAG_YX);
        let carry = flags & FLAG_C != 0;
        let subtract = flags & FLAG_N != 0;
        let parity_adjustment = if carry {
            if subtract {
                b.wrapping_sub(1) & 0x07
            } else {
                b.wrapping_add(1) & 0x07
            }
        } else {
            b & 0x07
        };

        if even_parity(parity_base ^ parity_adjustment) {
            flags |= FLAG_PV;
        } else {
            flags &= !FLAG_PV;
        }

        let half_carry = carry
            && if subtract {
                b & 0x0F == 0
            } else {
                b & 0x0F == 0x0F
            };
        if half_carry {
            flags |= FLAG_H;
        } else {
            flags &= !FLAG_H;
        }
    }

    flags
}

#[test]
fn all_ed_block_io_opcodes_match_nmos_flag_equations_at_boundaries() {
    let b_values = [0x00, 0x01, 0x02, 0x0F, 0x10, 0x7F, 0x80, 0xFF];
    let c_values = [0x00, 0x01, 0x7F, 0xFE, 0xFF];
    let hl_values = [0x0000, 0x00FF, 0x12FF, 0xFF00, 0xFFFD];
    let data_values = [0x00, 0x01, 0x7F, 0x80, 0xFE, 0xFF];
    let pc_values = [0x4000, 0x4800, 0x6000, 0x6800, 0xFFFE];
    let opcodes = [0xA2, 0xAA, 0xA3, 0xAB, 0xB2, 0xBA, 0xB3, 0xBB];

    let mut bus = FlatBus::default();

    for opcode in opcodes {
        let input = matches!(opcode, 0xA2 | 0xAA | 0xB2 | 0xBA);
        let repeat = matches!(opcode, 0xB2 | 0xBA | 0xB3 | 0xBB);
        let direction = if matches!(opcode, 0xAA | 0xBA | 0xAB | 0xBB) {
            -1
        } else {
            1
        };

        for initial_b in b_values {
            for c in c_values {
                for hl in hl_values {
                    for value in data_values {
                        for pc in pc_values {
                            let old_bc = u16::from_be_bytes([initial_b, c]);
                            let new_b = initial_b.wrapping_sub(1);
                            let new_bc = u16::from_be_bytes([new_b, c]);
                            let repeated = repeat && new_b != 0;
                            let expected_hl = add_direction_u16(hl, direction);
                            let expected_pc = if repeated {
                                pc
                            } else {
                                pc.wrapping_add(2)
                            };
                            let expected_wz = if repeated {
                                pc.wrapping_add(1)
                            } else if input {
                                add_direction_u16(old_bc, direction)
                            } else {
                                add_direction_u16(new_bc, direction)
                            };
                            let expected_tstates = if repeated { 21 } else { 16 };
                            let expected_f =
                                reference_flags(opcode, initial_b, c, hl, value, pc);

                            bus.last_port_read = None;
                            bus.last_port_write = None;
                            bus.memory[usize::from(pc)] = 0xED;
                            bus.memory[usize::from(pc.wrapping_add(1))] = opcode;
                            bus.memory[usize::from(hl)] = if input { value ^ 0xFF } else { value };
                            bus.ports[usize::from(old_bc)] = value;

                            let mut cpu = Z80::default();
                            cpu.registers.pc = pc;
                            cpu.registers.b = initial_b;
                            cpu.registers.c = c;
                            cpu.registers.h = (hl >> 8) as u8;
                            cpu.registers.l = hl as u8;
                            cpu.registers.f = 0xFF;

                            assert_eq!(
                                cpu.step(&mut bus),
                                Ok(expected_tstates),
                                "opcode={opcode:02X} B={initial_b:02X} C={c:02X} HL={hl:04X} data={value:02X} PC={pc:04X}: T-states"
                            );
                            assert_eq!(
                                cpu.registers.f, expected_f,
                                "opcode={opcode:02X} B={initial_b:02X} C={c:02X} HL={hl:04X} data={value:02X} PC={pc:04X}: flags"
                            );
                            assert_eq!(cpu.registers.b, new_b, "opcode={opcode:02X}: B");
                            assert_eq!(cpu.registers.hl(), expected_hl, "opcode={opcode:02X}: HL");
                            assert_eq!(cpu.registers.pc, expected_pc, "opcode={opcode:02X}: PC");
                            assert_eq!(cpu.wz, expected_wz, "opcode={opcode:02X}: WZ");

                            if input {
                                assert_eq!(
                                    bus.last_port_read,
                                    Some(old_bc),
                                    "opcode={opcode:02X}: input port"
                                );
                                assert_eq!(
                                    bus.memory[usize::from(hl)],
                                    value,
                                    "opcode={opcode:02X}: input memory write"
                                );
                            } else {
                                assert_eq!(
                                    bus.last_port_write,
                                    Some((new_bc, value)),
                                    "opcode={opcode:02X}: output port"
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}
