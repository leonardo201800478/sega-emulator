//! E7 F-037: DD/FD ALU A,IXH/IXL/IYH/IYL through the resumable Z80 path.
//!
//! The broad matrix differentially compares the microsequencer with the
//! already-audited atomic executor. Fixed vectors independently pin the
//! complete documented flags and the undocumented X/Y behavior of CP.

use sega_z80_cpu::{
    bus::CpuBus, Z80, Z80AlternateRegisters, Z80BusOperation, Z80BusRequest,
    Z80MicroSequencer, Z80MicroSequencerState, Z80MicroStep, Z80Registers,
};

const START_PC: u16 = 0x4000;
const INITIAL_Q: u8 = 0xA5;
const INITIAL_WZ: u16 = 0xBEEF;
const INITIAL_REFRESH: u8 = 0xFE;
const ALU_OPERANDS: [u8; 13] = [
    0x00, 0x01, 0x0F, 0x10, 0x1F, 0x38, 0x55, 0x7F, 0x80, 0x81, 0xAA, 0xFE, 0xFF,
];

struct FlatBus {
    memory: Vec<u8>,
    writes: Vec<(u16, u8)>,
}

impl Default for FlatBus {
    fn default() -> Self {
        Self {
            memory: vec![0; 0x1_0000],
            writes: Vec::new(),
        }
    }
}

impl CpuBus for FlatBus {
    fn read_memory(&mut self, address: u16) -> u8 {
        self.memory[usize::from(address)]
    }

    fn write_memory(&mut self, address: u16, value: u8) {
        self.memory[usize::from(address)] = value;
        self.writes.push((address, value));
    }
}

fn initial_registers(accumulator: u8, flags: u8) -> Z80Registers {
    Z80Registers {
        a: accumulator,
        f: flags,
        b: 0x10,
        c: 0x21,
        d: 0x32,
        e: 0x43,
        h: 0xD6,
        l: 0xE7,
        sp: 0x9000,
        pc: START_PC,
    }
}

fn initial_alternate() -> Z80AlternateRegisters {
    Z80AlternateRegisters {
        a: 0x87,
        f: 0x96,
        b: 0xA1,
        c: 0xB2,
        d: 0xC3,
        e: 0xD4,
        h: 0xE5,
        l: 0xF6,
    }
}

fn index_with_half(prefix: u8, source: u8, operand: u8) -> (u16, u16) {
    let selected = if source == 4 {
        (u16::from(operand) << 8) | 0x00A5
    } else {
        0x5A00 | u16::from(operand)
    };

    match prefix {
        0xDD => (selected, 0xC3D4),
        0xFD => (0xA1B2, selected),
        other => panic!("unexpected index prefix {other:#04X}"),
    }
}

fn seed_atomic(
    prefixes: &[u8],
    opcode: u8,
    accumulator: u8,
    flags: u8,
    ix: u16,
    iy: u16,
) -> (Z80, FlatBus) {
    let mut cpu = Z80::default();
    cpu.registers = initial_registers(accumulator, flags);
    cpu.alternate = initial_alternate();
    cpu.ix = ix;
    cpu.iy = iy;
    cpu.i = 0xA6;
    cpu.r = INITIAL_REFRESH;
    cpu.iff1 = false;
    cpu.iff2 = true;
    cpu.interrupt_mode = 2;
    cpu.wz = INITIAL_WZ;
    cpu.q = INITIAL_Q;
    cpu.p = true;

    let mut bus = FlatBus::default();
    for (offset, byte) in prefixes.iter().copied().chain([opcode]).enumerate() {
        bus.memory[usize::from(START_PC) + offset] = byte;
    }
    (cpu, bus)
}

fn seed_microsequencer(
    accumulator: u8,
    flags: u8,
    ix: u16,
    iy: u16,
) -> Z80MicroSequencer {
    let mut cpu = Z80MicroSequencer::default();
    cpu.registers = initial_registers(accumulator, flags);
    cpu.alternate = initial_alternate();
    cpu.ix = ix;
    cpu.iy = iy;
    cpu.i = 0xA6;
    cpu.refresh = INITIAL_REFRESH;
    cpu.q = INITIAL_Q;
    cpu.q_previous = 0x5A;
    cpu.iff1 = false;
    cpu.iff2 = true;
    cpu.interrupt_mode = 2;
    cpu.wz = INITIAL_WZ;
    cpu.p = true;
    cpu
}

fn request(step: Z80MicroStep) -> Z80BusRequest {
    match step {
        Z80MicroStep::BusRequest(request) => request,
        other => panic!("expected opcode-fetch request, got {other:?}"),
    }
}

fn round_trip_at(cpu: Z80MicroSequencer, stage: &str) -> Z80MicroSequencer {
    let encoded = cpu
        .export_state()
        .encode()
        .unwrap_or_else(|error| panic!("snapshot at {stage} encodes: {error:?}"));
    let decoded = Z80MicroSequencerState::decode(&encoded)
        .unwrap_or_else(|error| panic!("snapshot at {stage} decodes: {error:?}"));
    Z80MicroSequencer::restore_state(decoded)
        .unwrap_or_else(|error| panic!("snapshot at {stage} restores: {error:?}"))
}

fn execute_atomic(
    prefixes: &[u8],
    opcode: u8,
    accumulator: u8,
    flags: u8,
    ix: u16,
    iy: u16,
) -> (Z80, FlatBus, u32) {
    let (mut cpu, mut bus) = seed_atomic(prefixes, opcode, accumulator, flags, ix, iy);
    let tstates = cpu.step(&mut bus).unwrap();
    (cpu, bus, tstates)
}

struct MicroAluCase<'a> {
    prefixes: &'a [u8],
    opcode: u8,
    accumulator: u8,
    flags: u8,
    ix: u16,
    iy: u16,
    waits: &'a [u32],
    snapshot_after_prefix: bool,
}

fn execute_microsequencer(case: MicroAluCase<'_>) -> (Z80MicroSequencer, u32) {
    let MicroAluCase {
        prefixes,
        opcode,
        accumulator,
        flags,
        ix,
        iy,
        waits,
        snapshot_after_prefix,
    } = case;
    assert!(!prefixes.is_empty());
    assert_eq!(waits.len(), prefixes.len() + 1);

    let mut cpu = seed_microsequencer(accumulator, flags, ix, iy);
    let mut fetch = request(cpu.begin_instruction().unwrap());
    assert_eq!(fetch.operation, Z80BusOperation::OpcodeFetch);
    assert_eq!(fetch.base_tstates, 4);

    for (index, prefix) in prefixes.iter().copied().enumerate() {
        if waits[index] != 0 {
            cpu.insert_wait_tstates(waits[index]).unwrap();
            cpu = round_trip_at(cpu, "prefix M1 WAIT");
        }

        fetch = request(cpu.complete_bus_cycle(fetch.id, Some(prefix)).unwrap());
        assert_eq!(fetch.operation, Z80BusOperation::OpcodeFetch);
        assert_eq!(fetch.base_tstates, 4);
        if snapshot_after_prefix {
            cpu = round_trip_at(cpu, "between prefix and terminal M1");
        }
    }

    let opcode_wait = prefixes.len();
    if waits[opcode_wait] != 0 {
        cpu.insert_wait_tstates(waits[opcode_wait]).unwrap();
        cpu = round_trip_at(cpu, "terminal opcode M1 WAIT");
    }

    let step = cpu.complete_bus_cycle(fetch.id, Some(opcode)).unwrap();
    let tstates = match step {
        Z80MicroStep::InstructionRetired {
            opcode: retired_opcode,
            tstates,
        } => {
            assert_eq!(retired_opcode, opcode);
            tstates
        }
        other => panic!("register-source ALU must retire after terminal M1: {other:?}"),
    };
    (cpu, tstates)
}

fn expected_refresh(m1_count: usize) -> u8 {
    let m1_count = u8::try_from(m1_count).unwrap();
    (INITIAL_REFRESH & 0x80) | (INITIAL_REFRESH.wrapping_add(m1_count) & 0x7F)
}

struct ExpectedAluState<'a> {
    expected_a: u8,
    expected_f: u8,
    prefixes: &'a [u8],
    ix: u16,
    iy: u16,
    atomic_tstates: u32,
}

fn assert_shared_state(
    micro: &Z80MicroSequencer,
    atomic: &Z80,
    expected: ExpectedAluState<'_>,
) {
    let ExpectedAluState {
        expected_a,
        expected_f,
        prefixes,
        ix,
        iy,
        atomic_tstates,
    } = expected;
    let mut expected_registers = initial_registers(expected_a, expected_f);
    expected_registers.pc = START_PC.wrapping_add(u16::try_from(prefixes.len()).unwrap() + 1);

    assert_eq!(micro.registers, expected_registers);
    assert_eq!(atomic.registers, expected_registers);
    assert_eq!(micro.ix, ix);
    assert_eq!(atomic.ix, ix);
    assert_eq!(micro.iy, iy);
    assert_eq!(atomic.iy, iy);
    assert_eq!(micro.alternate, initial_alternate());
    assert_eq!(atomic.alternate, initial_alternate());
    assert_eq!(micro.refresh, expected_refresh(prefixes.len() + 1));
    assert_eq!(atomic.r, expected_refresh(prefixes.len() + 1));
    assert_eq!(micro.q_previous, INITIAL_Q);
    assert_eq!(micro.q, expected_f, "Q follows the flags written by the ALU");
    assert_eq!(atomic.q, expected_f);
    assert_eq!(micro.wz, INITIAL_WZ);
    assert_eq!(atomic.wz, INITIAL_WZ);
    assert_eq!(micro.i, 0xA6);
    assert_eq!(atomic.i, 0xA6);
    assert!(!micro.iff1 && !atomic.iff1);
    assert!(micro.iff2 && atomic.iff2);
    assert_eq!(micro.interrupt_mode, 2);
    assert_eq!(atomic.interrupt_mode, 2);
    assert!(!micro.p && !atomic.p);
    assert!(!micro.halted && !atomic.halted);
    assert_eq!(atomic.total_tstates, u64::from(atomic_tstates));
}

#[test]
fn all_16_dd_fd_half_register_alu_forms_match_atomic_boundary_matrix() {
    let mut tested = 0;

    for prefix in [0xDD, 0xFD] {
        for source in [4, 5] {
            for operation in 0..8 {
                let opcode = 0x80 | (operation << 3) | source;

                for accumulator in ALU_OPERANDS {
                    for operand in ALU_OPERANDS {
                        for carry in [0, 1] {
                            let (ix, iy) = index_with_half(prefix, source, operand);
                            let initial_flags = 0xA8 | carry;
                            let (atomic, bus, atomic_tstates) = execute_atomic(
                                &[prefix],
                                opcode,
                                accumulator,
                                initial_flags,
                                ix,
                                iy,
                            );
                            let (micro, micro_tstates) = execute_microsequencer(MicroAluCase {
                                prefixes: &[prefix],
                                opcode,
                                accumulator,
                                flags: initial_flags,
                                ix,
                                iy,
                                waits: &[0, 0],
                                snapshot_after_prefix: false,
                            });

                            assert_eq!(atomic_tstates, 8);
                            assert_eq!(micro_tstates, 8);
                            assert!(bus.writes.is_empty(), "register-source ALU must not access data memory");
                            assert_eq!(micro.registers.a, atomic.registers.a);
                            assert_eq!(micro.registers.f, atomic.registers.f);
                            assert_eq!(micro.ix, ix);
                            assert_eq!(micro.iy, iy);
                            assert_eq!(micro.registers.h, 0xD6);
                            assert_eq!(micro.registers.l, 0xE7);
                            assert_shared_state(
                                &micro,
                                &atomic,
                                ExpectedAluState {
                                    expected_a: atomic.registers.a,
                                    expected_f: atomic.registers.f,
                                    prefixes: &[prefix],
                                    ix,
                                    iy,
                                    atomic_tstates,
                                },
                            );
                            tested += 1;
                        }
                    }
                }
            }
        }
    }

    assert_eq!(tested, 10_816);
}

#[test]
fn fixed_vectors_pin_all_alu_flags_and_cp_operand_xy_behavior() {
    // The expected A/F pairs are independent vectors already used for the
    // indexed-memory ALU audit, now applied to IXH/IXL/IYH/IYL register sources.
    let vectors = [
        // ADD: signed overflow.
        (0x86, 0x7F, 0xA5, 0x01, 0x80, 0x94),
        // ADD: X/Y follow the result.
        (0x86, 0x20, 0x00, 0x08, 0x28, 0x28),
        // ADC: carry-in participates in overflow.
        (0x8E, 0x7F, 0x01, 0x00, 0x80, 0x94),
        // ADC: carry-in participates in the low-nibble sum.
        (0x8E, 0x20, 0x01, 0x07, 0x28, 0x28),
        // SUB: borrow, half-borrow, sign and X/Y.
        (0x96, 0x00, 0x00, 0x01, 0xFF, 0xBB),
        // SUB: X/Y follow the subtraction result.
        (0x96, 0x38, 0x00, 0x10, 0x28, 0x2A),
        // SBC: borrow-in, half-borrow and signed overflow.
        (0x9E, 0x80, 0x01, 0x00, 0x7F, 0x3E),
        // SBC: half-borrow with no full borrow.
        (0x9E, 0x38, 0x01, 0x0F, 0x28, 0x3A),
        // AND: Z, H and parity.
        (0xA6, 0xF0, 0xA5, 0x0F, 0x00, 0x54),
        // AND: X/Y follow the result.
        (0xA6, 0x28, 0x00, 0x28, 0x28, 0x3C),
        // XOR: zero and even parity.
        (0xAE, 0xAA, 0xA5, 0xAA, 0x00, 0x44),
        // XOR: X/Y follow the result.
        (0xAE, 0x20, 0x00, 0x08, 0x28, 0x2C),
        // OR: sign and parity.
        (0xB6, 0x80, 0xA5, 0x01, 0x81, 0x84),
        // OR: X/Y follow the result.
        (0xB6, 0x20, 0x00, 0x08, 0x28, 0x2C),
        // CP: X/Y are copied from the operand, not the subtraction result.
        (0xBE, 0x10, 0x00, 0x01, 0x10, 0x12),
        // CP: sign, half-borrow, carry and operand X/Y.
        (0xBE, 0x10, 0x00, 0x28, 0x10, 0xBB),
    ];

    for prefix in [0xDD, 0xFD] {
        for source in [4, 5] {
            for (base_opcode, accumulator, flags, operand, expected_a, expected_f) in vectors {
                let opcode = (base_opcode & 0xF8) | source;
                let (ix, iy) = index_with_half(prefix, source, operand);
                let (atomic, bus, atomic_tstates) =
                    execute_atomic(&[prefix], opcode, accumulator, flags, ix, iy);
                let (micro, micro_tstates) = execute_microsequencer(MicroAluCase {
                    prefixes: &[prefix],
                    opcode,
                    accumulator,
                    flags,
                    ix,
                    iy,
                    waits: &[0, 0],
                    snapshot_after_prefix: false,
                });

                assert_eq!(atomic_tstates, 8);
                assert_eq!(micro_tstates, 8);
                assert!(bus.writes.is_empty());
                assert_eq!(atomic.registers.a, expected_a, "atomic A {prefix:02X}/{opcode:02X}");
                assert_eq!(atomic.registers.f, expected_f, "atomic F {prefix:02X}/{opcode:02X}");
                assert_eq!(micro.registers.a, expected_a, "micro A {prefix:02X}/{opcode:02X}");
                assert_eq!(micro.registers.f, expected_f, "micro F {prefix:02X}/{opcode:02X}");
                assert_shared_state(
                    &micro,
                    &atomic,
                    ExpectedAluState {
                        expected_a,
                        expected_f,
                        prefixes: &[prefix],
                        ix,
                        iy,
                        atomic_tstates,
                    },
                );
            }
        }
    }
}

#[test]
fn all_half_register_alu_forms_resume_waits_and_snapshot_both_m1_safe_points() {
    for prefix in [0xDD, 0xFD] {
        for source in [4, 5] {
            for operation in 0..8 {
                let opcode = 0x80 | (operation << 3) | source;
                let accumulator = 0x7F;
                let flags = 0x01;
                let operand = 0x00;
                let (ix, iy) = index_with_half(prefix, source, operand);
                let (atomic, bus, atomic_tstates) =
                    execute_atomic(&[prefix], opcode, accumulator, flags, ix, iy);
                let (micro, micro_tstates) = execute_microsequencer(MicroAluCase {
                    prefixes: &[prefix],
                    opcode,
                    accumulator,
                    flags,
                    ix,
                    iy,
                    waits: &[1, 2],
                    snapshot_after_prefix: true,
                });

                assert_eq!(atomic_tstates, 8);
                assert_eq!(micro_tstates, 11);
                assert!(bus.writes.is_empty());
                assert_shared_state(
                    &micro,
                    &atomic,
                    ExpectedAluState {
                        expected_a: atomic.registers.a,
                        expected_f: atomic.registers.f,
                        prefixes: &[prefix],
                        ix,
                        iy,
                        atomic_tstates,
                    },
                );
            }
        }
    }
}

#[test]
fn repeated_dd_fd_prefixes_select_the_last_index_half_for_alu() {
    for (prefixes, opcode, accumulator, flags, source, operand) in [
        (&[0xDD, 0xFD][..], 0x8C, 0x7F, 0x01, 4, 0x00),
        (&[0xFD, 0xDD][..], 0xBD, 0x10, 0x00, 5, 0x01),
    ] {
        let effective_prefix = *prefixes.last().unwrap();
        let (ix, iy) = index_with_half(effective_prefix, source, operand);
        let (atomic, bus, atomic_tstates) =
            execute_atomic(prefixes, opcode, accumulator, flags, ix, iy);
        let waits = [0; 3];
        let (micro, micro_tstates) = execute_microsequencer(MicroAluCase {
            prefixes,
            opcode,
            accumulator,
            flags,
            ix,
            iy,
            waits: &waits,
            snapshot_after_prefix: true,
        });

        assert_eq!(atomic_tstates, 12);
        assert_eq!(micro_tstates, 12);
        assert!(bus.writes.is_empty());
        assert_shared_state(
            &micro,
            &atomic,
            ExpectedAluState {
                expected_a: atomic.registers.a,
                expected_f: atomic.registers.f,
                prefixes,
                ix,
                iy,
                atomic_tstates,
            },
        );
    }
}
