use sega_sms_core::SmsTemporalMachine;
use sega_sms_core::temporal_machine::SmsTemporalTraceEvent;
use sega_temporal_core::MachineTime;
use sega_z80_cpu::{Z80BusOperation, Z80HaltExit, Z80InterruptKind, Z80MicroStep};

fn nmi_machine() -> SmsTemporalMachine {
    let mut machine = SmsTemporalMachine::new(vec![0x00, 0x76], 3).unwrap();
    machine.cpu_mut().registers.pc = 1;
    machine.cpu_mut().registers.sp = 0xC100;
    machine.cpu_mut().refresh = 0x80;
    machine.cpu_mut().iff1 = true;
    machine.cpu_mut().iff2 = false;
    machine.cpu_mut().ei_delay = 1;
    machine.request_nmi();
    machine
}

#[test]
fn sms_temporal_nmi_uses_m1_ignores_data_and_pushes_pc_to_ram() {
    let mut machine = nmi_machine();
    machine.set_trace_enabled(true);

    assert_eq!(
        machine.step_instruction(0).unwrap(),
        Z80MicroStep::InterruptServiced {
            kind: Z80InterruptKind::Nmi,
            tstates: 11,
        }
    );

    assert_eq!(machine.cpu().registers.pc, 0x0066);
    assert_eq!(machine.cpu().registers.sp, 0xC0FE);
    assert_eq!(machine.cpu().refresh, 0x81);
    assert!(!machine.cpu().iff1);
    assert!(machine.cpu().iff2);
    assert_eq!(machine.cpu().ei_delay, 1);
    assert_eq!(machine.memory().read(0xC0FF), 0x00);
    assert_eq!(machine.memory().read(0xC0FE), 0x01);
    assert_eq!(machine.now(), MachineTime::from_ticks(33));

    let trace = machine.take_trace();
    let m1_pins: Vec<_> = trace
        .iter()
        .filter_map(|event| match event {
            SmsTemporalTraceEvent::M1TStateCompleted { pins, .. } => Some(*pins),
            _ => None,
        })
        .collect();
    assert_eq!(
        m1_pins.len(),
        4,
        "NMI response must execute one four-T-state M1"
    );
    assert_eq!(m1_pins[0].address, 0x0001);
    assert!(!m1_pins[0].m1_n);
    assert!(!m1_pins[1].mreq_n);
    assert!(!m1_pins[1].rd_n);
    assert!(!m1_pins[2].mreq_n);
    assert!(!m1_pins[2].rd_n);
    assert_eq!(
        m1_pins[3].address, 0x0081,
        "refresh address must use I:R after R increments"
    );
    assert!(!m1_pins[3].rfsh_n, "M1 T4 must expose the refresh phase");

    let bus_cycles: Vec<_> = trace
        .into_iter()
        .filter_map(|event| match event {
            SmsTemporalTraceEvent::BusCompleted {
                operation,
                address,
                data,
                ..
            } => Some((operation, address, data)),
            _ => None,
        })
        .collect();
    assert_eq!(
        bus_cycles,
        vec![
            (Z80BusOperation::MemoryWrite { value: 0x00 }, 0xC0FF, None),
            (Z80BusOperation::MemoryWrite { value: 0x01 }, 0xC0FE, None),
        ]
    );
}

#[test]
fn sms_temporal_nmi_response_snapshot_resumes_without_repeating_m1() {
    let mut machine = nmi_machine();
    let initial = machine.begin_instruction().unwrap();
    assert!(matches!(initial, Z80MicroStep::BusRequest(_)));

    // Schedule the NMI response M1, then snapshot while its first T-state is pending.
    machine.schedule_pending_bus(0).unwrap();
    let snapshot = machine.export_state();
    let mut restored = SmsTemporalMachine::restore_state(snapshot).unwrap();

    assert_eq!(
        restored.resume_instruction(0).unwrap(),
        Z80MicroStep::InterruptServiced {
            kind: Z80InterruptKind::Nmi,
            tstates: 11,
        }
    );
    assert_eq!(restored.cpu().registers.pc, 0x0066);
    assert_eq!(restored.cpu().registers.sp, 0xC0FE);
    assert_eq!(restored.memory().read(0xC0FF), 0x00);
    assert_eq!(restored.memory().read(0xC0FE), 0x01);
    assert_eq!(restored.now(), MachineTime::from_ticks(33));
}

#[test]
fn sms_temporal_nmi_waits_extend_m1_and_both_stack_writes() {
    let mut machine = nmi_machine();

    assert_eq!(
        machine.step_instruction(1).unwrap(),
        Z80MicroStep::InterruptServiced {
            kind: Z80InterruptKind::Nmi,
            tstates: 14,
        }
    );
    assert_eq!(machine.now(), MachineTime::from_ticks(42));
    assert_eq!(machine.cpu().registers.pc, 0x0066);
    assert_eq!(machine.cpu().registers.sp, 0xC0FE);
    assert_eq!(machine.memory().read(0xC0FF), 0x00);
    assert_eq!(machine.memory().read(0xC0FE), 0x01);
}


#[test]
fn sms_temporal_halt_samples_int_after_effective_t4_and_restores_the_latch() {
    let mut machine = SmsTemporalMachine::new(vec![0x76, 0x00], 3).unwrap();
    machine.cpu_mut().iff1 = true;
    machine.cpu_mut().iff2 = true;
    machine.cpu_mut().registers.sp = 0xC100;
    machine.set_int_line(Some(0xFF));

    // Two WAIT states extend this M1. The sample must occur at its effective
    // T4 completion, after the HALT opcode has been decoded and retired.
    assert_eq!(
        machine.step_instruction(2).unwrap(),
        Z80MicroStep::InstructionRetired {
            opcode: 0x76,
            tstates: 6,
        }
    );
    assert!(machine.cpu().halted);
    assert_eq!(
        machine.cpu().halt_exit_pending,
        Some(Z80HaltExit::Int)
    );
    assert_eq!(machine.now(), MachineTime::from_ticks(18));

    let state = machine.export_state();
    let mut restored = SmsTemporalMachine::restore_state(state).unwrap();
    restored.set_int_line(None);
    let acknowledge = match restored.begin_instruction().unwrap() {
        Z80MicroStep::BusRequest(request) => request,
        other => panic!("expected latched INT acknowledge, got {other:?}"),
    };
    assert_eq!(
        acknowledge.operation,
        Z80BusOperation::InterruptAcknowledge
    );
    assert!(!restored.cpu().halted);
}

#[test]
fn sms_temporal_int_asserted_after_halt_t4_waits_for_next_halt_cycle() {
    let mut machine = SmsTemporalMachine::new(vec![0x76, 0x00], 3).unwrap();
    machine.cpu_mut().iff1 = true;
    machine.cpu_mut().iff2 = true;

    assert_eq!(
        machine.step_instruction(0).unwrap(),
        Z80MicroStep::InstructionRetired {
            opcode: 0x76,
            tstates: 4,
        }
    );
    assert_eq!(machine.cpu().halt_exit_pending, None);

    machine.set_int_line(Some(0xFF));
    assert_eq!(
        machine.step_instruction(0).unwrap(),
        Z80MicroStep::InstructionRetired {
            opcode: 0x00,
            tstates: 4,
        }
    );
    assert_eq!(
        machine.cpu().halt_exit_pending,
        Some(Z80HaltExit::Int)
    );

    let acknowledge = match machine.begin_instruction().unwrap() {
        Z80MicroStep::BusRequest(request) => request,
        other => panic!("expected INT acknowledge after next HALT T4, got {other:?}"),
    };
    assert_eq!(
        acknowledge.operation,
        Z80BusOperation::InterruptAcknowledge
    );
}

#[test]
fn sms_temporal_ei_halt_int_observes_delay_consumption_at_halt_t4() {
    let mut machine = SmsTemporalMachine::new(vec![0xFB, 0x76, 0x00], 3).unwrap();
    machine.cpu_mut().registers.sp = 0xC100;
    machine.set_int_line(Some(0xFF));

    assert_eq!(
        machine.step_instruction(0).unwrap(),
        Z80MicroStep::InstructionRetired {
            opcode: 0xFB,
            tstates: 4,
        }
    );
    assert_eq!(machine.cpu().ei_delay, 1);
    assert!(machine.cpu().iff1 && machine.cpu().iff2);

    assert_eq!(
        machine.step_instruction(0).unwrap(),
        Z80MicroStep::InstructionRetired {
            opcode: 0x76,
            tstates: 4,
        }
    );
    assert!(machine.cpu().halted);
    assert_eq!(machine.cpu().ei_delay, 0);
    assert_eq!(
        machine.cpu().halt_exit_pending,
        Some(Z80HaltExit::Int)
    );

    let acknowledge = match machine.begin_instruction().unwrap() {
        Z80MicroStep::BusRequest(request) => request,
        other => panic!("expected EI;HALT;INT acknowledge, got {other:?}"),
    };
    assert_eq!(
        acknowledge.operation,
        Z80BusOperation::InterruptAcknowledge
    );
}
