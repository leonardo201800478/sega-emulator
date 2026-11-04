//! E5 DDCB/FDCB microsequencer coverage.
//!
//! Every outstanding bus request is snapshot/restored before completion,
//! exercising resumable safe points independently of the atomic executor.

use sega_z80_cpu::{
    Z80BusOperation, Z80BusRequest, Z80MicroSequencer, Z80MicroSequencerPhase,
    Z80MicroSequencerState, Z80MicroStep,
};

const FLAG_C: u8 = 0x01;
const FLAG_H: u8 = 0x10;
const FLAG_PV: u8 = 0x04;
const FLAG_S: u8 = 0x80;
const FLAG_XY: u8 = 0x28;
const FLAG_Z: u8 = 0x40;

#[derive(Debug, Clone, PartialEq, Eq)]
struct BusTrace {
    operation: Z80BusOperation,
    address: u16,
    base_tstates: u8,
}

struct RunResult {
    cpu: Z80MicroSequencer,
    memory: Vec<u8>,
    writes: Vec<(u16, u8)>,
    trace: Vec<BusTrace>,
    tstates: u32,
}

fn request(step: Z80MicroStep) -> Z80BusRequest {
    match step {
        Z80MicroStep::BusRequest(request) => request,
        other => panic!("expected bus request, got {other:?}"),
    }
}

fn round_trip(cpu: Z80MicroSequencer, stage: &str) -> Z80MicroSequencer {
    let encoded = cpu.export_state().encode()
        .unwrap_or_else(|error| panic!("snapshot at {stage} encodes: {error:?}"));
    let decoded = Z80MicroSequencerState::decode(&encoded)
        .unwrap_or_else(|error| panic!("snapshot at {stage} decodes: {error:?}"));
    Z80MicroSequencer::restore_state(decoded)
        .unwrap_or_else(|error| panic!("snapshot at {stage} restores: {error:?}"))
}

fn run_indexed_cb(
    prefixes: &[u8], ix: u16, iy: u16, pc: u16, displacement: u8,
    opcode: u8, old_value: u8, flags: u8, waits: &[u32],
) -> RunResult {
    assert!(!prefixes.is_empty());
    assert!(prefixes.iter().all(|prefix| matches!(prefix, 0xDD | 0xFD)));

    let mut cpu = Z80MicroSequencer::default();
    cpu.ix = ix;
    cpu.iy = iy;
    cpu.refresh = 0x80;
    cpu.registers.pc = pc;
    cpu.registers.f = flags;
    cpu.registers.a = 0x11;
    cpu.registers.b = 0x22;
    cpu.registers.c = 0x33;
    cpu.registers.d = 0x44;
    cpu.registers.e = 0x55;
    cpu.registers.h = 0x66;
    cpu.registers.l = 0x77;

    let mut memory = vec![0; 0x1_0000];
    for (offset, prefix) in prefixes.iter().copied().enumerate() {
        memory[pc.wrapping_add(offset as u16) as usize] = prefix;
    }
    let cb_address = pc.wrapping_add(prefixes.len() as u16);
    memory[cb_address as usize] = 0xCB;
    let displacement_address = cb_address.wrapping_add(1);
    memory[displacement_address as usize] = displacement;
    let opcode_address = displacement_address.wrapping_add(1);
    memory[opcode_address as usize] = opcode;

    let index_base = if prefixes.last() == Some(&0xDD) { ix } else { iy };
    let effective_address = index_base.wrapping_add_signed(i16::from(displacement as i8));
    memory[effective_address as usize] = old_value;

    let mut writes = Vec::new();
    let mut trace = Vec::new();
    let mut step = cpu.begin_instruction().unwrap();
    let mut pending = request(step);
    let mut request_index = 0_usize;

    loop {
        cpu = round_trip(cpu, "pending indexed-CB bus request");
        let wait = waits.get(request_index).copied().unwrap_or(0);
        if wait != 0 {
            cpu.insert_wait_tstates(wait).unwrap();
            cpu = round_trip(cpu, "indexed-CB WAIT pending");
        }

        trace.push(BusTrace {
            operation: pending.operation.clone(),
            address: pending.address,
            base_tstates: pending.base_tstates,
        });
        let data = match pending.operation.clone() {
            Z80BusOperation::OpcodeFetch | Z80BusOperation::MemoryRead => {
                Some(memory[pending.address as usize])
            }
            Z80BusOperation::MemoryWrite { value } => {
                memory[pending.address as usize] = value;
                writes.push((pending.address, value));
                None
            }
            other => panic!("unexpected indexed-CB bus operation: {other:?}"),
        };

        step = cpu.complete_bus_cycle(pending.id, data).unwrap();
        request_index += 1;
        match step {
            Z80MicroStep::BusRequest(next) => pending = next,
            Z80MicroStep::InstructionRetired { opcode: retired, tstates } => {
                assert_eq!(retired, opcode);
                return RunResult { cpu, memory, writes, trace, tstates };
            }
            other => panic!("indexed-CB must remain bus-suspended, got {other:?}"),
        }
    }
}

fn expected_result(opcode: u8, value: u8, flags: u8, address: u16) -> (u8, u8, bool) {
    let group = opcode >> 6;
    let bit = (opcode >> 3) & 7;
    match group {
        0 => {
            let carry_in = flags & FLAG_C != 0;
            let (result, carry) = match bit {
                0 => (value.rotate_left(1), value & 0x80 != 0),
                1 => (value.rotate_right(1), value & 0x01 != 0),
                2 => ((value << 1) | u8::from(carry_in), value & 0x80 != 0),
                3 => ((value >> 1) | (u8::from(carry_in) << 7), value & 0x01 != 0),
                4 => (value << 1, value & 0x80 != 0),
                5 => ((value >> 1) | (value & 0x80), value & 0x01 != 0),
                6 => ((value << 1) | 1, value & 0x80 != 0),
                _ => (value >> 1, value & 0x01 != 0),
            };
            let mut next_flags = result & (FLAG_S | FLAG_XY);
            if result == 0 { next_flags |= FLAG_Z; }
            if result.count_ones().is_multiple_of(2) { next_flags |= FLAG_PV; }
            if carry { next_flags |= FLAG_C; }
            (result, next_flags, true)
        }
        1 => {
            let bit_clear = value & (1 << bit) == 0;
            let mut next_flags = (flags & FLAG_C) | FLAG_H | ((address >> 8) as u8 & FLAG_XY);
            if bit_clear { next_flags |= FLAG_Z | FLAG_PV; }
            if bit == 7 && !bit_clear { next_flags |= FLAG_S; }
            (value, next_flags, false)
        }
        2 => (value & !(1 << bit), flags, true),
        _ => (value | (1 << bit), flags, true),
    }
}

fn register_value(cpu: &Z80MicroSequencer, code: u8) -> u8 {
    match code {
        0 => cpu.registers.b, 1 => cpu.registers.c, 2 => cpu.registers.d,
        3 => cpu.registers.e, 4 => cpu.registers.h, 5 => cpu.registers.l,
        7 => cpu.registers.a, _ => panic!("code 6 is memory, not a register"),
    }
}

#[test]
fn all_256_indexed_cb_subopcodes_are_executed_for_ix_and_iy() {
    for prefix in [0xDD, 0xFD] {
        for opcode in 0_u8..=u8::MAX {
            let ix = 0x2100;
            let iy = 0x3200;
            let displacement = 0xFE;
            let address = if prefix == 0xDD { ix } else { iy }.wrapping_add_signed(-2);
            let old_value = 0x96;
            let initial_flags = 0xA5;
            let result = run_indexed_cb(
                &[prefix], ix, iy, 0x1000, displacement, opcode,
                old_value, initial_flags, &[],
            );
            let (expected_memory, expected_flags, writes_memory) =
                expected_result(opcode, old_value, initial_flags, address);

            assert_eq!(result.cpu.registers.pc, 0x1004, "prefix={prefix:02X} opcode={opcode:02X}");
            assert_eq!(result.cpu.refresh, 0x82, "prefix={prefix:02X} opcode={opcode:02X}");
            assert_eq!(result.cpu.wz, address, "prefix={prefix:02X} opcode={opcode:02X}");
            assert_eq!(result.cpu.registers.f, expected_flags, "prefix={prefix:02X} opcode={opcode:02X}");
            assert_eq!(result.cpu.ix, ix);
            assert_eq!(result.cpu.iy, iy);
            assert_eq!(result.memory[address as usize], expected_memory);

            if writes_memory {
                assert_eq!(result.writes, vec![(address, expected_memory)]);
                if opcode >> 6 != 1 && opcode & 7 != 6 {
                    assert_eq!(register_value(&result.cpu, opcode & 7), expected_memory,
                        "destination copy: prefix={prefix:02X} opcode={opcode:02X}");
                }
            } else {
                assert!(result.writes.is_empty(), "BIT must not write: {prefix:02X} {opcode:02X}");
                assert_eq!(result.memory[address as usize], old_value);
            }

            let expected_tstates = if opcode >> 6 == 1 { 20 } else { 23 };
            assert_eq!(result.tstates, expected_tstates, "prefix={prefix:02X} opcode={opcode:02X}");
            assert_eq!(result.trace.len(), if opcode >> 6 == 1 { 5 } else { 6 });
            assert_eq!(result.trace[0].operation, Z80BusOperation::OpcodeFetch);
            assert_eq!(result.trace[0].base_tstates, 4);
            assert_eq!(result.trace[1].operation, Z80BusOperation::OpcodeFetch);
            assert_eq!(result.trace[1].base_tstates, 4);
            assert_eq!(result.trace[2].operation, Z80BusOperation::MemoryRead);
            assert_eq!(result.trace[2].base_tstates, 3);
            assert_eq!(result.trace[3].operation, Z80BusOperation::MemoryRead);
            assert_eq!(result.trace[3].base_tstates, 5);
            assert_eq!(result.trace[4].operation, Z80BusOperation::MemoryRead);
            assert_eq!(result.trace[4].base_tstates, 4);
            if opcode >> 6 != 1 {
                assert_eq!(result.trace[5].operation, Z80BusOperation::MemoryWrite { value: expected_memory });
                assert_eq!(result.trace[5].base_tstates, 3);
            }
        }
    }
}

#[test]
fn indexed_cb_register_destinations_use_h_and_l_not_index_halves() {
    for (prefix, opcode) in [(0xDD, 0x04), (0xFD, 0x0D), (0xDD, 0x84), (0xFD, 0xAD)] {
        let ix = 0x1234;
        let iy = 0x5678;
        let result = run_indexed_cb(&[prefix], ix, iy, 0x2000, 0, opcode, 0x81, 0x01, &[]);
        let address = if prefix == 0xDD { ix } else { iy };
        let (value, _, _) = expected_result(opcode, 0x81, 0x01, address);
        assert_eq!(result.cpu.ix, ix);
        assert_eq!(result.cpu.iy, iy);
        assert_eq!(register_value(&result.cpu, opcode & 7), value);
    }
}

#[test]
fn indexed_cb_waits_are_additive_and_snapshots_resume_each_bus_phase() {
    for (opcode, waits) in [
        (0x46, [1, 2, 3, 4, 5, 0]),
        (0x06, [1, 2, 3, 4, 5, 6]),
        (0x86, [1, 2, 3, 4, 5, 6]),
        (0xC6, [1, 2, 3, 4, 5, 6]),
    ] {
        let result = run_indexed_cb(&[0xDD], 0x4000, 0x5000, 0x1000, 0xFF, opcode, 0x80, 0x01, &waits);
        let base = if opcode >> 6 == 1 { 20 } else { 23 };
        let wait_total: u32 = waits.iter().take(result.trace.len()).copied().sum();
        assert_eq!(result.tstates, base + wait_total);
        assert_eq!(result.trace.len(), if opcode >> 6 == 1 { 5 } else { 6 });
    }
}

#[test]
fn indexed_cb_repeated_prefixes_use_last_index_and_count_only_m1_refreshes() {
    for prefixes in [
        vec![0xDD, 0xDD], vec![0xFD, 0xFD], vec![0xDD, 0xFD],
        vec![0xFD, 0xDD], vec![0xDD, 0xFD, 0xDD], vec![0xFD, 0xDD, 0xFD],
    ] {
        let ix = 0x1000;
        let iy = 0x2000;
        let result = run_indexed_cb(&prefixes, ix, iy, 0x3000, 0xFF, 0x06, 0x81, 0, &[]);
        let expected_base = if prefixes.last() == Some(&0xDD) { ix } else { iy };
        assert_eq!(result.cpu.wz, expected_base.wrapping_sub(1));
        assert_eq!(result.cpu.refresh, 0x80 | (prefixes.len() as u8 + 1));
        assert_eq!(result.cpu.registers.pc, 0x3000_u16.wrapping_add(prefixes.len() as u16 + 3));
        assert_eq!(result.tstates, 23 + 4 * (prefixes.len() as u32 - 1));
    }
}

#[test]
fn indexed_cb_wraps_effective_address_and_program_counter() {
    let result = run_indexed_cb(&[0xFD], 0, 0xFFFF, 0xFFFC, 0x01, 0xC6, 0x00, 0, &[]);
    assert_eq!(result.cpu.wz, 0x0000);
    assert_eq!(result.cpu.registers.pc, 0x0000);
    assert_eq!(result.cpu.refresh, 0x82);
    assert_eq!(result.tstates, 23);
}

#[test]
fn indexed_cb_bit_is_read_only_and_preserves_memory_for_all_z_fields() {
    for prefix in [0xDD, 0xFD] {
        for bit in 0_u8..8 {
            for destination in 0_u8..8 {
                let opcode = 0x40 | (bit << 3) | destination;
                let result = run_indexed_cb(
                    &[prefix], 0x6000, 0x7000, 0x1000, 0x00, opcode, 0xA5, 0x01, &[],
                );
                assert!(result.writes.is_empty(), "BIT write: {prefix:02X} {opcode:02X}");
                assert_eq!(result.memory[result.cpu.wz as usize], 0xA5);
                assert_eq!(result.tstates, 20);
            }
        }
    }
}

#[test]
fn indexed_cb_phase_snapshots_keep_pending_requests_and_phase_invariants() {
    let result = run_indexed_cb(
        &[0xDD], 0xABFF, 0xCD00, 0x1000, 0x01, 0x46, 0x80, 0x01, &[1, 1, 1, 1, 1],
    );
    assert_eq!(result.trace.len(), 5);
    assert_eq!(result.cpu.export_state().phase, Z80MicroSequencerPhase::Ready);
}
