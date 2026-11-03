//! E4A indexed-displacement load/store execution through the resumable sequencer.

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

fn feed_prefix(cpu: &mut Z80MicroSequencer, current: Z80BusRequest, byte: u8) -> Z80BusRequest {
    request(
        cpu.complete_bus_cycle(current.id, Some(byte))
            .expect("prefix schedules the next M1 fetch"),
    )
}

fn round_trip(cpu: Z80MicroSequencer) -> Z80MicroSequencer {
    let encoded = cpu.export_state().encode().expect("snapshot encodes");
    let decoded = Z80MicroSequencerState::decode(&encoded).expect("snapshot decodes");
    Z80MicroSequencer::restore_state(decoded).expect("snapshot restores")
}

fn complete_internal_cycles(cpu: &mut Z80MicroSequencer, count: usize) -> Z80MicroStep {
    let mut step = Z80MicroStep::InternalCycle { tstates: 1 };
    for _ in 0..count {
        step = cpu
            .complete_internal_cycle()
            .expect("internal T-state advances");
    }
    step
}

#[test]
fn ld_a_from_iy_minus_one_wraps_and_resumes_across_wait_and_snapshots() {
    let mut cpu = Z80MicroSequencer::default();
    cpu.iy = 0x0000;
    cpu.registers.a = 0x11;
    cpu.registers.f = 0xA5;

    let fetch = start(&mut cpu);
    let fetch = feed_prefix(&mut cpu, fetch, 0xDD);
    let fetch = feed_prefix(&mut cpu, fetch, 0xFD);
    let displacement = request(cpu.complete_bus_cycle(fetch.id, Some(0x7E)).unwrap());
    assert_eq!(displacement.address, 2);
    assert_eq!(displacement.operation, Z80BusOperation::MemoryRead);
    assert_eq!(cpu.export_state().mcycle_index, 3);

    cpu.insert_wait_tstate().unwrap();
    cpu = round_trip(cpu);
    assert_eq!(
        cpu.complete_bus_cycle(displacement.id, Some(0xFF)).unwrap(),
        Z80MicroStep::InternalCycle { tstates: 1 }
    );
    assert_eq!(cpu.registers.pc, 3);
    assert_eq!(cpu.wz, 0xFFFF);
    cpu = round_trip(cpu);

    let data_read = request(complete_internal_cycles(&mut cpu, 5));
    cpu = round_trip(cpu);
    assert_eq!(data_read.address, 0xFFFF);
    assert_eq!(data_read.operation, Z80BusOperation::MemoryRead);
    assert_eq!(cpu.export_state().mcycle_index, 5);

    cpu.insert_wait_tstates(2).unwrap();
    cpu = round_trip(cpu);
    assert_eq!(
        cpu.complete_bus_cycle(data_read.id, Some(0x42)).unwrap(),
        Z80MicroStep::InstructionRetired {
            opcode: 0x7E,
            tstates: 26,
        }
    );
    assert_eq!(cpu.registers.a, 0x42);
    assert_eq!(cpu.registers.f, 0xA5);
    assert_eq!(cpu.iy, 0x0000);
    assert_eq!(cpu.wz, 0xFFFF);
    assert_eq!(cpu.registers.pc, 3);
    assert_eq!(cpu.refresh & 0x7F, 3);
}

#[test]
fn ld_h_from_ix_displacement_writes_base_h_not_an_index_half() {
    let mut cpu = Z80MicroSequencer::default();
    cpu.ix = 0x20FF;
    cpu.registers.h = 0x11;
    cpu.registers.l = 0x22;
    cpu.registers.f = 0x5A;

    let fetch = start(&mut cpu);
    let fetch = feed_prefix(&mut cpu, fetch, 0xDD);
    let displacement = request(cpu.complete_bus_cycle(fetch.id, Some(0x66)).unwrap());
    let internal = cpu.complete_bus_cycle(displacement.id, Some(0x01)).unwrap();
    assert_eq!(internal, Z80MicroStep::InternalCycle { tstates: 1 });
    assert_eq!(cpu.wz, 0x2100);
    cpu = round_trip(cpu);

    let data_read = request(complete_internal_cycles(&mut cpu, 5));
    assert_eq!(data_read.address, 0x2100);
    cpu = round_trip(cpu);
    assert_eq!(
        cpu.complete_bus_cycle(data_read.id, Some(0x99)).unwrap(),
        Z80MicroStep::InstructionRetired {
            opcode: 0x66,
            tstates: 19,
        }
    );
    assert_eq!(cpu.registers.h, 0x99);
    assert_eq!(cpu.registers.l, 0x22);
    assert_eq!(cpu.ix, 0x20FF);
    assert_eq!(cpu.registers.f, 0x5A);
    assert_eq!(cpu.registers.pc, 3);
}

#[test]
fn ld_iy_displacement_store_reads_base_l_and_wraps_effective_address() {
    let mut cpu = Z80MicroSequencer::default();
    cpu.iy = 0xFFFF;
    cpu.registers.l = 0x5A;

    let fetch = start(&mut cpu);
    let fetch = feed_prefix(&mut cpu, fetch, 0xFD);
    let displacement = request(cpu.complete_bus_cycle(fetch.id, Some(0x75)).unwrap());
    assert_eq!(
        cpu.complete_bus_cycle(displacement.id, Some(0x01)).unwrap(),
        Z80MicroStep::InternalCycle { tstates: 1 }
    );
    assert_eq!(cpu.wz, 0x0000);
    cpu = round_trip(cpu);

    let write = request(complete_internal_cycles(&mut cpu, 5));
    assert_eq!(write.address, 0x0000);
    assert_eq!(
        write.operation,
        Z80BusOperation::MemoryWrite { value: 0x5A }
    );
    cpu.insert_wait_tstates(2).unwrap();
    cpu = round_trip(cpu);
    assert_eq!(
        cpu.complete_bus_cycle(write.id, None).unwrap(),
        Z80MicroStep::InstructionRetired {
            opcode: 0x75,
            tstates: 21,
        }
    );
    assert_eq!(cpu.iy, 0xFFFF);
    assert_eq!(cpu.registers.l, 0x5A);
    assert_eq!(cpu.registers.pc, 3);
}

#[test]
fn ld_ix_displacement_immediate_fetches_displacement_before_value() {
    let mut cpu = Z80MicroSequencer::default();
    cpu.ix = 0x0100;
    cpu.registers.f = 0xC3;

    let fetch = start(&mut cpu);
    let fetch = feed_prefix(&mut cpu, fetch, 0xDD);
    let displacement = request(cpu.complete_bus_cycle(fetch.id, Some(0x36)).unwrap());
    assert_eq!(displacement.address, 2);
    cpu.insert_wait_tstate().unwrap();
    cpu = round_trip(cpu);

    let immediate = request(cpu.complete_bus_cycle(displacement.id, Some(0xFE)).unwrap());
    assert_eq!(immediate.address, 3);
    assert_eq!(cpu.wz, 0x00FE);
    assert_eq!(cpu.registers.pc, 3);
    cpu.insert_wait_tstate().unwrap();
    cpu = round_trip(cpu);

    assert_eq!(
        cpu.complete_bus_cycle(immediate.id, Some(0xA5)).unwrap(),
        Z80MicroStep::InternalCycle { tstates: 1 }
    );
    cpu = round_trip(cpu);
    let first_internal = cpu.complete_internal_cycle().unwrap();
    assert_eq!(first_internal, Z80MicroStep::InternalCycle { tstates: 1 });
    cpu = round_trip(cpu);

    let write = request(cpu.complete_internal_cycle().unwrap());
    assert_eq!(write.address, 0x00FE);
    assert_eq!(
        write.operation,
        Z80BusOperation::MemoryWrite { value: 0xA5 }
    );
    assert_eq!(cpu.registers.pc, 4);
    assert_eq!(cpu.export_state().mcycle_index, 4);
    cpu.insert_wait_tstates(2).unwrap();
    cpu = round_trip(cpu);
    assert_eq!(
        cpu.complete_bus_cycle(write.id, None).unwrap(),
        Z80MicroStep::InstructionRetired {
            opcode: 0x36,
            tstates: 23,
        }
    );
    assert_eq!(cpu.registers.f, 0xC3);
    assert_eq!(cpu.ix, 0x0100);
    assert_eq!(cpu.wz, 0x00FE);
}


#[test]
fn indexed_hl_register_loads_cover_signed_displacement_boundaries_and_snapshots() {
    for prefix in [0xDD, 0xFD] {
        for (base, displacement, expected_address) in [
            (0x0000, 0x80_u8, 0xFF80),
            (0x0000, 0x00, 0x0000),
            (0xFF80, 0x7F, 0xFFFF),
            (0xFFFF, 0x01, 0x0000),
        ] {
            for opcode in [0x66, 0x6E] {
                let mut cpu = Z80MicroSequencer::default();
                if prefix == 0xDD {
                    cpu.ix = base;
                } else {
                    cpu.iy = base;
                }
                cpu.registers.h = 0x12;
                cpu.registers.l = 0x34;
                cpu.registers.f = 0xA5;

                let fetch = start(&mut cpu);
                let fetch = feed_prefix(&mut cpu, fetch, prefix);
                let displacement_request =
                    request(cpu.complete_bus_cycle(fetch.id, Some(opcode)).unwrap());
                assert_eq!(displacement_request.address, 2);
                let internal = cpu
                    .complete_bus_cycle(displacement_request.id, Some(displacement))
                    .unwrap();
                assert_eq!(internal, Z80MicroStep::InternalCycle { tstates: 1 });
                assert_eq!(
                    cpu.wz,
                    base.wrapping_add_signed(i16::from(displacement as i8))
                );
                assert_eq!(cpu.wz, expected_address);
                cpu = round_trip(cpu);

                let mut data_read = Z80MicroStep::InternalCycle { tstates: 1 };
                for _ in 0..5 {
                    data_read = cpu.complete_internal_cycle().unwrap();
                    cpu = round_trip(cpu);
                }
                let data_read = request(data_read);
                assert_eq!(data_read.operation, Z80BusOperation::MemoryRead);
                assert_eq!(data_read.address, expected_address);
                assert_eq!(
                    cpu.complete_bus_cycle(data_read.id, Some(0x9B)).unwrap(),
                    Z80MicroStep::InstructionRetired {
                        opcode,
                        tstates: 19,
                    }
                );
                if opcode == 0x66 {
                    assert_eq!(cpu.registers.h, 0x9B);
                    assert_eq!(cpu.registers.l, 0x34);
                } else {
                    assert_eq!(cpu.registers.h, 0x12);
                    assert_eq!(cpu.registers.l, 0x9B);
                }
                assert_eq!(cpu.registers.f, 0xA5);
                assert_eq!(cpu.registers.pc, 3);
            }
        }
    }
}

#[test]
fn indexed_load_waits_are_added_to_each_bus_cycle_and_survive_snapshots() {
    let mut cpu = Z80MicroSequencer::default();
    cpu.ix = 0x1000;
    let fetch = start(&mut cpu);
    cpu.insert_wait_tstates(2).unwrap();
    cpu = round_trip(cpu);
    let fetch = feed_prefix(&mut cpu, fetch, 0xDD);
    cpu.insert_wait_tstate().unwrap();
    cpu = round_trip(cpu);
    let displacement = request(cpu.complete_bus_cycle(fetch.id, Some(0x7E)).unwrap());
    cpu.insert_wait_tstates(3).unwrap();
    cpu = round_trip(cpu);
    assert_eq!(
        cpu.complete_bus_cycle(displacement.id, Some(0xFE)).unwrap(),
        Z80MicroStep::InternalCycle { tstates: 1 }
    );
    cpu = round_trip(cpu);

    let mut data_read = Z80MicroStep::InternalCycle { tstates: 1 };
    for _ in 0..5 {
        data_read = cpu.complete_internal_cycle().unwrap();
        cpu = round_trip(cpu);
    }
    let data_read = request(data_read);
    assert_eq!(data_read.address, 0x0FFE);
    cpu.insert_wait_tstates(4).unwrap();
    cpu = round_trip(cpu);
    assert_eq!(
        cpu.complete_bus_cycle(data_read.id, Some(0x55)).unwrap(),
        Z80MicroStep::InstructionRetired {
            opcode: 0x7E,
            tstates: 29,
        }
    );
    assert_eq!(cpu.registers.a, 0x55);
}

#[test]
fn indexed_immediate_store_wraps_pc_and_waits_across_every_bus_cycle() {
    let mut cpu = Z80MicroSequencer::default();
    cpu.iy = 0xFFFF;
    cpu.registers.pc = 0xFFFD;

    let mut fetch = start(&mut cpu);
    cpu.insert_wait_tstate().unwrap();
    cpu = round_trip(cpu);
    fetch = feed_prefix(&mut cpu, fetch, 0xFD);
    cpu.insert_wait_tstates(2).unwrap();
    cpu = round_trip(cpu);
    let displacement = request(cpu.complete_bus_cycle(fetch.id, Some(0x36)).unwrap());
    assert_eq!(displacement.address, 0xFFFF);
    cpu.insert_wait_tstates(3).unwrap();
    cpu = round_trip(cpu);
    let immediate = request(
        cpu.complete_bus_cycle(displacement.id, Some(0x02)).unwrap(),
    );
    assert_eq!(immediate.address, 0x0000);
    assert_eq!(cpu.registers.pc, 0x0000);
    cpu.insert_wait_tstates(4).unwrap();
    cpu = round_trip(cpu);
    assert_eq!(
        cpu.complete_bus_cycle(immediate.id, Some(0xA6)).unwrap(),
        Z80MicroStep::InternalCycle { tstates: 1 }
    );
    cpu = round_trip(cpu);
    assert_eq!(
        cpu.complete_internal_cycle().unwrap(),
        Z80MicroStep::InternalCycle { tstates: 1 }
    );
    cpu = round_trip(cpu);
    let write = request(cpu.complete_internal_cycle().unwrap());
    assert_eq!(write.address, 0x0001);
    assert_eq!(
        write.operation,
        Z80BusOperation::MemoryWrite { value: 0xA6 }
    );
    cpu.insert_wait_tstates(5).unwrap();
    cpu = round_trip(cpu);
    assert_eq!(
        cpu.complete_bus_cycle(write.id, None).unwrap(),
        Z80MicroStep::InstructionRetired {
            opcode: 0x36,
            tstates: 34,
        }
    );
    assert_eq!(cpu.registers.pc, 0x0001);
    assert_eq!(cpu.iy, 0xFFFF);
}
