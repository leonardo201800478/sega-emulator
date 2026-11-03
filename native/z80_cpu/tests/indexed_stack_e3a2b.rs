//! E3A2b indexed PUSH/POP operations through the resumable Z80 sequencer.

use sega_z80_cpu::{
    Z80BusOperation, Z80BusRequest, Z80MicroSequencer, Z80MicroSequencerState, Z80MicroStep,
};

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
            .expect("prefix schedules next M1 fetch"),
    )
}

fn round_trip(cpu: Z80MicroSequencer) -> Z80MicroSequencer {
    let encoded = cpu.export_state().encode().expect("snapshot encodes");
    let decoded = Z80MicroSequencerState::decode(&encoded).expect("snapshot decodes");
    Z80MicroSequencer::restore_state(decoded).expect("snapshot restores")
}

#[test]
fn push_ix_writes_high_then_low_with_waits_and_snapshot_resume() {
    let mut cpu = Z80MicroSequencer::default();
    cpu.ix = 0x1234;
    cpu.registers.sp = 0x0000;
    cpu.registers.f = 0xA5;
    cpu.wz = 0x4567;

    let fetch = start(&mut cpu);
    let fetch = feed_prefix(&mut cpu, fetch, 0xDD);
    assert_eq!(
        cpu.complete_bus_cycle(fetch.id, Some(0xE5)).unwrap(),
        Z80MicroStep::InternalCycle { tstates: 1 }
    );
    assert_eq!(cpu.export_state().mcycle_index, 4);
    cpu = round_trip(cpu);
    let high = request(cpu.complete_internal_cycle().unwrap());
    assert_eq!(cpu.export_state().mcycle_index, 5);
    assert_eq!(high.address, 0xFFFF);
    assert_eq!(high.operation, Z80BusOperation::MemoryWrite { value: 0x12 });
    cpu.insert_wait_tstate().unwrap();
    cpu = round_trip(cpu);

    let low = request(cpu.complete_bus_cycle(high.id, None).unwrap());
    assert_eq!(cpu.registers.sp, 0xFFFF);
    assert_eq!(low.address, 0xFFFE);
    assert_eq!(low.operation, Z80BusOperation::MemoryWrite { value: 0x34 });
    cpu.insert_wait_tstates(2).unwrap();
    cpu = round_trip(cpu);

    assert_eq!(
        cpu.complete_bus_cycle(low.id, None).unwrap(),
        Z80MicroStep::InstructionRetired {
            opcode: 0xE5,
            tstates: 18,
        }
    );
    assert_eq!(cpu.registers.sp, 0xFFFE);
    assert_eq!(cpu.ix, 0x1234);
    assert_eq!(cpu.registers.f, 0xA5);
    assert_eq!(cpu.wz, 0x4567);
    assert_eq!(cpu.registers.pc, 2);
}

#[test]
fn push_iy_uses_last_of_repeated_alternating_prefixes() {
    let mut cpu = Z80MicroSequencer::default();
    cpu.ix = 0x1111;
    cpu.iy = 0xABCD;
    cpu.registers.sp = 0x2000;
    cpu.registers.f = 0x5A;
    cpu.wz = 0x6789;

    let fetch = start(&mut cpu);
    let fetch = feed_prefix(&mut cpu, fetch, 0xDD);
    let fetch = feed_prefix(&mut cpu, fetch, 0xFD);
    let fetch = feed_prefix(&mut cpu, fetch, 0xDD);
    let fetch = feed_prefix(&mut cpu, fetch, 0xFD);
    assert_eq!(
        cpu.complete_bus_cycle(fetch.id, Some(0xE5)).unwrap(),
        Z80MicroStep::InternalCycle { tstates: 1 }
    );
    let high = request(cpu.complete_internal_cycle().unwrap());
    assert_eq!(high.address, 0x1FFF);
    assert_eq!(high.operation, Z80BusOperation::MemoryWrite { value: 0xAB });
    cpu = round_trip(cpu);
    let low = request(cpu.complete_bus_cycle(high.id, None).unwrap());
    assert_eq!(low.address, 0x1FFE);
    assert_eq!(low.operation, Z80BusOperation::MemoryWrite { value: 0xCD });
    cpu = round_trip(cpu);
    assert_eq!(
        cpu.complete_bus_cycle(low.id, None).unwrap(),
        Z80MicroStep::InstructionRetired {
            opcode: 0xE5,
            tstates: 27,
        }
    );
    assert_eq!(cpu.iy, 0xABCD);
    assert_eq!(cpu.ix, 0x1111);
    assert_eq!(cpu.registers.sp, 0x1FFE);
    assert_eq!(cpu.registers.f, 0x5A);
    assert_eq!(cpu.wz, 0x6789);
    assert_eq!(cpu.registers.pc, 5);
}

#[test]
fn pop_ix_reads_low_then_high_wraps_sp_and_restores_both_read_phases() {
    let mut cpu = Z80MicroSequencer::default();
    cpu.registers.sp = 0xFFFF;
    cpu.registers.f = 0x3C;
    cpu.wz = 0x789A;
    let fetch = start(&mut cpu);
    let fetch = feed_prefix(&mut cpu, fetch, 0xDD);
    let fetch = feed_prefix(&mut cpu, fetch, 0xFD);
    let fetch = feed_prefix(&mut cpu, fetch, 0xDD);
    let low = request(cpu.complete_bus_cycle(fetch.id, Some(0xE1)).unwrap());
    assert_eq!(cpu.export_state().mcycle_index, 4);
    assert_eq!(low.address, 0xFFFF);
    cpu = round_trip(cpu);

    let high = request(cpu.complete_bus_cycle(low.id, Some(0x78)).unwrap());
    assert_eq!(cpu.export_state().mcycle_index, 5);
    assert_eq!(cpu.registers.sp, 0x0000);
    assert_eq!(high.address, 0x0000);
    cpu = round_trip(cpu);

    assert_eq!(
        cpu.complete_bus_cycle(high.id, Some(0x56)).unwrap(),
        Z80MicroStep::InstructionRetired {
            opcode: 0xE1,
            tstates: 22,
        }
    );
    assert_eq!(cpu.ix, 0x5678);
    assert_eq!(cpu.registers.hl(), 0);
    assert_eq!(cpu.registers.sp, 0x0001);
    assert_eq!(cpu.registers.f, 0x3C);
    assert_eq!(cpu.wz, 0x789A);
    assert_eq!(cpu.registers.pc, 4);
}

#[test]
fn pop_iy_uses_fd_prefix_and_commits_only_after_high_byte() {
    let mut cpu = Z80MicroSequencer::default();
    cpu.iy = 0xAAAA;
    cpu.registers.sp = 0x4000;
    cpu.registers.f = 0xC3;
    cpu.wz = 0x9ABC;
    let fetch = start(&mut cpu);
    let fetch = feed_prefix(&mut cpu, fetch, 0xFD);
    let low = request(cpu.complete_bus_cycle(fetch.id, Some(0xE1)).unwrap());
    assert_eq!(low.address, 0x4000);
    cpu.insert_wait_tstate().unwrap();
    cpu = round_trip(cpu);
    assert_eq!(cpu.iy, 0xAAAA);

    let high = request(cpu.complete_bus_cycle(low.id, Some(0xCD)).unwrap());
    assert_eq!(high.address, 0x4001);
    cpu.insert_wait_tstates(2).unwrap();
    cpu = round_trip(cpu);
    assert_eq!(cpu.iy, 0xAAAA);
    assert_eq!(
        cpu.complete_bus_cycle(high.id, Some(0xAB)).unwrap(),
        Z80MicroStep::InstructionRetired {
            opcode: 0xE1,
            tstates: 17,
        }
    );
    assert_eq!(cpu.iy, 0xABCD);
    assert_eq!(cpu.registers.sp, 0x4002);
    assert_eq!(cpu.registers.f, 0xC3);
    assert_eq!(cpu.wz, 0x9ABC);
    assert_eq!(cpu.registers.pc, 2);
}
