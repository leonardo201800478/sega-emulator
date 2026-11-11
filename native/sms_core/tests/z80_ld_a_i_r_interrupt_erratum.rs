use sega_sms_core::{CpuBus, Z80};

const FLAG_C: u8 = 0x01;
const FLAG_PV: u8 = 0x04;

struct TestBus {
    memory: Box<[u8]>,
}

impl Default for TestBus {
    fn default() -> Self {
        Self {
            memory: vec![0; 0x1_0000].into_boxed_slice(),
        }
    }
}

impl CpuBus for TestBus {
    fn read_memory(&mut self, address: u16) -> u8 {
        self.memory[usize::from(address)]
    }

    fn write_memory(&mut self, address: u16, value: u8) {
        self.memory[usize::from(address)] = value;
    }

    fn read_port(&mut self, _address: u16) -> u8 {
        0xFF
    }

    fn write_port(&mut self, _address: u16, _value: u8) {}
}

fn cpu_and_bus(opcode: u8) -> (Z80, TestBus) {
    let mut bus = TestBus::default();
    bus.memory[..3].copy_from_slice(&[0xED, opcode, 0x00]);
    let mut cpu = Z80::default();
    cpu.i = 0xA8;
    cpu.r = 0x20;
    cpu.registers.f = FLAG_C;
    cpu.iff1 = true;
    cpu.iff2 = true;
    (cpu, bus)
}

#[test]
fn atomic_executor_clears_pv_when_int_is_accepted_after_ld_a_i_or_r() {
    for opcode in [0x57, 0x5F] {
        for iff2 in [false, true] {
            for carry in [0, FLAG_C] {
                let (mut cpu, mut bus) = cpu_and_bus(opcode);
                cpu.iff2 = iff2;
                cpu.registers.f = carry;

                assert_eq!(cpu.step(&mut bus), Ok(9), "ED {opcode:02X}");
                assert_eq!(cpu.registers.f & FLAG_PV != 0, iff2, "ED {opcode:02X}");
                assert_eq!(cpu.registers.f & FLAG_C, carry, "ED {opcode:02X}");

                cpu.request_int(0xFF);
                assert_eq!(cpu.step(&mut bus), Ok(13), "ED {opcode:02X}");
                assert_eq!(cpu.registers.f & FLAG_PV, 0, "ED {opcode:02X}");
                assert_eq!(cpu.registers.f & FLAG_C, carry, "ED {opcode:02X}");
            }
        }
    }
}

#[test]
fn atomic_executor_does_not_apply_ld_a_i_r_erratum_to_nmi() {
    for opcode in [0x57, 0x5F] {
        let (mut cpu, mut bus) = cpu_and_bus(opcode);
        assert_eq!(cpu.step(&mut bus), Ok(9), "ED {opcode:02X}");
        cpu.request_nmi();
        assert_eq!(cpu.step(&mut bus), Ok(11), "ED {opcode:02X}");
        assert_ne!(cpu.registers.f & FLAG_PV, 0, "ED {opcode:02X}");
        assert!(!cpu.export_architectural_state().p, "ED {opcode:02X}");
    }
}

#[test]
fn atomic_executor_consumes_ld_a_i_r_latch_on_the_next_normal_instruction() {
    for opcode in [0x57, 0x5F] {
        let (mut cpu, mut bus) = cpu_and_bus(opcode);
        assert_eq!(cpu.step(&mut bus), Ok(9), "ED {opcode:02X}");
        assert_eq!(cpu.step(&mut bus), Ok(4), "ED {opcode:02X}: NOP");
        assert!(!cpu.export_architectural_state().p, "ED {opcode:02X}");
        cpu.request_int(0xFF);
        assert_eq!(cpu.step(&mut bus), Ok(13), "ED {opcode:02X}");
        assert_ne!(cpu.registers.f & FLAG_PV, 0, "ED {opcode:02X}");
    }
}
