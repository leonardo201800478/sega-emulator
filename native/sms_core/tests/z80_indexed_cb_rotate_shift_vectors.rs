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

#[test]
fn indexed_cb_rotate_and_shift_fixed_vectors_cover_all_eight_operations() {
    // DD/FD CB 00 z: indexed memory rotate/shift operations, with z=6
    // selecting memory only. Expected results and flags are fixed reference
    // vectors, not outputs captured from another emulator. SLL and F3/F5 rely
    // on observed undocumented Z80 behavior; this test does not claim
    // independent differential or cycle-by-cycle bus validation.
    //
    // Fields: operation y, input byte, initial carry, result, expected F.
    // y=0..7: RLC, RRC, RL, RR, SLA, SRA, SLL, SRL.
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
        for (operation, input, carry_in, expected_value, expected_flags) in cases {
            let index = if prefix == 0xDD { 0xC100_u16 } else { 0xD200_u16 };
            let address = index;
            let mut bus = TestBus::default();
            bus.0[..4].copy_from_slice(&[prefix, 0xCB, 0x00, (operation << 3) | 0x06]);
            bus.0[usize::from(address)] = input;

            let mut cpu = Z80 {
                ix: if prefix == 0xDD { index } else { 0x1357 },
                iy: if prefix == 0xFD { index } else { 0x2468 },
                wz: 0xBEEF,
                ..Z80::default()
            };
            cpu.registers.f = if carry_in { 0x01 } else { 0x00 };
            cpu.registers.a = 0xA5;
            cpu.registers.b = 0x11;
            cpu.registers.c = 0x22;
            cpu.registers.d = 0x33;
            cpu.registers.e = 0x44;
            cpu.registers.h = 0x55;
            cpu.registers.l = 0x66;
            let registers_before = [
                cpu.registers.a,
                cpu.registers.b,
                cpu.registers.c,
                cpu.registers.d,
                cpu.registers.e,
                cpu.registers.h,
                cpu.registers.l,
            ];
            let context = format!(
                "prefix={prefix:#04x} operation={operation} input={input:#04x} carry_in={carry_in}"
            );

            assert_eq!(cpu.step(&mut bus), Ok(23), "{context}");
            assert_eq!(cpu.total_tstates, 23, "{context}");
            assert_eq!(cpu.registers.pc, 4, "{context}");
            assert_eq!(cpu.r, 2, "{context}");
            assert_eq!(cpu.wz, address, "{context}");
            assert_eq!(
                bus.0[usize::from(address)],
                expected_value,
                "{context}: memory result"
            );
            assert_eq!(cpu.registers.f, expected_flags, "{context}: flags");
            assert_eq!(
                [
                    cpu.registers.a,
                    cpu.registers.b,
                    cpu.registers.c,
                    cpu.registers.d,
                    cpu.registers.e,
                    cpu.registers.h,
                    cpu.registers.l,
                ],
                registers_before,
                "{context}: z=6 must not write a general register"
            );
            assert_eq!(
                cpu.ix,
                if prefix == 0xDD { index } else { 0x1357 },
                "{context}: IX preservation"
            );
            assert_eq!(
                cpu.iy,
                if prefix == 0xFD { index } else { 0x2468 },
                "{context}: IY preservation"
            );
        }
    }
}
