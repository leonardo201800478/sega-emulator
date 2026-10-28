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

struct IndexedHlMemoryCase {
    prefix: u8,
    opcode: u8,
    displacement: u8,
    initial_h: u8,
    initial_l: u8,
    initial_memory: u8,
    expected_h: u8,
    expected_l: u8,
    expected_memory: u8,
}

fn run_indexed_hl_memory_case(case: IndexedHlMemoryCase) {
    let IndexedHlMemoryCase {
        prefix,
        opcode,
        displacement,
        initial_h,
        initial_l,
        initial_memory,
        expected_h,
        expected_l,
        expected_memory,
    } = case;
    let mut bus = TestBus::default();
    bus.0[..3].copy_from_slice(&[prefix, opcode, displacement]);
    let address = 0xC100_u16.wrapping_add_signed(i16::from(displacement as i8));
    bus.0[usize::from(address)] = initial_memory;

    let mut cpu = Z80::default();
    if prefix == 0xDD {
        cpu.ix = 0xC100;
    } else {
        cpu.iy = 0xC100;
    }
    cpu.registers.h = initial_h;
    cpu.registers.l = initial_l;

    assert_eq!(cpu.step(&mut bus), Ok(19), "{prefix:02X} {opcode:02X}");
    assert_eq!(cpu.registers.h, expected_h, "{prefix:02X} {opcode:02X}: H");
    assert_eq!(cpu.registers.l, expected_l, "{prefix:02X} {opcode:02X}: L");
    assert_eq!(
        bus.0[usize::from(address)],
        expected_memory,
        "{prefix:02X} {opcode:02X}: memory"
    );
    assert_eq!(cpu.registers.pc, 3, "{prefix:02X} {opcode:02X}: PC");
    assert_eq!(cpu.r, 2, "{prefix:02X} {opcode:02X}: R");
    assert_eq!(cpu.wz, address, "{prefix:02X} {opcode:02X}: WZ");
    if prefix == 0xDD {
        assert_eq!(cpu.ix, 0xC100, "{prefix:02X} {opcode:02X}: IX");
        assert_eq!(cpu.iy, 0, "{prefix:02X} {opcode:02X}: IY");
    } else {
        assert_eq!(cpu.iy, 0xC100, "{prefix:02X} {opcode:02X}: IY");
        assert_eq!(cpu.ix, 0, "{prefix:02X} {opcode:02X}: IX");
    }
}

#[test]
fn indexed_memory_loads_and_stores_use_plain_h_l_not_index_halves() {
    for prefix in [0xDD, 0xFD] {
        // LD H,(IX/IY+d): destination is plain H, not IXH/IYH.
        run_indexed_hl_memory_case(IndexedHlMemoryCase {
            prefix,
            opcode: 0x66,
            displacement: 1,
            initial_h: 0x11,
            initial_l: 0x22,
            initial_memory: 0xA5,
            expected_h: 0xA5,
            expected_l: 0x22,
            expected_memory: 0xA5,
        });
        // LD L,(IX/IY+d): destination is plain L, not IXL/IYL.
        run_indexed_hl_memory_case(IndexedHlMemoryCase {
            prefix,
            opcode: 0x6E,
            displacement: 2,
            initial_h: 0x11,
            initial_l: 0x22,
            initial_memory: 0xB6,
            expected_h: 0x11,
            expected_l: 0xB6,
            expected_memory: 0xB6,
        });
        // LD (IX/IY+d),H: source is plain H, not IXH/IYH.
        run_indexed_hl_memory_case(IndexedHlMemoryCase {
            prefix,
            opcode: 0x74,
            displacement: 1,
            initial_h: 0x5A,
            initial_l: 0x22,
            initial_memory: 0xCC,
            expected_h: 0x5A,
            expected_l: 0x22,
            expected_memory: 0x5A,
        });
        // LD (IX/IY+d),L: source is plain L, not IXL/IYL.
        run_indexed_hl_memory_case(IndexedHlMemoryCase {
            prefix,
            opcode: 0x75,
            displacement: 2,
            initial_h: 0x11,
            initial_l: 0x6B,
            initial_memory: 0xCC,
            expected_h: 0x11,
            expected_l: 0x6B,
            expected_memory: 0x6B,
        });
    }
}
