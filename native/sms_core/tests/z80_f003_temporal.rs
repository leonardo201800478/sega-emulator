use sega_sms_core::SmsTemporalMachine;
use sega_sms_core::temporal_machine::SmsTemporalTraceEvent;
use sega_temporal_core::MachineTime;
use sega_z80_cpu::{Z80BusOperation, Z80MicroStep};

const INITIAL_Q: u8 = 0x3C;
const INITIAL_WZ: u16 = 0xA55A;

#[allow(clippy::too_many_arguments)]
fn assert_scheduler_case(
    program: &[u8],
    initial_a: u8,
    expected_a: u8,
    expected_wz: u16,
    expected_memory: Option<(u16, u8)>,
    expected_bus: Vec<(Z80BusOperation, u16, u8, u32, Option<u8>)>,
) {
    let mut continuous = SmsTemporalMachine::new(program.to_vec(), 3).unwrap();
    continuous.set_trace_enabled(true);
    continuous.cpu_mut().registers.a = initial_a;
    continuous.cpu_mut().registers.f = 0xA5;
    continuous.cpu_mut().registers.set_bc(0x1122);
    continuous.cpu_mut().registers.set_de(0x3344);
    continuous.cpu_mut().registers.set_hl(0x5566);
    continuous.cpu_mut().wz = INITIAL_WZ;
    continuous.cpu_mut().q = INITIAL_Q;
    continuous.cpu_mut().refresh = 0x80;

    let mut machine = SmsTemporalMachine::new(program.to_vec(), 3).unwrap();
    machine.set_trace_enabled(true);
    machine.cpu_mut().registers.a = initial_a;
    machine.cpu_mut().registers.f = 0xA5;
    machine.cpu_mut().registers.set_bc(0x1122);
    machine.cpu_mut().registers.set_de(0x3344);
    machine.cpu_mut().registers.set_hl(0x5566);
    machine.cpu_mut().wz = INITIAL_WZ;
    machine.cpu_mut().q = INITIAL_Q;
    machine.cpu_mut().refresh = 0x80;

    let continuous_step = continuous.step_instruction(1).unwrap();
    assert!(matches!(
        machine.begin_instruction().unwrap(),
        Z80MicroStep::BusRequest(_)
    ));
    machine.schedule_pending_bus(1).unwrap();

    // Snapshot while the first M1 scheduler continuation is pending.
    let snapshot = machine.export_state();
    let mut restored = SmsTemporalMachine::restore_state(snapshot).unwrap();
    restored.set_trace_enabled(true);
    assert_eq!(restored.resume_instruction(1).unwrap(), continuous_step);
    assert_eq!(
        continuous_step,
        Z80MicroStep::InstructionRetired {
            opcode: program[0],
            tstates: 17,
        }
    );
    assert_eq!(restored.export_state(), continuous.export_state());
    assert_eq!(
        restored.now(),
        MachineTime::from_ticks(51),
        "17 T-states at three master ticks per T-state"
    );
    assert_eq!(restored.cpu().registers.pc, 3);
    assert_eq!(restored.cpu().registers.a, expected_a);
    assert_eq!(restored.cpu().registers.f, 0xA5);
    assert_eq!(restored.cpu().wz, expected_wz);
    assert_eq!(restored.cpu().q, 0);
    assert_eq!(restored.cpu().q_previous, INITIAL_Q);
    assert_eq!(restored.cpu().refresh, 0x81);

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
    assert_eq!(
        bus_completions, expected_bus,
        "opcode {:02X} bus trace",
        program[0]
    );
}

#[test]
fn f003_absolute_accumulator_load_store_resume_identically_through_sms_scheduler() {
    // LD A,(0000): after reading its two-byte address, the data read returns
    // the first ROM byte (the opcode itself).
    assert_scheduler_case(
        &[0x3A, 0x00, 0x00],
        0xC3,
        0x3A,
        0x0001,
        None,
        vec![
            (Z80BusOperation::MemoryRead, 1, 3, 1, Some(0x00)),
            (Z80BusOperation::MemoryRead, 2, 3, 1, Some(0x00)),
            (Z80BusOperation::MemoryRead, 0, 3, 1, Some(0x3A)),
        ],
    );

    // LD (C000),A: verify the RAM write and the NMOS WZ high-byte=A rule.
    assert_scheduler_case(
        &[0x32, 0x00, 0xC0],
        0xA5,
        0xA5,
        0xA501,
        Some((0xC000, 0xA5)),
        vec![
            (Z80BusOperation::MemoryRead, 1, 3, 1, Some(0x00)),
            (Z80BusOperation::MemoryRead, 2, 3, 1, Some(0xC0)),
            (
                Z80BusOperation::MemoryWrite { value: 0xA5 },
                0xC000,
                3,
                1,
                None,
            ),
        ],
    );
}
