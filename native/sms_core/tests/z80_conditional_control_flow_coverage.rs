//! Conditional control-flow regression matrix for the SMS-owned Z80 core.
//!
//! Fixed expectations intentionally do not call the CPU condition helper or
//! derive expected PC/SP/timing from the implementation under test.
//! This is a deterministic semantic regression suite, not a hardware capture.

use sega_sms_core::{CpuBus, Z80};

const FLAG_C: u8 = 0x01;
const FLAG_PV: u8 = 0x04;
const FLAG_Z: u8 = 0x40;
const FLAG_S: u8 = 0x80;

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

fn flags_for_condition(code: u8, condition_true: bool) -> u8 {
    let (mask, set_means_true) = match code & 0x07 {
        0 => (FLAG_Z, false),  // NZ
        1 => (FLAG_Z, true),   // Z
        2 => (FLAG_C, false),  // NC
        3 => (FLAG_C, true),   // C
        4 => (FLAG_PV, false), // PO
        5 => (FLAG_PV, true),  // PE
        6 => (FLAG_S, false),  // P
        _ => (FLAG_S, true),   // M
    };

    let mut flags = 0x2A; // Preserve unrelated X/Y/N bits as discriminating state.
    if condition_true == set_means_true {
        flags |= mask;
    } else {
        flags &= !mask;
    }
    flags
}

const JR_OPCODES: [u8; 4] = [0x20, 0x28, 0x30, 0x38];
const JP_OPCODES: [u8; 8] = [0xC2, 0xCA, 0xD2, 0xDA, 0xE2, 0xEA, 0xF2, 0xFA];
const CALL_OPCODES: [u8; 8] = [0xC4, 0xCC, 0xD4, 0xDC, 0xE4, 0xEC, 0xF4, 0xFC];
const RET_OPCODES: [u8; 8] = [0xC0, 0xC8, 0xD0, 0xD8, 0xE0, 0xE8, 0xF0, 0xF8];

#[test]
fn conditional_jr_covers_taken_and_not_taken_paths() {
    for (condition, opcode) in JR_OPCODES.into_iter().enumerate() {
        for condition_true in [false, true] {
            let mut bus = TraceBus::default();
            bus.memory[..2].copy_from_slice(&[opcode, 0xFE]); // -2 from post-operand PC.
            let mut cpu = Z80::default();
            let flags = flags_for_condition(condition as u8, condition_true);
            cpu.registers.f = flags;

            let cycles = cpu.step(&mut bus).expect("conditional JR must execute");

            assert_eq!(
                cycles,
                if condition_true { 12 } else { 7 },
                "opcode={opcode:02X}"
            );
            assert_eq!(
                cpu.registers.pc,
                if condition_true { 0 } else { 2 },
                "opcode={opcode:02X}"
            );
            assert_eq!(
                cpu.registers.f, flags,
                "opcode={opcode:02X}: flags must be preserved"
            );
        }
    }
}

#[test]
fn conditional_jp_covers_all_conditions_without_changing_flags() {
    for (condition, opcode) in JP_OPCODES.into_iter().enumerate() {
        for condition_true in [false, true] {
            let mut bus = TraceBus::default();
            bus.memory[..3].copy_from_slice(&[opcode, 0x34, 0x12]);
            let mut cpu = Z80::default();
            let flags = flags_for_condition(condition as u8, condition_true);
            cpu.registers.f = flags;
            cpu.registers.sp = 0xBEEF;

            assert_eq!(cpu.step(&mut bus), Ok(10), "opcode={opcode:02X}");
            assert_eq!(
                cpu.registers.pc,
                if condition_true { 0x1234 } else { 3 },
                "opcode={opcode:02X}"
            );
            assert_eq!(
                cpu.registers.f, flags,
                "opcode={opcode:02X}: flags must be preserved"
            );
            assert_eq!(
                cpu.registers.sp, 0xBEEF,
                "opcode={opcode:02X}: JP must preserve SP"
            );
        }
    }
}

#[test]
fn conditional_call_covers_both_paths_and_stack_write_order() {
    for (condition, opcode) in CALL_OPCODES.into_iter().enumerate() {
        for condition_true in [false, true] {
            let mut bus = TraceBus::default();
            bus.memory[..3].copy_from_slice(&[opcode, 0x34, 0x12]);
            let mut cpu = Z80::default();
            cpu.registers.sp = 0x9000;
            let flags = flags_for_condition(condition as u8, condition_true);
            cpu.registers.f = flags;

            assert_eq!(
                cpu.step(&mut bus),
                Ok(if condition_true { 17 } else { 10 }),
                "opcode={opcode:02X}"
            );
            assert_eq!(
                cpu.registers.pc,
                if condition_true { 0x1234 } else { 3 },
                "opcode={opcode:02X}"
            );
            assert_eq!(
                cpu.registers.f, flags,
                "opcode={opcode:02X}: flags must be preserved"
            );
            if condition_true {
                assert_eq!(cpu.registers.sp, 0x8FFE, "opcode={opcode:02X}");
                assert_eq!(
                    bus.writes,
                    [(0x8FFF, 0x00), (0x8FFE, 0x03)],
                    "opcode={opcode:02X}"
                );
                assert_eq!(bus.memory[0x8FFE], 0x03);
                assert_eq!(bus.memory[0x8FFF], 0x00);
            } else {
                assert_eq!(cpu.registers.sp, 0x9000, "opcode={opcode:02X}");
                assert!(
                    bus.writes.is_empty(),
                    "opcode={opcode:02X}: untaken CALL must not write stack"
                );
            }
        }
    }
}

#[test]
fn conditional_ret_covers_both_paths_and_stack_read_order() {
    for (condition, opcode) in RET_OPCODES.into_iter().enumerate() {
        for condition_true in [false, true] {
            let mut bus = TraceBus::default();
            bus.memory[0] = opcode;
            bus.memory[0x8000] = 0x34;
            bus.memory[0x8001] = 0x12;
            let mut cpu = Z80::default();
            cpu.registers.sp = 0x8000;
            let flags = flags_for_condition(condition as u8, condition_true);
            cpu.registers.f = flags;

            assert_eq!(
                cpu.step(&mut bus),
                Ok(if condition_true { 11 } else { 5 }),
                "opcode={opcode:02X}"
            );
            assert_eq!(
                cpu.registers.pc,
                if condition_true { 0x1234 } else { 1 },
                "opcode={opcode:02X}"
            );
            assert_eq!(
                cpu.registers.f, flags,
                "opcode={opcode:02X}: flags must be preserved"
            );
            if condition_true {
                assert_eq!(cpu.registers.sp, 0x8002, "opcode={opcode:02X}");
                assert!(
                    bus.reads.ends_with(&[0x8000, 0x8001]),
                    "opcode={opcode:02X}: stack must be read low byte first"
                );
            } else {
                assert_eq!(cpu.registers.sp, 0x8000, "opcode={opcode:02X}");
                assert_eq!(
                    bus.reads,
                    [0],
                    "opcode={opcode:02X}: untaken RET must not read stack"
                );
            }
            assert!(bus.writes.is_empty());
        }
    }
}

#[test]
fn djnz_covers_taken_and_not_taken_paths_without_changing_flags() {
    for (initial_b, expected_b, expected_pc, expected_cycles) in [(2, 1, 0, 13), (1, 0, 2, 8)] {
        let mut bus = TraceBus::default();
        bus.memory[..2].copy_from_slice(&[0x10, 0xFE]);
        let mut cpu = Z80::default();
        cpu.registers.b = initial_b;
        let flags = 0xD7;
        cpu.registers.f = flags;

        assert_eq!(cpu.step(&mut bus), Ok(expected_cycles));
        assert_eq!(cpu.registers.b, expected_b);
        assert_eq!(cpu.registers.pc, expected_pc);
        assert_eq!(cpu.registers.f, flags);
    }
}

#[test]
fn conditional_jr_wraparound_and_signed_displacement_boundaries() {
    let cases = [
        (0x80_u8, 0xFF80_u16), // -128
        (0xFF, 0xFFFF),        // -1
        (0x00, 0x0000),        // 0
        (0x01, 0x0001),        // +1
        (0x7F, 0x007F),        // +127
    ];

    for (displacement, expected_pc) in cases {
        let mut bus = TraceBus::default();
        bus.memory[0xFFFE] = 0x20; // JR NZ,e
        bus.memory[0xFFFF] = displacement;
        let mut cpu = Z80::default();
        cpu.registers.pc = 0xFFFE;
        cpu.registers.f = 0; // NZ is true.

        assert_eq!(
            cpu.step(&mut bus),
            Ok(12),
            "displacement={displacement:02X}"
        );
        assert_eq!(
            cpu.registers.pc, expected_pc,
            "displacement={displacement:02X}"
        );
    }
}

#[test]
fn conditional_call_and_ret_wrap_stack_pointer_at_zero() {
    let mut call_bus = TraceBus::default();
    call_bus.memory[..3].copy_from_slice(&[0xC4, 0x34, 0x12]); // CALL NZ,nn
    let mut call_cpu = Z80::default();
    call_cpu.registers.sp = 0x0000;
    call_cpu.registers.f = 0;
    assert_eq!(call_cpu.step(&mut call_bus), Ok(17));
    assert_eq!(call_cpu.registers.pc, 0x1234);
    assert_eq!(call_cpu.registers.sp, 0xFFFE);
    assert_eq!(call_bus.writes, [(0xFFFF, 0x00), (0xFFFE, 0x03)]);
    assert_eq!(call_bus.memory[0xFFFE], 0x03);
    assert_eq!(call_bus.memory[0xFFFF], 0x00);

    let mut ret_bus = TraceBus::default();
    ret_bus.memory[0] = 0xC0; // RET NZ
    ret_bus.memory[0xFFFE] = 0x78;
    ret_bus.memory[0xFFFF] = 0x56;
    let mut ret_cpu = Z80::default();
    ret_cpu.registers.sp = 0xFFFE;
    ret_cpu.registers.f = 0;
    assert_eq!(ret_cpu.step(&mut ret_bus), Ok(11));
    assert_eq!(ret_cpu.registers.pc, 0x5678);
    assert_eq!(ret_cpu.registers.sp, 0x0000);
    assert!(ret_bus.reads.ends_with(&[0xFFFE, 0xFFFF]));
}
