//! E7.7 F-004: BASE 16-bit pair loads and HL absolute-word transfers.

use sega_z80_cpu::bus::CpuBus;
use sega_z80_cpu::{
    Z80, Z80BusOperation, Z80MicroSequencer, Z80MicroSequencerState, Z80MicroStep, Z80Registers,
};

const PROGRAM_COUNTER: u16 = 0x0200;
const INITIAL_WZ: u16 = 0xA55A;
const INITIAL_Q: u8 = 0x6D;
const PAIR_OPCODES: [(u8, u8); 4] = [(0x01, 0), (0x11, 1), (0x21, 2), (0x31, 3)];
const ABSOLUTE_ADDRESSES: [u16; 5] = [0x0000, 0x00FF, 0x1234, 0xFFFE, 0xFFFF];
const IMMEDIATE_WAITS: [u32; 3] = [1, 2, 3];
const ABSOLUTE_WAITS: [u32; 5] = [1, 2, 3, 1, 2];

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Access {
    operation: Z80BusOperation,
    address: u16,
    base_tstates: u8,
    wait_tstates: u32,
}

fn access(
    operation: Z80BusOperation,
    address: u16,
    base_tstates: u8,
    wait_tstates: u32,
) -> Access {
    Access {
        operation,
        address,
        base_tstates,
        wait_tstates,
    }
}

fn snapshot_round_trip(cpu: Z80MicroSequencer) -> Z80MicroSequencer {
    let encoded = cpu.export_state().encode().expect("snapshot encodes");
    let decoded = Z80MicroSequencerState::decode(&encoded).expect("snapshot decodes");
    Z80MicroSequencer::restore_state(decoded).expect("snapshot restores")
}

fn run_microsequenced(
    mut cpu: Z80MicroSequencer,
    memory: &mut [u8; 65_536],
    waits: &[u32],
) -> (Z80MicroSequencer, u32, Vec<Access>) {
    let mut step = cpu.begin_instruction().expect("instruction starts");
    let mut accesses = Vec::with_capacity(waits.len());

    loop {
        match step {
            Z80MicroStep::BusRequest(request) => {
                let wait_tstates = waits[accesses.len()];
                cpu.insert_wait_tstates(wait_tstates)
                    .expect("WAIT extends the outstanding bus request");
                cpu = snapshot_round_trip(cpu);
                accesses.push(access(
                    request.operation,
                    request.address,
                    request.base_tstates,
                    wait_tstates,
                ));
                let data = match request.operation {
                    Z80BusOperation::OpcodeFetch | Z80BusOperation::MemoryRead => {
                        Some(memory[usize::from(request.address)])
                    }
                    Z80BusOperation::MemoryWrite { value } => {
                        memory[usize::from(request.address)] = value;
                        None
                    }
                    other => panic!("unexpected F-004 bus operation: {other:?}"),
                };
                step = cpu
                    .complete_bus_cycle(request.id, data)
                    .expect("matching bus response completes");
                cpu = snapshot_round_trip(cpu);
            }
            Z80MicroStep::InstructionRetired { tstates, .. } => {
                return (cpu, tstates, accesses);
            }
            Z80MicroStep::InternalCycle { .. } => {
                panic!("F-004 load operations have no internal cycle");
            }
            other => panic!("unexpected F-004 microstep: {other:?}"),
        }
    }
}

fn initial_registers(pc: u16, flags: u8) -> Z80Registers {
    Z80Registers {
        a: 0xA5,
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

fn set_pair(registers: &mut Z80Registers, pair: u8, value: u16) {
    match pair {
        0 => registers.set_bc(value),
        1 => registers.set_de(value),
        2 => registers.set_hl(value),
        3 => registers.sp = value,
        _ => unreachable!("two-bit register pair"),
    }
}

fn assert_common_state(
    opcode: u8,
    atomic: &Z80,
    micro: &Z80MicroSequencer,
    expected_registers: Z80Registers,
    expected_wz: u16,
    expected_q_previous: u8,
) {
    let context = format!("opcode={opcode:02X}");
    assert_eq!(atomic.registers, expected_registers, "{context}: atomic registers");
    assert_eq!(micro.registers, expected_registers, "{context}: micro registers");
    assert_eq!(atomic.wz, expected_wz, "{context}: atomic WZ");
    assert_eq!(micro.wz, expected_wz, "{context}: micro WZ");
    assert_eq!(atomic.registers.f, expected_registers.f, "{context}: F preserved");
    assert_eq!(atomic.q, 0, "{context}: atomic Q cleared");
    assert_eq!(micro.q, 0, "{context}: micro Q cleared");
    assert_eq!(
        atomic.export_architectural_state().q_previous,
        expected_q_previous,
        "{context}: atomic previous-Q"
    );
    assert_eq!(
        micro.export_architectural_state().q_previous,
        expected_q_previous,
        "{context}: micro previous-Q"
    );
    assert_eq!(atomic.r, 0x80, "{context}: atomic refresh wraps low seven bits");
    assert_eq!(micro.refresh, 0x80, "{context}: micro refresh wraps low seven bits");
    assert_eq!(
        atomic.export_architectural_state(),
        micro.export_architectural_state(),
        "{context}: atomic/micro architectural differential"
    );
}

fn assert_pair_immediate_case(
    opcode: u8,
    pair: u8,
    value: u16,
    program_counter: u16,
    flags: u8,
    memory: &mut [u8; 65_536],
) {
    let initial = initial_registers(program_counter, flags);
    let [low, high] = value.to_le_bytes();
    memory[usize::from(program_counter)] = opcode;
    memory[usize::from(program_counter.wrapping_add(1))] = low;
    memory[usize::from(program_counter.wrapping_add(2))] = high;

    let mut expected_registers = initial;
    set_pair(&mut expected_registers, pair, value);
    expected_registers.pc = program_counter.wrapping_add(3);

    let mut atomic = Z80::default();
    atomic.registers = initial;
    atomic.wz = INITIAL_WZ;
    atomic.q = INITIAL_Q;
    atomic.r = 0xFF;
    let atomic_tstates = atomic
        .step(&mut FlatBus { memory })
        .expect("atomic LD rr,nn executes");

    let mut micro = Z80MicroSequencer::default();
    micro.registers = initial;
    micro.wz = INITIAL_WZ;
    micro.q = INITIAL_Q;
    micro.refresh = 0xFF;
    let (micro, micro_tstates, accesses) =
        run_microsequenced(micro, memory, &IMMEDIATE_WAITS);

    let expected_accesses = vec![
        access(
            Z80BusOperation::OpcodeFetch,
            program_counter,
            4,
            IMMEDIATE_WAITS[0],
        ),
        access(
            Z80BusOperation::MemoryRead,
            program_counter.wrapping_add(1),
            3,
            IMMEDIATE_WAITS[1],
        ),
        access(
            Z80BusOperation::MemoryRead,
            program_counter.wrapping_add(2),
            3,
            IMMEDIATE_WAITS[2],
        ),
    ];
    assert_eq!(atomic_tstates, 10, "LD rr,nn nominal timing");
    assert_eq!(micro_tstates, 16, "10 nominal T-states plus six WAITs");
    assert_eq!(accesses, expected_accesses, "LD rr,nn bus order");
    assert_common_state(opcode, &atomic, &micro, expected_registers, INITIAL_WZ, INITIAL_Q);
}

fn assert_absolute_word_case(
    opcode: u8,
    value: u16,
    address: u16,
    program_counter: u16,
    flags: u8,
    memory: &mut [u8; 65_536],
) {
    let is_load = opcode == 0x2A;
    let mut initial = initial_registers(program_counter, flags);
    if !is_load {
        initial.set_hl(value);
    }

    let [address_low, address_high] = address.to_le_bytes();
    let [value_low, value_high] = value.to_le_bytes();
    memory[usize::from(program_counter)] = opcode;
    memory[usize::from(program_counter.wrapping_add(1))] = address_low;
    memory[usize::from(program_counter.wrapping_add(2))] = address_high;
    let next_address = address.wrapping_add(1);
    let before_low = memory[usize::from(address)];
    let before_high = memory[usize::from(next_address)];
    if is_load {
        memory[usize::from(address)] = value_low;
        memory[usize::from(next_address)] = value_high;
    } else {
        memory[usize::from(address)] = 0x96;
        memory[usize::from(next_address)] = 0x69;
    }

    let mut expected_registers = initial;
    if is_load {
        expected_registers.set_hl(value);
    }
    expected_registers.pc = program_counter.wrapping_add(3);
    let expected_wz = next_address;

    let mut atomic = Z80::default();
    atomic.registers = initial;
    atomic.wz = INITIAL_WZ;
    atomic.q = INITIAL_Q;
    atomic.r = 0xFF;
    let atomic_tstates = atomic
        .step(&mut FlatBus { memory })
        .expect("atomic absolute HL transfer executes");
    let atomic_memory = (
        memory[usize::from(address)],
        memory[usize::from(next_address)],
    );

    memory[usize::from(address)] = if is_load { value_low } else { before_low };
    memory[usize::from(next_address)] = if is_load { value_high } else { before_high };

    let mut micro = Z80MicroSequencer::default();
    micro.registers = initial;
    micro.wz = INITIAL_WZ;
    micro.q = INITIAL_Q;
    micro.refresh = 0xFF;
    let (micro, micro_tstates, accesses) =
        run_microsequenced(micro, memory, &ABSOLUTE_WAITS);

    let low_operation = if is_load {
        Z80BusOperation::MemoryRead
    } else {
        Z80BusOperation::MemoryWrite { value: value_low }
    };
    let high_operation = if is_load {
        Z80BusOperation::MemoryRead
    } else {
        Z80BusOperation::MemoryWrite { value: value_high }
    };
    let expected_accesses = vec![
        access(
            Z80BusOperation::OpcodeFetch,
            program_counter,
            4,
            ABSOLUTE_WAITS[0],
        ),
        access(
            Z80BusOperation::MemoryRead,
            program_counter.wrapping_add(1),
            3,
            ABSOLUTE_WAITS[1],
        ),
        access(
            Z80BusOperation::MemoryRead,
            program_counter.wrapping_add(2),
            3,
            ABSOLUTE_WAITS[2],
        ),
        access(low_operation, address, 3, ABSOLUTE_WAITS[3]),
        access(high_operation, next_address, 3, ABSOLUTE_WAITS[4]),
    ];
    let expected_memory = (value_low, value_high);
    assert_eq!(atomic_tstates, 16, "LD (nn),HL / LD HL,(nn) nominal timing");
    assert_eq!(micro_tstates, 25, "16 nominal T-states plus nine WAITs");
    assert_eq!(accesses, expected_accesses, "absolute HL transfer bus order");
    assert_common_state(opcode, &atomic, &micro, expected_registers, expected_wz, INITIAL_Q);
    assert_eq!(atomic_memory, expected_memory, "atomic low/high memory result");
    assert_eq!(
        (
            memory[usize::from(address)],
            memory[usize::from(next_address)]
        ),
        expected_memory,
        "micro low/high memory result"
    );
}

fn data_word(index: u8) -> u16 {
    u16::from_le_bytes([index, index.rotate_left(3)])
}

#[test]
fn ld_register_pair_immediate_covers_all_pairs_data_bytes_and_flag_patterns() {
    let mut memory = Box::new([0u8; 65_536]);
    let mut tested = 0usize;

    for (opcode, pair) in PAIR_OPCODES {
        for index in u8::MIN..=u8::MAX {
            assert_pair_immediate_case(
                opcode,
                pair,
                data_word(index),
                PROGRAM_COUNTER,
                index,
                &mut memory,
            );
            tested += 1;
        }
        for value in [0x0000, 0x00FF, 0x0100, 0x7FFF, 0x8000, 0xFF00, 0xFFFF] {
            assert_pair_immediate_case(
                opcode,
                pair,
                value,
                PROGRAM_COUNTER,
                value as u8,
                &mut memory,
            );
            tested += 1;
        }
    }

    assert_eq!(tested, 1_052, "four pairs × (256 patterns + seven boundaries)");
}

#[test]
fn ld_register_pair_immediate_operand_fetch_wraps_pc_at_ffff() {
    let mut memory = Box::new([0u8; 65_536]);
    for (opcode, pair) in PAIR_OPCODES {
        assert_pair_immediate_case(opcode, pair, 0xA55A, 0xFFFE, 0xD7, &mut memory);
    }
}

#[test]
fn ld_hl_absolute_word_transfers_cover_all_data_and_address_boundaries() {
    let mut memory = Box::new([0u8; 65_536]);
    let mut tested = 0usize;

    for opcode in [0x22, 0x2A] {
        for address in ABSOLUTE_ADDRESSES {
            for index in u8::MIN..=u8::MAX {
                assert_absolute_word_case(
                    opcode,
                    data_word(index),
                    address,
                    PROGRAM_COUNTER,
                    index,
                    &mut memory,
                );
                tested += 1;
            }
        }
    }

    assert_eq!(
        tested, 2_560,
        "two BASE HL absolute transfers × five addresses × 256 words"
    );
}

#[test]
fn ld_hl_absolute_word_operand_fetch_wraps_pc_at_ffff() {
    let mut memory = Box::new([0u8; 65_536]);
    for opcode in [0x22, 0x2A] {
        assert_absolute_word_case(opcode, 0xA55A, 0x1234, 0xFFFE, 0xD7, &mut memory);
    }
}
