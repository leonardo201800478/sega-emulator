//! E7.7 F-003: absolute accumulator load/store semantic and temporal matrix.

use sega_z80_cpu::bus::CpuBus;
use sega_z80_cpu::{
    Z80, Z80BusOperation, Z80MicroSequencer, Z80MicroSequencerState, Z80MicroStep,
    Z80Registers,
};

const PROGRAM_COUNTER: u16 = 0x0200;
const INITIAL_WZ: u16 = 0xA55A;
const INITIAL_Q: u8 = 0x3C;
const WAIT_TSTATES: [u32; 4] = [1, 2, 3, 1];
const ABSOLUTE_ADDRESSES: [u16; 5] = [0x0000, 0x00FF, 0x1234, 0xFFFE, 0xFFFF];

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

fn snapshot_round_trip(cpu: Z80MicroSequencer) -> Z80MicroSequencer {
    let encoded = cpu.export_state().encode().expect("snapshot encodes");
    let decoded = Z80MicroSequencerState::decode(&encoded).expect("snapshot decodes");
    Z80MicroSequencer::restore_state(decoded).expect("snapshot restores")
}

fn run_microsequenced(
    mut cpu: Z80MicroSequencer,
    memory: &mut [u8; 65_536],
) -> (Z80MicroSequencer, u32, Vec<Access>) {
    let mut step = cpu.begin_instruction().expect("instruction starts");
    let mut accesses = Vec::with_capacity(4);

    loop {
        match step {
            Z80MicroStep::BusRequest(request) => {
                let wait_tstates = WAIT_TSTATES[accesses.len()];
                cpu.insert_wait_tstates(wait_tstates)
                    .expect("WAIT extends the outstanding bus request");
                cpu = snapshot_round_trip(cpu);
                accesses.push(Access {
                    operation: request.operation,
                    address: request.address,
                    base_tstates: request.base_tstates,
                    wait_tstates,
                });
                let data = match request.operation {
                    Z80BusOperation::OpcodeFetch | Z80BusOperation::MemoryRead => {
                        Some(memory[usize::from(request.address)])
                    }
                    Z80BusOperation::MemoryWrite { value } => {
                        memory[usize::from(request.address)] = value;
                        None
                    }
                    other => panic!("unexpected F-003 bus operation: {other:?}"),
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
                panic!("F-003 absolute accumulator transfers have no internal cycle");
            }
            other => panic!("unexpected F-003 microstep: {other:?}"),
        }
    }
}

fn expected_wz(opcode: u8, address: u16, accumulator: u8) -> u16 {
    let next = address.wrapping_add(1);
    if opcode == 0x3A {
        next
    } else {
        (u16::from(accumulator) << 8) | (next & 0x00FF)
    }
}

#[allow(clippy::too_many_arguments)]
fn assert_differential_case(
    opcode: u8,
    program_counter: u16,
    initial_registers: Z80Registers,
    memory: &mut [u8; 65_536],
    memory_result: (u16, u8),
    expected_registers: Z80Registers,
    expected_wz: u16,
    expected_accesses: Vec<Access>,
) {
    let (memory_address, expected_memory) = memory_result;
    let initial_memory = memory[usize::from(memory_address)];

    let mut atomic = Z80::default();
    atomic.registers = initial_registers;
    atomic.wz = INITIAL_WZ;
    atomic.q = INITIAL_Q;
    atomic.r = 0x80;
    let atomic_tstates = atomic
        .step(&mut FlatBus { memory })
        .expect("atomic F-003 instruction executes");
    let atomic_memory = memory[usize::from(memory_address)];
    memory[usize::from(memory_address)] = initial_memory;

    let mut micro = Z80MicroSequencer::default();
    micro.registers = initial_registers;
    micro.wz = INITIAL_WZ;
    micro.q = INITIAL_Q;
    micro.refresh = 0x80;
    let (micro, micro_tstates, accesses) = run_microsequenced(micro, memory);

    let context = format!("opcode={opcode:02X}, pc={program_counter:04X}");
    assert_eq!(atomic_tstates, 13, "{context}: atomic timing");
    assert_eq!(
        micro_tstates,
        20,
        "{context}: 13 nominal T-states plus 7 WAIT T-states"
    );
    assert_eq!(accesses, expected_accesses, "{context}: bus order and waits");
    assert_eq!(
        atomic.registers, expected_registers,
        "{context}: independent register/PC oracle"
    );
    assert_eq!(atomic.wz, expected_wz, "{context}: atomic WZ oracle");
    assert_eq!(micro.wz, expected_wz, "{context}: micro WZ oracle");
    assert_eq!(
        atomic.export_architectural_state(),
        micro.export_architectural_state(),
        "{context}: atomic/micro architectural differential"
    );
    assert_eq!(
        atomic.registers.f, initial_registers.f,
        "{context}: F preserved"
    );
    assert_eq!(atomic.q, 0, "{context}: Q cleared");
    assert_eq!(micro.q, 0, "{context}: micro Q cleared");
    assert_eq!(
        atomic.export_architectural_state().q_previous,
        INITIAL_Q,
        "{context}: previous Q retained"
    );
    assert_eq!(
        micro.export_architectural_state().q_previous,
        INITIAL_Q,
        "{context}: micro previous Q retained"
    );
    assert_eq!(atomic.r, 0x81, "{context}: refresh preserves R7");
    assert_eq!(micro.refresh, 0x81, "{context}: micro refresh preserves R7");
    assert_eq!(atomic_memory, expected_memory, "{context}: atomic memory");
    assert_eq!(
        memory[usize::from(memory_address)],
        expected_memory,
        "{context}: micro memory"
    );
}

fn absolute_accumulator_case(
    opcode: u8,
    value: u8,
    address: u16,
    program_counter: u16,
    flags: u8,
    memory: &mut [u8; 65_536],
) {
    let is_load = opcode == 0x3A;
    let registers = Z80Registers {
        a: if is_load { 0xC3 } else { value },
        f: flags,
        b: 0x11,
        c: 0x22,
        d: 0x33,
        e: 0x44,
        h: 0x55,
        l: 0x66,
        sp: 0xA000,
        pc: program_counter,
    };

    memory[usize::from(program_counter)] = opcode;
    memory[usize::from(program_counter.wrapping_add(1))] = address as u8;
    memory[usize::from(program_counter.wrapping_add(2))] = (address >> 8) as u8;
    memory[usize::from(address)] = if is_load { value } else { 0x96 };

    let mut expected_registers = registers;
    expected_registers.a = value;
    expected_registers.pc = program_counter.wrapping_add(3);

    let expected_accesses = vec![
        Access {
            operation: Z80BusOperation::OpcodeFetch,
            address: program_counter,
            base_tstates: 4,
            wait_tstates: 1,
        },
        Access {
            operation: Z80BusOperation::MemoryRead,
            address: program_counter.wrapping_add(1),
            base_tstates: 3,
            wait_tstates: 2,
        },
        Access {
            operation: Z80BusOperation::MemoryRead,
            address: program_counter.wrapping_add(2),
            base_tstates: 3,
            wait_tstates: 3,
        },
        Access {
            operation: if is_load {
                Z80BusOperation::MemoryRead
            } else {
                Z80BusOperation::MemoryWrite { value }
            },
            address,
            base_tstates: 3,
            wait_tstates: 1,
        },
    ];

    assert_differential_case(
        opcode,
        program_counter,
        registers,
        memory,
        (address, value),
        expected_registers,
        expected_wz(opcode, address, value),
        expected_accesses,
    );
}

#[test]
fn absolute_accumulator_load_store_cover_all_data_values_flags_and_address_boundaries() {
    let mut memory = Box::new([0u8; 65_536]);
    let mut tested = 0usize;

    for opcode in [0x3A, 0x32] {
        for address in ABSOLUTE_ADDRESSES {
            for value in u8::MIN..=u8::MAX {
                let flags = value.rotate_left(1) ^ 0xA5;
                absolute_accumulator_case(
                    opcode,
                    value,
                    address,
                    PROGRAM_COUNTER,
                    flags,
                    &mut memory,
                );
                tested += 1;
            }
        }
    }

    assert_eq!(
        tested, 2_560,
        "two absolute accumulator opcodes × five addresses × 256 bytes"
    );
}

#[test]
fn absolute_accumulator_operand_fetch_wraps_pc_from_ffff_to_0000() {
    let mut memory = Box::new([0u8; 65_536]);
    for opcode in [0x3A, 0x32] {
        absolute_accumulator_case(opcode, 0xA5, 0x1234, 0xFFFE, 0x5A, &mut memory);
    }
}
