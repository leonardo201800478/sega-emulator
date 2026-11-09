//! F-033: every reserved second byte after ED retires through the resumable core.

use sega_z80_cpu::{
    Z80BusOperation, Z80BusRequest, Z80MicroSequencer, Z80MicroSequencerState, Z80MicroStep,
};

fn request(step: Z80MicroStep) -> Z80BusRequest {
    match step {
        Z80MicroStep::BusRequest(request) => request,
        other => panic!("expected bus request, got {other:?}"),
    }
}

fn round_trip(cpu: Z80MicroSequencer) -> Z80MicroSequencer {
    let encoded = cpu.export_state().encode().expect("snapshot encodes");
    let decoded = Z80MicroSequencerState::decode(&encoded).expect("snapshot decodes");
    Z80MicroSequencer::restore_state(decoded).expect("snapshot restores")
}

fn complete_fetch(
    cpu: &mut Z80MicroSequencer,
    fetch: Z80BusRequest,
    value: u8,
) -> Z80MicroStep {
    *cpu = round_trip(cpu.clone());
    cpu.complete_bus_cycle(fetch.id, Some(value))
        .expect("opcode fetch completes")
}

fn is_explicit_ed_encoding(opcode: u8) -> bool {
    opcode & 0xC7 == 0x40
        || opcode & 0xC7 == 0x41
        || opcode & 0xC7 == 0x44
        || opcode & 0xC7 == 0x45
        || opcode & 0xCF == 0x42
        || opcode & 0xCF == 0x43
        || opcode & 0xCF == 0x4A
        || opcode & 0xCF == 0x4B
        || matches!(
            opcode,
            0x46 | 0x4E | 0x56 | 0x5E | 0x66 | 0x6E | 0x76 | 0x7E
                | 0x47 | 0x4F | 0x57 | 0x5F
                | 0x67 | 0x6F
                | 0xA0 | 0xA1 | 0xA2 | 0xA3
                | 0xA8 | 0xA9 | 0xAA | 0xAB
                | 0xB0 | 0xB1 | 0xB2 | 0xB3
                | 0xB8 | 0xB9 | 0xBA | 0xBB
        )
}

#[test]
fn all_178_reserved_ed_encodings_retire_after_two_m1_fetches() {
    let reserved: Vec<u8> = (0..=u8::MAX)
        .filter(|opcode| !is_explicit_ed_encoding(*opcode))
        .collect();
    assert_eq!(reserved.len(), 178);

    for opcode in reserved {
        let mut cpu = Z80MicroSequencer::default();
        cpu.registers.pc = 0xFFFE;
        cpu.registers.f = 0xA5;
        cpu.refresh = 0xFE;
        cpu.q = 0xA5;
        cpu.q_previous = 0x5A;
        cpu.wz = 0xBEEF;
        cpu.p = true;
        cpu.iff1 = true;
        cpu.iff2 = true;
        cpu.interrupt_mode = 2;

        let prefix_fetch = request(cpu.begin_instruction().expect("instruction starts"));
        assert_eq!(prefix_fetch.operation, Z80BusOperation::OpcodeFetch);
        assert_eq!(prefix_fetch.address, 0xFFFE);
        assert_eq!(prefix_fetch.base_tstates, 4);

        let terminal_fetch = request(complete_fetch(&mut cpu, prefix_fetch, 0xED));
        assert_eq!(terminal_fetch.operation, Z80BusOperation::OpcodeFetch);
        assert_eq!(terminal_fetch.address, 0xFFFF);
        assert_eq!(terminal_fetch.base_tstates, 4);
        assert_eq!(
            cpu.export_state().prefix_state,
            sega_z80_cpu::Z80PrefixState::Ed,
            "ED {opcode:02X}: prefix state survives snapshot/restore"
        );

        let retired = complete_fetch(&mut cpu, terminal_fetch, opcode);
        assert_eq!(
            retired,
            Z80MicroStep::InstructionRetired {
                opcode,
                tstates: 8,
            },
            "ED {opcode:02X}"
        );

        let state = cpu.export_state();
        assert_eq!(state.registers.pc, 0x0000, "ED {opcode:02X}: PC wrap");
        assert_eq!(state.refresh, 0x80, "ED {opcode:02X}: two M1 refreshes");
        assert_eq!(state.registers.f, 0xA5, "ED {opcode:02X}: F preserved");
        assert_eq!(state.q, 0x00, "ED {opcode:02X}: no F write");
        assert_eq!(state.q_previous, 0xA5, "ED {opcode:02X}: previous Q");
        assert_eq!(state.wz, 0xBEEF, "ED {opcode:02X}: WZ preserved");
        assert!(state.p, "ED {opcode:02X}: P latch preserved");
        assert!(state.iff1 && state.iff2, "ED {opcode:02X}: IFF preserved");
        assert_eq!(state.interrupt_mode, 2, "ED {opcode:02X}: IM preserved");
        assert_eq!(state.phase, sega_z80_cpu::Z80MicroSequencerPhase::Ready);
        assert!(cpu.pending_request().is_none(), "ED {opcode:02X}");
        assert!(!cpu.instruction_active(), "ED {opcode:02X}");
    }
}

#[test]
fn reserved_ed_fallback_preserves_wait_states_and_snapshot_resume() {
    let mut cpu = Z80MicroSequencer::default();
    cpu.registers.f = 0xD7;
    cpu.refresh = 0x7E;
    cpu.wz = 0x1234;

    let prefix_fetch = request(cpu.begin_instruction().expect("instruction starts"));
    cpu = round_trip(cpu);
    cpu.insert_wait_tstates(2).expect("prefix WAIT accepted");
    cpu = round_trip(cpu);
    let terminal_fetch = request(
        cpu.complete_bus_cycle(prefix_fetch.id, Some(0xED))
            .expect("ED prefix completes"),
    );

    cpu = round_trip(cpu);
    cpu.insert_wait_tstates(3).expect("terminal WAIT accepted");
    cpu = round_trip(cpu);
    assert_eq!(
        cpu.complete_bus_cycle(terminal_fetch.id, Some(0x00))
            .expect("reserved ED byte retires"),
        Z80MicroStep::InstructionRetired {
            opcode: 0x00,
            tstates: 13,
        }
    );

    let state = cpu.export_state();
    assert_eq!(state.registers.pc, 2);
    assert_eq!(state.refresh & 0x7F, 2);
    assert_eq!(state.registers.f, 0xD7);
    assert_eq!(state.wz, 0x1234);
    assert!(cpu.pending_request().is_none());
    assert!(!cpu.instruction_active());
}
