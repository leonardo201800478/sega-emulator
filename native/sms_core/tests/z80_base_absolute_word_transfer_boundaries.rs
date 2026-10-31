//! Boundary regression for the BASE Z80 instruction LD (nn),HL (0x22).
//!
//! The case checks 16-bit address wraparound, little-endian write order,
//! instruction-stream reads, architectural side effects and nominal T-states.
//! The recorded bus calls are logical accesses, not a per-T-state bus trace.

use sega_sms_core::{CpuBus, Z80};

struct TraceBus {
    memory: Box<[u8]>,
    reads: Vec<u16>,
    writes: Vec<(u16, u8)>,
}

impl Default for TraceBus {
    fn default() -> Self {
        Self {
            memory: vec![0; 0x1_0000].into_boxed_slice(),
            reads: Vec::new(),
            writes: Vec::new(),
        }
    }
}

impl CpuBus for TraceBus {
    fn read_memory(&mut self, address: u16) -> u8 {
        self.reads.push(address);
        self.memory[usize::from(address)]
    }

    fn write_memory(&mut self, address: u16, value: u8) {
        self.writes.push((address, value));
        self.memory[usize::from(address)] = value;
    }
}

#[test]
fn base_ld_nn_hl_wraps_little_endian_writes_from_ffff_to_0000() {
    const PROGRAM: u16 = 0x0100;

    let mut bus = TraceBus::default();
    bus.memory[usize::from(PROGRAM)..usize::from(PROGRAM) + 3]
        .copy_from_slice(&[0x22, 0xFF, 0xFF]);

    let mut cpu = Z80::default();
    cpu.registers.pc = PROGRAM;
    cpu.registers.sp = 0xBEEF;
    cpu.registers.set_hl(0xA1B2);
    cpu.registers.f = 0xD7;
    cpu.wz = 0xABCD;

    assert_eq!(cpu.step(&mut bus), Ok(16));
    assert_eq!(cpu.total_tstates, 16);

    assert_eq!(cpu.registers.pc, PROGRAM + 3);
    assert_eq!(cpu.registers.sp, 0xBEEF);
    assert_eq!(cpu.registers.hl(), 0xA1B2);
    assert_eq!(cpu.registers.f, 0xD7, "LD (nn),HL must preserve flags");
    assert_eq!(cpu.r & 0x7F, 1, "one opcode M1 fetch increments R once");
    assert_eq!(cpu.wz, 0x0000, "WZ = nn + 1 wraps at 16 bits");

    assert_eq!(bus.reads, [PROGRAM, PROGRAM + 1, PROGRAM + 2]);
    assert_eq!(
        bus.writes,
        [(0xFFFF, 0xB2), (0x0000, 0xA1)],
        "the low byte is written to nn before the high byte to nn + 1"
    );
    assert_eq!(bus.memory[0xFFFF], 0xB2);
    assert_eq!(bus.memory[0x0000], 0xA1);
}
