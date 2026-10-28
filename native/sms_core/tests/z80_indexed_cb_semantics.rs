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
fn dd_cb_rotate_updates_memory_register_flags_and_architectural_counters() {
    // DD CB 01 00: RLC (IX+1),B
    let mut bus = TestBus::default();
    bus.0[..4].copy_from_slice(&[0xDD, 0xCB, 0x01, 0x00]);
    bus.0[0xC001] = 0x81;

    let mut cpu = Z80::default();
    cpu.ix = 0xC000;
    cpu.registers.b = 0x11;
    cpu.registers.f = 0xFF;

    assert_eq!(cpu.step(&mut bus), Ok(23));
    assert_eq!(bus.0[0xC001], 0x03);
    assert_eq!(cpu.registers.b, 0x03);
    assert_eq!(cpu.registers.f, 0x05); // even parity and carry from bit 7
    assert_eq!(cpu.ix, 0xC000);
    assert_eq!(cpu.registers.pc, 4);
    assert_eq!(cpu.r, 2);
    assert_eq!(cpu.total_tstates, 23);
}

#[test]
fn fd_cb_res_clears_memory_bit_and_preserves_flags() {
    // FD CB FF 86: RES 0,(IY-1); effective address wraps from 0000 to FFFF.
    let mut bus = TestBus::default();
    bus.0[..4].copy_from_slice(&[0xFD, 0xCB, 0xFF, 0x86]);
    bus.0[0xFFFF] = 0x01;

    let mut cpu = Z80::default();
    cpu.iy = 0x0000;
    cpu.registers.f = 0xD7;

    assert_eq!(cpu.step(&mut bus), Ok(23));
    assert_eq!(bus.0[0xFFFF], 0x00);
    assert_eq!(cpu.registers.f, 0xD7, "RES preserves flags");
    assert_eq!(cpu.iy, 0x0000);
    assert_eq!(cpu.registers.pc, 4);
    assert_eq!(cpu.r, 2);
    assert_eq!(cpu.total_tstates, 23);
}
