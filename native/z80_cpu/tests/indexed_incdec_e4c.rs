//! E4C indexed-memory INC/DEC read-modify-write sequencing.

use sega_z80_cpu::{
    Z80BusOperation, Z80BusRequest, Z80MicroSequencer, Z80MicroSequencerPhase,
    Z80MicroSequencerState, Z80MicroStep,
};

struct IndexedIncDecCase<'a> {
    prefixes: &'a [u8],
    ix: u16,
    iy: u16,
    pc: u16,
    opcode: u8,
    flags: u8,
    displacement: u8,
    old_value: u8,
    expected_value: u8,
    waits: &'a [u32],
}

fn request(step: Z80MicroStep) -> Z80BusRequest {
    match step {
        Z80MicroStep::BusRequest(request) => request,
        other => panic!("expected bus request, got {other:?}"),
    }
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

fn execute_indexed_inc_dec(case: IndexedIncDecCase<'_>) -> Z80MicroSequencer {
    let IndexedIncDecCase {
        prefixes,
        ix,
        iy,
        pc,
        opcode,
        flags,
        displacement,
        old_value,
        expected_value,
        waits,
    } = case;
    assert!(matches!(opcode, 0x34 | 0x35));
    assert_eq!(waits.len(), prefixes.len() + 4);

    let mut cpu = Z80MicroSequencer::default();
    cpu.ix = ix;
    cpu.iy = iy;
    cpu.registers.pc = pc;
    cpu.registers.f = flags;

    let mut fetch = request(cpu.begin_instruction().unwrap());
    for (index, prefix) in prefixes.iter().copied().enumerate() {
        if waits[index] != 0 {
            cpu.insert_wait_tstates(waits[index]).unwrap();
            cpu = round_trip_at(cpu, "prefix M1 WAIT");
        }
        fetch = request(cpu.complete_bus_cycle(fetch.id, Some(prefix)).unwrap());
    }

    let opcode_wait = prefixes.len();
    if waits[opcode_wait] != 0 {
        cpu.insert_wait_tstates(waits[opcode_wait]).unwrap();
        cpu = round_trip_at(cpu, "opcode M1 WAIT");
    }
    let displacement_request = request(cpu.complete_bus_cycle(fetch.id, Some(opcode)).unwrap());
    assert_eq!(displacement_request.operation, Z80BusOperation::MemoryRead);
    assert_eq!(
        displacement_request.address,
        pc.wrapping_add(prefixes.len() as u16 + 1)
    );
    assert_eq!(displacement_request.base_tstates, 3);

    let displacement_wait = prefixes.len() + 1;
    if waits[displacement_wait] != 0 {
        cpu.insert_wait_tstates(waits[displacement_wait]).unwrap();
        cpu = round_trip_at(cpu, "displacement read WAIT");
    }
    assert_eq!(
        cpu.complete_bus_cycle(displacement_request.id, Some(displacement))
            .unwrap(),
        Z80MicroStep::InternalCycle { tstates: 1 }
    );
    cpu = round_trip_at(cpu, "after displacement read");

    let mut read_step = Z80MicroStep::InternalCycle { tstates: 1 };
    for _ in 0..5 {
        read_step = cpu.complete_internal_cycle().unwrap();
        cpu = round_trip_at(cpu, "indexed displacement internal T-state");
    }
    let read = request(read_step);
    let index_base = if prefixes.last() == Some(&0xDD) {
        ix
    } else {
        iy
    };
    let effective_address = index_base.wrapping_add_signed(i16::from(displacement as i8));
    let decrement = opcode == 0x35;
    assert_eq!(read.operation, Z80BusOperation::MemoryRead);
    assert_eq!(read.address, effective_address);
    assert_eq!(read.base_tstates, 3);
    assert_eq!(
        cpu.export_state().phase,
        Z80MicroSequencerPhase::MemoryReadIncDec {
            decrement,
            address: effective_address,
        }
    );
    assert_eq!(cpu.export_state().mcycle_index, prefixes.len() as u32 + 3);
    assert_eq!(cpu.wz, effective_address);
    assert_eq!(cpu.registers.pc, pc.wrapping_add(prefixes.len() as u16 + 2));
    assert_eq!(cpu.refresh & 0x7F, prefixes.len() as u8 + 1);

    let read_wait = prefixes.len() + 2;
    if waits[read_wait] != 0 {
        cpu.insert_wait_tstates(waits[read_wait]).unwrap();
        cpu = round_trip_at(cpu, "effective-address read WAIT");
    }
    assert_eq!(
        cpu.complete_bus_cycle(read.id, Some(old_value)).unwrap(),
        Z80MicroStep::InternalCycle { tstates: 1 }
    );
    cpu = round_trip_at(cpu, "read-modify-write internal T-state");
    assert_eq!(
        cpu.export_state().phase,
        Z80MicroSequencerPhase::MemoryIncDecInternal {
            decrement,
            old: old_value,
            address: effective_address,
        }
    );

    let write = request(cpu.complete_internal_cycle().unwrap());
    cpu = round_trip_at(cpu, "effective-address write pending");
    let computed_value = if decrement {
        old_value.wrapping_sub(1)
    } else {
        old_value.wrapping_add(1)
    };
    assert_eq!(computed_value, expected_value);
    assert_eq!(
        write.operation,
        Z80BusOperation::MemoryWrite {
            value: expected_value
        }
    );
    assert_eq!(write.address, effective_address);
    assert_eq!(write.base_tstates, 3);
    assert_eq!(cpu.export_state().mcycle_index, prefixes.len() as u32 + 4);

    let write_wait = prefixes.len() + 3;
    if waits[write_wait] != 0 {
        cpu.insert_wait_tstates(waits[write_wait]).unwrap();
        cpu = round_trip_at(cpu, "effective-address write WAIT");
    }
    let wait_tstates: u32 = waits.iter().sum();
    let nominal_tstates = 23 + 4 * (prefixes.len() as u32 - 1);
    assert_eq!(
        cpu.complete_bus_cycle(write.id, None).unwrap(),
        Z80MicroStep::InstructionRetired {
            opcode,
            tstates: nominal_tstates + wait_tstates,
        }
    );
    cpu
}

#[test]
fn indexed_inc_dec_vectors_cover_flags_and_carry_preservation_for_ix_and_iy() {
    let vectors = [
        // INC 7F -> 80: S, H and signed overflow; Carry preserved.
        (0x34, 0x7F, 0x01, 0x80, 0x95),
        // INC FF -> 00: Z and H; Carry preserved.
        (0x34, 0xFF, 0x01, 0x00, 0x51),
        // INC 27 -> 28: undocumented X/Y from result.
        (0x34, 0x27, 0x00, 0x28, 0x28),
        // DEC 80 -> 7F: N, H, signed overflow and X/Y; Carry preserved.
        (0x35, 0x80, 0x01, 0x7F, 0x3F),
        // DEC 00 -> FF: S, N, H and X/Y; Carry preserved.
        (0x35, 0x00, 0x01, 0xFF, 0xBB),
        // DEC 28 -> 27: N and undocumented Y.
        (0x35, 0x28, 0x00, 0x27, 0x22),
        // DEC 01 -> 00: Z and N.
        (0x35, 0x01, 0x00, 0x00, 0x42),
    ];

    for prefix in [0xDD, 0xFD] {
        for (opcode, old_value, flags, expected_value, expected_flags) in vectors {
            let cpu = execute_indexed_inc_dec(IndexedIncDecCase {
                prefixes: &[prefix],
                ix: 0x2002,
                iy: 0x3002,
                pc: 0,
                opcode,
                flags,
                displacement: 0xFE,
                old_value,
                expected_value,
                waits: &[0, 0, 0, 0, 0],
            });
            assert_eq!(
                cpu.registers.f, expected_flags,
                "prefix {prefix:02X}, opcode {opcode:02X}"
            );
            assert_eq!(cpu.wz, if prefix == 0xDD { 0x2000 } else { 0x3000 });
            assert_eq!(cpu.registers.pc, 3);
            assert_eq!(cpu.refresh & 0x7F, 2);
        }
    }
}

#[test]
fn indexed_inc_dec_wait_states_are_additive_and_snapshots_resume_each_bus_phase() {
    for prefix in [0xDD, 0xFD] {
        let cpu = execute_indexed_inc_dec(IndexedIncDecCase {
            prefixes: &[prefix],
            ix: 0x1002,
            iy: 0x2002,
            pc: 0,
            opcode: 0x34,
            flags: 0x01,
            displacement: 0xFE,
            old_value: 0x7F,
            expected_value: 0x80,
            waits: &[1, 2, 3, 4, 5],
        });
        assert_eq!(cpu.registers.f, 0x95);
    }
}

#[test]
fn indexed_inc_dec_repeated_and_alternating_prefixes_use_last_index_register() {
    for prefixes in [
        vec![0xDD, 0xDD],
        vec![0xFD, 0xFD],
        vec![0xDD, 0xFD],
        vec![0xFD, 0xDD],
        vec![0xDD, 0xFD, 0xDD],
        vec![0xFD, 0xDD, 0xFD],
    ] {
        let waits = vec![0; prefixes.len() + 4];
        let cpu = execute_indexed_inc_dec(IndexedIncDecCase {
            prefixes: &prefixes,
            ix: 0x1002,
            iy: 0x2002,
            pc: 0,
            opcode: 0x35,
            flags: 0x01,
            displacement: 0xFE,
            old_value: 0x00,
            expected_value: 0xFF,
            waits: &waits,
        });
        assert_eq!(
            cpu.wz,
            if prefixes.last() == Some(&0xDD) {
                0x1000
            } else {
                0x2000
            }
        );
        assert_eq!(cpu.registers.f, 0xBB);
    }
}

#[test]
fn indexed_inc_dec_wraps_effective_address_and_program_counter() {
    let cpu = execute_indexed_inc_dec(IndexedIncDecCase {
        prefixes: &[0xDD],
        ix: 0xFFFF,
        iy: 0x2000,
        pc: 0xFFFD,
        opcode: 0x34,
        flags: 0,
        displacement: 0x01,
        old_value: 0xFF,
        expected_value: 0x00,
        waits: &[0, 0, 0, 0, 0],
    });
    assert_eq!(cpu.wz, 0x0000);
    assert_eq!(cpu.registers.pc, 0x0000);
    assert_eq!(cpu.registers.f, 0x50);
}

#[test]
fn indexed_inc_dec_covers_zero_and_signed_displacement_extremes_for_ix_and_iy() {
    let cases = [
        // IX: zero displacement, negative boundary and positive boundary with 16-bit wrap.
        (0xDD, 0x1234, 0x0000, 0x00, 0x34, 0x01, 0x35, 0xFE),
        (0xDD, 0x0001, 0x0000, 0x80, 0x34, 0x01, 0x35, 0xFE),
        (0xDD, 0xFFFF, 0x0000, 0x7F, 0x34, 0x01, 0x35, 0xFE),
        // IY: the same displacement boundaries, including wrap in both directions.
        (0xFD, 0x0000, 0x1234, 0x00, 0x34, 0x01, 0x35, 0xFE),
        (0xFD, 0x0000, 0x0001, 0x80, 0x34, 0x01, 0x35, 0xFE),
        (0xFD, 0x0000, 0xFF80, 0x7F, 0x34, 0x01, 0x35, 0xFE),
    ];

    for (prefix, ix, iy, displacement, opcode, old_value, dec_opcode, dec_value) in cases {
        let index_base = if prefix == 0xDD { ix } else { iy };
        let effective_address = index_base.wrapping_add_signed(i16::from(displacement as i8));
        for (opcode, old_value, expected_value) in [
            (opcode, old_value, old_value.wrapping_add(1)),
            (dec_opcode, dec_value, dec_value.wrapping_sub(1)),
        ] {
            let cpu = execute_indexed_inc_dec(IndexedIncDecCase {
                prefixes: &[prefix],
                ix,
                iy,
                pc: 0,
                opcode,
                flags: 0x01,
                displacement,
                old_value,
                expected_value,
                waits: &[0, 0, 0, 0, 0],
            });
            assert_eq!(cpu.wz, effective_address);
            assert_eq!(cpu.registers.pc, 3);
        }
    }
}
