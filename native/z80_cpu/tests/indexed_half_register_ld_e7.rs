//! E7 F-036: DD/FD register-only LD r,r' using IXH/IXL/IYH/IYL.
//!
//! The matrix is checked against both the atomic executor and an explicit
//! register-image oracle. Indexed-memory H/L exceptions remain covered by
//! indexed_memory_e4a.rs and indexed_semantic_baseline.rs.

use sega_z80_cpu::{
    Z80, Z80AlternateRegisters, Z80BusOperation, Z80BusRequest, Z80MicroSequencer,
    Z80MicroSequencerState, Z80MicroStep, Z80Registers, bus::CpuBus,
};

const START_PC: u16 = 0x4000;
const INITIAL_IX: u16 = 0xABCD;
const INITIAL_IY: u16 = 0x9876;
const INITIAL_Q: u8 = 0xA5;
const INITIAL_WZ: u16 = 0xBEEF;
const INITIAL_REFRESH: u8 = 0xFE;

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

fn initial_registers() -> Z80Registers {
    Z80Registers {
        a: 0x76,
        f: 0xD7,
        b: 0x10,
        c: 0x21,
        d: 0x32,
        e: 0x43,
        h: 0x54,
        l: 0x65,
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

fn seed_atomic(prefixes: &[u8], opcode: u8) -> (Z80, FlatBus) {
    let mut cpu = Z80::default();
    cpu.registers = initial_registers();
    cpu.alternate = initial_alternate();
    cpu.ix = INITIAL_IX;
    cpu.iy = INITIAL_IY;
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

fn seed_microsequencer() -> Z80MicroSequencer {
    let mut cpu = Z80MicroSequencer::default();
    cpu.registers = initial_registers();
    cpu.alternate = initial_alternate();
    cpu.ix = INITIAL_IX;
    cpu.iy = INITIAL_IY;
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

fn execute_atomic(prefixes: &[u8], opcode: u8) -> (Z80, FlatBus, u32) {
    let (mut cpu, mut bus) = seed_atomic(prefixes, opcode);
    let tstates = cpu.step(&mut bus).unwrap();
    (cpu, bus, tstates)
}

fn execute_microsequencer(prefixes: &[u8], opcode: u8, waits: &[u32]) -> (Z80MicroSequencer, u32) {
    assert!(!prefixes.is_empty());
    assert_eq!(waits.len(), prefixes.len() + 1);

    let mut cpu = seed_microsequencer();
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
        cpu = round_trip_at(cpu, "between prefix and terminal M1");
        assert_eq!(cpu.q_previous, 0, "DD/FD prefix clears previous-Q before terminal opcode");
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
        other => panic!("register-only instruction must retire after terminal M1: {other:?}"),
    };
    (cpu, tstates)
}

fn selected_index(prefix: u8, ix: u16, iy: u16) -> u16 {
    match prefix {
        0xDD => ix,
        0xFD => iy,
        other => panic!("unexpected index prefix {other:#04X}"),
    }
}

fn read_register_image(code: u8, prefix: u8, registers: &Z80Registers, ix: u16, iy: u16) -> u8 {
    match code {
        0 => registers.b,
        1 => registers.c,
        2 => registers.d,
        3 => registers.e,
        4 => selected_index(prefix, ix, iy).to_be_bytes()[0],
        5 => selected_index(prefix, ix, iy).to_be_bytes()[1],
        7 => registers.a,
        6 => panic!("memory operand is excluded from F-036 register-only matrix"),
        other => panic!("invalid 8-bit register code {other}"),
    }
}

fn write_register_image(
    code: u8,
    value: u8,
    prefix: u8,
    registers: &mut Z80Registers,
    ix: &mut u16,
    iy: &mut u16,
) {
    match code {
        0 => registers.b = value,
        1 => registers.c = value,
        2 => registers.d = value,
        3 => registers.e = value,
        4 => {
            let index = selected_index(prefix, *ix, *iy);
            let updated = (index & 0x00FF) | (u16::from(value) << 8);
            if prefix == 0xDD {
                *ix = updated;
            } else {
                *iy = updated;
            }
        }
        5 => {
            let index = selected_index(prefix, *ix, *iy);
            let updated = (index & 0xFF00) | u16::from(value);
            if prefix == 0xDD {
                *ix = updated;
            } else {
                *iy = updated;
            }
        }
        7 => registers.a = value,
        6 => panic!("memory operand is excluded from F-036 register-only matrix"),
        other => panic!("invalid 8-bit register code {other}"),
    }
}

fn expected_ld_state(prefixes: &[u8], opcode: u8) -> (Z80Registers, u16, u16) {
    let prefix = *prefixes.last().unwrap();
    let mut registers = initial_registers();
    let mut ix = INITIAL_IX;
    let mut iy = INITIAL_IY;
    let destination = (opcode >> 3) & 0x07;
    let source = opcode & 0x07;
    let value = read_register_image(source, prefix, &registers, ix, iy);
    write_register_image(destination, value, prefix, &mut registers, &mut ix, &mut iy);
    registers.pc = START_PC.wrapping_add(u16::try_from(prefixes.len()).unwrap() + 1);
    (registers, ix, iy)
}

fn expected_refresh(m1_count: usize) -> u8 {
    let m1_count = u8::try_from(m1_count).unwrap();
    (INITIAL_REFRESH & 0x80) | (INITIAL_REFRESH.wrapping_add(m1_count) & 0x7F)
}

fn assert_architecture(
    micro: &Z80MicroSequencer,
    atomic: &Z80,
    expected_registers: &Z80Registers,
    expected_ix: u16,
    expected_iy: u16,
    prefixes: &[u8],
    expected_halted: bool,
) {
    assert_eq!(micro.registers, *expected_registers);
    assert_eq!(atomic.registers, *expected_registers);
    assert_eq!(micro.ix, expected_ix);
    assert_eq!(atomic.ix, expected_ix);
    assert_eq!(micro.iy, expected_iy);
    assert_eq!(atomic.iy, expected_iy);
    assert_eq!(micro.alternate, initial_alternate());
    assert_eq!(atomic.alternate, initial_alternate());
    assert_eq!(micro.refresh, expected_refresh(prefixes.len() + 1));
    assert_eq!(atomic.r, expected_refresh(prefixes.len() + 1));
    assert_eq!(micro.q_previous, 0);
    assert_eq!(atomic.export_architectural_state().q_previous, 0);
    assert_eq!(micro.q, 0, "LD/HALT does not write flags");
    assert_eq!(atomic.q, 0, "atomic executor Q latch");
    assert_eq!(micro.wz, INITIAL_WZ);
    assert_eq!(atomic.wz, INITIAL_WZ);
    assert_eq!(micro.i, 0xA6);
    assert_eq!(atomic.i, 0xA6);
    assert!(!micro.iff1 && !atomic.iff1);
    assert!(micro.iff2 && atomic.iff2);
    assert_eq!(micro.interrupt_mode, 2);
    assert_eq!(atomic.interrupt_mode, 2);
    assert!(
        !micro.p && !atomic.p,
        "normal opcode fetch consumes the LD A,I/R P latch"
    );
    assert_eq!(micro.halted, expected_halted);
    assert_eq!(atomic.halted, expected_halted);
}

#[test]
fn all_register_only_ld_forms_use_index_halves_in_resumable_execution() {
    let mut tested = 0;

    for prefix in [0xDD, 0xFD] {
        for opcode in 0x40..=0x7F {
            if opcode == 0x76 {
                continue;
            }
            let destination = (opcode >> 3) & 0x07;
            let source = opcode & 0x07;
            if destination == 6
                || source == 6
                || !(matches!(destination, 4 | 5) || matches!(source, 4 | 5))
            {
                continue;
            }

            let prefixes = [prefix];
            let (atomic, bus, atomic_tstates) = execute_atomic(&prefixes, opcode);
            let (micro, micro_tstates) = execute_microsequencer(&prefixes, opcode, &[0, 0]);
            let (expected_registers, expected_ix, expected_iy) =
                expected_ld_state(&prefixes, opcode);

            assert_eq!(atomic_tstates, 8, "atomic {prefix:#04X} {opcode:#04X}");
            assert_eq!(micro_tstates, 8, "micro {prefix:#04X} {opcode:#04X}");
            assert!(
                bus.writes.is_empty(),
                "register-only LD must not write memory"
            );
            assert_architecture(
                &micro,
                &atomic,
                &expected_registers,
                expected_ix,
                expected_iy,
                &prefixes,
                false,
            );
            assert_eq!(atomic.total_tstates, 8);
            tested += 1;
        }
    }

    assert_eq!(tested, 48, "24 forms for each of DD and FD");
}

#[test]
fn register_only_ld_preserves_waits_and_prefix_safe_point_snapshots() {
    for (prefix, opcode) in [(0xDD, 0x65), (0xFD, 0x6C)] {
        let prefixes = [prefix];
        let (atomic, bus, atomic_tstates) = execute_atomic(&prefixes, opcode);
        let (micro, micro_tstates) = execute_microsequencer(&prefixes, opcode, &[1, 2]);
        let (expected_registers, expected_ix, expected_iy) = expected_ld_state(&prefixes, opcode);

        assert_eq!(atomic_tstates, 8);
        assert_eq!(micro_tstates, 11, "two M1 cycles plus three WAIT T-states");
        assert!(bus.writes.is_empty());
        assert_architecture(
            &micro,
            &atomic,
            &expected_registers,
            expected_ix,
            expected_iy,
            &prefixes,
            false,
        );
    }
}

#[test]
fn repeated_dd_fd_prefixes_select_the_last_index_for_register_only_ld() {
    for (prefixes, opcode) in [(&[0xDD, 0xFD][..], 0x65), (&[0xFD, 0xDD][..], 0x6C)] {
        let (atomic, bus, atomic_tstates) = execute_atomic(prefixes, opcode);
        let waits = vec![0; prefixes.len() + 1];
        let (micro, micro_tstates) = execute_microsequencer(prefixes, opcode, &waits);
        let (expected_registers, expected_ix, expected_iy) = expected_ld_state(prefixes, opcode);

        assert_eq!(atomic_tstates, 12);
        assert_eq!(micro_tstates, 12);
        assert!(bus.writes.is_empty());
        assert_architecture(
            &micro,
            &atomic,
            &expected_registers,
            expected_ix,
            expected_iy,
            prefixes,
            false,
        );
    }
}

#[test]
fn dd_fd_halt_remains_halt_not_an_indexed_register_transfer() {
    let prefixes = [0xDD];
    let (atomic, bus, atomic_tstates) = execute_atomic(&prefixes, 0x76);
    let (micro, micro_tstates) = execute_microsequencer(&prefixes, 0x76, &[0, 0]);
    let expected_registers = atomic.registers;
    let expected_ix = atomic.ix;
    let expected_iy = atomic.iy;

    assert_eq!(atomic_tstates, 8);
    assert_eq!(micro_tstates, 8);
    assert!(bus.writes.is_empty());
    assert_architecture(
        &micro,
        &atomic,
        &expected_registers,
        expected_ix,
        expected_iy,
        &prefixes,
        true,
    );
}
