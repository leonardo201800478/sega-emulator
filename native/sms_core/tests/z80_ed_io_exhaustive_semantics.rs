//! Exhaustive semantic regressions for ED IN r,(C) and OUT (C),r families.
//!
//! This test uses a deterministic 64 KiB I/O bus and checks the documented
//! data/flag contract for all input byte values, all register encodings, port
//! selection, WZ update, and aggregate instruction timing. It does not settle
//! undocumented silicon differences such as INI/OUTI flags or clone-specific
//! behavior of OUT (C),0.
use sega_sms_core::{CpuBus, Z80};

const FLAG_C: u8 = 0x01;
const FLAG_PV: u8 = 0x04;
const FLAG_Z: u8 = 0x40;
const FLAG_S: u8 = 0x80;
const FLAG_XY: u8 = 0x28;

struct FlatIoBus {
    memory: Box<[u8]>,
    ports: Box<[u8]>,
}

impl Default for FlatIoBus {
    fn default() -> Self {
        Self {
            memory: vec![0; 0x1_0000].into_boxed_slice(),
            ports: vec![0; 0x1_0000].into_boxed_slice(),
        }
    }
}

impl CpuBus for FlatIoBus {
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

fn initialized_cpu() -> Z80 {
    let mut cpu = Z80::default();
    cpu.registers.a = 0xDE;
    cpu.registers.f = 0;
    cpu.registers.b = 0x12;
    cpu.registers.c = 0x34;
    cpu.registers.d = 0x56;
    cpu.registers.e = 0x78;
    cpu.registers.h = 0x9A;
    cpu.registers.l = 0xBC;
    cpu.registers.sp = 0x9000;
    cpu.wz = 0x7777;
    cpu
}

fn set_register(cpu: &mut Z80, code: u8, value: u8) {
    match code & 7 {
        0 => cpu.registers.b = value,
        1 => cpu.registers.c = value,
        2 => cpu.registers.d = value,
        3 => cpu.registers.e = value,
        4 => cpu.registers.h = value,
        5 => cpu.registers.l = value,
        6 => {} // ED IN (C) discards the input byte.
        _ => cpu.registers.a = value,
    }
}

fn get_register(cpu: &Z80, code: u8) -> Option<u8> {
    match code & 7 {
        0 => Some(cpu.registers.b),
        1 => Some(cpu.registers.c),
        2 => Some(cpu.registers.d),
        3 => Some(cpu.registers.e),
        4 => Some(cpu.registers.h),
        5 => Some(cpu.registers.l),
        6 => None,
        _ => Some(cpu.registers.a),
    }
}

fn even_parity(value: u8) -> bool {
    value.count_ones().is_multiple_of(2)
}

#[test]
fn ed_in_register_forms_cover_every_input_value_and_carry_state() {
    let opcodes = [0x40_u8, 0x48, 0x50, 0x58, 0x60, 0x68, 0x70, 0x78];

    for opcode in opcodes {
        let code = (opcode >> 3) & 7;
        for value in u8::MIN..=u8::MAX {
            for carry_in in [false, true] {
                let mut bus = FlatIoBus::default();
                bus.memory[0] = 0xED;
                bus.memory[1] = opcode;
                bus.ports[0x1234] = value;

                let mut cpu = initialized_cpu();
                cpu.registers.f = if carry_in { FLAG_C } else { 0 };
                let before = get_register(&cpu, code);
                let cycles = cpu.step(&mut bus).unwrap_or_else(|error| {
                    panic!("ED {opcode:02X}, input={value:02X}: {error:?}")
                });

                let expected_flags = {
                    let mut flags = value & (FLAG_S | FLAG_XY);
                    if value == 0 {
                        flags |= FLAG_Z;
                    }
                    if even_parity(value) {
                        flags |= FLAG_PV;
                    }
                    if carry_in {
                        flags |= FLAG_C;
                    }
                    flags
                };

                if code == 6 {
                    assert_eq!(get_register(&cpu, code), None);
                } else {
                    assert_eq!(
                        get_register(&cpu, code),
                        Some(value),
                        "ED {opcode:02X} destination, input={value:02X}"
                    );
                }
                assert_eq!(cpu.registers.f, expected_flags, "ED {opcode:02X} flags");
                assert_eq!(cpu.wz, 0x1235, "ED {opcode:02X} WZ");
                assert_eq!(cpu.registers.pc, 2, "ED {opcode:02X} PC");
                assert_eq!(cycles, 12, "ED {opcode:02X} nominal timing");
                if code != 6 {
                    assert_ne!(get_register(&cpu, code), before, "test sentinel");
                }
            }
        }
    }
}

#[test]
fn ed_out_register_forms_cover_all_source_values_and_preserve_flags() {
    let opcodes = [0x41_u8, 0x49, 0x51, 0x59, 0x61, 0x69, 0x71, 0x79];
    let flag_patterns = [0x00_u8, 0xFF, 0xA5, 0x5A];

    for opcode in opcodes {
        let code = (opcode >> 3) & 7;
        for value in u8::MIN..=u8::MAX {
            for initial_flags in flag_patterns {
                let mut bus = FlatIoBus::default();
                bus.memory[0] = 0xED;
                bus.memory[1] = opcode;

                let mut cpu = initialized_cpu();
                cpu.registers.f = initial_flags;
                if code != 6 {
                    set_register(&mut cpu, code, value);
                }
                let port = cpu.registers.bc();
                let expected_value = if code == 6 { 0 } else { value };
                let cycles = cpu.step(&mut bus).unwrap_or_else(|error| {
                    panic!("ED {opcode:02X}, source={value:02X}: {error:?}")
                });

                assert_eq!(
                    bus.ports[usize::from(port)],
                    expected_value,
                    "ED {opcode:02X} output at port {port:04X}"
                );
                assert_eq!(cpu.registers.f, initial_flags, "ED {opcode:02X} flags");
                assert_eq!(cpu.wz, port.wrapping_add(1), "ED {opcode:02X} WZ");
                assert_eq!(cpu.registers.pc, 2, "ED {opcode:02X} PC");
                assert_eq!(cycles, 12, "ED {opcode:02X} nominal timing");
            }
        }
    }
}

#[test]
fn ed_in_out_port_increment_wraps_from_ffff_to_zero() {
    let mut bus = FlatIoBus::default();
    bus.memory[0..4].copy_from_slice(&[0xED, 0x78, 0xED, 0x79]);
    bus.ports[0xFFFF] = 0x80;

    let mut cpu = Z80::default();
    cpu.registers.set_bc(0xFFFF);
    cpu.registers.a = 0x5A;

    assert_eq!(cpu.step(&mut bus), Ok(12));
    assert_eq!(cpu.registers.a, 0x80);
    assert_eq!(cpu.wz, 0x0000);

    cpu.registers.set_bc(0xFFFF);
    cpu.registers.a = 0xA6;
    assert_eq!(cpu.step(&mut bus), Ok(12));
    assert_eq!(bus.ports[0xFFFF], 0xA6);
    assert_eq!(cpu.wz, 0x0000);
}
