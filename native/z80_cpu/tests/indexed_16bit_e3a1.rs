//! E3A1 indexed 16-bit register operations in the resumable Z80 sequencer.

use sega_z80_cpu::{
    Z80BusRequest, Z80IndexRegister, Z80MicroSequencer, Z80MicroSequencerState, Z80MicroStep,
    Z80PrefixState,
};

const FLAG_S: u8 = 0x80;
const FLAG_Z: u8 = 0x40;
const FLAG_H: u8 = 0x10;
const FLAG_PV: u8 = 0x04;
const FLAG_N: u8 = 0x02;
const FLAG_C: u8 = 0x01;

fn request(step: Z80MicroStep) -> Z80BusRequest {
    match step {
        Z80MicroStep::BusRequest(request) => request,
        other => panic!("expected bus request, got {other:?}"),
    }
}

fn start(cpu: &mut Z80MicroSequencer) -> Z80BusRequest {
    request(cpu.begin_instruction().expect("instruction starts"))
}

fn feed_prefix(cpu: &mut Z80MicroSequencer, fetch: Z80BusRequest, byte: u8) -> Z80BusRequest {
    request(
        cpu.complete_bus_cycle(fetch.id, Some(byte))
            .expect("prefix schedules the next M1 fetch"),
    )
}

fn complete_internal(cpu: &mut Z80MicroSequencer, count: usize) -> Z80MicroStep {
    let mut step = Z80MicroStep::InternalCycle { tstates: 1 };
    for _ in 0..count {
        step = cpu
            .complete_internal_cycle()
            .expect("internal T-state completes");
    }
    step
}

fn load_index_pair(cpu: &mut Z80MicroSequencer, prefix: u8, low: u8, high: u8) -> Z80MicroStep {
    let fetch = start(cpu);
    let fetch = feed_prefix(cpu, fetch, prefix);
    let low_read = request(
        cpu.complete_bus_cycle(fetch.id, Some(0x21))
            .expect("LD IX/IY,nn schedules low-byte read"),
    );
    let high_read = request(
        cpu.complete_bus_cycle(low_read.id, Some(low))
            .expect("low byte schedules high-byte read"),
    );
    cpu.complete_bus_cycle(high_read.id, Some(high))
        .expect("high byte retires LD IX/IY,nn")
}

#[test]
fn ld_ix_iy_immediate_loads_little_endian_and_includes_prefix_m1() {
    let mut cpu = Z80MicroSequencer::default();
    assert_eq!(
        load_index_pair(&mut cpu, 0xDD, 0x34, 0x12),
        Z80MicroStep::InstructionRetired {
            opcode: 0x21,
            tstates: 14,
        }
    );
    assert_eq!(cpu.ix, 0x1234);
    assert_eq!(cpu.registers.pc, 4);

    let mut cpu = Z80MicroSequencer::default();
    assert_eq!(
        load_index_pair(&mut cpu, 0xFD, 0xCD, 0xAB),
        Z80MicroStep::InstructionRetired {
            opcode: 0x21,
            tstates: 14,
        }
    );
    assert_eq!(cpu.iy, 0xABCD);
    assert_eq!(cpu.registers.pc, 4);
}

#[test]
fn ld_ix_snapshot_restores_both_immediate_operand_fetch_phases() {
    let mut cpu = Z80MicroSequencer::default();
    let fetch = start(&mut cpu);
    let fetch = feed_prefix(&mut cpu, fetch, 0xDD);
    let low = request(cpu.complete_bus_cycle(fetch.id, Some(0x21)).unwrap());

    let encoded = cpu
        .export_state()
        .encode()
        .expect("low-byte snapshot encodes");
    let decoded = Z80MicroSequencerState::decode(&encoded).expect("low-byte snapshot decodes");
    cpu = Z80MicroSequencer::restore_state(decoded).expect("low-byte snapshot restores");

    let high = request(cpu.complete_bus_cycle(low.id, Some(0x34)).unwrap());
    let encoded = cpu
        .export_state()
        .encode()
        .expect("high-byte snapshot encodes");
    let decoded = Z80MicroSequencerState::decode(&encoded).expect("high-byte snapshot decodes");
    cpu = Z80MicroSequencer::restore_state(decoded).expect("high-byte snapshot restores");

    assert_eq!(
        cpu.complete_bus_cycle(high.id, Some(0x12)).unwrap(),
        Z80MicroStep::InstructionRetired {
            opcode: 0x21,
            tstates: 14,
        }
    );
    assert_eq!(cpu.ix, 0x1234);
}

#[test]
fn add_ix_snapshot_restores_after_the_internal_mcycle_boundary() {
    let mut cpu = Z80MicroSequencer::default();
    cpu.ix = 0x1000;
    cpu.registers.b = 0;
    cpu.registers.c = 1;
    let fetch = start(&mut cpu);
    let fetch = feed_prefix(&mut cpu, fetch, 0xDD);
    assert_eq!(
        cpu.complete_bus_cycle(fetch.id, Some(0x09)).unwrap(),
        Z80MicroStep::InternalCycle { tstates: 1 }
    );

    for _ in 0..4 {
        assert_eq!(
            cpu.complete_internal_cycle().unwrap(),
            Z80MicroStep::InternalCycle { tstates: 1 }
        );
    }
    let state = cpu.export_state();
    assert_eq!(state.mcycle_index, 3);
    let encoded = state.encode().expect("ADD IX snapshot encodes");
    let decoded = Z80MicroSequencerState::decode(&encoded).expect("ADD IX snapshot decodes");
    cpu = Z80MicroSequencer::restore_state(decoded).expect("ADD IX snapshot restores");

    assert_eq!(
        complete_internal(&mut cpu, 3),
        Z80MicroStep::InstructionRetired {
            opcode: 0x09,
            tstates: 15,
        }
    );
    assert_eq!(cpu.ix, 0x1001);
}

#[test]
fn repeated_alternating_prefixes_select_the_last_index_for_immediate_load() {
    let mut cpu = Z80MicroSequencer::default();
    let fetch = start(&mut cpu);
    let fetch = feed_prefix(&mut cpu, fetch, 0xDD);
    let fetch = feed_prefix(&mut cpu, fetch, 0xFD);
    assert_eq!(
        cpu.export_state().prefix_state,
        Z80PrefixState::Index(Z80IndexRegister::Iy)
    );

    let low = request(cpu.complete_bus_cycle(fetch.id, Some(0x21)).unwrap());
    let high = request(cpu.complete_bus_cycle(low.id, Some(0x78)).unwrap());
    assert_eq!(
        cpu.complete_bus_cycle(high.id, Some(0x56)).unwrap(),
        Z80MicroStep::InstructionRetired {
            opcode: 0x21,
            tstates: 18,
        }
    );
    assert_eq!(cpu.ix, 0);
    assert_eq!(cpu.iy, 0x5678);
}

#[test]
fn inc_ix_and_dec_iy_preserve_flags_and_resume_from_internal_snapshot() {
    let mut cpu = Z80MicroSequencer::default();
    cpu.ix = 0xFFFF;
    cpu.registers.f = 0xD7;
    let fetch = start(&mut cpu);
    let fetch = feed_prefix(&mut cpu, fetch, 0xDD);
    assert_eq!(
        cpu.complete_bus_cycle(fetch.id, Some(0x23)).unwrap(),
        Z80MicroStep::InternalCycle { tstates: 1 }
    );
    let encoded = cpu
        .export_state()
        .encode()
        .expect("indexed INC snapshot encodes");
    let decoded = Z80MicroSequencerState::decode(&encoded).expect("indexed INC snapshot decodes");
    cpu = Z80MicroSequencer::restore_state(decoded).expect("indexed INC snapshot restores");
    assert_eq!(
        complete_internal(&mut cpu, 2),
        Z80MicroStep::InstructionRetired {
            opcode: 0x23,
            tstates: 10,
        }
    );
    assert_eq!(cpu.ix, 0);
    assert_eq!(cpu.registers.f, 0xD7);

    let mut cpu = Z80MicroSequencer::default();
    cpu.iy = 0;
    cpu.registers.f = 0xD7;
    let fetch = start(&mut cpu);
    let fetch = feed_prefix(&mut cpu, fetch, 0xFD);
    assert_eq!(
        cpu.complete_bus_cycle(fetch.id, Some(0x2B)).unwrap(),
        Z80MicroStep::InternalCycle { tstates: 1 }
    );
    assert_eq!(
        complete_internal(&mut cpu, 2),
        Z80MicroStep::InstructionRetired {
            opcode: 0x2B,
            tstates: 10,
        }
    );
    assert_eq!(cpu.iy, 0xFFFF);
    assert_eq!(cpu.registers.f, 0xD7);
}

#[test]
fn add_ix_bc_uses_index_lhs_and_zilog_16bit_flag_rules() {
    let mut cpu = Z80MicroSequencer::default();
    cpu.ix = 0x0FFF;
    cpu.registers.b = 0x00;
    cpu.registers.c = 0x01;
    cpu.registers.h = 0x80;
    cpu.registers.l = 0x00;
    cpu.registers.f = FLAG_S | FLAG_Z | FLAG_PV | FLAG_N;
    let fetch = start(&mut cpu);
    let fetch = feed_prefix(&mut cpu, fetch, 0xDD);
    assert_eq!(
        cpu.complete_bus_cycle(fetch.id, Some(0x09)).unwrap(),
        Z80MicroStep::InternalCycle { tstates: 1 }
    );
    assert_eq!(
        complete_internal(&mut cpu, 7),
        Z80MicroStep::InstructionRetired {
            opcode: 0x09,
            tstates: 15,
        }
    );
    assert_eq!(cpu.ix, 0x1000);
    assert_eq!(cpu.registers.hl(), 0x8000);
    assert_eq!(cpu.wz, 0x1000);
    assert_eq!(cpu.registers.f, FLAG_S | FLAG_Z | FLAG_PV | FLAG_H);
}

#[test]
fn add_iy_iy_reads_and_writes_iy_and_sets_carry_on_wrap() {
    let mut cpu = Z80MicroSequencer::default();
    cpu.iy = 0x8000;
    let fetch = start(&mut cpu);
    let fetch = feed_prefix(&mut cpu, fetch, 0xFD);
    assert_eq!(
        cpu.complete_bus_cycle(fetch.id, Some(0x29)).unwrap(),
        Z80MicroStep::InternalCycle { tstates: 1 }
    );
    assert_eq!(
        complete_internal(&mut cpu, 7),
        Z80MicroStep::InstructionRetired {
            opcode: 0x29,
            tstates: 15,
        }
    );
    assert_eq!(cpu.iy, 0);
    assert_eq!(cpu.wz, 0x8001);
    assert_eq!(cpu.registers.f & FLAG_C, FLAG_C);
}

#[test]
fn jp_ix_and_ld_sp_iy_use_index_pair_without_extra_bus_cycles() {
    let mut cpu = Z80MicroSequencer::default();
    cpu.ix = 0x1234;
    let fetch = start(&mut cpu);
    let fetch = feed_prefix(&mut cpu, fetch, 0xDD);
    assert_eq!(
        cpu.complete_bus_cycle(fetch.id, Some(0xE9)).unwrap(),
        Z80MicroStep::InstructionRetired {
            opcode: 0xE9,
            tstates: 8,
        }
    );
    assert_eq!(cpu.registers.pc, 0x1234);

    let mut cpu = Z80MicroSequencer::default();
    cpu.iy = 0xABCD;
    let fetch = start(&mut cpu);
    let fetch = feed_prefix(&mut cpu, fetch, 0xFD);
    assert_eq!(
        cpu.complete_bus_cycle(fetch.id, Some(0xF9)).unwrap(),
        Z80MicroStep::InternalCycle { tstates: 1 }
    );
    assert_eq!(
        complete_internal(&mut cpu, 2),
        Z80MicroStep::InstructionRetired {
            opcode: 0xF9,
            tstates: 10,
        }
    );
    assert_eq!(cpu.registers.sp, 0xABCD);
}

#[test]
fn ld_ix_immediate_wait_states_extend_operand_cycles() {
    let mut cpu = Z80MicroSequencer::default();
    let fetch = start(&mut cpu);
    let fetch = feed_prefix(&mut cpu, fetch, 0xDD);
    let low = request(cpu.complete_bus_cycle(fetch.id, Some(0x21)).unwrap());
    cpu.insert_wait_tstate().unwrap();
    let high = request(cpu.complete_bus_cycle(low.id, Some(0x34)).unwrap());
    cpu.insert_wait_tstates(2).unwrap();
    assert_eq!(
        cpu.complete_bus_cycle(high.id, Some(0x12)).unwrap(),
        Z80MicroStep::InstructionRetired {
            opcode: 0x21,
            tstates: 17,
        }
    );
    assert_eq!(cpu.ix, 0x1234);
}

#[test]
fn indexed_displacement_family_uses_ix_and_not_hl_for_effective_address() {
    let mut cpu = Z80MicroSequencer::default();
    cpu.ix = 0x1234;
    cpu.registers.h = 0x56;
    cpu.registers.l = 0x78;
    let fetch = start(&mut cpu);
    let fetch = feed_prefix(&mut cpu, fetch, 0xDD);
    let displacement = request(cpu.complete_bus_cycle(fetch.id, Some(0x66)).unwrap());
    assert_eq!(displacement.address, 2);
    assert_eq!(
        cpu.complete_bus_cycle(displacement.id, Some(0x01)).unwrap(),
        Z80MicroStep::InternalCycle { tstates: 1 }
    );

    let mut data_read = Z80MicroStep::InternalCycle { tstates: 1 };
    for _ in 0..5 {
        data_read = cpu.complete_internal_cycle().unwrap();
    }
    let data_read = request(data_read);
    assert_eq!(data_read.address, 0x1235);
    assert_eq!(
        cpu.complete_bus_cycle(data_read.id, Some(0x9A)).unwrap(),
        Z80MicroStep::InstructionRetired {
            opcode: 0x66,
            tstates: 19,
        }
    );
    assert_eq!(cpu.ix, 0x1234);
    assert_eq!(cpu.registers.h, 0x9A);
    assert_eq!(cpu.registers.l, 0x78);
}
