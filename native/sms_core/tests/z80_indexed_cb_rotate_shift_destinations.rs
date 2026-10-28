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

#[test]
fn indexed_cb_rotate_shift_copies_results_to_all_ordinary_register_destinations() {
    // Fixed semantic vectors for y=0..7: RLC, RRC, RL, RR, SLA, SRA, SLL, SRL.
    // These cases match the fixed-value oracle in
    // z80_indexed_cb_rotate_shift_vectors.rs; this test expands destination
    // coverage rather than claiming an independent oracle or bus-cycle trace.
    let cases = [
        (0_u8, 0x84_u8, false, 0x09_u8, 0x0D_u8),
        (1, 0x03, false, 0x81, 0x85),
        (2, 0x80, true, 0x01, 0x01),
        (3, 0x01, true, 0x80, 0x81),
        (4, 0x81, false, 0x02, 0x01),
        (5, 0x81, false, 0xC0, 0x85),
        (6, 0x81, false, 0x03, 0x05),
        (7, 0x81, false, 0x40, 0x01),
    ];

    for prefix in [0xDD_u8, 0xFD_u8] {
        let address = if prefix == 0xDD {
            0xC100_u16
        } else {
            0xD200_u16
        };
        for (operation, input, carry_in, expected_value, expected_flags) in cases {
            for destination in 0..8_u8 {
                let mut bus = TestBus::default();
                bus.0[..4].copy_from_slice(&[prefix, 0xCB, 0x00, (operation << 3) | destination]);
                bus.0[usize::from(address)] = input;

                let mut cpu = Z80::default();
                cpu.ix = if prefix == 0xDD { address } else { 0x1357 };
                cpu.iy = if prefix == 0xFD { address } else { 0x2468 };
                cpu.wz = 0xBEEF;
                cpu.registers.f = u8::from(carry_in);
                cpu.registers.a = 0xA5;
                cpu.registers.b = 0x11;
                cpu.registers.c = 0x22;
                cpu.registers.d = 0x33;
                cpu.registers.e = 0x44;
                cpu.registers.h = 0x55;
                cpu.registers.l = 0x66;
                let before = general_registers(&cpu);
                let context = format!(
                    "prefix={prefix:#04x} operation={operation} destination={destination} input={input:#04x}"
                );

                assert_eq!(cpu.step(&mut bus), Ok(23), "{context}");
                assert_eq!(cpu.total_tstates, 23, "{context}");
                assert_eq!(cpu.registers.pc, 4, "{context}");
                assert_eq!(cpu.r, 2, "{context}");
                assert_eq!(cpu.wz, address, "{context}: effective address");
                assert_eq!(
                    bus.0[usize::from(address)],
                    expected_value,
                    "{context}: memory result"
                );
                assert_eq!(cpu.registers.f, expected_flags, "{context}: flags");
                assert_eq!(
                    cpu.ix,
                    if prefix == 0xDD { address } else { 0x1357 },
                    "{context}: IX preservation"
                );
                assert_eq!(
                    cpu.iy,
                    if prefix == 0xFD { address } else { 0x2468 },
                    "{context}: IY preservation"
                );

                let mut expected_registers = before;
                match destination {
                    0 => expected_registers[1] = expected_value, // B
                    1 => expected_registers[2] = expected_value, // C
                    2 => expected_registers[3] = expected_value, // D
                    3 => expected_registers[4] = expected_value, // E
                    4 => expected_registers[5] = expected_value, // H, not IXH/IYH
                    5 => expected_registers[6] = expected_value, // L, not IXL/IYL
                    6 => {}                                      // memory-only form
                    7 => expected_registers[0] = expected_value, // A
                    _ => unreachable!(),
                }
                assert_eq!(
                    general_registers(&cpu),
                    expected_registers,
                    "{context}: destination copy and unrelated-register preservation"
                );
            }
        }
    }
}
