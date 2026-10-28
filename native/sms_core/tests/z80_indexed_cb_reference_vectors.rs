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
fn indexed_bit_flags_and_wz_match_published_reference_vectors() {
    // Reference basis:
    // - Zilog Z80 CPU User Manual, UM008011-0816, pp. 247-249:
    //   BIT b,(IX/IY+d), 20 T-states, H=1, N=0, C preserved, memory unchanged.
    //   Zilog marks S and P/V as unknown; their tested silicon behavior below
    //   is taken from "The Undocumented Z80 Documented", section 4.1.
    // - redcode/Z80 MEMPTR wiki: indexed BIT updates MEMPTR to IX/IY+d.
    //
    // These are fixed, source-derived reference vectors, not a claim that a
    // separate emulator or physical CPU was executed in this CI run.
    // The high byte covers each combination of undocumented F3/F5 bits.
    let cases = [
        (0xDD, 0x0040_u16, 0x00, 0x46, 0x55),
        (0xFD, 0x0840_u16, 0x00, 0x46, 0x5D),
        (0xDD, 0x2040_u16, 0x00, 0x46, 0x75),
        (0xFD, 0x2840_u16, 0x00, 0x46, 0x7D),
        (0xDD, 0x8040_u16, 0x80, 0x7E, 0x91),
        (0xFD, 0x8840_u16, 0x80, 0x7E, 0x99),
        (0xDD, 0xA040_u16, 0x80, 0x7E, 0xB1),
        (0xFD, 0xA840_u16, 0x80, 0x7E, 0xB9),
    ];

    for (prefix, address, value, opcode, expected_flags) in cases {
        let mut bus = TestBus::default();
        bus.0[..4].copy_from_slice(&[prefix, 0xCB, 0x00, opcode]);
        bus.0[usize::from(address)] = value;

        let mut cpu = Z80::default();
        cpu.ix = if prefix == 0xDD { address } else { 0x1357 };
        cpu.iy = if prefix == 0xFD { address } else { 0x2468 };
        cpu.wz = 0xBEEF;
        cpu.registers.f = 0xC1;

        let old_ix = cpu.ix;
        let old_iy = cpu.iy;
        let context = format!(
            "prefix={prefix:#04x} address={address:#06x} value={value:#04x} opcode={opcode:#04x}"
        );

        assert_eq!(cpu.step(&mut bus), Ok(20), "{context}");
        assert_eq!(cpu.total_tstates, 20, "{context}");
        assert_eq!(cpu.registers.pc, 4, "{context}");
        assert_eq!(cpu.r, 2, "{context}");
        assert_eq!(cpu.wz, address, "{context}: MEMPTR must latch IX/IY+d");
        assert_eq!(cpu.registers.f, expected_flags, "{context}: fixed flag oracle");
        assert_eq!(cpu.ix, old_ix, "{context}: IX unchanged");
        assert_eq!(cpu.iy, old_iy, "{context}: IY unchanged");
        assert_eq!(
            bus.0[usize::from(address)],
            value,
            "{context}: BIT must not modify memory"
        );
    }
}
