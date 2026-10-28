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
fn ei_delay_expires_after_ignored_index_prefix_instruction_before_int_acceptance() {
    let mut bus = TestBus::default();
    // EI; DD NOP (DD is ignored for NOP); NOP
    bus.0[..4].copy_from_slice(&[0xFB, 0xDD, 0x00, 0x00]);

    let mut cpu = Z80::default();
    cpu.interrupt_mode = 1;
    cpu.registers.sp = 0xC100;
    cpu.request_int(0xFF);

    assert_eq!(cpu.step(&mut bus), Ok(4), "EI");
    assert!(cpu.iff1);
    assert_eq!(cpu.ei_delay, 1);
    assert_eq!(cpu.registers.pc, 1);

    // The prefixed instruction is one instruction for EI-delay purposes.
    assert_eq!(cpu.step(&mut bus), Ok(8), "DD NOP");
    assert_eq!(cpu.ei_delay, 0);
    assert_eq!(cpu.registers.pc, 3);
    assert_eq!(cpu.r, 3);

    // The pending maskable interrupt must now be accepted, before the NOP
    // at address 0x0003 executes.
    assert_eq!(cpu.step(&mut bus), Ok(13), "IM 1 acknowledge");
    assert_eq!(cpu.registers.pc, 0x0038);
    assert_eq!(cpu.registers.sp, 0xC0FE);
    assert_eq!((bus.0[0xC0FE], bus.0[0xC0FF]), (0x03, 0x00));
    assert_eq!(cpu.r, 4);
    assert_eq!(cpu.total_tstates, 25);
    assert_eq!(cpu.int_pending, None);
}
