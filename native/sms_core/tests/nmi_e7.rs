use sega_sms_core::{SmsTemporalMachine, SmsTemporalTraceEvent};
use sega_temporal_core::MachineTime;
use sega_z80_cpu::{Z80BusOperation, Z80InterruptKind, Z80MicroStep};

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
    assert_eq!(m1_pins.len(), 4, "NMI response must execute one four-T-state M1");
    assert_eq!(m1_pins[0].address, 0x0001);
    assert!(!m1_pins[0].m1_n);
    assert!(!m1_pins[1].mreq_n);
    assert!(!m1_pins[1].rd_n);
    assert!(!m1_pins[2].mreq_n);
    assert!(!m1_pins[2].rd_n);
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
