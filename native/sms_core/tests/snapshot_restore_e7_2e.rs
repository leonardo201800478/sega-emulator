use sega_sms_core::SmsTemporalMachine;
use sega_temporal_core::MachineTime;
use sega_z80_cpu::{Z80InterruptKind, Z80MicroSequencerState, Z80MicroStep};

fn machine(rom: Vec<u8>) -> SmsTemporalMachine {
    SmsTemporalMachine::new(rom, 3).unwrap()
}

fn snapshot(machine: &SmsTemporalMachine) -> SmsTemporalMachine {
    let mut state = machine.export_state();
    let bytes = state.cpu.encode().unwrap();
    let decoded = Z80MicroSequencerState::decode(&bytes).unwrap();
    assert_eq!(decoded, state.cpu);
    state.cpu = decoded;
    let mut restored = SmsTemporalMachine::restore_state(state).unwrap();
    restored.set_trace_enabled(true);
    restored
}

// Snapshot only at scheduler-owned safe points, after an event has been scheduled.
fn run_boundary(
    machine: &mut SmsTemporalMachine,
    snapshot_each: bool,
    waits: u32,
) -> (Z80MicroStep, Vec<sega_sms_core::temporal_machine::SmsTemporalTraceEvent>) {
    let mut captured_trace = Vec::new();
    let mut step = machine.begin_instruction().unwrap();
    for _ in 0..128 {
        step = match step {
            Z80MicroStep::BusRequest(_) => {
                machine.schedule_pending_bus(waits).unwrap();
                if snapshot_each {
                    captured_trace.extend(machine.take_trace());
                    *machine = snapshot(machine);
                }
                machine.dispatch_next().unwrap()
            }
            Z80MicroStep::InternalCycle { tstates } => {
                machine.schedule_internal_cycle(tstates).unwrap();
                if snapshot_each {
                    captured_trace.extend(machine.take_trace());
                    *machine = snapshot(machine);
                }
                machine.dispatch_next().unwrap()
            }
            Z80MicroStep::InterruptAcknowledge { .. } => {
                if snapshot_each {
                    captured_trace.extend(machine.take_trace());
                    *machine = snapshot(machine);
                }
                machine.cpu_mut().begin_instruction().unwrap()
            }
            terminal @ (Z80MicroStep::InstructionRetired { .. }
            | Z80MicroStep::InterruptServiced { .. }) => {
                captured_trace.extend(machine.take_trace());
                if snapshot_each {
                    *machine = snapshot(machine);
                }
                return (terminal, captured_trace);
            }
        };
    }
    panic!("microsequencer did not reach an instruction boundary");
}

fn compare_runs(
    continuous: &mut SmsTemporalMachine,
    resumed: &mut SmsTemporalMachine,
    waits: u32,
) -> Z80MicroStep {
    continuous.set_trace_enabled(true);
    resumed.set_trace_enabled(true);
    let (expected, continuous_trace) = run_boundary(continuous, false, waits);
    let (actual, resumed_trace) = run_boundary(resumed, true, waits);
    assert_eq!(actual, expected);
    assert_eq!(resumed_trace, continuous_trace, "continuous and restored Scheduler traces must match");
    assert_eq!(resumed.export_state(), continuous.export_state());
    assert_eq!(resumed.now(), continuous.now());
    actual
}

#[test]
fn e7_2e_nmi_restores_response_internal_and_stack_phases_with_waits() {
    let mut continuous = machine(vec![0; 0x4000]);
    let mut resumed = machine(vec![0; 0x4000]);
    for cpu in [&mut continuous, &mut resumed] {
        cpu.cpu_mut().registers.pc = 0x1234;
        cpu.cpu_mut().registers.sp = 0xC100;
        cpu.cpu_mut().refresh = 0xFE;
        cpu.cpu_mut().iff1 = true;
        cpu.request_nmi();
    }
    assert_eq!(
        compare_runs(&mut continuous, &mut resumed, 1),
        Z80MicroStep::InterruptServiced {
            kind: Z80InterruptKind::Nmi,
            tstates: 14,
        }
    );
    assert_eq!(resumed.cpu().registers.pc, 0x0066);
    assert_eq!(resumed.cpu().registers.sp, 0xC0FE);
    assert_eq!(resumed.memory().read(0xC0FF), 0x12);
    assert_eq!(resumed.memory().read(0xC0FE), 0x34);
    assert_eq!(resumed.now(), MachineTime::from_ticks(42));
}

#[test]
fn e7_2e_im0_restores_acknowledge_and_injected_rst() {
    let mut continuous = machine(vec![0; 0x4000]);
    let mut resumed = machine(vec![0; 0x4000]);
    for cpu in [&mut continuous, &mut resumed] {
        cpu.cpu_mut().iff1 = true;
        cpu.cpu_mut().iff2 = true;
        cpu.cpu_mut().registers.pc = 0x2345;
        cpu.cpu_mut().registers.sp = 0xC100;
        cpu.set_int_line(Some(0xC7));
    }
    let step = compare_runs(&mut continuous, &mut resumed, 1);
    assert!(matches!(
        step,
        Z80MicroStep::InstructionRetired { opcode: 0xC7, .. }
    ));
    assert_eq!(resumed.cpu().registers.pc, 0x0000);
    assert_eq!(resumed.cpu().registers.sp, 0xC0FE);
    assert_eq!(resumed.memory().read(0xC0FF), 0x23);
    assert_eq!(resumed.memory().read(0xC0FE), 0x45);
}

#[test]
fn e7_2e_im1_restores_dispatch_and_both_stack_writes() {
    let mut continuous = machine(vec![0; 0x4000]);
    let mut resumed = machine(vec![0; 0x4000]);
    for cpu in [&mut continuous, &mut resumed] {
        cpu.cpu_mut().interrupt_mode = 1;
        cpu.cpu_mut().iff1 = true;
        cpu.cpu_mut().iff2 = true;
        cpu.cpu_mut().registers.pc = 0x4567;
        cpu.cpu_mut().registers.sp = 0xC100;
        cpu.set_int_line(Some(0xFF));
    }
    assert_eq!(
        compare_runs(&mut continuous, &mut resumed, 1),
        Z80MicroStep::InterruptServiced {
            kind: Z80InterruptKind::Int,
            tstates: 16,
        }
    );
    assert_eq!(resumed.cpu().registers.pc, 0x0038);
    assert_eq!(resumed.cpu().registers.sp, 0xC0FE);
    assert_eq!(resumed.memory().read(0xC0FF), 0x45);
    assert_eq!(resumed.memory().read(0xC0FE), 0x67);
}

#[test]
fn e7_2e_im2_restores_stack_and_both_vector_reads() {
    let mut rom = vec![0; 0x4000];
    rom[0x2080] = 0x78;
    rom[0x2081] = 0x56;
    let mut continuous = machine(rom.clone());
    let mut resumed = machine(rom);
    for cpu in [&mut continuous, &mut resumed] {
        cpu.cpu_mut().interrupt_mode = 2;
        cpu.cpu_mut().iff1 = true;
        cpu.cpu_mut().iff2 = true;
        cpu.cpu_mut().i = 0x20;
        cpu.cpu_mut().registers.pc = 0x4567;
        cpu.cpu_mut().registers.sp = 0xC100;
        cpu.set_int_line(Some(0x80));
    }
    assert_eq!(
        compare_runs(&mut continuous, &mut resumed, 1),
        Z80MicroStep::InterruptServiced {
            kind: Z80InterruptKind::Int,
            tstates: 24,
        }
    );
    assert_eq!(resumed.cpu().registers.pc, 0x5678);
    assert_eq!(resumed.cpu().wz, 0x5678);
    assert_eq!(resumed.cpu().registers.sp, 0xC0FE);
    assert_eq!(resumed.memory().read(0xC0FF), 0x45);
    assert_eq!(resumed.memory().read(0xC0FE), 0x67);
}

#[test]
fn e7_2e_ei_delay_survives_machine_snapshot_until_following_instruction() {
    let rom = vec![0xFB, 0x00, 0x00, 0x00];
    let mut continuous = machine(rom.clone());
    let mut resumed = machine(rom);
    for cpu in [&mut continuous, &mut resumed] {
        cpu.cpu_mut().interrupt_mode = 1;
        cpu.set_int_line(Some(0xFF));
    }
    assert_eq!(
        compare_runs(&mut continuous, &mut resumed, 0),
        Z80MicroStep::InstructionRetired {
            opcode: 0xFB,
            tstates: 4
        }
    );
    assert_eq!(resumed.cpu().ei_delay, 1);
    assert_eq!(
        compare_runs(&mut continuous, &mut resumed, 0),
        Z80MicroStep::InstructionRetired {
            opcode: 0x00,
            tstates: 4
        }
    );
    assert_eq!(resumed.cpu().ei_delay, 0);
    assert_eq!(
        compare_runs(&mut continuous, &mut resumed, 0),
        Z80MicroStep::InterruptServiced {
            kind: Z80InterruptKind::Int,
            tstates: 13
        }
    );
}

#[test]
fn e7_2e_halt_m1_hold_restores_each_tstate_and_ignores_data() {
    let rom = vec![0x76, 0xDD, 0x00, 0x00];
    let mut continuous = machine(rom.clone());
    let mut resumed = machine(rom);
    assert_eq!(
        compare_runs(&mut continuous, &mut resumed, 1),
        Z80MicroStep::InstructionRetired {
            opcode: 0x76,
            tstates: 5
        }
    );
    assert!(resumed.cpu().halted);
    assert_eq!(
        compare_runs(&mut continuous, &mut resumed, 1),
        Z80MicroStep::InstructionRetired {
            opcode: 0x00,
            tstates: 5
        }
    );
    assert!(resumed.cpu().halted);
    assert_eq!(resumed.cpu().registers.pc, 1);
    assert_eq!(resumed.cpu().refresh, 2);
}

#[test]
fn e7_2e_nmi_restore_matches_trace_with_zero_and_multiple_wait_states() {
    for waits in [0, 3] {
        let mut continuous = machine(vec![0; 0x4000]);
        let mut resumed = machine(vec![0; 0x4000]);
        for cpu in [&mut continuous, &mut resumed] {
            cpu.cpu_mut().registers.pc = 0x2345;
            cpu.cpu_mut().registers.sp = 0xC100;
            cpu.cpu_mut().iff1 = true;
            cpu.request_nmi();
        }
        assert!(matches!(
            compare_runs(&mut continuous, &mut resumed, waits),
            Z80MicroStep::InterruptServiced {
                kind: Z80InterruptKind::Nmi,
                ..
            }
        ));
    }
}
