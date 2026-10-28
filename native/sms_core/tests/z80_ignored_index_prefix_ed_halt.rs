use sega_sms_core::{CpuBus, Z80};

struct TracingBus {
    memory: [u8; 0x10000],
    reads: Vec<u16>,
}

impl Default for TracingBus {
    fn default() -> Self {
        Self {
            memory: [0; 0x10000],
            reads: Vec::new(),
        }
    }
}

impl CpuBus for TracingBus {
    fn read_memory(&mut self, address: u16) -> u8 {
        self.reads.push(address);
        self.memory[usize::from(address)]
    }

    fn write_memory(&mut self, address: u16, value: u8) {
        self.memory[usize::from(address)] = value;
    }
}

#[test]
fn ignored_index_prefix_before_ed_fetches_each_opcode_byte_once() {
    for prefix in [0xDD, 0xFD] {
        let mut bus = TracingBus::default();
        bus.memory[..3].copy_from_slice(&[prefix, 0xED, 0x47]); // LD I,A

        let mut cpu = Z80::default();
        cpu.registers.a = 0xC7;
        cpu.registers.f = 0xA5;
        cpu.r = 0xFE;

        assert_eq!(cpu.step(&mut bus), Ok(13), "prefix {prefix:02X}");
        assert_eq!(cpu.i, 0xC7, "prefix {prefix:02X}");
        assert_eq!(cpu.registers.pc, 3, "prefix {prefix:02X}");
        assert_eq!(cpu.r, 0x81, "R keeps bit 7 and counts three M1 fetches");
        assert_eq!(cpu.registers.f, 0xA5, "LD I,A preserves flags");
        assert_eq!(cpu.total_tstates, 13, "prefix {prefix:02X}");
        assert_eq!(bus.reads, vec![0, 1, 2], "prefix {prefix:02X}");
    }
}

#[test]
fn ignored_index_prefix_before_halt_fetches_each_opcode_byte_once() {
    for prefix in [0xDD, 0xFD] {
        let mut bus = TracingBus::default();
        bus.memory[..2].copy_from_slice(&[prefix, 0x76]); // HALT

        let mut cpu = Z80::default();
        assert_eq!(cpu.step(&mut bus), Ok(8), "prefix {prefix:02X}");
        assert!(cpu.halted, "prefix {prefix:02X}");
        assert_eq!(cpu.registers.pc, 2, "prefix {prefix:02X}");
        assert_eq!(cpu.r, 2, "prefix {prefix:02X}");
        assert_eq!(bus.reads, vec![0, 1], "prefix {prefix:02X}");

        assert_eq!(cpu.step(&mut bus), Ok(4));
        assert_eq!(cpu.r, 3);
        assert_eq!(cpu.total_tstates, 12);
        assert_eq!(bus.reads, vec![0, 1], "HALT cycle does not fetch memory");
    }
}
