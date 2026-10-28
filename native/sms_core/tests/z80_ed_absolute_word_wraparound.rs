//! Boundary regressions for ED absolute 16-bit loads and stores.
//!
//! These cases verify little-endian access across address FFFF -> 0000 for
//! BC, DE, HL and SP, along with WZ wraparound, PC and nominal T-states.
//! They are directed boundary vectors, not cycle-ordered bus traces.
use sega_sms_core::{CpuBus, Z80};

struct FlatBus {
    memory: Box<[u8]>,
}

impl Default for FlatBus {
    fn default() -> Self {
        Self {
            memory: vec![0; 0x1_0000].into_boxed_slice(),
        }
    }
}

impl CpuBus for FlatBus {
    fn read_memory(&mut self, address: u16) -> u8 {
        self.memory[usize::from(address)]
    }

    fn write_memory(&mut self, address: u16, value: u8) {
        self.memory[usize::from(address)] = value;
    }
}

const PROGRAM: u16 = 0x0100;

fn prepare(bus: &mut FlatBus, opcode: u8) {
    bus.memory[usize::from(PROGRAM)..usize::from(PROGRAM) + 4]
        .copy_from_slice(&[0xED, opcode, 0xFF, 0xFF]);
}

fn set_pair(cpu: &mut Z80, pair_code: u8, value: u16) {
    match pair_code {
        0 => cpu.registers.set_bc(value),
        1 => cpu.registers.set_de(value),
        2 => cpu.registers.set_hl(value),
        3 => cpu.registers.sp = value,
        _ => unreachable!(),
    }
}

fn get_pair(cpu: &Z80, pair_code: u8) -> u16 {
    match pair_code {
        0 => cpu.registers.bc(),
        1 => cpu.registers.de(),
        2 => cpu.registers.hl(),
        3 => cpu.registers.sp,
        _ => unreachable!(),
    }
}

#[test]
fn ed_absolute_word_stores_wrap_little_endian_writes_at_ffff_to_0000() {
    let source_values = [0xA1B2_u16, 0xC3D4, 0xE5F6, 0x9786];
    let opcodes = [0x43_u8, 0x53, 0x63, 0x73];

    for (pair_code, (opcode, value)) in opcodes.into_iter().zip(source_values).enumerate() {
        let mut bus = FlatBus::default();
        prepare(&mut bus, opcode);

        let mut cpu = Z80::default();
        cpu.registers.pc = PROGRAM;
        for (code, other_value) in source_values.into_iter().enumerate() {
            set_pair(&mut cpu, code as u8, other_value);
        }
        set_pair(&mut cpu, pair_code as u8, value);
        cpu.wz = 0xABCD;

        assert_eq!(cpu.step(&mut bus), Ok(20), "ED {opcode:02X}");
        assert_eq!(bus.memory[0xFFFF], value as u8, "ED {opcode:02X} low byte");
        assert_eq!(
            bus.memory[0x0000],
            (value >> 8) as u8,
            "ED {opcode:02X} high byte"
        );
        assert_eq!(cpu.wz, 0x0000, "ED {opcode:02X} WZ wraparound");
        assert_eq!(cpu.registers.pc, PROGRAM + 4, "ED {opcode:02X} PC");
    }
}

#[test]
fn ed_absolute_word_loads_wrap_little_endian_reads_at_ffff_to_0000() {
    let opcodes = [0x4B_u8, 0x5B, 0x6B, 0x7B];
    let expected_values = [0x1357_u16, 0x2468, 0xA5C3, 0x7E91];

    for (pair_code, (opcode, value)) in opcodes.into_iter().zip(expected_values).enumerate() {
        let mut bus = FlatBus::default();
        prepare(&mut bus, opcode);
        bus.memory[0xFFFF] = value as u8;
        bus.memory[0x0000] = (value >> 8) as u8;

        let mut cpu = Z80::default();
        cpu.registers.pc = PROGRAM;
        for code in 0_u8..4 {
            set_pair(&mut cpu, code, 0xDEAD);
        }
        cpu.wz = 0xABCD;

        assert_eq!(cpu.step(&mut bus), Ok(20), "ED {opcode:02X}");
        assert_eq!(
            get_pair(&cpu, pair_code as u8),
            value,
            "ED {opcode:02X} destination pair"
        );
        assert_eq!(cpu.wz, 0x0000, "ED {opcode:02X} WZ wraparound");
        assert_eq!(cpu.registers.pc, PROGRAM + 4, "ED {opcode:02X} PC");
    }
}
