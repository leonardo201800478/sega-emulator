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
fn dd_indexed_load_uses_signed_minimum_displacement() {
    // DD 46 80: LD B,(IX-128). Displacement is signed and wraps from IX.
    let mut bus = TestBus::default();
    bus.0[..3].copy_from_slice(&[0xDD, 0x46, 0x80]);
    bus.0[0xC000] = 0xA6;

    let mut cpu = Z80::default();
    cpu.ix = 0xC080;
    cpu.registers.b = 0x11;
    cpu.registers.f = 0xD7;

    assert_eq!(cpu.step(&mut bus), Ok(19));
    assert_eq!(cpu.registers.b, 0xA6);
    assert_eq!(cpu.registers.f, 0xD7, "LD must preserve flags");
    assert_eq!(cpu.ix, 0xC080);
    assert_eq!(cpu.registers.pc, 3);
    assert_eq!(cpu.r, 2);
    assert_eq!(cpu.total_tstates, 19);
}

#[test]
fn fd_indexed_load_uses_signed_maximum_displacement() {
    // FD 7E 7F: LD A,(IY+127).
    let mut bus = TestBus::default();
    bus.0[..3].copy_from_slice(&[0xFD, 0x7E, 0x7F]);
    bus.0[0xBFFF] = 0x5C;

    let mut cpu = Z80::default();
    cpu.iy = 0xBF80;
    cpu.registers.a = 0x11;
    cpu.registers.f = 0xA5;

    assert_eq!(cpu.step(&mut bus), Ok(19));
    assert_eq!(cpu.registers.a, 0x5C);
    assert_eq!(cpu.registers.f, 0xA5, "LD must preserve flags");
    assert_eq!(cpu.iy, 0xBF80);
    assert_eq!(cpu.registers.pc, 3);
    assert_eq!(cpu.r, 2);
    assert_eq!(cpu.total_tstates, 19);
}
