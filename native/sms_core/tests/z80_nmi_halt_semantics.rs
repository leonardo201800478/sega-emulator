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
fn nmi_exits_halt_even_when_maskable_interrupt_is_disabled() {
    let mut bus = TestBus::default();
    bus.0[..2].copy_from_slice(&[0x76, 0x00]); // HALT; NOP
    let mut cpu = Z80::default();
    cpu.registers.sp = 0xC100;

    assert_eq!(cpu.step(&mut bus), Ok(4));
    assert!(cpu.halted);
    assert_eq!(cpu.registers.pc, 1);
    assert_eq!(cpu.r, 1);

    // INT is pending but masked; HALT continues and refresh R still advances.
    cpu.request_int(0xFF);
    assert_eq!(cpu.step(&mut bus), Ok(4));
    assert!(cpu.halted);
    assert_eq!(cpu.registers.pc, 1);
    assert_eq!(cpu.r, 2);
    assert_eq!(cpu.total_tstates, 8);

    // NMI has priority over masked INT and stacks the post-HALT PC.
    cpu.request_nmi();
    assert_eq!(cpu.step(&mut bus), Ok(11));
    assert!(!cpu.halted);
    assert_eq!(cpu.registers.pc, 0x0066);
    assert_eq!(cpu.registers.sp, 0xC0FE);
    assert_eq!((bus.0[0xC0FE], bus.0[0xC0FF]), (0x01, 0x00));
    assert_eq!(cpu.r, 3);
    assert_eq!(cpu.total_tstates, 19);
    assert!(!cpu.iff1);
    assert!(!cpu.iff2);
}
