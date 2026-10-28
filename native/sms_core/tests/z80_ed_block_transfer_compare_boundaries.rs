//! Directed boundary regressions for the remaining ED block families.
//!
//! Scope: LDI/LDD/LDIR/LDDR and CPI/CPD/CPIR/CPDR.
//! These tests use hand-calculated architectural invariants; they are not
//! external differential vectors and do not validate ordered bus cycles.
//! External-oracle coverage and physical-hardware confirmation remain separate.
use sega_sms_core::{CpuBus, Z80};

const FLAG_C: u8 = 0x01;
const FLAG_N: u8 = 0x02;
const FLAG_PV: u8 = 0x04;
const FLAG_Z: u8 = 0x40;
const FLAG_S: u8 = 0x80;

struct FlatBus {
    memory: Box<[u8]>,
    ports: Box<[u8]>,
}

impl Default for FlatBus {
    fn default() -> Self {
        Self {
            memory: vec![0; 0x1_0000].into_boxed_slice(),
            ports: vec![0; 0x1_0000].into_boxed_slice(),
        }
    }
}

impl CpuBus for FlatBus {
    fn read_memory(&mut self, address: u16) -> u8 {
        self.memory[usize::from(address)]
    }

    fn write_memory(&mut self, address: u16, value: u8) {
        self.memory[usize::from(address)] = value;
    }

    fn read_port(&mut self, address: u16) -> u8 {
        self.ports[usize::from(address)]
    }

    fn write_port(&mut self, address: u16, value: u8) {
        self.ports[usize::from(address)] = value;
    }
}

fn program_at(bus: &mut FlatBus, address: u16, opcode: u8) {
    bus.memory[usize::from(address)] = 0xED;
    bus.memory[usize::from(address.wrapping_add(1))] = opcode;
}

fn program(bus: &mut FlatBus, opcode: u8) {
    program_at(bus, 0, opcode);
}

#[test]
fn ldi_wraps_hl_and_de_and_bc_zero_decrements_to_ffff() {
    let mut bus = FlatBus::default();
    program(&mut bus, 0xA0);
    bus.memory[0xFFFF] = 0xF9;

    let mut cpu = Z80::default();
    cpu.registers.a = 0x10;
    cpu.registers.f = 0xC1; // S, Z and C are preserved.
    cpu.registers.set_hl(0xFFFF);
    cpu.registers.set_de(0x0000);
    cpu.registers.set_bc(0x0000);

    assert_eq!(cpu.step(&mut bus), Ok(16));
    assert_eq!(bus.memory[0x0000], 0xF9);
    assert_eq!(cpu.registers.hl(), 0x0000);
    assert_eq!(cpu.registers.de(), 0x0001);
    assert_eq!(cpu.registers.bc(), 0xFFFF);
    // A + transferred byte = 0x109: PV from BC!=0, undocumented bit 3 set.
    assert_eq!(cpu.registers.f, 0xCD);
}

#[test]
fn ldir_final_iteration_does_not_repeat_and_clears_pv() {
    let mut bus = FlatBus::default();
    program(&mut bus, 0xB0);
    bus.memory[0xFFFF] = 0x12;

    let mut cpu = Z80::default();
    cpu.registers.a = 0x00;
    cpu.registers.f = 0x41; // Z and C are preserved.
    cpu.registers.set_hl(0xFFFF);
    cpu.registers.set_de(0x0000);
    cpu.registers.set_bc(0x0001);

    assert_eq!(cpu.step(&mut bus), Ok(16));
    assert_eq!(cpu.registers.pc, 0x0002);
    assert_eq!(cpu.registers.hl(), 0x0000);
    assert_eq!(cpu.registers.de(), 0x0001);
    assert_eq!(cpu.registers.bc(), 0x0000);
    assert_eq!(bus.memory[0x0000], 0x12);
    assert_eq!(cpu.registers.f, 0x61); // preserved Z/C plus undocumented Y.
}

#[test]
fn lddr_repeats_when_bc_wraps_from_zero() {
    let mut bus = FlatBus::default();
    program_at(&mut bus, 0x0100, 0xB8);
    bus.memory[0x0000] = 0x01;

    let mut cpu = Z80::default();
    cpu.registers.pc = 0x0100;
    cpu.registers.a = 0;
    cpu.registers.set_hl(0x0000);
    cpu.registers.set_de(0x8000);
    cpu.registers.set_bc(0x0000);

    assert_eq!(cpu.step(&mut bus), Ok(21));
    assert_eq!(cpu.registers.pc, 0x0100);
    assert_eq!(cpu.wz, 0x0101);
    assert_eq!(cpu.registers.hl(), 0xFFFF);
    assert_eq!(cpu.registers.de(), 0x7FFF);
    assert_eq!(cpu.registers.bc(), 0xFFFF);
    assert_eq!(bus.memory[0x8000], 0x01);
    assert_eq!(cpu.registers.f & FLAG_PV, FLAG_PV);
}

#[test]
fn cpi_match_stops_with_bc_zero_and_preserves_carry() {
    let mut bus = FlatBus::default();
    program(&mut bus, 0xA1);
    bus.memory[0xFFFF] = 0x20;

    let mut cpu = Z80::default();
    cpu.registers.a = 0x20;
    cpu.registers.f = FLAG_C;
    cpu.registers.set_hl(0xFFFF);
    cpu.registers.set_bc(0x0001);

    assert_eq!(cpu.step(&mut bus), Ok(16));
    assert_eq!(cpu.registers.hl(), 0x0000);
    assert_eq!(cpu.registers.bc(), 0x0000);
    assert_eq!(cpu.registers.pc, 0x0002);
    assert_eq!(cpu.wz, 0x0001);
    assert_eq!(cpu.registers.f, FLAG_C | FLAG_Z | FLAG_N);
}

#[test]
fn cpir_with_zero_initial_bc_wraps_counter_and_repeats_on_nonmatch() {
    let mut bus = FlatBus::default();
    program(&mut bus, 0xB1);
    bus.memory[0xFFFF] = 0x20;

    let mut cpu = Z80::default();
    cpu.registers.a = 0x10;
    cpu.registers.set_hl(0xFFFF);
    cpu.registers.set_bc(0x0000);

    assert_eq!(cpu.step(&mut bus), Ok(21));
    assert_eq!(cpu.registers.pc, 0x0000);
    assert_eq!(cpu.wz, 0x0001);
    assert_eq!(cpu.registers.hl(), 0x0000);
    assert_eq!(cpu.registers.bc(), 0xFFFF);
    assert_eq!(cpu.registers.f, FLAG_S | FLAG_N | FLAG_PV);
}

#[test]
fn cpd_decrements_hl_across_zero_and_finishes_when_bc_reaches_zero() {
    let mut bus = FlatBus::default();
    program_at(&mut bus, 0x0100, 0xA9);
    bus.memory[0x0000] = 0x20;

    let mut cpu = Z80::default();
    cpu.registers.pc = 0x0100;
    cpu.registers.a = 0x10;
    cpu.registers.f = FLAG_C;
    cpu.registers.set_hl(0x0000);
    cpu.registers.set_bc(0x0001);
    cpu.wz = 0x2345;

    assert_eq!(cpu.step(&mut bus), Ok(16));
    assert_eq!(cpu.registers.pc, 0x0102);
    assert_eq!(cpu.registers.hl(), 0xFFFF);
    assert_eq!(cpu.registers.bc(), 0x0000);
    assert_eq!(cpu.wz, 0x2344);
    assert_eq!(cpu.registers.f, FLAG_S | FLAG_N | FLAG_C);
}

#[test]
fn cpdr_match_stops_repeat_and_decrements_wz() {
    let mut bus = FlatBus::default();
    program_at(&mut bus, 0x0100, 0xB9);
    bus.memory[0x0000] = 0x7F;

    let mut cpu = Z80::default();
    cpu.registers.pc = 0x0100;
    cpu.registers.a = 0x7F;
    cpu.registers.f = FLAG_C;
    cpu.registers.set_hl(0x0000);
    cpu.registers.set_bc(0x0001);
    cpu.wz = 0x2000;

    assert_eq!(cpu.step(&mut bus), Ok(16));
    assert_eq!(cpu.registers.pc, 0x0102);
    assert_eq!(cpu.registers.hl(), 0xFFFF);
    assert_eq!(cpu.registers.bc(), 0x0000);
    assert_eq!(cpu.wz, 0x1FFF);
    assert_eq!(cpu.registers.f, FLAG_C | FLAG_Z | FLAG_N);
}
