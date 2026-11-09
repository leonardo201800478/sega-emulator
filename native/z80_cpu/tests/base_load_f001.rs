//! E7.7 F-001: BASE register, (HL), and immediate LD forms.
//!
//! The matrix keeps HALT (0x76) outside F-001 and validates its own F-026
//! family. All 256 data bytes are paired with all 256 initial F patterns.

use sega_z80_cpu::bus::CpuBus;
use sega_z80_cpu::{
    Z80, Z80BusOperation, Z80MicroSequencer, Z80MicroSequencerState, Z80MicroStep,
    Z80Registers,
};

const PROGRAM_COUNTER: u16 = 0x0100;
const INITIAL_WZ: u16 = 0xA55A;
const INITIAL_Q: u8 = 0x3C;
const HL_ADDRESS_VECTORS: [u16; 4] = [0xC100, 0x00FF, 0xFFFF, 0x0001];

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
    let mut accesses = Vec::with_capacity(3);

    loop {
        match step {
            Z80MicroStep::BusRequest(request) => {
                cpu.insert_wait_tstates(1)
                    .expect("one WAIT T-state extends the outstanding request");
                cpu = snapshot_round_trip(cpu);
                accesses.push(Access {
                    operation: request.operation,
                    address: request.address,
                    base_tstates: request.base_tstates,
                });
                let data = match request.operation {
                    Z80BusOperation::OpcodeFetch | Z80BusOperation::MemoryRead => {
                        Some(memory[usize::from(request.address)])
                    }
                    Z80BusOperation::MemoryWrite { value } => {
                        memory[usize::from(request.address)] = value;
                        None
                    }
                    other => panic!("unexpected F-001 bus operation: {other:?}"),
                };
                step = cpu
                    .complete_bus_cycle(request.id, data)
                    .expect("bus response completes the matching request");
                cpu = snapshot_round_trip(cpu);
            }
            Z80MicroStep::InstructionRetired { tstates, .. } => {
                return (cpu, tstates, accesses);
            }
            Z80MicroStep::InternalCycle { .. } => {
                panic!("F-001 LD forms must not invent an internal M-cycle");
            }
            other => panic!("unexpected F-001 microstep: {other:?}"),
        }
    }
}

fn read_register(registers: &Z80Registers, code: u8, memory: &[u8; 65_536]) -> u8 {
    match code {
        0 => registers.b,
        1 => registers.c,
        2 => registers.d,
        3 => registers.e,
        4 => registers.h,
        5 => registers.l,
        6 => memory[usize::from(registers.hl())],
        7 => registers.a,
        _ => unreachable!("three-bit register code"),
    }
}

fn write_register(registers: &mut Z80Registers, code: u8, value: u8) {
    match code {
        0 => registers.b = value,
        1 => registers.c = value,
        2 => registers.d = value,
        3 => registers.e = value,
        4 => registers.h = value,
        5 => registers.l = value,
        6 => unreachable!("(HL) is a bus destination, not a register field"),
        7 => registers.a = value,
        _ => unreachable!("three-bit register code"),
    }
}

fn initial_registers(
    source: u8,
    destination: u8,
    value: u8,
    address: u16,
    flags: u8,
    program_counter: u16,
) -> Z80Registers {
    let mut registers = Z80Registers {
        a: 0x77,
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

    let hl = match source {
        // Keep H/L as the selected source while varying all 256 byte values.
        4 => (u16::from(value) << 8) | 0x005A,
        5 => 0x5A00 | u16::from(value),
        // Memory forms use the pre-instruction HL address.
        6 => address,
        _ if destination == 6 => address,
        _ => 0xC100,
    };
    registers.set_hl(hl);

    if !matches!(source, 4 | 5 | 6) {
        write_register(&mut registers, source, value);
    }
    registers
}

fn assert_differential_case(
    opcode: u8,
    program_counter: u16,
    initial_registers: Z80Registers,
    memory: &mut [u8; 65_536],
    memory_result: Option<(u16, u8)>,
    expected_registers: Z80Registers,
    expected_accesses: Vec<Access>,
    expected_tstates: u32,
) {
    let initial_memory = memory_result.map(|(address, _)| memory[usize::from(address)]);

    let mut atomic = Z80::default();
    atomic.registers = initial_registers;
    atomic.wz = INITIAL_WZ;
    atomic.q = INITIAL_Q;
    atomic.r = 0x80;
    let atomic_tstates = atomic
        .step(&mut FlatBus { memory })
        .expect("atomic F-001 instruction executes");
    let atomic_memory = memory_result.map(|(address, _)| memory[usize::from(address)]);

    if let (Some((address, _)), Some(initial)) = (memory_result, initial_memory) {
        memory[usize::from(address)] = initial;
    }

    let mut micro = Z80MicroSequencer::default();
    micro.registers = initial_registers;
    micro.wz = INITIAL_WZ;
    micro.q = INITIAL_Q;
    micro.refresh = 0x80;
    let (micro, micro_tstates, accesses) = run_microsequenced(micro, memory);

    let context = format!("opcode={opcode:02X}, pc={program_counter:04X}");
    assert_eq!(atomic_tstates, expected_tstates, "{context}: atomic timing");
    assert_eq!(
        micro_tstates,
        expected_tstates + expected_accesses.len() as u32,
        "{context}: nominal timing plus one WAIT per bus request"
    );
    assert_eq!(accesses, expected_accesses, "{context}: bus order");
    assert_eq!(atomic.registers, expected_registers, "{context}: semantic oracle");
    assert_eq!(
        atomic.export_architectural_state(),
        micro.export_architectural_state(),
        "{context}: atomic/micro architectural differential"
    );
    assert_eq!(atomic.registers.f, initial_registers.f, "{context}: F preserved");
    assert_eq!(atomic.wz, INITIAL_WZ, "{context}: WZ preserved");
    assert_eq!(micro.wz, INITIAL_WZ, "{context}: micro WZ preserved");
    assert_eq!(atomic.q, 0, "{context}: Q cleared by non-flag LD");
    assert_eq!(micro.q, 0, "{context}: micro Q cleared by non-flag LD");
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
    assert_eq!(atomic.r, 0x81, "{context}: one M1 refresh preserves R7");
    assert_eq!(micro.refresh, 0x81, "{context}: micro M1 refresh preserves R7");

    if let Some((address, expected)) = memory_result {
        assert_eq!(atomic_memory, Some(expected), "{context}: atomic memory");
        assert_eq!(memory[usize::from(address)], expected, "{context}: micro memory");
    }
}

fn register_matrix_case(
    opcode: u8,
    value: u8,
    address: u16,
    memory: &mut [u8; 65_536],
) {
    let destination = (opcode >> 3) & 0x07;
    let source = opcode & 0x07;
    let flags = value.rotate_left(1) ^ 0xA5;
    let registers = initial_registers(source, destination, value, address, flags, PROGRAM_COUNTER);
    let hl = registers.hl();

    memory[usize::from(PROGRAM_COUNTER)] = opcode;
    let memory_result = if source == 6 {
        memory[usize::from(hl)] = value;
        Some((hl, value))
    } else if destination == 6 {
        memory[usize::from(hl)] = 0x96;
        Some((hl, read_register(&registers, source, memory)))
    } else {
        None
    };

    let transferred = read_register(&registers, source, memory);
    let mut expected_registers = registers;
    if destination != 6 {
        write_register(&mut expected_registers, destination, transferred);
    }
    expected_registers.pc = PROGRAM_COUNTER.wrapping_add(1);

    let mut expected_accesses = vec![Access {
        operation: Z80BusOperation::OpcodeFetch,
        address: PROGRAM_COUNTER,
        base_tstates: 4,
    }];
    if source == 6 {
        expected_accesses.push(Access {
            operation: Z80BusOperation::MemoryRead,
            address: hl,
            base_tstates: 3,
        });
    } else if destination == 6 {
        expected_accesses.push(Access {
            operation: Z80BusOperation::MemoryWrite { value: transferred },
            address: hl,
            base_tstates: 3,
        });
    }

    let expected_tstates = if source == 6 || destination == 6 {
        7
    } else {
        4
    };
    assert_differential_case(
        opcode,
        PROGRAM_COUNTER,
        registers,
        memory,
        memory_result,
        expected_registers,
        expected_accesses,
        expected_tstates,
    );
}

fn immediate_load_case(
    opcode: u8,
    value: u8,
    address: u16,
    program_counter: u16,
    memory: &mut [u8; 65_536],
) {
    let destination = (opcode >> 3) & 0x07;
    let flags = value.rotate_left(1) ^ 0xA5;
    let mut registers = Z80Registers {
        a: 0x77,
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
    if destination == 6 {
        registers.set_hl(address);
    } else {
        registers.set_hl(0xC100);
    }

    memory[usize::from(program_counter)] = opcode;
    memory[usize::from(program_counter.wrapping_add(1))] = value;
    let memory_result = if destination == 6 {
        memory[usize::from(address)] = 0x96;
        Some((address, value))
    } else {
        None
    };

    let mut expected_registers = registers;
    if destination != 6 {
        write_register(&mut expected_registers, destination, value);
    }
    expected_registers.pc = program_counter.wrapping_add(2);

    let mut expected_accesses = vec![
        Access {
            operation: Z80BusOperation::OpcodeFetch,
            address: program_counter,
            base_tstates: 4,
        },
        Access {
            operation: Z80BusOperation::MemoryRead,
            address: program_counter.wrapping_add(1),
            base_tstates: 3,
        },
    ];
    if destination == 6 {
        expected_accesses.push(Access {
            operation: Z80BusOperation::MemoryWrite { value },
            address,
            base_tstates: 3,
        });
    }

    assert_differential_case(
        opcode,
        program_counter,
        registers,
        memory,
        memory_result,
        expected_registers,
        expected_accesses,
        if destination == 6 { 10 } else { 7 },
    );
}

#[test]
fn all_base_register_and_hl_ld_encodings_match_independent_oracle_and_microsequencer() {
    let mut memory = Box::new([0u8; 65_536]);
    let mut tested = 0usize;

    for opcode in 0x40_u8..=0x7F {
        if opcode == 0x76 {
            continue; // HALT belongs to F-026, not F-001.
        }
        let destination = (opcode >> 3) & 0x07;
        let source = opcode & 0x07;
        for value in u8::MIN..=u8::MAX {
            if source != 6 && destination != 6 {
                register_matrix_case(opcode, value, 0xC100, &mut memory);
                tested += 1;
            } else if destination == 6 && matches!(source, 4 | 5) {
                let address = if source == 4 {
                    (u16::from(value) << 8) | 0x005A
                } else {
                    0x5A00 | u16::from(value)
                };
                register_matrix_case(opcode, value, address, &mut memory);
                tested += 1;
            } else {
                for address in HL_ADDRESS_VECTORS {
                    register_matrix_case(opcode, value, address, &mut memory);
                    tested += 1;
                }
            }
        }
    }

    assert_eq!(tested, 25_344, "all F-001 LD r,r' forms and memory boundaries");
}

#[test]
fn all_base_immediate_ld_encodings_cover_every_byte_and_hl_boundary() {
    let mut memory = Box::new([0u8; 65_536]);
    let mut tested = 0usize;

    for opcode in [0x06, 0x0E, 0x16, 0x1E, 0x26, 0x2E, 0x3E] {
        for value in u8::MIN..=u8::MAX {
            immediate_load_case(opcode, value, 0xC100, PROGRAM_COUNTER, &mut memory);
            tested += 1;
        }
    }

    for value in u8::MIN..=u8::MAX {
        for address in HL_ADDRESS_VECTORS {
            immediate_load_case(0x36, value, address, PROGRAM_COUNTER, &mut memory);
            tested += 1;
        }
    }

    assert_eq!(tested, 2_816, "all F-001 immediate LD values and (HL) boundaries");
}

#[test]
fn base_immediate_ld_wraps_operand_fetch_from_ffff_to_0000() {
    let mut memory = Box::new([0u8; 65_536]);
    immediate_load_case(0x3E, 0xA5, 0xC100, 0xFFFF, &mut memory);
}
