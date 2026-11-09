//! F-033 regression: all ED encodings that fall through the atomic decoder
//! must retire as the same 8-T-state reserved NOP on the resumable sequencer.

use sega_z80_cpu::{
    Z80BusOperation, Z80BusRequest, Z80MicroSequencer, Z80MicroSequencerState, Z80MicroStep,
    Z80PrefixState,
};

const EXPLICIT_ED: [u8; 78] = [
    0x47, 0x4F, 0x57, 0x5F, 0x43, 0x53, 0x63, 0x73, 0x4B, 0x5B, 0x6B, 0x7B, 0x4A, 0x5A,
    0x6A, 0x7A, 0x42, 0x52, 0x62, 0x72, 0x40, 0x48, 0x50, 0x58, 0x60, 0x68, 0x70, 0x78,
    0x41, 0x49, 0x51, 0x59, 0x61, 0x69, 0x71, 0x79, 0x44, 0x4C, 0x54, 0x5C, 0x64, 0x6C,
    0x74, 0x7C, 0x45, 0x4D, 0x55, 0x5D, 0x65, 0x6D, 0x75, 0x7D, 0x46, 0x4E, 0x66, 0x6E,
    0x56, 0x76, 0x5E, 0x7E, 0x67, 0x6F, 0xA0, 0xA8, 0xB0, 0xB8, 0xA1, 0xA9, 0xB1, 0xB9,
    0xA2, 0xAA, 0xB2, 0xBA, 0xA3, 0xAB, 0xB3, 0xBB,
];

fn request(step: Z80MicroStep) -> Z80BusRequest {
    match step {
        Z80MicroStep::BusRequest(request) => request,
        other => panic!("expected ED opcode-fetch request, got {other:?}"),
    }
}

fn assert_reserved_ed_retires_as_nop(opcode: u8) {
    let mut cpu = Z80MicroSequencer::default();
    cpu.registers.pc = 0x1000;
    cpu.registers.sp = 0xD000;
    cpu.registers.a = 0xA5;
    cpu.registers.f = 0x5A;
    cpu.registers.set_bc(0x1234);
    cpu.registers.set_de(0x5678);
    cpu.registers.set_hl(0x9ABC);
    cpu.ix = 0x1357;
    cpu.iy = 0x2468;
    cpu.wz = 0xBEEF;
    cpu.i = 0x12;
    cpu.refresh = 0x80;
    cpu.q = 0x3C;
    cpu.p = true;
    cpu.iff1 = true;
    cpu.iff2 = false;
    cpu.interrupt_mode = 2;

    let initial_registers = cpu.registers;
    let initial_alternate = cpu.alternate;
    let initial_ix = cpu.ix;
    let initial_iy = cpu.iy;
    let initial_wz = cpu.wz;
    let initial_i = cpu.i;
    let initial_iff1 = cpu.iff1;
    let initial_iff2 = cpu.iff2;
    let initial_im = cpu.interrupt_mode;

    let first = request(cpu.begin_instruction().expect("instruction starts"));
    assert_eq!(first.operation, Z80BusOperation::OpcodeFetch);
    assert_eq!(first.address, 0x1000);
    assert_eq!(first.base_tstates, 4);

    let second = request(
        cpu.complete_bus_cycle(first.id, Some(0xED))
            .expect("ED prefix schedules terminal M1"),
    );
    assert_eq!(second.operation, Z80BusOperation::OpcodeFetch);
    assert_eq!(second.address, 0x1001);
    assert_eq!(second.base_tstates, 4);
    assert_eq!(cpu.export_state().prefix_state, Z80PrefixState::Ed);
    assert_eq!(cpu.q_previous, 0x3C, "ED does not clear previous-Q");
    assert_eq!(cpu.q, 0);
    assert!(!cpu.p);

    // The reserved ED opcode must survive a safe-point snapshot between M1s.
    let encoded = cpu.export_state().encode().expect("ED-prefix snapshot encodes");
    let decoded = Z80MicroSequencerState::decode(&encoded).expect("ED-prefix snapshot decodes");
    cpu = Z80MicroSequencer::restore_state(decoded).expect("ED-prefix snapshot restores");

    let terminal = cpu.complete_bus_cycle(second.id, Some(opcode));
    match terminal {
        Ok(Z80MicroStep::InstructionRetired {
            opcode: retired_opcode,
            tstates,
        }) => {
            assert_eq!(retired_opcode, opcode, "ED {opcode:02X}: retired opcode");
            assert_eq!(tstates, 8, "ED {opcode:02X}: nominal T-states");
        }
        other => panic!("ED {opcode:02X}: expected 8T NOP retirement, got {other:?}"),
    }

    assert_eq!(cpu.registers.a, initial_registers.a, "ED {opcode:02X}: A");
    assert_eq!(cpu.registers.f, initial_registers.f, "ED {opcode:02X}: F");
    assert_eq!(cpu.registers.bc(), initial_registers.bc(), "ED {opcode:02X}: BC");
    assert_eq!(cpu.registers.de(), initial_registers.de(), "ED {opcode:02X}: DE");
    assert_eq!(cpu.registers.hl(), initial_registers.hl(), "ED {opcode:02X}: HL");
    assert_eq!(cpu.registers.sp, initial_registers.sp, "ED {opcode:02X}: SP");
    assert_eq!(cpu.registers.pc, 0x1002, "ED {opcode:02X}: PC");
    assert_eq!(cpu.alternate, initial_alternate, "ED {opcode:02X}: alternate registers");
    assert_eq!(cpu.ix, initial_ix, "ED {opcode:02X}: IX");
    assert_eq!(cpu.iy, initial_iy, "ED {opcode:02X}: IY");
    assert_eq!(cpu.wz, initial_wz, "ED {opcode:02X}: WZ");
    assert_eq!(cpu.i, initial_i, "ED {opcode:02X}: I");
    assert_eq!(cpu.refresh, 0x82, "ED {opcode:02X}: two M1 refresh increments");
    assert_eq!(cpu.q, 0, "ED {opcode:02X}: Q");
    assert_eq!(cpu.q_previous, 0x3C, "ED {opcode:02X}: previous-Q");
    assert!(!cpu.p, "ED {opcode:02X}: P");
    assert_eq!(cpu.iff1, initial_iff1, "ED {opcode:02X}: IFF1");
    assert_eq!(cpu.iff2, initial_iff2, "ED {opcode:02X}: IFF2");
    assert_eq!(cpu.interrupt_mode, initial_im, "ED {opcode:02X}: IM");
    assert_eq!(cpu.export_state().prefix_state, Z80PrefixState::None);
    assert!(!cpu.instruction_active());
}

#[test]
fn all_178_ed_fallback_encodings_retire_as_8t_nops_on_resumable_sequencer() {
    assert_eq!(EXPLICIT_ED.len(), 78);
    let mut tested = 0usize;

    for opcode in 0_u8..=u8::MAX {
        if EXPLICIT_ED.contains(&opcode) {
            continue;
        }
        assert_reserved_ed_retires_as_nop(opcode);
        tested += 1;
    }

    assert_eq!(tested, 178);
}
