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
fn mixed_fd_dd_prefix_chain_uses_last_index_for_indexed_cb() {
    // FD DD CB 01 00: the last index prefix selects IX.
    // RLC (IX+1),B transforms 0x81 to 0x03; IY must remain untouched.
    let mut bus = TestBus::default();
    bus.0[..5].copy_from_slice(&[0xFD, 0xDD, 0xCB, 0x01, 0x00]);
    bus.0[0xC101] = 0x81;

    let mut cpu = Z80::default();
    cpu.ix = 0xC100;
    cpu.iy = 0xC000;
    cpu.registers.b = 0x11;
    cpu.registers.f = 0xFF;

    assert_eq!(cpu.step(&mut bus), Ok(27));
    assert_eq!(bus.0[0xC101], 0x03, "effective IX address must be written");
    assert_eq!(cpu.registers.b, 0x03, "indexed-CB result must copy to B");
    assert_eq!(cpu.registers.f, 0x05, "RLC flags: even parity and carry");
    assert_eq!(cpu.ix, 0xC100, "index register itself must be preserved");
    assert_eq!(cpu.iy, 0xC000, "earlier FD prefix must not select IY");
    assert_eq!(cpu.wz, 0xC101);
    assert_eq!(cpu.registers.pc, 5);
    assert_eq!(cpu.r, 3, "each M1 prefix/opcode fetch increments R");
    assert_eq!(cpu.total_tstates, 27);
}
