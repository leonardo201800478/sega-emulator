//! E7.7 F-015: accumulator rotate integration with the SMS MachineScheduler.

use sega_sms_core::SmsTemporalMachine;
use sega_sms_core::temporal_machine::SmsTemporalTraceEvent;
use sega_temporal_core::MachineTime;
use sega_z80_cpu::{Z80MicroStep, Z80Registers};

const INITIAL_WZ: u16 = 0xA55A;
const INITIAL_Q: u8 = 0x3C;
const FLAG_C: u8 = 0x01;
const FLAG_PV: u8 = 0x04;
const FLAG_Z: u8 = 0x40;
const FLAG_S: u8 = 0x80;
const FLAG_XY: u8 = 0x28;

fn initial_registers(accumulator: u8, flags: u8) -> Z80Registers {
    Z80Registers {
        a: accumulator,
        f: flags,
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

fn expected_rotate(accumulator: u8, flags: u8, operation: u8) -> (u8, u8) {
    let old_carry = flags & FLAG_C != 0;
    let (result, carry) = match operation {
        0 => (accumulator.rotate_left(1), accumulator & 0x80 != 0),
        1 => (accumulator.rotate_right(1), accumulator & 0x01 != 0),
        2 => (
            (accumulator << 1) | u8::from(old_carry),
            accumulator & 0x80 != 0,
        ),
        3 => (
            (accumulator >> 1) | (u8::from(old_carry) << 7),
            accumulator & 0x01 != 0,
        ),
        _ => unreachable!("two-bit accumulator rotate operation"),
    };
    let expected_flags =
        (flags & (FLAG_S | FLAG_Z | FLAG_PV)) | (result & FLAG_XY) | if carry { FLAG_C } else { 0 };
    (result, expected_flags)
}

fn configure(machine: &mut SmsTemporalMachine, registers: Z80Registers) {
    machine.set_trace_enabled(true);
    machine.cpu_mut().registers = registers;
    machine.cpu_mut().wz = INITIAL_WZ;
    machine.cpu_mut().q = INITIAL_Q;
    machine.cpu_mut().refresh = 0xFF;
}

fn assert_scheduler_case(opcode: u8, operation: u8, accumulator: u8, flags: u8) {
    let initial = initial_registers(accumulator, flags);
    let (expected_a, expected_f) = expected_rotate(accumulator, flags, operation);
    let mut expected_registers = initial;
    expected_registers.a = expected_a;
    expected_registers.f = expected_f;
    expected_registers.pc = 1;

    let mut continuous = SmsTemporalMachine::new(vec![opcode], 3).unwrap();
    configure(&mut continuous, initial);
    let continuous_step = continuous.step_instruction(1).unwrap();

    let mut machine = SmsTemporalMachine::new(vec![opcode], 3).unwrap();
    configure(&mut machine, initial);
    assert!(matches!(
        machine.begin_instruction().unwrap(),
        Z80MicroStep::BusRequest(_)
    ));
    machine.schedule_pending_bus(1).unwrap();
    let snapshot = machine.export_state();
    let mut restored = SmsTemporalMachine::restore_state(snapshot).unwrap();
    restored.set_trace_enabled(true);
    let resumed_step = restored.resume_instruction(1).unwrap();

    assert_eq!(continuous_step, resumed_step);
    assert_eq!(
        resumed_step,
        Z80MicroStep::InstructionRetired { opcode, tstates: 5 }
    );
    assert_eq!(restored.export_state(), continuous.export_state());
    assert_eq!(restored.now(), MachineTime::from_ticks(15));
    assert_eq!(restored.cpu().registers, expected_registers);
    assert_eq!(restored.cpu().wz, INITIAL_WZ);
    assert_eq!(restored.cpu().q, expected_f);
    assert_eq!(restored.cpu().q_previous, INITIAL_Q);
    assert_eq!(restored.cpu().refresh, 0x80);

    let restored_trace = restored.take_trace();
    assert_eq!(restored_trace, continuous.take_trace());
    assert!(
        restored_trace
            .iter()
            .all(|event| !matches!(event, SmsTemporalTraceEvent::BusCompleted { .. })),
        "single-M1 accumulator rotate must not issue data bus transactions"
    );
}

#[test]
fn accumulator_rotates_resume_identically_through_sms_scheduler() {
    let accumulator_values = [0x00, 0x01, 0x02, 0x7F, 0x80, 0x81, 0xFE, 0xFF];
    let flag_patterns = [0x00, 0x01, 0x04, 0x45, 0xD7, 0xFF];
    let rotates = [(0x07, 0), (0x0F, 1), (0x17, 2), (0x1F, 3)];

    for (opcode, operation) in rotates {
        for accumulator in accumulator_values {
            for flags in flag_patterns {
                assert_scheduler_case(opcode, operation, accumulator, flags);
            }
        }
    }
}
