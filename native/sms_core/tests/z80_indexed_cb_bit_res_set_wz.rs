use sega_sms_core::{CpuBus, Z80};

struct TestBus([u8; 0x10000]);

impl Default for TestBus {
    fn default() -> Self {
        Self([0; 0x10000])
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

fn read_register(cpu: &Z80, code: u8) -> u8 {
    match code {
        0 => cpu.registers.b,
        1 => cpu.registers.c,
        2 => cpu.registers.d,
        3 => cpu.registers.e,
        4 => cpu.registers.h,
        5 => cpu.registers.l,
        7 => cpu.registers.a,
        _ => unreachable!("code 6 is the indexed memory operand"),
    }
}

fn write_register(cpu: &mut Z80, code: u8, value: u8) {
    match code {
        0 => cpu.registers.b = value,
        1 => cpu.registers.c = value,
        2 => cpu.registers.d = value,
        3 => cpu.registers.e = value,
        4 => cpu.registers.h = value,
        5 => cpu.registers.l = value,
        7 => cpu.registers.a = value,
        _ => unreachable!("code 6 is the indexed memory operand"),
    }
}

fn general_registers(cpu: &Z80) -> [u8; 7] {
    [
        cpu.registers.a,
        cpu.registers.b,
        cpu.registers.c,
        cpu.registers.d,
        cpu.registers.e,
        cpu.registers.h,
        cpu.registers.l,
    ]
}

fn expected_bit_flags(old_flags: u8, value: u8, bit: u8, address: u16) -> u8 {
    const CARRY: u8 = 0x01;
    const HALF_CARRY: u8 = 0x10;
    const PARITY_OVERFLOW: u8 = 0x04;
    const ZERO: u8 = 0x40;
    const SIGN: u8 = 0x80;
    const XY: u8 = 0x28;

    let bit_is_clear = value & (1 << bit) == 0;
    let mut flags = (old_flags & CARRY) | HALF_CARRY | ((address >> 8) as u8 & XY);
    if bit_is_clear {
        flags |= ZERO | PARITY_OVERFLOW;
    }
    if bit == 7 && !bit_is_clear {
        flags |= SIGN;
    }
    flags
}

#[test]
fn indexed_cb_bit_res_set_cover_all_bit_indices_destinations_and_wz() {
    // The low three opcode bits encode B,C,D,E,H,L,(IX/IY+d),A.
    // BIT ignores this field for indexed memory forms; RES/SET write the
    // selected ordinary register as well as memory when the field is not 6.
    // WZ/MEMPTR is expected to latch the effective address for all three
    // operation families. These are semantic vectors, not bus-cycle traces.
    let prefixes = [(0xDD, 0x28FE_u16), (0xFD, 0xDFFF_u16)];
    let initial_values = [0x00_u8, 0xFF_u8];

    for (prefix, index) in prefixes {
        for bit in 0..8_u8 {
            for destination in 0..8_u8 {
                for initial_value in initial_values {
                    for operation in 1..=3_u8 {
                        let address = index.wrapping_add(1);
                        let mut bus = TestBus::default();
                        bus.0[..4].copy_from_slice(&[
                            prefix,
                            0xCB,
                            0x01,
                            (operation << 6) | (bit << 3) | destination,
                        ]);
                        bus.0[usize::from(address)] = initial_value;

                        let mut cpu = Z80::default();
                        cpu.ix = if prefix == 0xDD { index } else { 0x1111 };
                        cpu.iy = if prefix == 0xFD { index } else { 0x2222 };
                        cpu.wz = 0xBEEF;
                        cpu.registers.a = 0x10;
                        cpu.registers.f = 0xD7;
                        cpu.registers.b = 0x21;
                        cpu.registers.c = 0x32;
                        cpu.registers.d = 0x43;
                        cpu.registers.e = 0x54;
                        cpu.registers.h = 0x65;
                        cpu.registers.l = 0x76;
                        let before_registers = general_registers(&cpu);
                        let before_flags = cpu.registers.f;

                        let result = match operation {
                            1 => initial_value,
                            2 => initial_value & !(1 << bit),
                            3 => initial_value | (1 << bit),
                            _ => unreachable!(),
                        };
                        let expected_cycles = if operation == 1 { 20 } else { 23 };
                        let context = format!(
                            "prefix={prefix:#04x} op={operation} bit={bit} dst={destination} value={initial_value:#04x}"
                        );

                        assert_eq!(cpu.step(&mut bus), Ok(expected_cycles), "{context}");
                        assert_eq!(cpu.total_tstates, expected_cycles as u64, "{context}");
                        assert_eq!(cpu.registers.pc, 4, "{context}");
                        assert_eq!(cpu.r, 2, "{context}");
                        assert_eq!(cpu.wz, address, "{context}");
                        assert_eq!(
                            cpu.ix,
                            if prefix == 0xDD { index } else { 0x1111 },
                            "{context}"
                        );
                        assert_eq!(
                            cpu.iy,
                            if prefix == 0xFD { index } else { 0x2222 },
                            "{context}"
                        );
                        assert_eq!(bus.0[usize::from(address)], result, "{context}");

                        if operation == 1 {
                            assert_eq!(
                                cpu.registers.f,
                                expected_bit_flags(before_flags, initial_value, bit, address),
                                "{context}"
                            );
                            assert_eq!(
                                general_registers(&cpu),
                                before_registers,
                                "{context}: BIT must not write a register"
                            );
                            assert_eq!(
                                bus.0[usize::from(address)],
                                initial_value,
                                "{context}: BIT must not write memory"
                            );
                        } else {
                            assert_eq!(
                                cpu.registers.f, before_flags,
                                "{context}: RES/SET preserve flags"
                            );
                            let mut expected_registers = before_registers;
                            if destination != 6 {
                                let mut expected_cpu = Z80::default();
                                expected_cpu.registers.a = before_registers[0];
                                expected_cpu.registers.b = before_registers[1];
                                expected_cpu.registers.c = before_registers[2];
                                expected_cpu.registers.d = before_registers[3];
                                expected_cpu.registers.e = before_registers[4];
                                expected_cpu.registers.h = before_registers[5];
                                expected_cpu.registers.l = before_registers[6];
                                write_register(&mut expected_cpu, destination, result);
                                expected_registers = general_registers(&expected_cpu);
                                assert_eq!(
                                    read_register(&cpu, destination),
                                    result,
                                    "{context}: register destination"
                                );
                            }
                            assert_eq!(
                                general_registers(&cpu),
                                expected_registers,
                                "{context}: unrelated registers must be preserved"
                            );
                        }
                    }
                }
            }
        }
    }
}
