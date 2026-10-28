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
fn im0_prefix_acknowledge_reads_following_opcode_and_operands_from_memory() {
    // The device supplies DD during acknowledge; the next opcode and nn
    // bytes are fetched from the current PC as usual.
    let mut bus = TestBus::default();
    bus.0[0x1234..0x1237].copy_from_slice(&[0x21, 0x78, 0x56]);
    let mut cpu = Z80::default();
    cpu.registers.pc = 0x1234;
    cpu.r = 0x85;
    cpu.iff1 = true;
    cpu.iff2 = true;
    cpu.request_int(0xDD);

    assert_eq!(cpu.step(&mut bus), Ok(16));
    assert_eq!(cpu.ix, 0x5678);
    assert_eq!(cpu.registers.pc, 0x1237);
    assert_eq!(cpu.r, 0x87);
    assert_eq!(cpu.total_tstates, 16);
}

#[test]
fn interrupt_acknowledge_increments_r_once_in_each_interrupt_mode() {
    // IM 0: externally supplied RST 08h opcode.
    let mut bus = TestBus::default();
    let mut cpu = Z80::default();
    cpu.registers.pc = 0x1234;
    cpu.registers.sp = 0xC100;
    cpu.r = 0x85;
    cpu.iff1 = true;
    cpu.iff2 = true;
    cpu.request_int(0xCF);

    assert_eq!(cpu.step(&mut bus), Ok(13));
    assert_eq!(cpu.r, 0x86);
    assert_eq!(cpu.registers.pc, 0x0008);
    assert_eq!((bus.0[0xC0FE], bus.0[0xC0FF]), (0x34, 0x12));

    // R wraps its low seven bits while preserving bit 7.
    let mut bus = TestBus::default();
    let mut cpu = Z80::default();
    cpu.registers.pc = 0x1234;
    cpu.registers.sp = 0xC100;
    cpu.r = 0xFF;
    cpu.iff1 = true;
    cpu.request_int(0xCF);
    assert_eq!(cpu.step(&mut bus), Ok(13));
    assert_eq!(cpu.r, 0x80);

    // IM 1: acknowledge M1 increments R once.
    let mut bus = TestBus::default();
    let mut cpu = Z80::default();
    cpu.interrupt_mode = 1;
    cpu.registers.pc = 0x5678;
    cpu.registers.sp = 0xC200;
    cpu.r = 0x85;
    cpu.iff1 = true;
    cpu.iff2 = true;
    cpu.request_int(0xFF);

    assert_eq!(cpu.step(&mut bus), Ok(13));
    assert_eq!(cpu.r, 0x86);
    assert_eq!(cpu.registers.pc, 0x0038);

    // IM 2: acknowledge M1 increments R once before vector-table reads.
    let mut bus = TestBus::default();
    bus.0[0x20FF] = 0x78;
    bus.0[0x2100] = 0x56;
    let mut cpu = Z80::default();
    cpu.interrupt_mode = 2;
    cpu.i = 0x20;
    cpu.registers.pc = 0x9ABC;
    cpu.registers.sp = 0xC300;
    cpu.r = 0x85;
    cpu.iff1 = true;
    cpu.iff2 = true;
    cpu.request_int(0xFF);

    assert_eq!(cpu.step(&mut bus), Ok(19));
    assert_eq!(cpu.r, 0x86);
    assert_eq!(cpu.registers.pc, 0x5678);
}
