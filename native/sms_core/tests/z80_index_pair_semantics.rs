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
fn dd_fd_load_immediate_replace_hl_with_ix_iy() {
    let mut dd_bus = TestBus::default();
    dd_bus.0[..4].copy_from_slice(&[0xDD, 0x21, 0x34, 0x12]);
    let mut dd = Z80::default();
    dd.registers.set_hl(0xABCD);

    assert_eq!(dd.step(&mut dd_bus), Ok(14));
    assert_eq!(dd.ix, 0x1234);
    assert_eq!(dd.registers.hl(), 0xABCD, "LD IX,nn must preserve HL");
    assert_eq!(dd.registers.pc, 4);
    assert_eq!(dd.r, 2);
    assert_eq!(dd.total_tstates, 14);

    let mut fd_bus = TestBus::default();
    fd_bus.0[..4].copy_from_slice(&[0xFD, 0x21, 0x78, 0x56]);
    let mut fd = Z80::default();
    fd.registers.set_hl(0x9876);

    assert_eq!(fd.step(&mut fd_bus), Ok(14));
    assert_eq!(fd.iy, 0x5678);
    assert_eq!(fd.registers.hl(), 0x9876, "LD IY,nn must preserve HL");
    assert_eq!(fd.registers.pc, 4);
    assert_eq!(fd.r, 2);
    assert_eq!(fd.total_tstates, 14);
}

#[test]
fn dd_inc_ix_and_fd_dec_iy_update_index_pair_without_changing_flags() {
    let mut dd_bus = TestBus::default();
    dd_bus.0[..2].copy_from_slice(&[0xDD, 0x23]);
    let mut dd = Z80::default();
    dd.ix = 0x12FF;
    dd.registers.f = 0xD7;

    assert_eq!(dd.step(&mut dd_bus), Ok(10));
    assert_eq!(dd.ix, 0x1300);
    assert_eq!(dd.registers.f, 0xD7, "INC IX preserves flags");
    assert_eq!(dd.registers.pc, 2);
    assert_eq!(dd.r, 2);
    assert_eq!(dd.total_tstates, 10);

    let mut fd_bus = TestBus::default();
    fd_bus.0[..2].copy_from_slice(&[0xFD, 0x2B]);
    let mut fd = Z80::default();
    fd.iy = 0x0100;
    fd.registers.f = 0xA5;

    assert_eq!(fd.step(&mut fd_bus), Ok(10));
    assert_eq!(fd.iy, 0x00FF);
    assert_eq!(fd.registers.f, 0xA5, "DEC IY preserves flags");
    assert_eq!(fd.registers.pc, 2);
    assert_eq!(fd.r, 2);
    assert_eq!(fd.total_tstates, 10);
}

#[test]
fn dd_fd_load_accumulator_from_index_high_and_low_bytes() {
    let mut dd_bus = TestBus::default();
    dd_bus.0[..2].copy_from_slice(&[0xDD, 0x7C]); // LD A,IXH
    let mut dd = Z80::default();
    dd.ix = 0x12A7;
    dd.registers.a = 0xEE;
    dd.registers.f = 0xD7;

    assert_eq!(dd.step(&mut dd_bus), Ok(8));
    assert_eq!(dd.registers.a, 0x12);
    assert_eq!(dd.registers.f, 0xD7, "LD preserves flags");
    assert_eq!(dd.ix, 0x12A7);
    assert_eq!(dd.registers.pc, 2);
    assert_eq!(dd.r, 2);
    assert_eq!(dd.total_tstates, 8);

    let mut fd_bus = TestBus::default();
    fd_bus.0[..2].copy_from_slice(&[0xFD, 0x7D]); // LD A,IYL
    let mut fd = Z80::default();
    fd.iy = 0x56B4;
    fd.registers.a = 0xEE;
    fd.registers.f = 0xA5;

    assert_eq!(fd.step(&mut fd_bus), Ok(8));
    assert_eq!(fd.registers.a, 0xB4);
    assert_eq!(fd.registers.f, 0xA5, "LD preserves flags");
    assert_eq!(fd.iy, 0x56B4);
    assert_eq!(fd.registers.pc, 2);
    assert_eq!(fd.r, 2);
    assert_eq!(fd.total_tstates, 8);
}
