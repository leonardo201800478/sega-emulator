//! E7.7 F-004: scheduler integration for BASE 16-bit pair loads.

use sega_sms_core::SmsTemporalMachine;
use sega_sms_core::temporal_machine::SmsTemporalTraceEvent;
use sega_temporal_core::MachineTime;
use sega_z80_cpu::{Z80BusOperation, Z80MicroStep, Z80Registers};

const INITIAL_WZ: u16 = 0xA55A;
const INITIAL_Q: u8 = 0x3C;

fn initial_registers() -> Z80Registers {
    Z80Registers {
        a: 0xA5,
        f: 0xD7,
        b: 0x11,
        c: 0x22,
        d: 0x33,
        e: 0x44,
        h: 0x55,
        l: 0x66,
        sp: 0x7788,
        pc: 0,
    }
}

fn configure(machine: &mut SmsTemporalMachine, registers: Z80Registers) {
    machine.set_trace_enabled(true);
    machine.cpu_mut().registers = registers;
    machine.cpu_mut().wz = INITIAL_WZ;
    machine.cpu_mut().q = INITIAL_Q;
    machine.cpu_mut().refresh = 0xFF;
}

fn set_pair(registers: &mut Z80Registers, pair: u8, value: u16) {
    match pair {
        0 => registers.set_bc(value),
        1 => registers.set_de(value),
        2 => registers.set_hl(value),
        3 => registers.sp = value,
        _ => unreachable!("two-bit register pair"),
    }
}

fn assert_resume_case(
    program: &[u8],
    expected_opcode: u8,
    expected_tstates: u32,
    initial: Z80Registers,
    expected_registers: Z80Registers,
    expected_wz: u16,
    expected_memory: &[(u16, u8)],
    expected_bus: Vec<(Z80BusOperation, u16, u8, u32, Option<u8>)>,
) {
    let mut continuous = SmsTemporalMachine::new(program.to_vec(), 3).unwrap();
    configure(&mut continuous, initial);

    let mut machine = SmsTemporalMachine::new(program.to_vec(), 3).unwrap();
    configure(&mut machine, initial);

    let continuous_step = continuous.step_instruction(1).unwrap();
    assert!(matches!(
        machine.begin_instruction().unwrap(),
        Z80MicroStep::BusRequest(_)
    ));
    machine.schedule_pending_bus(1).unwrap();

    // Save while the first M1 scheduler continuation is pending.
    let snapshot = machine.export_state();
    let mut restored = SmsTemporalMachine::restore_state(snapshot).unwrap();
    restored.set_trace_enabled(true);
    let resumed_step = restored.resume_instruction(1).unwrap();

    assert_eq!(resumed_step, continuous_step);
    assert_eq!(
        resumed_step,
        Z80MicroStep::InstructionRetired {
            opcode: expected_opcode,
            tstates: expected_tstates,
        }
    );
    assert_eq!(restored.export_state(), continuous.export_state());
    assert_eq!(
        restored.now(),
        MachineTime::from_ticks(u128::from(expected_tstates) * 3),
        "scheduler time is the retired T-state count × three master ticks"
    );
    assert_eq!(restored.cpu().registers, expected_registers);
    assert_eq!(restored.cpu().wz, expected_wz);
    assert_eq!(restored.cpu().registers.f, initial.f, "F is not affected");
    assert_eq!(restored.cpu().q, 0);
    assert_eq!(restored.cpu().q_previous, INITIAL_Q);
    assert_eq!(
        restored.cpu().refresh,
        0x80,
        "R7 preserved across refresh wrap"
    );

    for &(address, value) in expected_memory {
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
        "opcode {expected_opcode:02X} bus trace"
    );
}

#[test]
fn ld_register_pair_immediate_all_four_pairs_resume_identically_through_sms_scheduler() {
    let cases: [(u8, u8, u16); 4] = [
        (0x01, 0, 0xA55A),
        (0x11, 1, 0x5AA5),
        (0x21, 2, 0xC39D),
        (0x31, 3, 0xF00D),
    ];
    for (opcode, pair, value) in cases {
        let [low, high] = value.to_le_bytes();
        let program = [opcode, low, high];
        let initial = initial_registers();
        let mut expected_registers = initial;
        set_pair(&mut expected_registers, pair, value);
        expected_registers.pc = 3;

        assert_resume_case(
            &program,
            opcode,
            13,
            initial,
            expected_registers,
            INITIAL_WZ,
            &[],
            vec![
                (Z80BusOperation::MemoryRead, 1, 3, 1, Some(low)),
                (Z80BusOperation::MemoryRead, 2, 3, 1, Some(high)),
            ],
        );
    }
}

#[test]
fn ld_hl_absolute_word_store_and_load_resume_through_sms_scheduler() {
    // LD (C000),HL: low byte is written first, then high byte.
    let store_program = [0x22, 0x00, 0xC0];
    let mut store_initial = initial_registers();
    store_initial.set_hl(0xBEEF);
    let mut store_registers = store_initial;
    store_registers.pc = 3;
    assert_resume_case(
        &store_program,
        0x22,
        21,
        store_initial,
        store_registers,
        0xC001,
        &[(0xC000, 0xEF), (0xC001, 0xBE)],
        vec![
            (Z80BusOperation::MemoryRead, 1, 3, 1, Some(0x00)),
            (Z80BusOperation::MemoryRead, 2, 3, 1, Some(0xC0)),
            (
                Z80BusOperation::MemoryWrite { value: 0xEF },
                0xC000,
                3,
                1,
                None,
            ),
            (
                Z80BusOperation::MemoryWrite { value: 0xBE },
                0xC001,
                3,
                1,
                None,
            ),
        ],
    );

    // LD HL,(0000): reads the opcode byte at 0000 as the low byte and byte
    // 0001 as the high byte, exercising the low/high memory order.
    let load_program = [0x2A, 0x00, 0x00];
    let load_initial = initial_registers();
    let mut load_registers = load_initial;
    load_registers.set_hl(0x002A);
    load_registers.pc = 3;
    assert_resume_case(
        &load_program,
        0x2A,
        21,
        load_initial,
        load_registers,
        0x0001,
        &[],
        vec![
            (Z80BusOperation::MemoryRead, 1, 3, 1, Some(0x00)),
            (Z80BusOperation::MemoryRead, 2, 3, 1, Some(0x00)),
            (Z80BusOperation::MemoryRead, 0, 3, 1, Some(0x2A)),
            (Z80BusOperation::MemoryRead, 1, 3, 1, Some(0x00)),
        ],
    );
}
