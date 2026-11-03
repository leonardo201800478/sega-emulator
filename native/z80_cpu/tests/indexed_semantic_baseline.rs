//! Directed DD/FD semantic baselines for the atomic Z80 executor.
//!
//! These are project-local synthetic invariants, not independent hardware
//! oracles and not evidence of microsequencer or cycle-accurate correctness.
use sega_z80_cpu::{bus::CpuBus, Z80};

#[derive(Clone)]
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

fn indexed_cb(prefix: u8, opcode: u8, value: u8) -> (Z80, FlatBus, u32) {
    let mut bus = FlatBus::default();
    bus.memory[..4].copy_from_slice(&[prefix, 0xCB, 0x01, opcode]);
    let mut cpu = Z80 {
        ix: if prefix == 0xDD { 0xC000 } else { 0x1357 },
        iy: if prefix == 0xFD { 0xC000 } else { 0x2468 },
        ..Z80::default()
    };
    bus.memory[0xC001] = value;
    let cycles = cpu.step(&mut bus).unwrap();
    (cpu, bus, cycles)
}

#[test]
fn indexed_bit_is_read_only_for_both_index_prefixes() {
    for prefix in [0xDD, 0xFD] {
        let (cpu, bus, cycles) = indexed_cb(prefix, 0x46, 0x01); // BIT 0,(IX/IY+1)
        assert_eq!(cycles, 20);
        assert_eq!(bus.memory[0xC001], 0x01);
        assert!(bus.writes.is_empty(), "BIT must not issue write-back");
        assert_eq!(cpu.registers.pc, 4);
        assert_eq!(cpu.r & 0x7F, 2, "DD/FD and CB are the two M1 fetches");
        assert_eq!(cpu.registers.f, 0x11, "C is preserved and H is set");
        assert_eq!(cpu.wz, 0xC001);
    }
}

#[test]
fn indexed_rotate_shift_performs_write_back_and_register_copy() {
    for prefix in [0xDD, 0xFD] {
        let (cpu, bus, cycles) = indexed_cb(prefix, 0x00, 0x81); // RLC (IX/IY+1),B
        assert_eq!(cycles, 23);
        assert_eq!(bus.memory[0xC001], 0x03);
        assert_eq!(bus.writes, vec![(0xC001, 0x03)]);
        assert_eq!(cpu.registers.b, 0x03);
        assert_eq!(cpu.registers.f & 0x05, 0x05); // even parity and carry
        assert_eq!(cpu.r & 0x7F, 2);
    }
}

#[test]
fn indexed_memory_h_and_l_operands_use_plain_hl_registers() {
    for prefix in [0xDD, 0xFD] {
        let mut load_bus = FlatBus::default();
        load_bus.memory[..3].copy_from_slice(&[prefix, 0x66, 0x01]); // LD H,(IX/IY+1)
        load_bus.memory[0xC001] = 0x5A;
        let mut load_cpu = Z80 {
            ix: if prefix == 0xDD { 0xC000 } else { 0x1357 },
            iy: if prefix == 0xFD { 0xC000 } else { 0x2468 },
            registers: sega_z80_cpu::Z80Registers {
                h: 0x12,
                l: 0x34,
                ..sega_z80_cpu::Z80Registers::default()
            },
            ..Z80::default()
        };
        assert_eq!(load_cpu.step(&mut load_bus), Ok(19));
        assert_eq!(load_cpu.registers.h, 0x5A);
        assert_eq!(load_cpu.registers.l, 0x34);

        let mut store_bus = FlatBus::default();
        store_bus.memory[..3].copy_from_slice(&[prefix, 0x74, 0x02]); // LD (IX/IY+2),H
        let mut store_cpu = Z80 {
            ix: if prefix == 0xDD { 0xC000 } else { 0x1357 },
            iy: if prefix == 0xFD { 0xC000 } else { 0x2468 },
            registers: sega_z80_cpu::Z80Registers {
                h: 0xA5,
                ..sega_z80_cpu::Z80Registers::default()
            },
            ..Z80::default()
        };
        assert_eq!(store_cpu.step(&mut store_bus), Ok(19));
        assert_eq!(store_bus.memory[0xC002], 0xA5);
        assert_eq!(store_bus.writes, vec![(0xC002, 0xA5)]);
    }
}

#[test]
fn index_half_registers_are_selected_by_the_effective_prefix() {
    for (prefix, index) in [(0xDD, 0xABCD), (0xFD, 0x9876)] {
        let mut bus = FlatBus::default();
        bus.memory[..2].copy_from_slice(&[prefix, 0x44]); // LD B,IXH/IYH
        let mut cpu = Z80 {
            ix: if prefix == 0xDD { index } else { 0x1357 },
            iy: if prefix == 0xFD { index } else { 0x2468 },
            ..Z80::default()
        };
        assert_eq!(cpu.step(&mut bus), Ok(8));
        assert_eq!(cpu.registers.b, (index >> 8) as u8);
        assert_eq!(cpu.r & 0x7F, 2);
    }
}

#[test]
fn signed_index_displacement_wraps_the_effective_address() {
    let mut bus = FlatBus::default();
    bus.memory[..3].copy_from_slice(&[0xDD, 0x7E, 0xFF]); // LD A,(IX-1)
    bus.memory[0xFFFF] = 0x6C;
    let mut cpu = Z80::default();
    cpu.ix = 0x0000;

    assert_eq!(cpu.step(&mut bus), Ok(19));
    assert_eq!(cpu.registers.a, 0x6C);
    assert_eq!(cpu.wz, 0xFFFF);
    assert_eq!(cpu.registers.pc, 3);
}
