use super::{read_phase, write_phase, Reader, Writer, Z80MicroSequencerPhase};

fn assert_phase_round_trip(phase: Z80MicroSequencerPhase) {
    let mut writer = Writer::default();
    write_phase(&mut writer, phase);

    let mut reader = Reader::new(&writer.bytes);
    let decoded = read_phase(&mut reader).expect("phase tag and payload decode");
    assert_eq!(decoded, phase);
    assert_eq!(reader.offset, writer.bytes.len(), "phase decoder consumes exact payload");
}

#[test]
fn e7_2e_all_seventy_phase_variants_round_trip_with_distinct_tags() {
    let phases = [
        Z80MicroSequencerPhase::Ready,
        Z80MicroSequencerPhase::OpcodeFetch,
        Z80MicroSequencerPhase::InterruptAcknowledge,
        Z80MicroSequencerPhase::InterruptAcknowledgeComplete { vector: 0xFF, mode: 2 },
        Z80MicroSequencerPhase::InterruptModeDispatchInternal {
            mode: 2, vector: 0x80, return_address: 0x4567, remaining_tstates: 1,
        },
        Z80MicroSequencerPhase::InterruptStackPushWrite {
            mode: 2, vector: 0x80, stage: 1, return_address: 0x4567,
        },
        Z80MicroSequencerPhase::InterruptMode2VectorReadLow { vector_address: 0x2080 },
        Z80MicroSequencerPhase::InterruptMode2VectorReadHigh {
            vector_address: 0x2081, low: 0x34,
        },
        Z80MicroSequencerPhase::NmiResponse,
        Z80MicroSequencerPhase::NmiStackPushInternal { return_address: 0x1234 },
        Z80MicroSequencerPhase::NmiStackPushWrite { stage: 1, return_address: 0x1234 },
        Z80MicroSequencerPhase::ImmediateReadRegister { destination: 7 },
        Z80MicroSequencerPhase::ImmediateReadRegisterPairLow { pair: 2 },
        Z80MicroSequencerPhase::ImmediateReadRegisterPairHigh { pair: 2, low: 0x34 },
        Z80MicroSequencerPhase::BaseAbsoluteTransfer { stage: 3, address: 0x4567, low: 0x34 },
        Z80MicroSequencerPhase::BaseRelativeBranchImmediate,
        Z80MicroSequencerPhase::BaseRelativeBranchInternal {
            displacement: -2, branch_taken: true, remaining_tstates: 1,
        },
        Z80MicroSequencerPhase::BaseAbsoluteJump { stage: 1, low: 0x34 },
        Z80MicroSequencerPhase::BaseStackPushInternal { high: 0x12, low: 0x34 },
        Z80MicroSequencerPhase::BaseStackPushWrite { stage: 1, low: 0x34 },
        Z80MicroSequencerPhase::BaseStackPop { stage: 1, low: 0x34 },
        Z80MicroSequencerPhase::BaseCallImmediate { stage: 1, low: 0x34 },
        Z80MicroSequencerPhase::BaseControlPushInternal {
            target: 0x1234, high: 0x12, low: 0x34,
        },
        Z80MicroSequencerPhase::BaseControlPushWrite { stage: 1, target: 0x1234, low: 0x34 },
        Z80MicroSequencerPhase::BaseReturnConditionInternal { branch_taken: true },
        Z80MicroSequencerPhase::BaseReturnPop { stage: 1, low: 0x34 },
        Z80MicroSequencerPhase::BaseExchangeStack {
            stage: 2, low: 0x34, high: 0x12, old_hl: 0x5678,
        },
        Z80MicroSequencerPhase::BaseExchangeStackInternal {
            stage: 1, low: 0x34, high: 0x12, old_hl: 0x5678, remaining_tstates: 1,
        },
        Z80MicroSequencerPhase::BaseImmediatePortAddress { opcode: 0xDB },
        Z80MicroSequencerPhase::BaseImmediatePortRead { port_address: 0x12FE },
        Z80MicroSequencerPhase::BaseImmediatePortWrite { port_address: 0x12FE, value: 0xA5 },
        Z80MicroSequencerPhase::ImmediateReadMemoryWrite,
        Z80MicroSequencerPhase::IndexDisplacementRead,
        Z80MicroSequencerPhase::IndexedCbDisplacementRead,
        Z80MicroSequencerPhase::IndexedCbOpcodeRead { address: 0x1234 },
        Z80MicroSequencerPhase::IndexedCbMemoryRead { address: 0x1234 },
        Z80MicroSequencerPhase::IndexedCbMemoryWrite {
            address: 0x1234, value: 0xA5, destination: Some(7),
        },
        Z80MicroSequencerPhase::IndexDisplacementInternal {
            address: 0x1234, remaining_tstates: 2,
        },
        Z80MicroSequencerPhase::IndexedImmediateReadMemoryWrite { address: 0x1234 },
        Z80MicroSequencerPhase::IndexedImmediateStoreInternal {
            address: 0x1234, value: 0xA5, remaining_tstates: 1,
        },
        Z80MicroSequencerPhase::ImmediateReadAlu { operation: 7 },
        Z80MicroSequencerPhase::MemoryReadRegister { destination: 7 },
        Z80MicroSequencerPhase::MemoryReadAlu { operation: 7, address: 0x1234 },
        Z80MicroSequencerPhase::MemoryReadIncDec { decrement: true, address: 0x1234 },
        Z80MicroSequencerPhase::CbMemoryRead { address: 0x1234 },
        Z80MicroSequencerPhase::CbMemoryInternal { address: 0x1234, result: Some(0xA5) },
        Z80MicroSequencerPhase::CbMemoryWrite { address: 0x1234, value: 0xA5 },
        Z80MicroSequencerPhase::EdRegisterInternal { opcode: 0x44 },
        Z80MicroSequencerPhase::EdPortRead { register: 7, address: 0x1234 },
        Z80MicroSequencerPhase::EdPortWrite { address: 0x1234, value: 0xA5 },
        Z80MicroSequencerPhase::Ed16ArithmeticInternal { opcode: 0x4A, remaining_tstates: 3 },
        Z80MicroSequencerPhase::EdAbsoluteTransfer {
            stage: 2, pair: 2, store: true, address: 0x1234, low: 0x34, pair_value: 0x5678,
        },
        Z80MicroSequencerPhase::EdReturn { stage: 1, low: 0x34 },
        Z80MicroSequencerPhase::EdNibbleRotateRead { opcode: 0x67, address: 0x1234 },
        Z80MicroSequencerPhase::EdNibbleRotateInternal {
            opcode: 0x67, address: 0x1234, result_a: 0x12, result_memory: 0x34,
            remaining_tstates: 2,
        },
        Z80MicroSequencerPhase::EdBlockTransferRead {
            opcode: 0xA0, source: 0x1234, destination: 0x5678,
        },
        Z80MicroSequencerPhase::EdBlockTransferWrite {
            opcode: 0xA0, source: 0x1234, destination: 0x5678, value: 0xA5,
        },
        Z80MicroSequencerPhase::EdBlockRepeatInternal { opcode: 0xB0, remaining_tstates: 1 },
        Z80MicroSequencerPhase::EdBlockCompareRead { opcode: 0xA1, address: 0x1234 },
        Z80MicroSequencerPhase::EdBlockCompareInternal {
            opcode: 0xA1, remaining_tstates: 1, repeat: true,
        },
        Z80MicroSequencerPhase::EdBlockCompareRepeatInternal {
            opcode: 0xB1, remaining_tstates: 1,
        },
        Z80MicroSequencerPhase::EdBlockIoInitialInternal { opcode: 0xA2 },
        Z80MicroSequencerPhase::EdBlockIoInputPortRead {
            opcode: 0xA2, port_address: 0x1234, memory_address: 0x5678,
        },
        Z80MicroSequencerPhase::EdBlockIoInputMemoryWrite {
            opcode: 0xA2, port_address: 0x1234, memory_address: 0x5678, value: 0xA5,
        },
        Z80MicroSequencerPhase::EdBlockIoOutputMemoryRead {
            opcode: 0xA3, memory_address: 0x5678,
        },
        Z80MicroSequencerPhase::EdBlockIoOutputPortWrite {
            opcode: 0xA3, port_address: 0x1234, memory_address: 0x5678, value: 0xA5,
        },
        Z80MicroSequencerPhase::EdBlockIoRepeatInternal { opcode: 0xB2, remaining_tstates: 1 },
        Z80MicroSequencerPhase::BaseInternal { opcode: 0x09, remaining_tstates: 1 },
        Z80MicroSequencerPhase::MemoryIncDecInternal {
            decrement: false, old: 0x7F, address: 0x1234,
        },
        Z80MicroSequencerPhase::MemoryWrite { address: 0x1234, value: 0xA5 },
    ];

    assert_eq!(phases.len(), 70);
    for phase in phases {
        assert_phase_round_trip(phase);
    }
}
