use sega_sms_core::SmsTemporalMachine;
use sega_sms_core::temporal_machine::SmsTemporalTraceEvent;
use sega_temporal_core::MachineTime;
use sega_z80_cpu::{Z80BusOperation, Z80MicroStep};

const INITIAL_WZ: u16 = 0xA55A;
const INITIAL_Q: u8 = 0x3C;

#[allow(clippy::too_many_arguments)]
fn assert_scheduler_case(
    program: &[u8],
    a: u8,
    bc: u16,
    de: u16,
    hl: u16,
    expected_a: Option<u8>,
    expected_b: Option<u8>,
    expected_hl: Option<u16>,
    expected_memory: Option<(u16, u8)>,
    expected_tstates: u32,
    expected_bus: Vec<(Z80BusOperation, u16, u8, u32, Option<u8>)>,
) {
    let mut continuous = SmsTemporalMachine::new(program.to_vec(), 3).unwrap();
    continuous.set_trace_enabled(true);
    continuous.cpu_mut().registers.a = a;
    continuous.cpu_mut().registers.f = 0xA5;
    continuous.cpu_mut().registers.set_bc(bc);
    continuous.cpu_mut().registers.set_de(de);
    continuous.cpu_mut().registers.set_hl(hl);
    continuous.cpu_mut().wz = INITIAL_WZ;
    continuous.cpu_mut().q = INITIAL_Q;

    let mut machine = SmsTemporalMachine::new(program.to_vec(), 3).unwrap();
    machine.set_trace_enabled(true);
    machine.cpu_mut().registers.a = a;
    machine.cpu_mut().registers.f = 0xA5;
    machine.cpu_mut().registers.set_bc(bc);
    machine.cpu_mut().registers.set_de(de);
    machine.cpu_mut().registers.set_hl(hl);
    machine.cpu_mut().wz = INITIAL_WZ;
    machine.cpu_mut().q = INITIAL_Q;

    let continuous_step = continuous.step_instruction(1).unwrap();
    assert!(matches!(
        machine.begin_instruction().unwrap(),
        Z80MicroStep::BusRequest(_)
    ));
    machine.schedule_pending_bus(1).unwrap();

    // Snapshot with the first M1 scheduler continuation pending.
    let snapshot = machine.export_state();
    let mut restored = SmsTemporalMachine::restore_state(snapshot).unwrap();
    restored.set_trace_enabled(true);
    assert_eq!(restored.resume_instruction(1).unwrap(), continuous_step);
    assert_eq!(
        continuous_step,
        Z80MicroStep::InstructionRetired {
            opcode: program[0],
            tstates: expected_tstates,
        }
    );
    assert_eq!(restored.export_state(), continuous.export_state());
    assert_eq!(restored.now(), MachineTime::from_ticks(u128::from(expected_tstates) * 3));
    assert_eq!(restored.cpu().registers.pc, program.len() as u16);
    assert_eq!(restored.cpu().registers.f, 0xA5);
    assert_eq!(restored.cpu().wz, INITIAL_WZ);
    assert_eq!(restored.cpu().q, 0);
    assert_eq!(restored.cpu().q_previous, INITIAL_Q);
    assert_eq!(restored.cpu().refresh & 0x7F, 1);

    if let Some(value) = expected_a {
        assert_eq!(restored.cpu().registers.a, value);
    }
    if let Some(value) = expected_b {
        assert_eq!(restored.cpu().registers.b, value);
    }
    if let Some(value) = expected_hl {
        assert_eq!(restored.cpu().registers.hl(), value);
    }
    if let Some((address, value)) = expected_memory {
        assert_eq!(restored.memory().read(address), value);
    }

    let restored_trace = restored.take_trace();
    assert_eq!(restored_trace, continuous.take_trace());
    let bus_completions: Vec<_> = restored_trace
        .into_iter()
        .filter_map(|event| match event {
            SmsTemporalTraceEvent::BusCompleted {
                operation,
                address,
                base_tstates,
                wait_tstates,
                data,
                ..
            } => Some((operation, address, base_tstates, wait_tstates, data)),
            _ => None,
        })
        .collect();
    assert_eq!(bus_completions, expected_bus, "opcode {:02X} bus trace", program[0]);
}

#[test]
fn f001_register_immediate_and_hl_forms_resume_identically_through_sms_scheduler() {
    // Register-only LD B,C: one M1, 4 nominal T-states.
    assert_scheduler_case(
        &[0x41],
        0x77,
        0x1122,
        0x3344,
        0xC100,
        None,
        Some(0x22),
        None,
        None,
        5,
        vec![],
    );

    // LD A,n: M1 followed by the immediate operand read.
    assert_scheduler_case(
        &[0x3E, 0xA5],
        0x11,
        0x1122,
        0x3344,
        0xC100,
        Some(0xA5),
        None,
        None,
        None,
        9,
        vec![(Z80BusOperation::MemoryRead, 1, 3, 1, Some(0xA5))],
    );

    // LD A,(HL): the data read uses the original HL address.
    assert_scheduler_case(
        &[0x7E],
        0x11,
        0x1122,
        0x3344,
        0x0000,
        Some(0x7E),
        None,
        None,
        None,
        9,
        vec![(Z80BusOperation::MemoryRead, 0, 3, 1, Some(0x7E))],
    );

    // LD H,(HL): the read address is sampled before H changes.
    assert_scheduler_case(
        &[0x66],
        0x11,
        0x1122,
        0x3344,
        0x0000,
        None,
        None,
        Some(0x6600),
        None,
        9,
        vec![(Z80BusOperation::MemoryRead, 0, 3, 1, Some(0x66))],
    );

    // LD (HL),A: the write uses the original HL address.
    assert_scheduler_case(
        &[0x77],
        0x5A,
        0x1122,
        0x3344,
        0xC000,
        None,
        None,
        None,
        Some((0xC000, 0x5A)),
        9,
        vec![(
            Z80BusOperation::MemoryWrite { value: 0x5A },
            0xC000,
            3,
            1,
            None,
        )],
    );

    // LD (HL),H: source H is sampled from the original HL before the write.
    assert_scheduler_case(
        &[0x74],
        0x11,
        0x1122,
        0x3344,
        0xC000,
        None,
        None,
        Some(0xC000),
        Some((0xC000, 0xC0)),
        9,
        vec![(
            Z80BusOperation::MemoryWrite { value: 0xC0 },
            0xC000,
            3,
            1,
            None,
        )],
    );

    // LD (HL),n: immediate read precedes the memory write; HL wraps at FFFF.
    assert_scheduler_case(
        &[0x36, 0xA5],
        0x11,
        0x1122,
        0x3344,
        0xFFFF,
        None,
        None,
        Some(0xFFFF),
        Some((0xFFFF, 0xA5)),
        13,
        vec![
            (Z80BusOperation::MemoryRead, 1, 3, 1, Some(0xA5)),
            (
                Z80BusOperation::MemoryWrite { value: 0xA5 },
                0xFFFF,
                3,
                1,
                None,
            ),
        ],
    );
}
