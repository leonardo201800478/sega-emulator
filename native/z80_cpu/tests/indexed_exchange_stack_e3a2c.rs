//! E3A2c EX (SP),IX/IY through the resumable Z80 sequencer.

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
fn ex_sp_ix_preserves_bus_order_waits_and_every_snapshot_boundary() {
    let mut cpu = Z80MicroSequencer::default();
    cpu.ix = 0x3988;
    cpu.registers.sp = 0x0100;
    cpu.registers.f = 0xA5;

    let fetch = start(&mut cpu);
    let fetch = feed_prefix(&mut cpu, fetch, 0xDD);
    let read_low = request(cpu.complete_bus_cycle(fetch.id, Some(0xE3)).unwrap());
    assert_eq!(read_low.address, 0x0100);
    cpu.insert_wait_tstate().unwrap();
    cpu = round_trip(cpu);

    let read_high = request(cpu.complete_bus_cycle(read_low.id, Some(0x90)).unwrap());
    assert_eq!(read_high.address, 0x0101);
    cpu.insert_wait_tstates(2).unwrap();
    cpu = round_trip(cpu);

    assert_eq!(
        cpu.complete_bus_cycle(read_high.id, Some(0x48)).unwrap(),
        Z80MicroStep::InternalCycle { tstates: 1 }
    );
    cpu = round_trip(cpu);

    let write_low = request(cpu.complete_internal_cycle().unwrap());
    assert_eq!(write_low.address, 0x0100);
    assert_eq!(
        write_low.operation,
        Z80BusOperation::MemoryWrite { value: 0x88 }
    );
    cpu.insert_wait_tstate().unwrap();
    cpu = round_trip(cpu);

    let write_high = request(cpu.complete_bus_cycle(write_low.id, None).unwrap());
    assert_eq!(write_high.address, 0x0101);
    assert_eq!(
        write_high.operation,
        Z80BusOperation::MemoryWrite { value: 0x39 }
    );
    cpu = round_trip(cpu);

    assert_eq!(
        cpu.complete_bus_cycle(write_high.id, None).unwrap(),
        Z80MicroStep::InternalCycle { tstates: 1 }
    );
    cpu = round_trip(cpu);
    assert_eq!(
        cpu.complete_internal_cycle().unwrap(),
        Z80MicroStep::InternalCycle { tstates: 1 }
    );
    assert_eq!(
        cpu.complete_internal_cycle().unwrap(),
        Z80MicroStep::InstructionRetired {
            opcode: 0xE3,
            tstates: 27,
        }
    );

    assert_eq!(cpu.ix, 0x4890);
    assert_eq!(cpu.registers.sp, 0x0100);
    assert_eq!(cpu.wz, 0x4890);
    assert_eq!(cpu.registers.f, 0xA5);
    assert_eq!(cpu.registers.pc, 2);
}

#[test]
fn ex_sp_iy_wraps_memory_addresses_without_changing_sp() {
    let mut cpu = Z80MicroSequencer::default();
    cpu.iy = 0xBEEF;
    cpu.registers.sp = 0xFFFF;
    cpu.registers.f = 0x5A;

    let fetch = start(&mut cpu);
    let fetch = feed_prefix(&mut cpu, fetch, 0xDD);
    let fetch = feed_prefix(&mut cpu, fetch, 0xFD);
    let read_low = request(cpu.complete_bus_cycle(fetch.id, Some(0xE3)).unwrap());
    assert_eq!(read_low.address, 0xFFFF);

    let read_high = request(cpu.complete_bus_cycle(read_low.id, Some(0x11)).unwrap());
    assert_eq!(read_high.address, 0x0000);

    assert_eq!(
        cpu.complete_bus_cycle(read_high.id, Some(0x22)).unwrap(),
        Z80MicroStep::InternalCycle { tstates: 1 }
    );
    let write_low = request(cpu.complete_internal_cycle().unwrap());
    assert_eq!(write_low.address, 0xFFFF);
    assert_eq!(
        write_low.operation,
        Z80BusOperation::MemoryWrite { value: 0xEF }
    );

    let write_high = request(cpu.complete_bus_cycle(write_low.id, None).unwrap());
    assert_eq!(write_high.address, 0x0000);
    assert_eq!(
        write_high.operation,
        Z80BusOperation::MemoryWrite { value: 0xBE }
    );

    assert_eq!(
        cpu.complete_bus_cycle(write_high.id, None).unwrap(),
        Z80MicroStep::InternalCycle { tstates: 1 }
    );
    assert_eq!(
        cpu.complete_internal_cycle().unwrap(),
        Z80MicroStep::InternalCycle { tstates: 1 }
    );
    assert_eq!(
        cpu.complete_internal_cycle().unwrap(),
        Z80MicroStep::InstructionRetired {
            opcode: 0xE3,
            tstates: 27,
        }
    );
    assert_eq!(cpu.iy, 0x2211);
    assert_eq!(cpu.registers.sp, 0xFFFF);
    assert_eq!(cpu.wz, 0x2211);
    assert_eq!(cpu.registers.f, 0x5A);
    assert_eq!(cpu.registers.pc, 3);
}
