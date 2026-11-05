use sega_sms_core::{CpuBus, Z80};

const PROGRAM_PC: u16 = 0x4000;
const DISPLACEMENT_NEGATIVE: u8 = 0xFE;
const DISPLACEMENT_POSITIVE: u8 = 0x03;

#[derive(Clone, Copy)]
struct RegisterImage {
    a: u8,
    b: u8,
    c: u8,
    d: u8,
    e: u8,
    h: u8,
    l: u8,
    ix: u16,
    iy: u16,
}

impl RegisterImage {
    fn index(self, prefix: u8) -> u16 {
        if prefix == 0xDD { self.ix } else { self.iy }
    }

    fn set_index(&mut self, prefix: u8, value: u16) {
        if prefix == 0xDD {
            self.ix = value;
        } else {
            self.iy = value;
        }
    }

    fn read_register(self, prefix: u8, code: u8, memory_operand: bool, memory_value: u8) -> u8 {
        match code {
            0 => self.b,
            1 => self.c,
            2 => self.d,
            3 => self.e,
            4 if memory_operand => self.h,
            5 if memory_operand => self.l,
            4 => (self.index(prefix) >> 8) as u8,
            5 => self.index(prefix) as u8,
            6 => memory_value,
            7 => self.a,
            _ => unreachable!("three-bit register code"),
        }
    }

    fn write_register(&mut self, prefix: u8, code: u8, memory_operand: bool, value: u8) {
        match code {
            0 => self.b = value,
            1 => self.c = value,
            2 => self.d = value,
            3 => self.e = value,
            4 if memory_operand => self.h = value,
            5 if memory_operand => self.l = value,
            4 => {
                let index = self.index(prefix);
                self.set_index(prefix, (u16::from(value) << 8) | (index & 0x00FF));
            }
            5 => {
                let index = self.index(prefix);
                self.set_index(prefix, (index & 0xFF00) | u16::from(value));
            }
            6 => unreachable!("memory destination is handled by the bus"),
            7 => self.a = value,
            _ => unreachable!("three-bit register code"),
        }
    }
}

struct TestBus {
    memory: Box<[u8]>,
}

impl Default for TestBus {
    fn default() -> Self {
        Self {
            memory: vec![0; 0x1_0000].into_boxed_slice(),
        }
    }
}

impl CpuBus for TestBus {
    fn read_memory(&mut self, address: u16) -> u8 {
        self.memory[usize::from(address)]
    }

    fn write_memory(&mut self, address: u16, value: u8) {
        self.memory[usize::from(address)] = value;
    }

    fn read_port(&mut self, _address: u16) -> u8 {
        0xFF
    }

    fn write_port(&mut self, _address: u16, _value: u8) {}
}

fn signed_effective_address(index: u16, displacement: u8) -> u16 {
    index.wrapping_add_signed(i16::from(displacement as i8))
}

fn initial_registers() -> RegisterImage {
    RegisterImage {
        a: 0x76,
        b: 0x10,
        c: 0x21,
        d: 0x32,
        e: 0x43,
        h: 0x54,
        l: 0x65,
        ix: 0xA1B2,
        iy: 0xC3D4,
    }
}

fn assert_registers(cpu: &Z80, expected: RegisterImage, prefix: u8, opcode: u8) {
    assert_eq!(
        cpu.registers.a, expected.a,
        "prefix={prefix:02X} opcode={opcode:02X}: A"
    );
    assert_eq!(
        cpu.registers.b, expected.b,
        "prefix={prefix:02X} opcode={opcode:02X}: B"
    );
    assert_eq!(
        cpu.registers.c, expected.c,
        "prefix={prefix:02X} opcode={opcode:02X}: C"
    );
    assert_eq!(
        cpu.registers.d, expected.d,
        "prefix={prefix:02X} opcode={opcode:02X}: D"
    );
    assert_eq!(
        cpu.registers.e, expected.e,
        "prefix={prefix:02X} opcode={opcode:02X}: E"
    );
    assert_eq!(
        cpu.registers.h, expected.h,
        "prefix={prefix:02X} opcode={opcode:02X}: H"
    );
    assert_eq!(
        cpu.registers.l, expected.l,
        "prefix={prefix:02X} opcode={opcode:02X}: L"
    );
    assert_eq!(
        cpu.ix, expected.ix,
        "prefix={prefix:02X} opcode={opcode:02X}: IX"
    );
    assert_eq!(
        cpu.iy, expected.iy,
        "prefix={prefix:02X} opcode={opcode:02X}: IY"
    );
}

#[test]
fn dd_fd_ld_register_matrix_uses_index_halves_and_hl_memory_exceptions() {
    for prefix in [0xDD, 0xFD] {
        for opcode in 0x40..=0x7F {
            // DD/FD 76 is HALT, not LD (IX/IY+d),(IX/IY+d).
            if opcode == 0x76 {
                continue;
            }

            let destination = (opcode >> 3) & 0x07;
            let source = opcode & 0x07;
            let memory_operand = destination == 6 || source == 6;
            let displacement = if opcode & 0x08 == 0 {
                DISPLACEMENT_NEGATIVE
            } else {
                DISPLACEMENT_POSITIVE
            };

            let mut expected = initial_registers();
            let index = expected.index(prefix);
            let effective_address = signed_effective_address(index, displacement);
            let memory_value = 0x87;
            let source_value = expected.read_register(prefix, source, memory_operand, memory_value);

            if destination != 6 {
                expected.write_register(prefix, destination, memory_operand, source_value);
            }

            let mut bus = TestBus::default();
            bus.memory[usize::from(PROGRAM_PC)] = prefix;
            bus.memory[usize::from(PROGRAM_PC.wrapping_add(1))] = opcode;
            if memory_operand {
                bus.memory[usize::from(PROGRAM_PC.wrapping_add(2))] = displacement;
                bus.memory[usize::from(effective_address)] = memory_value;
            }

            let mut cpu = Z80::default();
            cpu.registers.pc = PROGRAM_PC;
            cpu.registers.a = 0x76;
            cpu.registers.b = 0x10;
            cpu.registers.c = 0x21;
            cpu.registers.d = 0x32;
            cpu.registers.e = 0x43;
            cpu.registers.h = 0x54;
            cpu.registers.l = 0x65;
            cpu.registers.f = 0xD7;
            cpu.ix = 0xA1B2;
            cpu.iy = 0xC3D4;
            cpu.r = 0x20;

            let expected_tstates = if memory_operand { 19 } else { 8 };
            assert_eq!(
                cpu.step(&mut bus),
                Ok(expected_tstates),
                "prefix={prefix:02X} opcode={opcode:02X}"
            );
            assert_registers(&cpu, expected, prefix, opcode);
            assert_eq!(
                cpu.registers.pc,
                PROGRAM_PC + if memory_operand { 3 } else { 2 },
                "prefix={prefix:02X} opcode={opcode:02X}: PC"
            );
            assert_eq!(
                cpu.registers.f, 0xD7,
                "prefix={prefix:02X} opcode={opcode:02X}: F"
            );
            assert_eq!(cpu.r, 0x22, "prefix={prefix:02X} opcode={opcode:02X}: R");

            if destination == 6 {
                assert_eq!(
                    bus.memory[usize::from(effective_address)],
                    source_value,
                    "prefix={prefix:02X} opcode={opcode:02X}: indexed memory write"
                );
            } else if source == 6 {
                assert_eq!(
                    bus.memory[usize::from(effective_address)],
                    memory_value,
                    "prefix={prefix:02X} opcode={opcode:02X}: indexed memory read is non-destructive"
                );
            }
        }
    }
}
