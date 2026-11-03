//! E2 prefix-FSM integration tests for the resumable Z80 sequencer.

use sega_z80_cpu::{
    Z80BusOperation, Z80BusRequest, Z80IndexOpcodeClass, Z80IndexRegister, Z80MicroSequencer,
    Z80MicroSequencerError, Z80MicroSequencerPhase, Z80MicroStep, Z80PrefixState,
    classify_index_opcode,
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

    cpu.ix = 0x1000;
    cpu.iy = 0x2000;
    let displacement = request(
        cpu.complete_bus_cycle(fetch.id, Some(0x86))
            .expect("indexed ALU opcode requests its displacement"),
    );
    assert_eq!(displacement.operation, Z80BusOperation::MemoryRead);
    assert_eq!(displacement.address, 4);
    assert_eq!(displacement.base_tstates, 3);

    let encoded = cpu
        .export_state()
        .encode()
        .expect("indexed ALU displacement request snapshot encodes");
    let decoded = sega_z80_cpu::Z80MicroSequencerState::decode(&encoded)
        .expect("indexed ALU displacement request snapshot decodes");
    cpu = Z80MicroSequencer::restore_state(decoded)
        .expect("indexed ALU displacement request snapshot restores");

    assert_eq!(
        cpu.complete_bus_cycle(displacement.id, Some(0xFF)).unwrap(),
        Z80MicroStep::InternalCycle { tstates: 1 }
    );
    for _ in 0..5 {
        let step = cpu.complete_internal_cycle().unwrap();
        let encoded = cpu.export_state().encode().expect("internal phase snapshot encodes");
        let decoded = sega_z80_cpu::Z80MicroSequencerState::decode(&encoded)
            .expect("internal phase snapshot decodes");
        cpu = Z80MicroSequencer::restore_state(decoded).expect("internal phase snapshot restores");
        if let Z80MicroStep::BusRequest(bus_request) = step {
            assert_eq!(bus_request.operation, Z80BusOperation::MemoryRead);
            assert_eq!(bus_request.address, 0x0FFF);
            assert_eq!(bus_request.base_tstates, 3);
            assert_eq!(
                cpu.export_state().phase,
                Z80MicroSequencerPhase::MemoryReadAlu {
                    operation: 0,
                    address: 0x0FFF,
                }
            );
            let encoded = cpu.export_state().encode().expect("ALU read snapshot encodes");
            let decoded = sega_z80_cpu::Z80MicroSequencerState::decode(&encoded)
                .expect("ALU read snapshot decodes");
            cpu = Z80MicroSequencer::restore_state(decoded).expect("ALU read snapshot restores");
            assert_eq!(
                cpu.complete_bus_cycle(bus_request.id, Some(0x01)).unwrap(),
                Z80MicroStep::InstructionRetired {
                    opcode: 0x86,
                    tstates: 27,
                }
            );
        } else {
            assert_eq!(step, Z80MicroStep::InternalCycle { tstates: 1 });
        }
    }
    assert_eq!(cpu.registers.a, 0x11);
    assert_eq!(cpu.registers.f, 0x00);
    assert_eq!(cpu.wz, 0x0FFF);
    assert_eq!(cpu.registers.pc, 5);
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
            tstates: 12,
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
fn dd_fd_prefix_classifies_ignored_indexed_and_index_cb_opcodes() {
    assert_eq!(classify_index_opcode(0x00), Z80IndexOpcodeClass::Ignored);
    assert_eq!(classify_index_opcode(0xEB), Z80IndexOpcodeClass::Ignored);
    assert_eq!(classify_index_opcode(0x76), Z80IndexOpcodeClass::Ignored);
    assert_eq!(classify_index_opcode(0x21), Z80IndexOpcodeClass::UsesIndex);
    assert_eq!(classify_index_opcode(0x66), Z80IndexOpcodeClass::UsesIndex);
    assert_eq!(classify_index_opcode(0x84), Z80IndexOpcodeClass::UsesIndex);
    assert_eq!(classify_index_opcode(0xCB), Z80IndexOpcodeClass::IndexedCb);

    let mut cpu = Z80MicroSequencer::default();
    let fetch = start(&mut cpu);
    let fetch = feed_prefix(&mut cpu, fetch, 0xDD);
    assert!(matches!(
        cpu.complete_bus_cycle(fetch.id, Some(0xEB)),
        Err(Z80MicroSequencerError::UnsupportedOpcode { opcode: 0xEB, .. })
    ));
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
        cpu.complete_bus_cycle(fetch.id, Some(0xC6)),
        Err(Z80MicroSequencerError::UnsupportedOpcode { opcode: 0xC6, .. })
    ));
}
