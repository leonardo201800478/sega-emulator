use sega_sms_core::SmsTemporalMachine;
use sega_sms_core::temporal_machine::SmsTemporalTraceEvent;
use sega_temporal_core::MachineTime;
use sega_z80_cpu::{Z80BusOperation, Z80MicroStep};

#[test]
fn f002_bc_de_transfers_resume_from_scheduler_snapshot_with_wait() {
    for (opcode, address, store) in [
        (0x02, 0xC000, true),
        (0x12, 0xC001, true),
        (0x0A, 0x0000, false),
        (0x1A, 0x0000, false),
    ] {
        let mut machine = SmsTemporalMachine::new(vec![opcode], 3).unwrap();
        machine.set_trace_enabled(true);
        machine.cpu_mut().registers.a = 0x5A;
        machine.cpu_mut().registers.f = 0xA5;
        if matches!(opcode, 0x02 | 0x0A) {
            machine.cpu_mut().registers.set_bc(address);
        } else {
            machine.cpu_mut().registers.set_de(address);
        }

        assert!(matches!(
            machine.begin_instruction().unwrap(),
            Z80MicroStep::BusRequest(_)
        ));
        machine.schedule_pending_bus(1).unwrap();

        // Snapshot while the opcode M1's first scheduler continuation is pending.
        let snapshot = machine.export_state();
        let mut restored = SmsTemporalMachine::restore_state(snapshot).unwrap();
        assert_eq!(
            restored.resume_instruction(1).unwrap(),
            Z80MicroStep::InstructionRetired { opcode, tstates: 9 }
        );

        assert_eq!(restored.now(), MachineTime::from_ticks(27));
        assert_eq!(restored.cpu().registers.pc, 1);
        assert_eq!(restored.cpu().refresh & 0x7F, 1);
        assert_eq!(restored.cpu().registers.f, 0xA5);

        let (expected_operation, expected_data) = if store {
            assert_eq!(restored.memory().read(address), 0x5A);
            let expected_wz = (0x5Au16 << 8) | (address.wrapping_add(1) & 0x00FF);
            assert_eq!(restored.cpu().wz, expected_wz);
            (Z80BusOperation::MemoryWrite { value: 0x5A }, None)
        } else {
            // The fixture ROM begins with the opcode, making address zero a
            // deterministic source for the indirect accumulator read.
            assert_eq!(restored.cpu().registers.a, opcode);
            assert_eq!(restored.cpu().wz, 1);
            (Z80BusOperation::MemoryRead, Some(opcode))
        };

        let bus_completions: Vec<_> = restored
            .take_trace()
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
            bus_completions,
            vec![(expected_operation, address, 3, 1, expected_data)],
            "opcode {opcode:02X} must issue one correctly ordered data transaction"
        );
    }
}
