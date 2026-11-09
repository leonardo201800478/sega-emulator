//! F-005: LD SP,HL/IX/IY semantic and resumable timing coverage.

use sega_z80_cpu::{
    Z80BusOperation, Z80BusRequest, Z80MicroSequencer, Z80MicroSequencerState, Z80MicroStep,
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

fn feed_prefix(cpu: &mut Z80MicroSequencer, fetch: Z80BusRequest, byte: u8) -> Z80BusRequest {
    let next = cpu
        .complete_bus_cycle(fetch.id, Some(byte))
        .expect("prefix schedules the next M1 fetch");
    request(next)
}

fn complete_internal(cpu: &mut Z80MicroSequencer, count: usize) -> Z80MicroStep {
    let mut step = Z80MicroStep::InternalCycle { tstates: 1 };
    for _ in 0..count {
        step = cpu
            .complete_internal_cycle()
            .expect("internal T-state completes");
    }
    step
}

fn execute_ld_sp(cpu: &mut Z80MicroSequencer, prefixes: &[u8]) -> Z80MicroStep {
    let mut fetch = start(cpu);
    for prefix in prefixes {
        assert_eq!(fetch.operation, Z80BusOperation::OpcodeFetch);
        assert_eq!(fetch.base_tstates, 4);
        fetch = feed_prefix(cpu, fetch, *prefix);
    }
    assert_eq!(fetch.operation, Z80BusOperation::OpcodeFetch);
    assert_eq!(fetch.base_tstates, 4);
    assert_eq!(
        cpu.complete_bus_cycle(fetch.id, Some(0xF9))
            .expect("F9 enters the internal transfer cycle"),
        Z80MicroStep::InternalCycle { tstates: 1 }
    );
    complete_internal(cpu, 2)
}

fn snapshot_restore(cpu: &Z80MicroSequencer) -> Z80MicroSequencer {
    let encoded = cpu.export_state().encode().expect("snapshot encodes");
    let decoded = Z80MicroSequencerState::decode(&encoded).expect("snapshot decodes");
    Z80MicroSequencer::restore_state(decoded).expect("snapshot restores")
}

#[test]
fn base_f9_copies_hl_to_sp_in_one_mcycle_and_preserves_architectural_state() {
    let mut cpu = Z80MicroSequencer::default();
    cpu.registers.a = 0x91;
    cpu.registers.b = 0x12;
    cpu.registers.c = 0x34;
    cpu.registers.d = 0x56;
    cpu.registers.e = 0x78;
    cpu.registers.set_hl(0xA55A);
    cpu.registers.sp = 0xFFFF;
    cpu.registers.f = 0xD7;
    cpu.alternate.a = 0xC3;
    cpu.alternate.f = 0x5A;
    cpu.alternate.b = 0x11;
    cpu.alternate.c = 0x22;
    cpu.alternate.d = 0x33;
    cpu.alternate.e = 0x44;
    cpu.alternate.h = 0x55;
    cpu.alternate.l = 0x66;
    cpu.ix = 0x1357;
    cpu.iy = 0x2468;
    cpu.i = 0xA4;
    cpu.refresh = 0xFE;
    cpu.wz = 0xBEEF;
    cpu.q = 0x5A;
    cpu.q_previous = 0xA5;
    let before = cpu.registers;
    let alternate_before = cpu.alternate;

    let fetch = start(&mut cpu);
    assert_eq!(fetch.operation, Z80BusOperation::OpcodeFetch);
    assert_eq!(fetch.base_tstates, 4);
    assert_eq!(
        cpu.complete_bus_cycle(fetch.id, Some(0xF9)).unwrap(),
        Z80MicroStep::InternalCycle { tstates: 1 }
    );

    cpu = snapshot_restore(&cpu);
    assert_eq!(
        complete_internal(&mut cpu, 1),
        Z80MicroStep::InternalCycle { tstates: 1 }
    );
    cpu = snapshot_restore(&cpu);
    assert_eq!(
        complete_internal(&mut cpu, 1),
        Z80MicroStep::InstructionRetired {
            opcode: 0xF9,
            tstates: 6,
        }
    );

    let mut expected_registers = before;
    expected_registers.sp = 0xA55A;
    assert_eq!(cpu.registers, expected_registers);
    assert_eq!(cpu.alternate, alternate_before);
    assert_eq!((cpu.ix, cpu.iy, cpu.i), (0x1357, 0x2468, 0xA4));
    assert_eq!(cpu.wz, 0xBEEF);
    assert_eq!(cpu.refresh, 0xFF);
    assert_eq!(cpu.registers.f, 0xD7);
    assert_eq!(cpu.q, 0);
    assert_eq!(cpu.q_previous, 0x5A);
    assert_eq!(cpu.registers.pc, 1);
}

#[test]
fn dd_fd_f9_select_ix_or_iy_and_take_two_mcycles_ten_tstates() {
    for (prefix, expected_sp) in [(0xDD, 0x1234), (0xFD, 0xABCD)] {
        let mut cpu = Z80MicroSequencer::default();
        cpu.registers.set_hl(0x5678);
        cpu.registers.sp = 0xC000;
        cpu.registers.f = 0xA5;
        cpu.ix = 0x1234;
        cpu.iy = 0xABCD;
        cpu.refresh = 0x80;
        cpu.wz = 0x9ABC;
        cpu.q = 0x77;
        cpu.q_previous = 0x88;

        assert_eq!(
            execute_ld_sp(&mut cpu, &[prefix]),
            Z80MicroStep::InstructionRetired {
                opcode: 0xF9,
                tstates: 10,
            }
        );
        assert_eq!(cpu.registers.sp, expected_sp);
        assert_eq!(cpu.registers.hl(), 0x5678);
        assert_eq!((cpu.ix, cpu.iy), (0x1234, 0xABCD));
        assert_eq!(cpu.registers.f, 0xA5);
        assert_eq!(cpu.wz, 0x9ABC);
        assert_eq!(cpu.refresh, 0x82);
        assert_eq!(cpu.q, 0);
        assert_eq!(cpu.q_previous, 0);
        assert_eq!(cpu.registers.pc, 2);
    }
}

#[test]
fn repeated_and_alternating_dd_fd_prefixes_use_the_last_index_prefix() {
    let cases: &[(&[u8], u16)] = &[
        (&[0xDD, 0xDD], 0x1234),
        (&[0xDD, 0xFD], 0xABCD),
        (&[0xFD, 0xDD], 0x1234),
        (&[0xFD, 0xFD], 0xABCD),
        (&[0xDD, 0xFD, 0xDD], 0x1234),
    ];

    for (prefixes, expected_sp) in cases {
        let mut cpu = Z80MicroSequencer::default();
        cpu.registers.set_hl(0x5678);
        cpu.registers.sp = 0xC000;
        cpu.registers.f = 0xD7;
        cpu.ix = 0x1234;
        cpu.iy = 0xABCD;
        cpu.refresh = 0xFE;
        cpu.wz = 0xBEEF;

        let prefix_count = prefixes.len() as u32;
        let expected_tstates = 4 * (prefix_count + 1) + 2;
        assert_eq!(
            execute_ld_sp(&mut cpu, prefixes),
            Z80MicroStep::InstructionRetired {
                opcode: 0xF9,
                tstates: expected_tstates,
            }
        );
        assert_eq!(cpu.registers.sp, *expected_sp);
        assert_eq!(cpu.registers.hl(), 0x5678);
        assert_eq!(cpu.registers.f, 0xD7);
        assert_eq!(cpu.wz, 0xBEEF);
        assert_eq!(
            cpu.refresh,
            0x80 | ((0x7E + prefix_count + 1) & 0x7F) as u8
        );
        assert_eq!(cpu.registers.pc, prefix_count as u16 + 1);
    }
}

fn run_waited_indexed_f9(with_snapshots: bool) -> (Z80MicroStep, Z80MicroSequencerState) {
    let mut cpu = Z80MicroSequencer::default();
    cpu.ix = 0xCAFE;
    cpu.registers.sp = 0x1234;
    cpu.registers.f = 0xD7;
    cpu.wz = 0xBEEF;

    let fetch = start(&mut cpu);
    cpu.insert_wait_tstates(1)
        .expect("WAIT extends the prefix M1");
    if with_snapshots {
        cpu = snapshot_restore(&cpu);
    }

    let opcode_fetch = feed_prefix(&mut cpu, fetch, 0xDD);
    cpu.insert_wait_tstates(2)
        .expect("WAIT extends the terminal-opcode M1");
    if with_snapshots {
        cpu = snapshot_restore(&cpu);
    }

    assert_eq!(
        cpu.complete_bus_cycle(opcode_fetch.id, Some(0xF9))
            .unwrap(),
        Z80MicroStep::InternalCycle { tstates: 1 }
    );
    if with_snapshots {
        cpu = snapshot_restore(&cpu);
    }

    assert_eq!(
        complete_internal(&mut cpu, 1),
        Z80MicroStep::InternalCycle { tstates: 1 }
    );
    if with_snapshots {
        cpu = snapshot_restore(&cpu);
    }

    let retired = complete_internal(&mut cpu, 1);
    (retired, cpu.export_state())
}

#[test]
fn wait_states_and_snapshot_restore_preserve_prefix_and_internal_continuations() {
    let uninterrupted = run_waited_indexed_f9(false);
    let resumed = run_waited_indexed_f9(true);

    assert_eq!(
        resumed.0,
        Z80MicroStep::InstructionRetired {
            opcode: 0xF9,
            tstates: 13,
        }
    );
    assert_eq!(resumed, uninterrupted);
    assert_eq!(resumed.1.registers.sp, 0xCAFE);
    assert_eq!(resumed.1.registers.f, 0xD7);
    assert_eq!(resumed.1.wz, 0xBEEF);
    assert_eq!(resumed.1.registers.pc, 2);
    assert_eq!(resumed.1.refresh, 2);
}
