use sega_z80_cpu::bus::CpuBus;
use sega_z80_cpu::{
    Z80, Z80AlternateRegisters, Z80ArchitecturalState, Z80BusOperation, Z80MicroSequencer,
    Z80MicroSequencerState, Z80MicroStep, Z80Registers,
};

const FLAG_C: u8 = 0x01;
const FLAG_PV: u8 = 0x04;
const FLAG_Z: u8 = 0x40;
const FLAG_S: u8 = 0x80;
const FLAG_XY: u8 = 0x28;
const INITIAL_F: u8 = 0xFE;

#[derive(Clone)]
struct TestBus {
    memory: Box<[u8; 0x1_0000]>,
    reads: Vec<u16>,
    writes: Vec<(u16, u8)>,
}

impl Default for TestBus {
    fn default() -> Self {
        Self {
            memory: Box::new([0; 0x1_0000]),
            reads: Vec::new(),
            writes: Vec::new(),
        }
    }
}

impl CpuBus for TestBus {
    fn read_memory(&mut self, address: u16) -> u8 {
        self.reads.push(address);
        self.memory[usize::from(address)]
    }

    fn write_memory(&mut self, address: u16, value: u8) {
        self.writes.push((address, value));
        self.memory[usize::from(address)] = value;
    }
}

fn refresh_after_two_m1(value: u8) -> u8 {
    (value & 0x80) | (value.wrapping_add(2) & 0x7F)
}

fn expected_ld_a_i_r_flags(value: u8, iff2: bool, carry: u8) -> u8 {
    let mut flags = carry | (value & (FLAG_S | FLAG_XY));
    if value == 0 {
        flags |= FLAG_Z;
    }
    if iff2 {
        flags |= FLAG_PV;
    }
    flags
}

fn initial_state(opcode: u8, value: u8, iff2: bool, carry: u8) -> Z80ArchitecturalState {
    let (a, i, r) = match opcode {
        0x47 | 0x4F => (value, 0x5A, 0xFE),
        0x57 => (0xC3, value, 0xFE),
        0x5F => (0xC3, 0x5A, value),
        _ => unreachable!("only the four documented I/R transfer opcodes are tested"),
    };
    Z80ArchitecturalState {
        registers: Z80Registers {
            a,
            f: INITIAL_F | carry,
            b: 0x12,
            c: 0x34,
            d: 0x56,
            e: 0x78,
            h: 0xC1,
            l: 0x23,
            pc: 0x1000,
            sp: 0xD000,
        },
        alternate: Z80AlternateRegisters::default(),
        ix: 0x1357,
        iy: 0x2468,
        i,
        r,
        iff1: false,
        iff2,
        interrupt_mode: 1,
        wz: 0xA55A,
        ei_delay: 0,
        nmi_pending: false,
        int_pending: None,
        q: 0x3C,
        q_previous: 0xA5,
        p: false,
        halted: false,
    }
}

fn round_trip(cpu: &mut Z80MicroSequencer) {
    let encoded = cpu.export_state().encode().unwrap();
    let decoded = Z80MicroSequencerState::decode(&encoded).unwrap();
    *cpu = Z80MicroSequencer::restore_state(decoded).unwrap();
}

fn bus_request(step: Z80MicroStep) -> sega_z80_cpu::Z80BusRequest {
    match step {
        Z80MicroStep::BusRequest(request) => request,
        other => panic!("expected opcode-fetch request, got {other:?}"),
    }
}

fn run_sequenced_with_waits_and_snapshots(
    cpu: &mut Z80MicroSequencer,
    bus: &mut TestBus,
    opcode: u8,
) {
    let first = bus_request(cpu.begin_instruction().unwrap());
    assert_eq!(first.operation, Z80BusOperation::OpcodeFetch);
    assert_eq!(first.address, 0x1000);
    cpu.insert_wait_tstates(1).unwrap();
    let first_data = bus.read_memory(first.address);
    let second = cpu.complete_bus_cycle(first.id, Some(first_data)).unwrap();
    round_trip(cpu);

    let second = bus_request(second);
    assert_eq!(second.operation, Z80BusOperation::OpcodeFetch);
    assert_eq!(second.address, 0x1001);
    cpu.insert_wait_tstates(1).unwrap();
    let second_data = bus.read_memory(second.address);
    let internal = cpu
        .complete_bus_cycle(second.id, Some(second_data))
        .unwrap();
    round_trip(cpu);

    assert_eq!(internal, Z80MicroStep::InternalCycle { tstates: 1 });
    let retired = cpu.complete_internal_cycle().unwrap();
    round_trip(cpu);
    assert_eq!(
        retired,
        Z80MicroStep::InstructionRetired {
            opcode,
            tstates: 11,
        }
    );
}

#[test]
fn all_four_i_r_transfer_opcodes_cover_all_byte_values_iff2_and_carry() {
    let mut atomic_bus = TestBus::default();
    let mut sequenced_bus = TestBus::default();

    for opcode in [0x47, 0x4F, 0x57, 0x5F] {
        atomic_bus.memory[0x1000] = 0xED;
        atomic_bus.memory[0x1001] = opcode;
        sequenced_bus.memory[0x1000] = 0xED;
        sequenced_bus.memory[0x1001] = opcode;

        for value in u8::MIN..=u8::MAX {
            for iff2 in [false, true] {
                for carry in [0, FLAG_C] {
                    let state = initial_state(opcode, value, iff2, carry);
                    atomic_bus.reads.clear();
                    atomic_bus.writes.clear();
                    let mut atomic = Z80::default();
                    atomic.import_architectural_state(state).unwrap();
                    assert_eq!(
                        atomic.step(&mut atomic_bus),
                        Ok(9),
                        "atomic ED {opcode:02X}, value={value:02X}, IFF2={iff2}, C={carry}"
                    );

                    let loaded = if opcode == 0x5F {
                        refresh_after_two_m1(value)
                    } else {
                        value
                    };
                    let expected_a = loaded;
                    let expected_i = if opcode == 0x47 || opcode == 0x57 {
                        value
                    } else {
                        0x5A
                    };
                    let expected_r = match opcode {
                        0x4F => value,
                        0x5F => refresh_after_two_m1(value),
                        _ => refresh_after_two_m1(0xFE),
                    };
                    let expected_flags = if matches!(opcode, 0x57 | 0x5F) {
                        expected_ld_a_i_r_flags(loaded, iff2, carry)
                    } else {
                        INITIAL_F | carry
                    };
                    let expected_p = matches!(opcode, 0x57 | 0x5F);

                    assert_eq!(atomic.registers.a, expected_a);
                    assert_eq!(atomic.i, expected_i);
                    assert_eq!(atomic.r, expected_r);
                    assert_eq!(atomic.registers.f, expected_flags);
                    assert_eq!(atomic.registers.pc, 0x1002);
                    assert_eq!(atomic.wz, 0xA55A);
                    assert_eq!(atomic.iff1, false);
                    assert_eq!(atomic.iff2, iff2);
                    assert_eq!(atomic.export_architectural_state().p, expected_p);
                    assert_eq!(atomic_bus.reads, vec![0x1000, 0x1001]);
                    assert!(atomic_bus.writes.is_empty());

                    sequenced_bus.reads.clear();
                    sequenced_bus.writes.clear();
                    let mut sequenced = Z80MicroSequencer::default();
                    sequenced.import_architectural_state(state).unwrap();
                    run_sequenced_with_waits_and_snapshots(
                        &mut sequenced,
                        &mut sequenced_bus,
                        opcode,
                    );

                    assert_eq!(
                        sequenced.export_architectural_state(),
                        atomic.export_architectural_state(),
                        "atomic/sequenced mismatch ED {opcode:02X}, value={value:02X}, IFF2={iff2}, C={carry}"
                    );
                    assert_eq!(sequenced_bus.reads, vec![0x1000, 0x1001]);
                    assert!(sequenced_bus.writes.is_empty());
                    assert_eq!(sequenced_bus.memory, atomic_bus.memory);
                }
            }
        }
    }
}
