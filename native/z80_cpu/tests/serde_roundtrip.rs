use sega_z80_cpu::{
    Z80, Z80ArchitecturalStateError, Z80BusHandshake, Z80BusOperation, Z80MicroSequencer,
    Z80MicroSequencerError, Z80MicroSequencerPhase, Z80MicroStep,
};

fn bus_request(step: Z80MicroStep) -> sega_z80_cpu::Z80BusRequest {
    match step {
        Z80MicroStep::BusRequest(request) => request,
        other => panic!("expected bus request, got {other:?}"),
    }
}

#[test]
fn architectural_state_round_trips_through_json() {
    let mut cpu = Z80::default();
    cpu.registers.a = 0xA5;
    cpu.registers.f = 0xD7;
    cpu.registers.set_hl(0xC123);
    cpu.alternate.a = 0x5A;
    cpu.ix = 0x1357;
    cpu.iy = 0x2468;
    cpu.i = 0x81;
    cpu.r = 0xFE;
    cpu.iff1 = true;
    cpu.iff2 = true;
    cpu.interrupt_mode = 2;
    cpu.wz = 0xABCD;
    cpu.ei_delay = 1;
    cpu.nmi_pending = true;
    cpu.int_pending = Some(0xCF);
    cpu.q = 0x28;
    cpu.p = true;
    cpu.halted = true;

    let mut state = cpu.export_architectural_state();
    state.q_previous = 0xD7;
    cpu.import_architectural_state(state).unwrap();

    let expected = cpu.export_architectural_state();
    let json = serde_json::to_string(&expected).unwrap();
    let decoded: sega_z80_cpu::Z80ArchitecturalState = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded, expected);
}

#[test]
fn handshake_state_round_trips_through_json_with_wait() {
    let mut handshake = Z80BusHandshake::default();
    handshake
        .begin(Z80BusOperation::MemoryRead, 0xC123, 3)
        .unwrap();
    handshake.insert_wait_tstates(2).unwrap();

    let expected = handshake.export_state();
    let json = serde_json::to_string(&expected).unwrap();
    let decoded = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded, expected);

    let restored = Z80BusHandshake::restore_state(decoded).unwrap();
    assert_eq!(restored.export_state(), expected);
}

#[test]
fn sequencer_json_round_trip_preserves_pending_fetch_and_wait() {
    let mut cpu = Z80MicroSequencer::default();
    let fetch = bus_request(cpu.begin_instruction().unwrap());
    cpu.insert_wait_tstates(3).unwrap();

    let expected = cpu.export_state();
    assert_eq!(expected.handshake.pending_request, Some(fetch));
    assert_eq!(expected.handshake.wait_tstates, 3);

    let json = serde_json::to_string(&expected).unwrap();
    let decoded = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded, expected);

    let restored = Z80MicroSequencer::restore_state(decoded).unwrap();
    assert_eq!(restored.export_state(), expected);
}

#[test]
fn sequencer_json_round_trip_preserves_internal_incdec_safe_point() {
    let mut cpu = Z80MicroSequencer::default();
    cpu.registers.set_hl(0xC123);

    let fetch = bus_request(cpu.begin_instruction().unwrap());
    let read = bus_request(cpu.complete_bus_cycle(fetch.id, Some(0x34)).unwrap());
    assert_eq!(
        cpu.complete_bus_cycle(read.id, Some(0x7F)).unwrap(),
        Z80MicroStep::InternalCycle { tstates: 1 }
    );

    let expected = cpu.export_state();
    assert!(matches!(
        expected.phase,
        Z80MicroSequencerPhase::MemoryIncDecInternal { .. }
    ));
    assert!(expected.handshake.pending_request.is_none());

    let json = serde_json::to_string(&expected).unwrap();
    let decoded = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded, expected);

    let restored = Z80MicroSequencer::restore_state(decoded).unwrap();
    assert_eq!(restored.export_state(), expected);
}

#[test]
fn invalid_architectural_json_is_rejected_without_mutating_atomic_cpu() {
    let mut cpu = Z80::default();
    cpu.registers.a = 0x42;
    let before = cpu.export_architectural_state();
    let valid_json = serde_json::to_value(before).unwrap();

    for (field, invalid_value, expected_error) in [
        (
            "interrupt_mode",
            3,
            Z80ArchitecturalStateError::InvalidInterruptMode(3),
        ),
        ("ei_delay", 2, Z80ArchitecturalStateError::InvalidEiDelay(2)),
    ] {
        let mut value = valid_json.clone();
        value[field] = serde_json::json!(invalid_value);
        let invalid = serde_json::from_value(value).unwrap();
        assert_eq!(cpu.import_architectural_state(invalid), Err(expected_error));
        assert_eq!(cpu.export_architectural_state(), before);
    }
}

#[test]
fn json_phase_handshake_mismatch_is_rejected_by_restore() {
    let mut cpu = Z80MicroSequencer::default();
    cpu.begin_instruction().unwrap();
    let mut value = serde_json::to_value(cpu.export_state()).unwrap();
    value["phase"] = serde_json::json!("Ready");

    let decoded = serde_json::from_value(value).unwrap();
    assert_eq!(
        Z80MicroSequencer::restore_state(decoded),
        Err(Z80MicroSequencerError::InvalidState)
    );
}
