use sega_z80_cpu::{
    Z80BusOperation, Z80BusRequest, Z80MicroSequencer, Z80MicroSequencerState, Z80MicroStep,
};

const FLAG_C: u8 = 0x01;
const FLAG_PV: u8 = 0x04;

fn request(step: Z80MicroStep) -> Z80BusRequest {
    match step {
        Z80MicroStep::BusRequest(request) => request,
        other => panic!("expected bus request, got {other:?}"),
    }
}

fn execute_ld_a_i_or_r(cpu: &mut Z80MicroSequencer, opcode: u8, int_during_instruction: bool) {
    let prefix = request(cpu.begin_instruction().unwrap());
    assert_eq!(prefix.operation, Z80BusOperation::OpcodeFetch);
    let terminal = request(cpu.complete_bus_cycle(prefix.id, Some(0xED)).unwrap());
    assert_eq!(terminal.operation, Z80BusOperation::OpcodeFetch);
    assert_eq!(terminal.address, 1);

    if int_during_instruction {
        cpu.set_int_line(Some(0xFF));
    }

    assert_eq!(
        cpu.complete_bus_cycle(terminal.id, Some(opcode)).unwrap(),
        Z80MicroStep::InternalCycle { tstates: 1 }
    );
    assert_eq!(
        cpu.complete_internal_cycle().unwrap(),
        Z80MicroStep::InstructionRetired { opcode, tstates: 9 }
    );
    assert!(
        cpu.export_state().p,
        "LD A,I/R must arm the one-boundary erratum latch"
    );
    assert_ne!(cpu.registers.f & FLAG_PV, 0, "IFF2 is copied into P/V");
    assert_ne!(cpu.registers.f & FLAG_C, 0, "carry must be preserved");
}

fn cpu_with_iff_enabled() -> Z80MicroSequencer {
    let mut cpu = Z80MicroSequencer::default();
    cpu.registers.pc = 0;
    cpu.i = 0xA8;
    cpu.refresh = 0x20;
    cpu.registers.f = FLAG_C;
    cpu.iff1 = true;
    cpu.iff2 = true;
    cpu
}

#[test]
fn int_acknowledge_clears_pv_after_ld_a_i_and_ld_a_r_even_after_snapshot_restore() {
    for opcode in [0x57, 0x5F] {
        let mut cpu = cpu_with_iff_enabled();
        execute_ld_a_i_or_r(&mut cpu, opcode, true);

        let state = cpu.export_state();
        let encoded = state.encode().unwrap();
        let decoded = Z80MicroSequencerState::decode(&encoded).unwrap();
        assert_eq!(decoded, state);
        cpu = Z80MicroSequencer::restore_state(decoded).unwrap();

        let acknowledge = request(cpu.begin_instruction().unwrap());
        assert_eq!(acknowledge.operation, Z80BusOperation::InterruptAcknowledge);
        assert_eq!(cpu.registers.f & FLAG_PV, 0, "ED {opcode:02X}");
        assert_eq!(cpu.export_state().q, cpu.registers.f, "ED {opcode:02X}: Q");
        assert_ne!(cpu.registers.f & FLAG_C, 0, "ED {opcode:02X}");
        assert!(!cpu.export_state().p, "ED {opcode:02X}");
    }
}

#[test]
fn normal_instruction_consumes_ld_a_i_r_erratum_latch_without_clearing_pv() {
    for opcode in [0x57, 0x5F] {
        let mut cpu = cpu_with_iff_enabled();
        execute_ld_a_i_or_r(&mut cpu, opcode, false);

        let next = request(cpu.begin_instruction().unwrap());
        assert_eq!(next.operation, Z80BusOperation::OpcodeFetch);
        assert!(
            !cpu.export_state().p,
            "ED {opcode:02X}: the next normal fetch consumes P"
        );
        assert_ne!(cpu.registers.f & FLAG_PV, 0);

        assert_eq!(
            cpu.complete_bus_cycle(next.id, Some(0x00)).unwrap(),
            Z80MicroStep::InstructionRetired {
                opcode: 0x00,
                tstates: 4,
            }
        );
        cpu.set_int_line(Some(0xFF));
        let acknowledge = request(cpu.begin_instruction().unwrap());
        assert_eq!(acknowledge.operation, Z80BusOperation::InterruptAcknowledge);
        assert_ne!(
            cpu.registers.f & FLAG_PV,
            0,
            "ED {opcode:02X}: INT after an intervening instruction must preserve P/V"
        );
    }
}

#[test]
fn nmi_after_ld_a_i_or_r_does_not_apply_maskable_interrupt_pv_erratum() {
    for opcode in [0x57, 0x5F] {
        let mut cpu = cpu_with_iff_enabled();
        execute_ld_a_i_or_r(&mut cpu, opcode, false);

        cpu.request_nmi();
        let response = request(cpu.begin_instruction().unwrap());
        assert_eq!(response.operation, Z80BusOperation::OpcodeFetch);
        assert_ne!(
            cpu.registers.f & FLAG_PV,
            0,
            "ED {opcode:02X}: NMI must not clear P/V"
        );
        assert!(!cpu.export_state().p, "ED {opcode:02X}");
    }
}
