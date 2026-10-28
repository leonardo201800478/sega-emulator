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
fn indexed_memory_forms_use_h_l_while_register_only_forms_use_index_halves() {
    // DD 66 d: LD H,(IX+d) writes H, not IXH.
    let mut bus = TestBus::default();
    bus.0[..3].copy_from_slice(&[0xDD, 0x66, 0x02]);
    bus.0[0xC002] = 0x5A;
    let mut cpu = Z80::default();
    cpu.ix = 0xC000;
    cpu.registers.h = 0x11;

    assert_eq!(cpu.step(&mut bus), Ok(19));
    assert_eq!(cpu.registers.h, 0x5A);
    assert_eq!(cpu.ix, 0xC000);
    assert_eq!(cpu.registers.pc, 3);
    assert_eq!(cpu.r, 2);
    assert_eq!(cpu.total_tstates, 19);

    // FD 74 d: LD (IY+d),H uses H, not IYH; -1 wraps the displacement.
    let mut bus = TestBus::default();
    bus.0[..3].copy_from_slice(&[0xFD, 0x74, 0xFF]);
    let mut cpu = Z80::default();
    cpu.iy = 0xC100;
    cpu.registers.h = 0x42;

    assert_eq!(cpu.step(&mut bus), Ok(19));
    assert_eq!(bus.0[0xC0FF], 0x42);
    assert_eq!(cpu.iy, 0xC100);
    assert_eq!(cpu.registers.pc, 3);
    assert_eq!(cpu.r, 2);
    assert_eq!(cpu.total_tstates, 19);

    // DD 65: LD IXH,IXL; both operands are index halves in this form.
    let mut bus = TestBus::default();
    bus.0[..2].copy_from_slice(&[0xDD, 0x65]);
    let mut cpu = Z80::default();
    cpu.ix = 0x1234;

    assert_eq!(cpu.step(&mut bus), Ok(8));
    assert_eq!(cpu.ix, 0x3434);
    assert_eq!(cpu.registers.pc, 2);
    assert_eq!(cpu.r, 2);
    assert_eq!(cpu.total_tstates, 8);

    // FD 65: LD IYH,IYL.
    let mut bus = TestBus::default();
    bus.0[..2].copy_from_slice(&[0xFD, 0x65]);
    let mut cpu = Z80::default();
    cpu.iy = 0x5678;

    assert_eq!(cpu.step(&mut bus), Ok(8));
    assert_eq!(cpu.iy, 0x7878);
    assert_eq!(cpu.registers.pc, 2);
    assert_eq!(cpu.r, 2);
    assert_eq!(cpu.total_tstates, 8);
}
