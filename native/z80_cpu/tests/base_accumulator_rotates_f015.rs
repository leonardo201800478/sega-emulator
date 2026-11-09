//! E7.7 F-015: exhaustive BASE accumulator rotate semantics.

use sega_z80_cpu::bus::CpuBus;
use sega_z80_cpu::{
    Z80, Z80BusOperation, Z80MicroSequencer, Z80MicroSequencerState, Z80MicroStep, Z80Registers,
};

const PROGRAM_COUNTER: u16 = 0x0200;
const INITIAL_WZ: u16 = 0xA55A;
const INITIAL_Q: u8 = 0x6D;
const ROTATES: [(u8, u8); 4] = [(0x07, 0), (0x0F, 1), (0x17, 2), (0x1F, 3)];
const FLAG_C: u8 = 0x01;
const FLAG_PV: u8 = 0x04;
const FLAG_Z: u8 = 0x40;
const FLAG_S: u8 = 0x80;
const FLAG_XY: u8 = 0x28;

struct FlatBus<'a> {
    memory: &'a mut [u8; 65_536],
}

impl CpuBus for FlatBus<'_> {
    fn read_memory(&mut self, address: u16) -> u8 {
        self.memory[usize::from(address)]
    }

    fn write_memory(&mut self, address: u16, value: u8) {
        self.memory[usize::from(address)] = value;
    }
}

fn initial_registers(pc: u16, accumulator: u8, flags: u8) -> Z80Registers {
    Z80Registers {
        a: accumulator,
        f: flags,
        b: 0x11,
        c: 0x22,
        d: 0x33,
        e: 0x44,
        h: 0x55,
        l: 0x66,
        sp: 0x7788,
        pc,
    }
}

fn expected_rotate(accumulator: u8, flags: u8, operation: u8) -> (u8, u8) {
    let old_carry = flags & FLAG_C != 0;
    let (result, carry) = match operation {
        0 => (accumulator.rotate_left(1), accumulator & 0x80 != 0),
        1 => (accumulator.rotate_right(1), accumulator & 0x01 != 0),
        2 => (
            (accumulator << 1) | u8::from(old_carry),
            accumulator & 0x80 != 0,
        ),
        3 => (
            (accumulator >> 1) | (u8::from(old_carry) << 7),
            accumulator & 0x01 != 0,
        ),
        _ => unreachable!("two-bit accumulator rotate operation"),
    };
    let expected_flags =
        (flags & (FLAG_S | FLAG_Z | FLAG_PV)) | (result & FLAG_XY) | if carry { FLAG_C } else { 0 };
    (result, expected_flags)
}

fn snapshot_round_trip(cpu: Z80MicroSequencer) -> Z80MicroSequencer {
    let encoded = cpu.export_state().encode().expect("snapshot encodes");
    let decoded = Z80MicroSequencerState::decode(&encoded).expect("snapshot decodes");
    Z80MicroSequencer::restore_state(decoded).expect("snapshot restores")
}

fn retire_micro(
    mut cpu: Z80MicroSequencer,
    memory: &[u8; 65_536],
    wait_tstates: u32,
    snapshot: bool,
) -> (Z80MicroSequencer, u32, Z80BusOperation, u16, u8) {
    let Z80MicroStep::BusRequest(request) = cpu.begin_instruction().expect("instruction starts")
    else {
        panic!("accumulator rotate must begin with M1");
    };
    assert_eq!(request.operation, Z80BusOperation::OpcodeFetch);
    assert_eq!(request.base_tstates, 4);
    let operation = request.operation;
    let address = request.address;
    let base_tstates = request.base_tstates;
    cpu.insert_wait_tstates(wait_tstates)
        .expect("WAIT extends M1");
    if snapshot {
        cpu = snapshot_round_trip(cpu);
    }
    let data = memory[usize::from(request.address)];
    let step = cpu
        .complete_bus_cycle(request.id, Some(data))
        .expect("M1 response completes");
    if snapshot {
        cpu = snapshot_round_trip(cpu);
    }
    let Z80MicroStep::InstructionRetired { tstates, .. } = step else {
        panic!("accumulator rotate retires at the end of M1");
    };
    (cpu, tstates, operation, address, base_tstates)
}

fn assert_rotate_case(
    opcode: u8,
    operation: u8,
    accumulator: u8,
    flags: u8,
    pc: u16,
    memory: &mut [u8; 65_536],
    wait_tstates: u32,
    snapshot: bool,
) -> (Z80, Z80MicroSequencer, u32) {
    memory[usize::from(pc)] = opcode;
    let initial = initial_registers(pc, accumulator, flags);
    let (expected_a, expected_f) = expected_rotate(accumulator, flags, operation);
    let mut expected_registers = initial;
    expected_registers.a = expected_a;
    expected_registers.f = expected_f;
    expected_registers.pc = pc.wrapping_add(1);

    let mut atomic = Z80::default();
    atomic.registers = initial;
    atomic.wz = INITIAL_WZ;
    atomic.q = INITIAL_Q;
    atomic.r = 0xFF;
    let atomic_tstates = atomic
        .step(&mut FlatBus { memory })
        .expect("atomic accumulator rotate executes");

    let mut micro = Z80MicroSequencer::default();
    micro.registers = initial;
    micro.wz = INITIAL_WZ;
    micro.q = INITIAL_Q;
    micro.refresh = 0xFF;
    let (micro, micro_tstates, operation_seen, address_seen, base_tstates_seen) =
        retire_micro(micro, memory, wait_tstates, snapshot);

    assert_eq!(atomic_tstates, 4, "BASE accumulator rotate is one M1 / 4T");
    assert_eq!(
        micro_tstates,
        4 + wait_tstates,
        "microsequencer includes only the injected M1 WAIT"
    );
    assert_eq!(operation_seen, Z80BusOperation::OpcodeFetch);
    assert_eq!(address_seen, pc);
    assert_eq!(base_tstates_seen, 4);
    assert_eq!(atomic.registers, expected_registers);
    assert_eq!(micro.registers, expected_registers);
    assert_eq!(atomic.wz, INITIAL_WZ, "atomic WZ is preserved");
    assert_eq!(micro.wz, INITIAL_WZ, "micro WZ is preserved");
    assert_eq!(atomic.q, expected_f, "Q stores the last flags byte written");
    assert_eq!(
        micro.q, expected_f,
        "micro Q stores the last flags byte written"
    );
    assert_eq!(
        atomic.export_architectural_state().q_previous,
        INITIAL_Q,
        "previous-Q captures the prior flags latch"
    );
    assert_eq!(
        micro.export_architectural_state().q_previous,
        INITIAL_Q,
        "micro previous-Q captures the prior flags latch"
    );
    assert_eq!(
        atomic.r, 0x80,
        "atomic R7 preserved while low seven bits wrap"
    );
    assert_eq!(
        micro.refresh, 0x80,
        "micro R7 preserved while low seven bits wrap"
    );
    assert_eq!(
        atomic.export_architectural_state(),
        micro.export_architectural_state(),
        "atomic/micro architectural differential"
    );
    (atomic, micro, micro_tstates)
}

#[test]
fn accumulator_rotates_cover_all_a_and_f_values_for_all_four_opcodes() {
    let mut memory = Box::new([0u8; 65_536]);
    let mut tested = 0usize;

    for (opcode, operation) in ROTATES {
        for accumulator in u8::MIN..=u8::MAX {
            for flags in u8::MIN..=u8::MAX {
                assert_rotate_case(
                    opcode,
                    operation,
                    accumulator,
                    flags,
                    PROGRAM_COUNTER,
                    &mut memory,
                    1,
                    false,
                );
                tested += 1;
            }
        }
    }

    assert_eq!(tested, 262_144, "four opcodes × 256 A × 256 F");
}

#[test]
fn accumulator_rotate_m1_snapshot_restore_preserves_every_accumulator_value() {
    let mut memory = Box::new([0u8; 65_536]);
    for (opcode, operation) in ROTATES {
        for accumulator in u8::MIN..=u8::MAX {
            let flags = accumulator.rotate_left(3) ^ 0xA5;
            let (_, continuous, continuous_tstates) = assert_rotate_case(
                opcode,
                operation,
                accumulator,
                flags,
                PROGRAM_COUNTER,
                &mut memory,
                2,
                false,
            );
            let (_, resumed, resumed_tstates) = assert_rotate_case(
                opcode,
                operation,
                accumulator,
                flags,
                PROGRAM_COUNTER,
                &mut memory,
                2,
                true,
            );
            assert_eq!(continuous_tstates, 6, "4T M1 plus two WAITs");
            assert_eq!(resumed_tstates, continuous_tstates);
            assert_eq!(
                resumed.export_architectural_state(),
                continuous.export_architectural_state(),
                "snapshot restore preserves opcode {opcode:02X}, A={accumulator:02X}"
            );
        }
    }
}

#[test]
fn accumulator_rotate_opcode_fetch_wraps_pc_from_ffff_to_0000() {
    let mut memory = Box::new([0u8; 65_536]);
    for (opcode, operation) in ROTATES {
        assert_rotate_case(opcode, operation, 0x81, 0xD7, 0xFFFF, &mut memory, 1, true);
    }
}
