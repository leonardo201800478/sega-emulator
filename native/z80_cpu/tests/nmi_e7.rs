use sega_z80_cpu::{
    Z80BusOperation, Z80BusRequest, Z80HaltExit, Z80InterruptKind, Z80MicroSequencer,
    Z80MicroSequencerError, Z80MicroSequencerPhase, Z80MicroSequencerState, Z80MicroStep,
};

fn request(step: Z80MicroStep) -> Z80BusRequest {
    match step {
        Z80MicroStep::BusRequest(request) => request,
        other => panic!("expected bus request, got {other:?}"),
    }
}

fn round_trip(cpu: Z80MicroSequencer) -> Z80MicroSequencer {
    let state = cpu.export_state();
    let encoded = state.encode().unwrap();
    let decoded = Z80MicroSequencerState::decode(&encoded).unwrap();
    assert_eq!(decoded, state);
    Z80MicroSequencer::restore_state(decoded).unwrap()
}

#[test]
fn nmi_response_ignores_m1_data_pushes_pc_high_then_low_and_preserves_state() {
    let mut cpu = Z80MicroSequencer::default();
    cpu.registers.pc = 0x1234;
    cpu.registers.sp = 0x0000;
    cpu.refresh = 0xFE;
    cpu.iff1 = true;
    cpu.iff2 = false;
    cpu.ei_delay = 1;
    cpu.interrupt_mode = 0;
    cpu.int_pending = Some(0xFF);
    cpu.request_nmi();

    let response = request(cpu.begin_instruction().unwrap());
    assert_eq!(response.operation, Z80BusOperation::OpcodeFetch);
    assert_eq!(response.address, 0x1234);
    assert_eq!(response.base_tstates, 4);
    assert_eq!(
        cpu.export_state().phase,
        Z80MicroSequencerPhase::NmiResponse
    );
    assert!(!cpu.nmi_pending);
    assert!(!cpu.halted);
    assert!(!cpu.iff1);
    assert!(cpu.iff2);
    assert!(!cpu.p);

    // The byte read during the NMI M1 response is deliberately a HALT opcode.
    // It must be ignored and must not advance PC or enter HALT.
    cpu.insert_wait_tstates(2).unwrap();
    cpu = round_trip(cpu);
    assert_eq!(
        cpu.complete_bus_cycle(response.id, Some(0x76)).unwrap(),
        Z80MicroStep::InternalCycle { tstates: 1 }
    );
    assert_eq!(cpu.registers.pc, 0x1234);
    assert_eq!(cpu.refresh, 0xFF);
    let response_state = cpu.export_state();
    assert_eq!(
        response_state.phase,
        Z80MicroSequencerPhase::NmiStackPushInternal {
            return_address: 0x1234
        }
    );
    assert_eq!(response_state.mcycle_index, 0);
    assert_eq!(response_state.tstate_index, 4);

    cpu = round_trip(cpu);
    let high = request(cpu.complete_internal_cycle().unwrap());
    let high_state = cpu.export_state();
    assert_eq!(high_state.mcycle_index, 1);
    assert_eq!(high_state.tstate_index, 0);
    assert_eq!(high.operation, Z80BusOperation::MemoryWrite { value: 0x12 });
    assert_eq!(high.address, 0xFFFF);
    assert_eq!(high.base_tstates, 3);
    assert_eq!(cpu.registers.sp, 0xFFFF);

    cpu.insert_wait_tstates(1).unwrap();
    cpu = round_trip(cpu);
    let low = request(cpu.complete_bus_cycle(high.id, None).unwrap());
    let low_state = cpu.export_state();
    assert_eq!(low_state.mcycle_index, 2);
    assert_eq!(low_state.tstate_index, 0);
    assert_eq!(low.operation, Z80BusOperation::MemoryWrite { value: 0x34 });
    assert_eq!(low.address, 0xFFFE);
    assert_eq!(low.base_tstates, 3);
    assert_eq!(cpu.registers.sp, 0xFFFE);
    assert_eq!(cpu.registers.pc, 0x1234);

    cpu.insert_wait_tstates(3).unwrap();
    cpu = round_trip(cpu);
    assert_eq!(
        cpu.complete_bus_cycle(low.id, None).unwrap(),
        Z80MicroStep::InterruptServiced {
            kind: Z80InterruptKind::Nmi,
            tstates: 17,
        }
    );
    assert_eq!(cpu.registers.pc, 0x0066);
    assert_eq!(cpu.wz, 0x0066);
    assert_eq!(cpu.registers.sp, 0xFFFE);
    assert!(!cpu.iff1);
    assert!(cpu.iff2);
    assert_eq!(cpu.ei_delay, 1, "NMI must not consume EI delay");
    assert_eq!(cpu.int_pending, Some(0xFF), "NMI must not clear INT");
    assert!(!cpu.halted);
    assert!(!cpu.nmi_pending);
    assert_eq!(cpu.refresh, 0xFF);
}

#[test]
fn nmi_edge_latched_during_an_instruction_is_accepted_at_the_next_boundary() {
    let mut cpu = Z80MicroSequencer::default();
    cpu.registers.pc = 0x4000;

    let fetch = request(cpu.begin_instruction().unwrap());
    cpu.request_nmi();
    assert!(cpu.nmi_pending);
    cpu = round_trip(cpu);
    assert!(
        cpu.nmi_pending,
        "the pending NMI edge must survive snapshot restore"
    );
    assert_eq!(
        cpu.complete_bus_cycle(fetch.id, Some(0x00)).unwrap(),
        Z80MicroStep::InstructionRetired {
            opcode: 0x00,
            tstates: 4,
        }
    );
    assert_eq!(cpu.registers.pc, 0x4001);
    assert!(cpu.nmi_pending);

    let response = request(cpu.begin_instruction().unwrap());
    assert_eq!(response.operation, Z80BusOperation::OpcodeFetch);
    assert_eq!(response.address, 0x4001);
    assert_eq!(
        cpu.export_state().phase,
        Z80MicroSequencerPhase::NmiResponse
    );
    assert!(!cpu.nmi_pending);
}

#[test]
fn nmi_requested_during_halt_m1_is_serviced_at_the_next_boundary() {
    let mut cpu = Z80MicroSequencer::default();
    cpu.registers.pc = 0x4321;
    cpu.registers.sp = 0xC100;
    cpu.halted = true;

    let halt_m1 = request(cpu.begin_instruction().unwrap());
    assert_eq!(halt_m1.operation, Z80BusOperation::OpcodeFetch);
    assert_eq!(halt_m1.address, 0x4321);

    cpu.request_nmi();
    assert_eq!(
        cpu.complete_bus_cycle(halt_m1.id, Some(0x76)).unwrap(),
        Z80MicroStep::InstructionRetired {
            opcode: 0x00,
            tstates: 4,
        }
    );
    assert_eq!(cpu.registers.pc, 0x4321);
    assert!(cpu.halted);
    assert!(cpu.nmi_pending);

    cpu.sample_halt_interrupts_at_t4().unwrap();
    assert!(!cpu.nmi_pending);
    assert_eq!(cpu.export_state().halt_exit_pending, Some(Z80HaltExit::Nmi));
    cpu = round_trip(cpu);

    let nmi_response = request(cpu.begin_instruction().unwrap());
    assert_eq!(nmi_response.address, 0x4321);
    assert_eq!(
        cpu.export_state().phase,
        Z80MicroSequencerPhase::NmiResponse
    );
    assert!(!cpu.halted);
    assert!(!cpu.nmi_pending);
}

#[test]
fn nmi_service_is_independent_of_iff1_and_interrupt_mode() {
    let mut cpu = Z80MicroSequencer::default();
    cpu.registers.pc = 0x2345;
    cpu.registers.sp = 0xC100;
    cpu.iff1 = false;
    cpu.iff2 = true;
    cpu.interrupt_mode = 2;
    cpu.request_nmi();

    let response = request(cpu.begin_instruction().unwrap());
    assert_eq!(response.address, 0x2345);
    assert!(!cpu.iff1);
    assert!(!cpu.iff2, "IFF2 receives the previous IFF1 value");
    assert!(!cpu.nmi_pending);
}

#[test]
fn nmi_snapshot_rejects_a_response_request_for_the_wrong_address() {
    let mut cpu = Z80MicroSequencer::default();
    cpu.registers.pc = 0x3456;
    cpu.request_nmi();
    let _response = request(cpu.begin_instruction().unwrap());

    let mut state = cpu.export_state();
    state
        .handshake
        .pending_request
        .as_mut()
        .expect("NMI response must own an M1 request")
        .address = 0x3457;

    assert_eq!(
        Z80MicroSequencer::restore_state(state),
        Err(Z80MicroSequencerError::InvalidState)
    );
}

#[test]
fn retn_restores_iff1_after_resumable_nmi_entry() {
    let mut cpu = Z80MicroSequencer::default();
    cpu.registers.pc = 0x2000;
    cpu.registers.sp = 0xC100;
    cpu.iff1 = true;
    cpu.request_nmi();

    let response = request(cpu.begin_instruction().unwrap());
    assert_eq!(
        cpu.complete_bus_cycle(response.id, Some(0xA5)).unwrap(),
        Z80MicroStep::InternalCycle { tstates: 1 }
    );
    let high = request(cpu.complete_internal_cycle().unwrap());
    assert_eq!(high.operation, Z80BusOperation::MemoryWrite { value: 0x20 });
    let low = request(cpu.complete_bus_cycle(high.id, None).unwrap());
    assert_eq!(low.operation, Z80BusOperation::MemoryWrite { value: 0x00 });
    assert_eq!(
        cpu.complete_bus_cycle(low.id, None).unwrap(),
        Z80MicroStep::InterruptServiced {
            kind: Z80InterruptKind::Nmi,
            tstates: 11,
        }
    );
    assert!(!cpu.iff1);
    assert!(cpu.iff2);
    assert_eq!(cpu.registers.pc, 0x0066);

    let ed = request(cpu.begin_instruction().unwrap());
    let retn = request(cpu.complete_bus_cycle(ed.id, Some(0xED)).unwrap());
    assert_eq!(retn.address, 0x0067);
    let stack_low = request(cpu.complete_bus_cycle(retn.id, Some(0x45)).unwrap());
    assert_eq!(stack_low.operation, Z80BusOperation::MemoryRead);
    assert_eq!(stack_low.address, 0xC0FE);
    let stack_high = request(cpu.complete_bus_cycle(stack_low.id, Some(0x00)).unwrap());
    assert_eq!(stack_high.address, 0xC0FF);
    assert_eq!(
        cpu.complete_bus_cycle(stack_high.id, Some(0x20)).unwrap(),
        Z80MicroStep::InstructionRetired {
            opcode: 0x45,
            tstates: 14,
        }
    );
    assert_eq!(cpu.registers.pc, 0x2000);
    assert_eq!(cpu.registers.sp, 0xC100);
    assert!(cpu.iff1);
    assert!(cpu.iff2);
}

#[test]
fn halt_int_is_sampled_at_t4_and_latched_after_the_line_deasserts() {
    let mut cpu = Z80MicroSequencer {
        registers: sega_z80_cpu::Z80Registers {
            pc: 0x4321,
            ..sega_z80_cpu::Z80Registers::default()
        },
        iff1: true,
        iff2: true,
        halted: true,
        ..Z80MicroSequencer::default()
    };
    cpu.set_int_line(Some(0xFF));

    let halt_m1 = request(cpu.begin_instruction().unwrap());
    assert_eq!(halt_m1.operation, Z80BusOperation::OpcodeFetch);
    assert_eq!(
        cpu.complete_bus_cycle(halt_m1.id, Some(0x00)).unwrap(),
        Z80MicroStep::InstructionRetired {
            opcode: 0x00,
            tstates: 4,
        }
    );
    assert!(cpu.halted);
    assert_eq!(cpu.export_state().halt_exit_pending, None);

    cpu.sample_halt_interrupts_at_t4().unwrap();
    assert_eq!(cpu.export_state().halt_exit_pending, Some(Z80HaltExit::Int));

    // The accepted request is edge-sampled; deasserting INT after T4 must
    // not cancel the already-latched HALT exit.
    cpu.set_int_line(None);
    cpu = round_trip(cpu);
    let acknowledge = request(cpu.begin_instruction().unwrap());
    assert_eq!(acknowledge.operation, Z80BusOperation::InterruptAcknowledge);
    assert_eq!(
        cpu.export_state().phase,
        Z80MicroSequencerPhase::InterruptAcknowledge
    );
    assert!(!cpu.halted);
}

#[test]
fn int_asserted_after_halt_t4_waits_for_the_next_halt_m1_t4() {
    let mut cpu = Z80MicroSequencer {
        iff1: true,
        iff2: true,
        halted: true,
        ..Z80MicroSequencer::default()
    };

    let first_m1 = request(cpu.begin_instruction().unwrap());
    cpu.complete_bus_cycle(first_m1.id, Some(0x00)).unwrap();
    cpu.sample_halt_interrupts_at_t4().unwrap();
    assert_eq!(cpu.export_state().halt_exit_pending, None);

    cpu.set_int_line(Some(0xFF));
    let second_m1 = request(cpu.begin_instruction().unwrap());
    assert_eq!(second_m1.operation, Z80BusOperation::OpcodeFetch);
    cpu.complete_bus_cycle(second_m1.id, Some(0x00)).unwrap();
    cpu.sample_halt_interrupts_at_t4().unwrap();
    assert_eq!(cpu.export_state().halt_exit_pending, Some(Z80HaltExit::Int));

    let acknowledge = request(cpu.begin_instruction().unwrap());
    assert_eq!(acknowledge.operation, Z80BusOperation::InterruptAcknowledge);
}

#[test]
fn ei_halt_int_is_accepted_at_the_halt_instruction_t4() {
    let mut cpu = Z80MicroSequencer {
        iff1: true,
        iff2: true,
        ei_delay: 1,
        ..Z80MicroSequencer::default()
    };
    cpu.set_int_line(Some(0xFF));

    // INT is still inhibited by EI's one-instruction delay, so HALT itself
    // must be fetched and retired before the T4 sampler evaluates INT.
    let halt_opcode = request(cpu.begin_instruction().unwrap());
    assert_eq!(halt_opcode.operation, Z80BusOperation::OpcodeFetch);
    assert_eq!(
        cpu.complete_bus_cycle(halt_opcode.id, Some(0x76)).unwrap(),
        Z80MicroStep::InstructionRetired {
            opcode: 0x76,
            tstates: 4,
        }
    );
    assert!(cpu.halted);
    assert_eq!(cpu.ei_delay, 0);

    cpu.sample_halt_interrupts_at_t4().unwrap();
    assert_eq!(cpu.export_state().halt_exit_pending, Some(Z80HaltExit::Int));
    let acknowledge = request(cpu.begin_instruction().unwrap());
    assert_eq!(acknowledge.operation, Z80BusOperation::InterruptAcknowledge);
}

#[test]
fn nmi_has_priority_over_int_when_both_are_present_at_halt_t4() {
    let mut cpu = Z80MicroSequencer {
        iff1: true,
        iff2: true,
        halted: true,
        ..Z80MicroSequencer::default()
    };
    cpu.set_int_line(Some(0xFF));
    cpu.request_nmi();

    let halt_m1 = request(cpu.begin_instruction().unwrap());
    cpu.complete_bus_cycle(halt_m1.id, Some(0x00)).unwrap();
    cpu.sample_halt_interrupts_at_t4().unwrap();
    assert_eq!(cpu.export_state().halt_exit_pending, Some(Z80HaltExit::Nmi));
    assert_eq!(cpu.int_pending, Some(0xFF));

    let response = request(cpu.begin_instruction().unwrap());
    assert_eq!(response.operation, Z80BusOperation::OpcodeFetch);
    assert_eq!(
        cpu.export_state().phase,
        Z80MicroSequencerPhase::NmiResponse
    );
    assert!(!cpu.halted);
}

#[test]
fn masked_int_is_not_latched_until_iff1_is_enabled_at_a_later_halt_t4() {
    let mut cpu = Z80MicroSequencer {
        iff1: false,
        iff2: false,
        halted: true,
        ..Z80MicroSequencer::default()
    };
    cpu.set_int_line(Some(0xFF));

    let first_m1 = request(cpu.begin_instruction().unwrap());
    cpu.complete_bus_cycle(first_m1.id, Some(0x00)).unwrap();
    cpu.sample_halt_interrupts_at_t4().unwrap();
    assert_eq!(cpu.export_state().halt_exit_pending, None);

    // Enabling IFF1 between T4 boundaries must not accept the unsampled line.
    cpu.iff1 = true;
    let second_m1 = request(cpu.begin_instruction().unwrap());
    assert_eq!(second_m1.operation, Z80BusOperation::OpcodeFetch);
    cpu.complete_bus_cycle(second_m1.id, Some(0x00)).unwrap();
    cpu.sample_halt_interrupts_at_t4().unwrap();
    assert_eq!(cpu.export_state().halt_exit_pending, Some(Z80HaltExit::Int));
    assert_eq!(
        request(cpu.begin_instruction().unwrap()).operation,
        Z80BusOperation::InterruptAcknowledge
    );
}

#[test]
fn nmi_is_sampled_during_halt_even_when_iff1_is_disabled() {
    let mut cpu = Z80MicroSequencer {
        iff1: false,
        iff2: false,
        halted: true,
        ..Z80MicroSequencer::default()
    };
    cpu.request_nmi();

    let halt_m1 = request(cpu.begin_instruction().unwrap());
    cpu.complete_bus_cycle(halt_m1.id, Some(0x00)).unwrap();
    cpu.sample_halt_interrupts_at_t4().unwrap();
    assert_eq!(cpu.export_state().halt_exit_pending, Some(Z80HaltExit::Nmi));

    let response = request(cpu.begin_instruction().unwrap());
    assert_eq!(response.operation, Z80BusOperation::OpcodeFetch);
    assert_eq!(
        cpu.export_state().phase,
        Z80MicroSequencerPhase::NmiResponse
    );
    assert!(!cpu.halted);
}

#[test]
fn nmi_edge_after_halt_t4_is_sampled_on_the_next_halt_cycle() {
    let mut cpu = Z80MicroSequencer {
        iff1: false,
        iff2: false,
        halted: true,
        ..Z80MicroSequencer::default()
    };

    let first_m1 = request(cpu.begin_instruction().unwrap());
    cpu.complete_bus_cycle(first_m1.id, Some(0x00)).unwrap();
    cpu.sample_halt_interrupts_at_t4().unwrap();
    assert_eq!(cpu.export_state().halt_exit_pending, None);

    // An edge after T4 is pending, but cannot be accepted until the next
    // HALT M1 reaches its own effective T4 boundary.
    cpu.request_nmi();
    let second_m1 = request(cpu.begin_instruction().unwrap());
    assert_eq!(second_m1.operation, Z80BusOperation::OpcodeFetch);
    cpu.complete_bus_cycle(second_m1.id, Some(0x00)).unwrap();
    cpu.sample_halt_interrupts_at_t4().unwrap();
    assert_eq!(
        cpu.export_state().halt_exit_pending,
        Some(Z80HaltExit::Nmi)
    );

    let response = request(cpu.begin_instruction().unwrap());
    assert_eq!(response.operation, Z80BusOperation::OpcodeFetch);
    assert_eq!(
        cpu.export_state().phase,
        Z80MicroSequencerPhase::NmiResponse
    );
}
