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
fn repeated_dd_prefix_adds_prefix_timing_and_refresh_increment() {
    // DD DD 21 34 12: the repeated DD is ignored; final operation loads IX.
    let mut bus = TestBus::default();
    bus.0[..5].copy_from_slice(&[0xDD, 0xDD, 0x21, 0x34, 0x12]);

    let mut cpu = Z80::default();
    cpu.ix = 0xAAAA;
    cpu.registers.set_hl(0x5678);

    assert_eq!(cpu.step(&mut bus), Ok(18));
    assert_eq!(cpu.ix, 0x1234);
    assert_eq!(cpu.iy, 0);
    assert_eq!(cpu.registers.hl(), 0x5678);
    assert_eq!(cpu.registers.pc, 5);
    assert_eq!(cpu.r, 3);
    assert_eq!(cpu.total_tstates, 18);
}

#[test]
fn last_index_prefix_selects_iy_after_dd_fd_sequence() {
    // DD FD 21 78 56: the last index prefix wins, so IY is loaded.
    let mut bus = TestBus::default();
    bus.0[..5].copy_from_slice(&[0xDD, 0xFD, 0x21, 0x78, 0x56]);

    let mut cpu = Z80::default();
    cpu.ix = 0xAAAA;
    cpu.iy = 0xBBBB;
    cpu.registers.set_hl(0x1357);

    assert_eq!(cpu.step(&mut bus), Ok(18));
    assert_eq!(cpu.ix, 0xAAAA);
    assert_eq!(cpu.iy, 0x5678);
    assert_eq!(cpu.registers.hl(), 0x1357);
    assert_eq!(cpu.registers.pc, 5);
    assert_eq!(cpu.r, 3);
    assert_eq!(cpu.total_tstates, 18);
}

#[test]
fn indexed_displacement_wraps_effective_address_at_ffff() {
    // DD 46 FE: LD B,(IX-2), with IX=0001 gives effective address FFFF.
    let mut bus = TestBus::default();
    bus.0[..3].copy_from_slice(&[0xDD, 0x46, 0xFE]);
    bus.0[0xFFFF] = 0xC3;

    let mut cpu = Z80::default();
    cpu.ix = 0x0001;
    cpu.registers.b = 0x12;
    cpu.registers.f = 0xA5;

    assert_eq!(cpu.step(&mut bus), Ok(19));
    assert_eq!(cpu.registers.b, 0xC3);
    assert_eq!(cpu.registers.f, 0xA5);
    assert_eq!(cpu.ix, 0x0001);
    assert_eq!(cpu.registers.pc, 3);
    assert_eq!(cpu.r, 2);
    assert_eq!(cpu.total_tstates, 19);
}
