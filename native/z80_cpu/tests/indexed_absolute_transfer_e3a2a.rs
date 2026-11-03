//! E3A2a absolute-memory transfers through IX/IY in the resumable Z80 sequencer.

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
            .expect("prefix schedules the next M1 fetch"),
    )
}

#[test]
fn ld_mem_nn_ix_writes_low_then_high_and_accounts_for_wait_states() {
    let mut cpu = Z80MicroSequencer::default();
    cpu.ix = 0xBEEF;

    let fetch = start(&mut cpu);
    let fetch = feed_prefix(&mut cpu, fetch, 0xDD);
    let low_address = request(cpu.complete_bus_cycle(fetch.id, Some(0x22)).unwrap());
    assert_eq!(low_address.address, 2);

    let high_address = request(cpu.complete_bus_cycle(low_address.id, Some(0x00)).unwrap());
    assert_eq!(high_address.address, 3);

    let low_write = request(cpu.complete_bus_cycle(high_address.id, Some(0x40)).unwrap());
    assert_eq!(low_write.address, 0x4000);
    assert_eq!(
        low_write.operation,
        Z80BusOperation::MemoryWrite { value: 0xEF }
    );
    cpu.insert_wait_tstate().unwrap();

    let high_write = request(cpu.complete_bus_cycle(low_write.id, None).unwrap());
    assert_eq!(high_write.address, 0x4001);
    assert_eq!(
        high_write.operation,
        Z80BusOperation::MemoryWrite { value: 0xBE }
    );
    cpu.insert_wait_tstates(2).unwrap();

    assert_eq!(
        cpu.complete_bus_cycle(high_write.id, None).unwrap(),
        Z80MicroStep::InstructionRetired {
            opcode: 0x22,
            tstates: 23,
        }
    );
    assert_eq!(cpu.ix, 0xBEEF);
    assert_eq!(cpu.wz, 0x4001);
    assert_eq!(cpu.registers.pc, 4);
}

#[test]
fn ld_iy_mem_nn_restores_both_data_read_phases() {
    let mut cpu = Z80MicroSequencer::default();
    let fetch = start(&mut cpu);
    let fetch = feed_prefix(&mut cpu, fetch, 0xFD);
    let low_address = request(cpu.complete_bus_cycle(fetch.id, Some(0x2A)).unwrap());
    let high_address = request(cpu.complete_bus_cycle(low_address.id, Some(0x00)).unwrap());
    let low_read = request(cpu.complete_bus_cycle(high_address.id, Some(0x60)).unwrap());
    assert_eq!(low_read.address, 0x6000);

    let encoded = cpu.export_state().encode().expect("stage-2 snapshot encodes");
    let decoded = Z80MicroSequencerState::decode(&encoded).expect("stage-2 snapshot decodes");
    cpu = Z80MicroSequencer::restore_state(decoded).expect("stage-2 snapshot restores");

    let high_read = request(cpu.complete_bus_cycle(low_read.id, Some(0x34)).unwrap());
    assert_eq!(high_read.address, 0x6001);
    let encoded = cpu.export_state().encode().expect("stage-3 snapshot encodes");
    let decoded = Z80MicroSequencerState::decode(&encoded).expect("stage-3 snapshot decodes");
    cpu = Z80MicroSequencer::restore_state(decoded).expect("stage-3 snapshot restores");

    assert_eq!(
        cpu.complete_bus_cycle(high_read.id, Some(0x12)).unwrap(),
        Z80MicroStep::InstructionRetired {
            opcode: 0x2A,
            tstates: 20,
        }
    );
    assert_eq!(cpu.iy, 0x1234);
    assert_eq!(cpu.wz, 0x6001);
    assert_eq!(cpu.registers.pc, 4);
}

#[test]
fn repeated_dd_fd_prefixes_store_the_last_selected_index_pair() {
    let mut cpu = Z80MicroSequencer::default();
    cpu.ix = 0x1111;
    cpu.iy = 0xBEEF;

    let fetch = start(&mut cpu);
    let fetch = feed_prefix(&mut cpu, fetch, 0xDD);
    let fetch = feed_prefix(&mut cpu, fetch, 0xFD);
    let low_address = request(cpu.complete_bus_cycle(fetch.id, Some(0x22)).unwrap());
    let high_address = request(cpu.complete_bus_cycle(low_address.id, Some(0x00)).unwrap());
    let low_write = request(cpu.complete_bus_cycle(high_address.id, Some(0x70)).unwrap());
    assert_eq!(low_write.address, 0x7000);
    assert_eq!(
        low_write.operation,
        Z80BusOperation::MemoryWrite { value: 0xEF }
    );
    let high_write = request(cpu.complete_bus_cycle(low_write.id, None).unwrap());
    assert_eq!(high_write.address, 0x7001);
    assert_eq!(
        high_write.operation,
        Z80BusOperation::MemoryWrite { value: 0xBE }
    );

    assert_eq!(
        cpu.complete_bus_cycle(high_write.id, None).unwrap(),
        Z80MicroStep::InstructionRetired {
            opcode: 0x22,
            tstates: 24,
        }
    );
    assert_eq!(cpu.ix, 0x1111);
    assert_eq!(cpu.iy, 0xBEEF);
    assert_eq!(cpu.wz, 0x7001);
    assert_eq!(cpu.registers.pc, 5);
}
