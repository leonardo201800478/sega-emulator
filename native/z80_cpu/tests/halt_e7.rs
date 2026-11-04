use sega_z80_cpu::{
    Z80BusOperation, Z80HaltExit, Z80MicroSequencer, Z80MicroSequencerPhase, Z80MicroSequencerState,
    Z80MicroStep, Z80PrefixState,
};

fn bus_request(step: Z80MicroStep) -> sega_z80_cpu::Z80BusRequest {
    match step {
        Z80MicroStep::BusRequest(request) => request,
        other => panic!("expected bus request, got {other:?}"),
    }
}

#[test]
fn halt_repeats_m1_at_next_pc_ignores_data_and_preserves_snapshot() {
    let mut cpu = Z80MicroSequencer::default();
    cpu.registers.pc = 0xFFFF;
    cpu.refresh = 0xFF;

    let halt_fetch = bus_request(cpu.begin_instruction().unwrap());
    assert_eq!(halt_fetch.operation, Z80BusOperation::OpcodeFetch);
    assert_eq!(halt_fetch.address, 0xFFFF);
    assert_eq!(halt_fetch.base_tstates, 4);

    assert_eq!(
        cpu.complete_bus_cycle(halt_fetch.id, Some(0x76)).unwrap(),
        Z80MicroStep::InstructionRetired {
            opcode: 0x76,
            tstates: 4,
        }
    );
    assert!(cpu.halted);
    assert_eq!(cpu.registers.pc, 0x0000);
    assert_eq!(cpu.refresh, 0x80);

    let repeated_fetch = bus_request(cpu.begin_instruction().unwrap());
    assert_eq!(repeated_fetch.operation, Z80BusOperation::OpcodeFetch);
    assert_eq!(repeated_fetch.address, 0x0000);
    assert_eq!(repeated_fetch.base_tstates, 4);

    cpu.insert_wait_tstates(2).unwrap();
    let encoded = cpu.export_state().encode().unwrap();
    let state = Z80MicroSequencerState::decode(&encoded).unwrap();
    let mut restored = Z80MicroSequencer::restore_state(state).unwrap();

    assert_eq!(
        restored
            .complete_bus_cycle(repeated_fetch.id, Some(0x3E))
            .unwrap(),
        Z80MicroStep::InstructionRetired {
            opcode: 0x00,
            tstates: 6,
        }
    );
    assert!(restored.halted);
    assert_eq!(restored.registers.pc, 0x0000);
    assert_eq!(restored.refresh, 0x81);
    assert_eq!(restored.export_state().phase, Z80MicroSequencerPhase::Ready);
}

#[test]
fn halt_hold_cycle_does_not_decode_prefix_or_change_architectural_flags() {
    let mut cpu = Z80MicroSequencer::default();
    cpu.registers.f = 0xD7;

    let halt_fetch = bus_request(cpu.begin_instruction().unwrap());
    cpu.complete_bus_cycle(halt_fetch.id, Some(0x76)).unwrap();
    let flags_after_halt = cpu.registers.f;

    let repeated_fetch = bus_request(cpu.begin_instruction().unwrap());
    assert_eq!(repeated_fetch.address, 1);
    assert_eq!(
        cpu.complete_bus_cycle(repeated_fetch.id, Some(0xDD))
            .unwrap(),
        Z80MicroStep::InstructionRetired {
            opcode: 0x00,
            tstates: 4,
        }
    );
    assert!(cpu.halted);
    assert_eq!(cpu.registers.pc, 1);
    assert_eq!(cpu.registers.f, flags_after_halt);
    assert_eq!(cpu.export_state().prefix_state, Z80PrefixState::None);
}

#[test]
fn halt_snapshot_rejects_incompatible_in_flight_phase() {
    let mut cpu = Z80MicroSequencer::default();
    let halt_fetch = bus_request(cpu.begin_instruction().unwrap());
    cpu.complete_bus_cycle(halt_fetch.id, Some(0x76)).unwrap();

    let mut invalid = cpu.export_state();
    invalid.phase = Z80MicroSequencerPhase::ImmediateReadRegister { destination: 0 };

    assert!(matches!(
        Z80MicroSequencer::restore_state(invalid),
        Err(sega_z80_cpu::Z80MicroSequencerError::InvalidState)
    ));
}


#[test]
fn halt_exit_latch_round_trips_and_rejects_non_halted_or_in_flight_states() {
    let mut cpu = Z80MicroSequencer {
        iff1: true,
        iff2: true,
        halted: true,
        ..Z80MicroSequencer::default()
    };
    cpu.set_int_line(Some(0xFF));

    let hold = bus_request(cpu.begin_instruction().unwrap());
    cpu.complete_bus_cycle(hold.id, Some(0x00)).unwrap();
    cpu.sample_halt_interrupts_at_t4().unwrap();
    assert_eq!(
        cpu.export_state().halt_exit_pending,
        Some(Z80HaltExit::Int)
    );

    let encoded = cpu.export_state().encode().unwrap();
    let decoded = Z80MicroSequencerState::decode(&encoded).unwrap();
    assert_eq!(decoded, cpu.export_state());
    let restored = Z80MicroSequencer::restore_state(decoded).unwrap();
    assert_eq!(
        restored.export_state().halt_exit_pending,
        Some(Z80HaltExit::Int)
    );

    let mut non_halted = restored.export_state();
    non_halted.halted = false;
    assert!(Z80MicroSequencer::restore_state(non_halted).is_err());

    let mut in_flight = restored.export_state();
    in_flight.phase = Z80MicroSequencerPhase::OpcodeFetch;
    assert!(Z80MicroSequencer::restore_state(in_flight).is_err());
}
