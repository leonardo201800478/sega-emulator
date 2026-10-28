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
fn dd_prefix_is_ignored_for_register_only_instruction_without_h_or_l() {
    // DD 41: LD B,C remains LD B,C; DD does not affect this opcode.
    let mut bus = TestBus::default();
    bus.0[..2].copy_from_slice(&[0xDD, 0x41]);
    let mut cpu = Z80::default();
    cpu.registers.b = 0x11;
    cpu.registers.c = 0xA7;
    cpu.ix = 0x1234;

    assert_eq!(cpu.step(&mut bus), Ok(8));
    assert_eq!(cpu.registers.b, 0xA7);
    assert_eq!(cpu.registers.c, 0xA7);
    assert_eq!(cpu.ix, 0x1234);
    assert_eq!(cpu.registers.pc, 2);
    assert_eq!(cpu.r, 2);
    assert_eq!(cpu.total_tstates, 8);
}

#[test]
fn fd_prefix_is_ignored_for_register_only_instruction_without_h_or_l() {
    // FD 41: LD B,C remains LD B,C; FD does not affect this opcode.
    let mut bus = TestBus::default();
    bus.0[..2].copy_from_slice(&[0xFD, 0x41]);
    let mut cpu = Z80::default();
    cpu.registers.b = 0x22;
    cpu.registers.c = 0x5C;
    cpu.iy = 0x5678;

    assert_eq!(cpu.step(&mut bus), Ok(8));
    assert_eq!(cpu.registers.b, 0x5C);
    assert_eq!(cpu.registers.c, 0x5C);
    assert_eq!(cpu.iy, 0x5678);
    assert_eq!(cpu.registers.pc, 2);
    assert_eq!(cpu.r, 2);
    assert_eq!(cpu.total_tstates, 8);
}
