//! E2 prefix-FSM integration tests for the resumable Z80 sequencer.

use sega_z80_cpu::{
    Z80BusRequest, Z80IndexRegister, Z80MicroSequencer, Z80MicroSequencerError, Z80MicroStep,
    Z80PrefixState,
};

fn request(step: Z80MicroStep) -> Z80BusRequest {
    match step {
        Z80MicroStep::BusRequest(request) => request,
        other => panic!("expected opcode-fetch request, got {other:?}"),
    }
}

fn start(cpu: &mut Z80MicroSequencer) -> Z80BusRequest {
    request(cpu.begin_instruction().expect("instruction starts"))
}

fn feed_prefix(cpu: &mut Z80MicroSequencer, current: Z80BusRequest, byte: u8) -> Z80BusRequest {
    request(
        cpu.complete_bus_cycle(current.id, Some(byte))
            .expect("prefix transition schedules the next M1 fetch"),
    )
}

#[test]
fn dd_fd_alternation_keeps_the_last_index_prefix_and_round_trips_snapshot() {
    let mut cpu = Z80MicroSequencer::default();
    let mut fetch = start(&mut cpu);

    fetch = feed_prefix(&mut cpu, fetch, 0xDD);
    assert_eq!(
        cpu.export_state().prefix_state,
        Z80PrefixState::Index(Z80IndexRegister::Ix)
    );

    let encoded = cpu
        .export_state()
        .encode()
        .expect("IX prefix snapshot encodes");
    let decoded =
        sega_z80_cpu::Z80MicroSequencerState::decode(&encoded).expect("IX prefix snapshot decodes");
    cpu = Z80MicroSequencer::restore_state(decoded).expect("IX prefix snapshot restores");
    assert_eq!(
        cpu.export_state().prefix_state,
        Z80PrefixState::Index(Z80IndexRegister::Ix)
    );

    fetch = feed_prefix(&mut cpu, fetch, 0xFD);
    assert_eq!(
        cpu.export_state().prefix_state,
        Z80PrefixState::Index(Z80IndexRegister::Iy)
    );
    fetch = feed_prefix(&mut cpu, fetch, 0xDD);
    assert_eq!(
        cpu.export_state().prefix_state,
        Z80PrefixState::Index(Z80IndexRegister::Ix)
    );

    assert!(matches!(
        cpu.complete_bus_cycle(fetch.id, Some(0x21)),
        Err(Z80MicroSequencerError::UnsupportedOpcode { opcode: 0x21, .. })
    ));
    assert_eq!(cpu.export_state().prefix_state, Z80PrefixState::None);
    assert!(!cpu.instruction_active());
}

#[test]
fn ed_after_index_prefix_cancels_index_mode_before_ed_decode() {
    let mut cpu = Z80MicroSequencer::default();
    let fetch = start(&mut cpu);
    let fetch = feed_prefix(&mut cpu, fetch, 0xFD);
    let fetch = feed_prefix(&mut cpu, fetch, 0xED);
    assert_eq!(cpu.export_state().prefix_state, Z80PrefixState::Ed);

    assert_eq!(
        cpu.complete_bus_cycle(fetch.id, Some(0x46)).unwrap(),
        Z80MicroStep::InstructionRetired {
            opcode: 0x46,
            tstates: 8,
        }
    );
    assert_eq!(cpu.interrupt_mode, 0);
    assert_eq!(cpu.export_state().prefix_state, Z80PrefixState::None);
}

#[test]
fn prefix_free_nop_keeps_the_existing_four_tstate_path() {
    let mut cpu = Z80MicroSequencer::default();
    let fetch = start(&mut cpu);
    assert_eq!(
        cpu.complete_bus_cycle(fetch.id, Some(0x00)).unwrap(),
        Z80MicroStep::InstructionRetired {
            opcode: 0x00,
            tstates: 4,
        }
    );
    assert_eq!(cpu.registers.pc, 1);
    assert_eq!(cpu.refresh & 0x7F, 1);
    assert_eq!(cpu.export_state().prefix_state, Z80PrefixState::None);
}

#[test]
fn prefix_snapshot_rejects_index_latch_that_disagrees_with_last_prefix_byte() {
    let mut cpu = Z80MicroSequencer::default();
    let fetch = start(&mut cpu);
    let _fetch = feed_prefix(&mut cpu, fetch, 0xFD);
    let mut state = cpu.export_state();
    state.prefix_state = Z80PrefixState::Index(Z80IndexRegister::Ix);
    assert_eq!(
        Z80MicroSequencer::restore_state(state),
        Err(Z80MicroSequencerError::InvalidState)
    );
}

#[test]
fn repeated_prefix_stream_has_no_artificial_length_limit() {
    let mut cpu = Z80MicroSequencer::default();
    let mut fetch = start(&mut cpu);

    for index in 0..257 {
        let prefix = if index % 2 == 0 { 0xDD } else { 0xFD };
        fetch = feed_prefix(&mut cpu, fetch, prefix);
    }

    let state = cpu.export_state();
    assert_eq!(
        state.prefix_state,
        Z80PrefixState::Index(Z80IndexRegister::Ix)
    );
    assert_eq!(state.mcycle_index, 257);
    assert_eq!(state.instruction_tstates, 257 * 4);
    assert_eq!(state.registers.pc, 257);
    assert!(matches!(
        cpu.complete_bus_cycle(fetch.id, Some(0x21)),
        Err(Z80MicroSequencerError::UnsupportedOpcode { opcode: 0x21, .. })
    ));
}
