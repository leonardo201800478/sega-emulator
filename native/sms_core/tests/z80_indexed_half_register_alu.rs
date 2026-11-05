use sega_sms_core::{CpuBus, Z80};

const PREFIXES: [u8; 2] = [0xDD, 0xFD];
const HALF_REGISTER_CODES: [u8; 2] = [4, 5];
const ALU_OPERANDS: [u8; 13] = [
    0x00, 0x01, 0x0F, 0x10, 0x1F, 0x38, 0x55, 0x7F, 0x80, 0x81, 0xAA, 0xFE, 0xFF,
];

struct TestBus([u8; 4]);

impl CpuBus for TestBus {
    fn read_memory(&mut self, address: u16) -> u8 {
        self.0[usize::from(address)]
    }

    fn write_memory(&mut self, address: u16, value: u8) {
        self.0[usize::from(address)] = value;
    }

    fn read_port(&mut self, _address: u16) -> u8 {
        0xFF
    }

    fn write_port(&mut self, _address: u16, _value: u8) {}
}

fn execute(
    program: &[u8],
    accumulator: u8,
    flags: u8,
    hl: u16,
    ix: u16,
    iy: u16,
) -> (Z80, u32) {
    let mut bus = TestBus([0; 4]);
    bus.0[..program.len()].copy_from_slice(program);

    let mut cpu = Z80::default();
    cpu.registers.pc = 0;
    cpu.registers.a = accumulator;
    cpu.registers.f = flags;
    cpu.registers.h = (hl >> 8) as u8;
    cpu.registers.l = hl as u8;
    cpu.ix = ix;
    cpu.iy = iy;
    cpu.r = 0x20;

    let cycles = cpu.step(&mut bus).expect("instruction must execute");
    (cpu, cycles)
}

fn index_with_half(prefix: u8, code: u8, value: u8) -> (u16, u16) {
    let half = if code == 4 {
        (u16::from(value) << 8) | 0xA5
    } else {
        0x5A00 | u16::from(value)
    };

    if prefix == 0xDD {
        (half, 0xC3D4)
    } else {
        (0xA1B2, half)
    }
}

fn indexed_half(prefix: u8, code: u8, ix: u16, iy: u16) -> u8 {
    let index = if prefix == 0xDD { ix } else { iy };
    if code == 4 {
        (index >> 8) as u8
    } else {
        index as u8
    }
}

#[test]
fn dd_fd_alu_half_registers_match_the_corresponding_base_register_semantics() {
    for prefix in PREFIXES {
        for source in HALF_REGISTER_CODES {
            for operation in 0..8 {
                let opcode = 0x80 | (operation << 3) | source;

                for accumulator in ALU_OPERANDS {
                    for operand in ALU_OPERANDS {
                        for carry in [0, 1] {
                            let (ix, iy) = index_with_half(prefix, source, operand);
                            let hl = if source == 4 {
                                (u16::from(operand) << 8) | 0x00E7
                            } else {
                                (0x5A << 8) | u16::from(operand)
                            };
                            let initial_flags = 0xA8 | carry;

                            let (indexed, indexed_cycles) = execute(
                                &[prefix, opcode],
                                accumulator,
                                initial_flags,
                                0xD6E7,
                                ix,
                                iy,
                            );
                            let (base, base_cycles) = execute(
                                &[opcode],
                                accumulator,
                                initial_flags,
                                hl,
                                0xA1B2,
                                0xC3D4,
                            );

                            assert_eq!(
                                indexed_cycles, 8,
                                "prefix={prefix:02X} opcode={opcode:02X}"
                            );
                            assert_eq!(
                                base_cycles, 4,
                                "base opcode={opcode:02X}"
                            );
                            assert_eq!(
                                indexed.registers.a, base.registers.a,
                                "A: prefix={prefix:02X} opcode={opcode:02X} A={accumulator:02X} operand={operand:02X} C={carry}"
                            );
                            assert_eq!(
                                indexed.registers.f, base.registers.f,
                                "F: prefix={prefix:02X} opcode={opcode:02X} A={accumulator:02X} operand={operand:02X} C={carry}"
                            );
                            assert_eq!(indexed.ix, ix, "IX must remain unchanged");
                            assert_eq!(indexed.iy, iy, "IY must remain unchanged");
                            assert_eq!(
                                indexed_half(prefix, source, indexed.ix, indexed.iy),
                                operand,
                                "index half must remain unchanged: prefix={prefix:02X} opcode={opcode:02X}"
                            );
                            assert_eq!(
                                indexed.registers.pc, 2,
                                "indexed PC: prefix={prefix:02X} opcode={opcode:02X}"
                            );
                            assert_eq!(base.registers.pc, 1, "base PC: opcode={opcode:02X}");
                            assert_eq!(
                                indexed.r,
                                base.r + 1,
                                "indexed prefix adds one M1 refresh: prefix={prefix:02X} opcode={opcode:02X}"
                            );
                            assert_eq!(
                                indexed.total_tstates,
                                base.total_tstates + 4,
                                "indexed prefix adds 4T: prefix={prefix:02X} opcode={opcode:02X}"
                            );
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn dd_fd_inc_dec_half_registers_match_base_h_l_for_every_byte_and_carry() {
    for prefix in PREFIXES {
        for (opcode, code) in [(0x24, 4), (0x25, 4), (0x2C, 5), (0x2D, 5)] {
            for value in 0..=u8::MAX {
                for carry in [0, 1] {
                    let (ix, iy) = index_with_half(prefix, code, value);
                    let hl = if code == 4 {
                        (u16::from(value) << 8) | 0x00E7
                    } else {
                        (0x5A << 8) | u16::from(value)
                    };
                    let initial_flags = 0xA8 | carry;

                    let (indexed, indexed_cycles) = execute(
                        &[prefix, opcode],
                        0x96,
                        initial_flags,
                        0xD6E7,
                        ix,
                        iy,
                    );
                    let (base, base_cycles) =
                        execute(&[opcode], 0x96, initial_flags, hl, 0xA1B2, 0xC3D4);

                    assert_eq!(
                        indexed_cycles, 8,
                        "prefix={prefix:02X} opcode={opcode:02X} value={value:02X}"
                    );
                    assert_eq!(
                        base_cycles, 4,
                        "base opcode={opcode:02X} value={value:02X}"
                    );
                    let expected_value = if opcode == 0x24 || opcode == 0x2C {
                        value.wrapping_add(1)
                    } else {
                        value.wrapping_sub(1)
                    };
                    let current_index = if prefix == 0xDD { ix } else { iy };
                    let updated_index = if code == 4 {
                        (u16::from(expected_value) << 8) | (current_index & 0x00FF)
                    } else {
                        (current_index & 0xFF00) | u16::from(expected_value)
                    };
                    let (expected_ix, expected_iy) = if prefix == 0xDD {
                        (updated_index, iy)
                    } else {
                        (ix, updated_index)
                    };
                    assert_eq!(indexed.ix, expected_ix);
                    assert_eq!(indexed.iy, expected_iy);
                    assert_eq!(
                        indexed_half(prefix, code, indexed.ix, indexed.iy),
                        expected_value,
                        "index result: prefix={prefix:02X} opcode={opcode:02X} value={value:02X}"
                    );
                    assert_eq!(
                        indexed.registers.f, base.registers.f,
                        "F: prefix={prefix:02X} opcode={opcode:02X} value={value:02X} C={carry}"
                    );
                    assert_eq!(
                        indexed.registers.a, base.registers.a,
                        "A: prefix={prefix:02X} opcode={opcode:02X} value={value:02X}"
                    );
                    assert_eq!(
                        indexed.registers.h, 0xD6,
                        "ordinary H must remain unchanged: prefix={prefix:02X} opcode={opcode:02X}"
                    );
                    assert_eq!(
                        indexed.registers.l, 0xE7,
                        "ordinary L must remain unchanged: prefix={prefix:02X} opcode={opcode:02X}"
                    );
                    assert_eq!(
                        indexed.registers.pc, 2,
                        "indexed PC: prefix={prefix:02X} opcode={opcode:02X}"
                    );
                    assert_eq!(base.registers.pc, 1, "base PC: opcode={opcode:02X}");
                    assert_eq!(
                        indexed.r,
                        base.r + 1,
                        "indexed prefix adds one M1 refresh: prefix={prefix:02X} opcode={opcode:02X}"
                    );
                    assert_eq!(
                        indexed.total_tstates,
                        base.total_tstates + 4,
                        "indexed prefix adds 4T: prefix={prefix:02X} opcode={opcode:02X}"
                    );
                }
            }
        }
    }
}
