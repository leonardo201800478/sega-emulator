use sega_sms_core::temporal_machine::SmsTemporalMachine;
use sega_temporal_core::MachineTime;
use sega_z80_cpu::Z80MicroStep;

fn complete_m1_fetch(machine: &mut SmsTemporalMachine, wait_tstates: u32) -> Z80MicroStep {
    loop {
        let m1 = machine.export_state().m1;
        if m1.is_none() || m1.is_some_and(|state| state.pending_continuation_id.is_none()) {
            machine.schedule_pending_bus(wait_tstates).unwrap();
        }
        let step = machine.dispatch_next().unwrap();
        if machine.export_state().m1.is_none() {
            return step;
        }
    }
}

#[test]
fn base_ld_sp_hl_internal_cycle_is_scheduler_driven_and_snapshot_restorable() {
    let mut machine = SmsTemporalMachine::new(vec![0xF9; 0x4000], 3).unwrap();
    machine.cpu_mut().registers.set_hl(0xABCD);
    machine.cpu_mut().registers.sp = 0xC100;
    machine.cpu_mut().registers.f = 0xD7;
    machine.cpu_mut().wz = 0x1234;
    machine.begin_instruction().unwrap();

    assert_eq!(
        complete_m1_fetch(&mut machine, 1),
        Z80MicroStep::InternalCycle { tstates: 1 }
    );
    machine.schedule_internal_cycle(1).unwrap();
    let snapshot = machine.export_state();
    let mut restored = SmsTemporalMachine::restore_state(snapshot).unwrap();

    assert_eq!(
        restored.resume_instruction(0).unwrap(),
        Z80MicroStep::InstructionRetired {
            opcode: 0xF9,
            tstates: 7,
        }
    );
    assert_eq!(restored.cpu().registers.sp, 0xABCD);
    assert_eq!(restored.cpu().registers.f, 0xD7);
    assert_eq!(restored.cpu().wz, 0x1234);
    assert_eq!(restored.cpu().registers.pc, 1);
    assert_eq!(restored.cpu().refresh, 1);
    assert_eq!(restored.now(), MachineTime::from_ticks(21));
}
