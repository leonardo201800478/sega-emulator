//! E4B indexed-memory ALU execution through the shared Z80 microsequencer.

use sega_z80_cpu::{
    Z80BusOperation, Z80BusRequest, Z80MicroSequencer, Z80MicroSequencerPhase,
    Z80MicroSequencerState, Z80MicroStep,
};

fn request(step: Z80MicroStep) -> Z80BusRequest {
    match step {
        Z80MicroStep::BusRequest(request) => request,
        other => panic!("expected bus request, got {other:?}"),
    }
}

fn start(cpu: &mut Z80MicroSequencer) -> Z80BusRequest {
    request(cpu.begin_instruction().expect("instruction starts"))
}

fn feed_prefix(cpu: &mut Z80MicroSequencer, current: Z80BusRequest, byte: u8) -> Z80BusRequest {
    request(
        cpu.complete_bus_cycle(current.id, Some(byte))
            .expect("prefix schedules the next M1 fetch"),
    )
}

fn round_trip_at(cpu: Z80MicroSequencer, stage: &str) -> Z80MicroSequencer {
    let encoded = cpu
        .export_state()
        .encode()
        .unwrap_or_else(|error| panic!("snapshot at {stage} encodes: {error:?}"));
    let decoded = Z80MicroSequencerState::decode(&encoded)
        .unwrap_or_else(|error| panic!("snapshot at {stage} decodes: {error:?}"));
    Z80MicroSequencer::restore_state(decoded)
        .unwrap_or_else(|error| panic!("snapshot at {stage} restores: {error:?}"))
}

struct IndexedAluCase<'a> {
    prefixes: &'a [u8],
    ix: u16,
    iy: u16,
    opcode: u8,
    accumulator: u8,
    flags: u8,
    displacement: u8,
    operand: u8,
    waits: &'a [u32],
}

fn execute_indexed_alu(case: IndexedAluCase<'_>) -> Z80MicroSequencer {
    let IndexedAluCase {
        prefixes,
        ix,
        iy,
        opcode,
        accumulator,
        flags,
        displacement,
        operand,
        waits,
    } = case;
    assert_eq!(waits.len(), prefixes.len() + 3);

    let mut cpu = Z80MicroSequencer::default();
    cpu.ix = ix;
    cpu.iy = iy;
    cpu.registers.a = accumulator;
    cpu.registers.f = flags;

    let mut fetch = start(&mut cpu);
    for (index, prefix) in prefixes.iter().copied().enumerate() {
        if waits[index] != 0 {
            cpu.insert_wait_tstates(waits[index]).unwrap();
            cpu = round_trip_at(cpu, "prefix M1 WAIT");
        }
        fetch = feed_prefix(&mut cpu, fetch, prefix);
    }

    let opcode_wait = prefixes.len();
    if waits[opcode_wait] != 0 {
        cpu.insert_wait_tstates(waits[opcode_wait]).unwrap();
        cpu = round_trip_at(cpu, "opcode M1 WAIT");
    }
    let displacement_request = request(cpu.complete_bus_cycle(fetch.id, Some(opcode)).unwrap());
    assert_eq!(displacement_request.operation, Z80BusOperation::MemoryRead);
    assert_eq!(displacement_request.address, prefixes.len() as u16 + 1);
    assert_eq!(displacement_request.base_tstates, 3);

    let displacement_wait = prefixes.len() + 1;
    if waits[displacement_wait] != 0 {
        cpu.insert_wait_tstates(waits[displacement_wait]).unwrap();
        cpu = round_trip_at(cpu, "displacement WAIT");
    }
    assert_eq!(
        cpu.complete_bus_cycle(displacement_request.id, Some(displacement))
            .unwrap(),
        Z80MicroStep::InternalCycle { tstates: 1 }
    );
    cpu = round_trip_at(cpu, "after displacement completion");

    let mut data_read_step = Z80MicroStep::InternalCycle { tstates: 1 };
    for _ in 0..5 {
        data_read_step = cpu.complete_internal_cycle().unwrap();
        cpu = round_trip_at(cpu, "indexed ALU internal T-state");
    }
    let data_read = request(data_read_step);
    let last_prefix = prefixes.last().copied().expect("at least one index prefix");
    let index_base = if last_prefix == 0xDD { ix } else { iy };
    let effective_address = index_base.wrapping_add_signed(i16::from(displacement as i8));
    assert_eq!(data_read.operation, Z80BusOperation::MemoryRead);
    assert_eq!(data_read.address, effective_address);
    assert_eq!(data_read.base_tstates, 3);
    assert_eq!(
        cpu.export_state().phase,
        Z80MicroSequencerPhase::MemoryReadAlu {
            operation: (opcode >> 3) & 0x07,
            address: effective_address,
        }
    );
    assert_eq!(cpu.export_state().mcycle_index, prefixes.len() as u32 + 3);
    assert_eq!(cpu.wz, effective_address);
    assert_eq!(cpu.registers.pc, prefixes.len() as u16 + 2);
    assert_eq!(cpu.refresh & 0x7F, prefixes.len() as u8 + 1);

    let data_wait = prefixes.len() + 2;
    if waits[data_wait] != 0 {
        cpu.insert_wait_tstates(waits[data_wait]).unwrap();
        cpu = round_trip_at(cpu, "effective-address read WAIT");
    }
    let wait_tstates: u32 = waits.iter().sum();
    let nominal_tstates = 19 + 4 * (prefixes.len() as u32 - 1);
    assert_eq!(
        cpu.complete_bus_cycle(data_read.id, Some(operand)).unwrap(),
        Z80MicroStep::InstructionRetired {
            opcode,
            tstates: nominal_tstates + wait_tstates,
        }
    );
    cpu
}

#[test]
fn indexed_alu_matrix_matches_fixed_arithmetic_and_flag_vectors_for_ix_and_iy() {
    let vectors = [
        // ADD: signed overflow.
        (0x86, 0x7F, 0xA5, 0x01, 0x80, 0x94),
        // ADD: undocumented X/Y bits follow the result.
        (0x86, 0x20, 0x00, 0x08, 0x28, 0x28),
        // ADC: carry-in participates in overflow.
        (0x8E, 0x7F, 0x01, 0x00, 0x80, 0x94),
        // ADC: carry-in participates in the low-nibble sum.
        (0x8E, 0x20, 0x01, 0x07, 0x28, 0x28),
        // SUB: borrow, half-borrow, sign and X/Y.
        (0x96, 0x00, 0x00, 0x01, 0xFF, 0xBB),
        // SUB: X/Y from the subtraction result.
        (0x96, 0x38, 0x00, 0x10, 0x28, 0x2A),
        // SBC: borrow-in, half-borrow and signed overflow.
        (0x9E, 0x80, 0x01, 0x00, 0x7F, 0x3E),
        // SBC: half-borrow with no full borrow.
        (0x9E, 0x38, 0x01, 0x0F, 0x28, 0x3A),
        // AND: Z, H and parity.
        (0xA6, 0xF0, 0xA5, 0x0F, 0x00, 0x54),
        // AND: X/Y from result.
        (0xA6, 0x28, 0x00, 0x28, 0x28, 0x3C),
        // XOR: zero and even parity.
        (0xAE, 0xAA, 0xA5, 0xAA, 0x00, 0x44),
        // XOR: X/Y from result.
        (0xAE, 0x20, 0x00, 0x08, 0x28, 0x2C),
        // OR: sign and parity.
        (0xB6, 0x80, 0xA5, 0x01, 0x81, 0x84),
        // OR: X/Y from result.
        (0xB6, 0x20, 0x00, 0x08, 0x28, 0x2C),
        // CP: X/Y are copied from the operand, not the subtraction result.
        (0xBE, 0x10, 0x00, 0x01, 0x10, 0x12),
        // CP: sign, half-borrow, carry and operand X/Y.
        (0xBE, 0x10, 0x00, 0x28, 0x10, 0xBB),
    ];

    for prefix in [0xDD, 0xFD] {
        for (opcode, accumulator, flags, operand, expected_a, expected_f) in vectors {
            let cpu = execute_indexed_alu(IndexedAluCase {
                prefixes: &[prefix],
                ix: 0x2002,
                iy: 0x3002,
                opcode,
                accumulator,
                flags,
                displacement: 0xFE,
                operand,
                waits: &[0, 0, 0, 0],
            });
            assert_eq!(
                cpu.registers.a, expected_a,
                "prefix {prefix:02X}, opcode {opcode:02X}"
            );
            assert_eq!(
                cpu.registers.f, expected_f,
                "prefix {prefix:02X}, opcode {opcode:02X}"
            );
            assert_eq!(cpu.registers.pc, 3);
            assert_eq!(cpu.wz, if prefix == 0xDD { 0x2000 } else { 0x3000 });
        }
    }
}

#[test]
fn indexed_alu_wait_states_are_additive_and_snapshots_resume_every_bus_phase() {
    for prefix in [0xDD, 0xFD] {
        let cpu = execute_indexed_alu(IndexedAluCase {
            prefixes: &[prefix],
            ix: 0x1002,
            iy: 0x2002,
            opcode: 0x86,
            accumulator: 0x20,
            flags: 0x00,
            displacement: 0xFE,
            operand: 0x08,
            waits: &[1, 2, 3, 4],
        });
        assert_eq!(cpu.registers.a, 0x28);
        assert_eq!(cpu.registers.f, 0x28);
        assert_eq!(cpu.registers.pc, 3);
    }
}

#[test]
fn indexed_alu_repeated_and_alternating_prefixes_use_last_index_register() {
    for prefixes in [
        vec![0xDD, 0xDD],
        vec![0xFD, 0xFD],
        vec![0xDD, 0xFD],
        vec![0xFD, 0xDD],
        vec![0xDD, 0xFD, 0xDD],
        vec![0xFD, 0xDD, 0xFD],
    ] {
        let waits = vec![0; prefixes.len() + 3];
        let cpu = execute_indexed_alu(IndexedAluCase {
            prefixes: &prefixes,
            ix: 0x1002,
            iy: 0x2002,
            opcode: 0x86,
            accumulator: 0x10,
            flags: 0x00,
            displacement: 0xFE,
            operand: 0x02,
            waits: &waits,
        });
        assert_eq!(cpu.registers.a, 0x12);
        assert_eq!(cpu.registers.f, 0x00);
        assert_eq!(
            cpu.wz,
            if prefixes.last() == Some(&0xDD) {
                0x1000
            } else {
                0x2000
            }
        );
        assert_eq!(cpu.registers.pc, prefixes.len() as u16 + 2);
    }
}
