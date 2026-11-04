//! E6.1 tests for ignored DD/FD prefix fallback into the existing BASE/ED paths.

use sega_z80_cpu::{
    Z80BusOperation, Z80BusRequest, Z80MicroSequencer, Z80MicroSequencerState, Z80MicroStep,
};

fn request(step: Z80MicroStep) -> Z80BusRequest {
    match step {
        Z80MicroStep::BusRequest(request) => request,
        other => panic!("expected opcode-fetch request, got {other:?}"),
    }
}

fn round_trip(cpu: Z80MicroSequencer) -> Z80MicroSequencer {
    let encoded = cpu.export_state().encode().expect("snapshot encodes");
    let decoded = Z80MicroSequencerState::decode(&encoded).expect("snapshot decodes");
    Z80MicroSequencer::restore_state(decoded).expect("snapshot restores")
}

fn complete_fetch(
    mut cpu: Z80MicroSequencer,
    request: Z80BusRequest,
    opcode: u8,
    wait_tstates: u8,
) -> (Z80MicroSequencer, Z80MicroStep) {
    cpu = round_trip(cpu);
    for _ in 0..wait_tstates {
        cpu.insert_wait_tstate().expect("WAIT is accepted");
    }
    cpu = round_trip(cpu);
    let step = cpu
        .complete_bus_cycle(request.id, Some(opcode))
        .expect("opcode fetch completes");
    (cpu, step)
}

#[test]
fn ignored_dd_fd_prefixes_dispatch_the_already_fetched_base_opcode() {
    for prefixes in [
        vec![0xDD],
        vec![0xFD],
        vec![0xDD, 0xFD],
        vec![0xFD, 0xDD, 0xFD],
    ] {
        let mut cpu = Z80MicroSequencer::default();
        cpu.registers.set_de(0x1234);
        cpu.registers.set_hl(0xABCD);
        cpu.ix = 0x5678;
        cpu.iy = 0x9ABC;

        let mut fetch = request(cpu.begin_instruction().expect("instruction starts"));
        for (index, prefix) in prefixes.iter().copied().enumerate() {
            assert_eq!(fetch.operation, Z80BusOperation::OpcodeFetch);
            assert_eq!(fetch.address, index as u16);
            assert_eq!(fetch.base_tstates, 4);
            let (next_cpu, step) = complete_fetch(cpu, fetch, prefix, 0);
            cpu = next_cpu;
            fetch = request(step);
        }

        let terminal_address = prefixes.len() as u16;
        assert_eq!(fetch.operation, Z80BusOperation::OpcodeFetch);
        assert_eq!(fetch.address, terminal_address);
        assert_eq!(fetch.base_tstates, 4);

        let (cpu, step) = complete_fetch(cpu, fetch, 0xEB, 0);
        assert_eq!(
            step,
            Z80MicroStep::InstructionRetired {
                opcode: 0xEB,
                tstates: 4 * (prefixes.len() as u32 + 1),
            }
        );
        assert_eq!(cpu.registers.pc, terminal_address + 1);
        assert_eq!(cpu.refresh & 0x7F, prefixes.len() as u8 + 1);
        assert_eq!(cpu.registers.de(), 0xABCD);
        assert_eq!(cpu.registers.hl(), 0x1234);
        assert_eq!(cpu.ix, 0x5678);
        assert_eq!(cpu.iy, 0x9ABC);
        assert_eq!(cpu.export_state().prefix_state, sega_z80_cpu::Z80PrefixState::None);
        assert!(cpu.pending_request().is_none());
        assert!(!cpu.instruction_active());
    }
}

#[test]
fn ignored_prefix_fetches_preserve_waits_and_snapshot_resume() {
    let mut cpu = Z80MicroSequencer::default();
    let mut fetch = request(cpu.begin_instruction().expect("instruction starts"));
    let waits = [1, 2, 3];

    for (index, (opcode, wait)) in [0xDD, 0xFD, 0x00].into_iter().zip(waits).enumerate() {
        assert_eq!(fetch.operation, Z80BusOperation::OpcodeFetch);
        assert_eq!(fetch.address, index as u16);
        let (next_cpu, step) = complete_fetch(cpu, fetch, opcode, wait);
        cpu = next_cpu;
        if index < 2 {
            fetch = request(step);
        } else {
            assert_eq!(
                step,
                Z80MicroStep::InstructionRetired {
                    opcode: 0x00,
                    tstates: 12 + waits.iter().map(|wait| u32::from(*wait)).sum::<u32>(),
                }
            );
        }
    }

    assert_eq!(cpu.registers.pc, 3);
    assert_eq!(cpu.refresh & 0x7F, 3);
    assert!(!cpu.instruction_active());
}

#[test]
fn index_prefix_before_ed_uses_the_existing_ed_decoder_and_keeps_m1_cost() {
    for prefixes in [vec![0xDD], vec![0xFD, 0xDD]] {
        let mut cpu = Z80MicroSequencer::default();
        cpu.registers.a = 0x12;
        let mut fetch = request(cpu.begin_instruction().expect("instruction starts"));

        for (index, prefix) in prefixes.iter().copied().enumerate() {
            let (next_cpu, step) = complete_fetch(cpu, fetch, prefix, 0);
            cpu = next_cpu;
            fetch = request(step);
            assert_eq!(fetch.address, index as u16 + 1);
        }

        let (next_cpu, step) = complete_fetch(cpu, fetch, 0xED, 0);
        cpu = next_cpu;
        fetch = request(step);

        let (cpu, step) = complete_fetch(cpu, fetch, 0x44, 0);
        assert_eq!(
            step,
            Z80MicroStep::InstructionRetired {
                opcode: 0x44,
                tstates: 4 * (prefixes.len() as u32 + 2),
            }
        );
        assert_eq!(cpu.registers.a, 0xEE);
        assert_eq!(cpu.registers.pc, prefixes.len() as u16 + 2);
        assert_eq!(cpu.refresh & 0x7F, prefixes.len() as u8 + 2);
        assert!(!cpu.instruction_active());
    }
}
