from __future__ import annotations

import pytest

from sega_emulator.cpu.z80 import Z80Cpu
from sega_emulator.machine.sg1000 import SG1000Machine


class MemoryBus:
    def __init__(self, program: bytes) -> None:
        self.memory = bytearray(0x10000)
        self.memory[: len(program)] = program

    def read(self, address: int) -> int:
        return self.memory[address & 0xFFFF]

    def write(self, address: int, value: int) -> None:
        self.memory[address & 0xFFFF] = value & 0xFF


def _rust_cpu():
    native = pytest.importorskip("sega_z80_core")
    return native.RustZ80Cpu


def test_rust_core_matches_python_for_supported_instruction_sequence() -> None:
    RustZ80Cpu = _rust_cpu()
    program = bytes(
        [
            0x31,
            0x00,
            0xF0,  # LD SP, F000
            0x21,
            0x00,
            0x20,  # LD HL, 2000
            0x3E,
            0x7F,  # LD A, 7F
            0x06,
            0x01,  # LD B, 01
            0x80,  # ADD A, B
            0x77,  # LD (HL), A
            0x7E,  # LD A, (HL)
            0xFE,
            0x80,  # CP 80
            0x20,
            0x02,  # JR NZ, +2 (not taken)
            0x76,  # HALT
        ]
    )
    python_bus = MemoryBus(program)
    rust_bus = MemoryBus(program)
    python_cpu = Z80Cpu(python_bus)
    rust_cpu = RustZ80Cpu(rust_bus, None)

    attributes = (
        "a",
        "f",
        "b",
        "c",
        "d",
        "e",
        "h",
        "l",
        "sp",
        "pc",
        "ix",
        "iy",
        "i",
        "r",
        "iff1",
        "iff2",
        "interrupt_mode",
        "halted",
        "wz",
        "total_cycles",
    )
    for _ in range(20):
        python_cycles = python_cpu.step()
        rust_cycles = rust_cpu.step()
        assert rust_cycles == python_cycles
        assert {name: getattr(rust_cpu, name) for name in attributes} == {
            name: getattr(python_cpu, name) for name in attributes
        }
        assert rust_bus.memory == python_bus.memory
        if python_cpu.halted:
            break
    assert python_cpu.halted
    assert rust_cpu.halted


@pytest.mark.parametrize(
    ("opcode", "byte_count", "expected_cycles"),
    [
        (0xA0, 1, 16),  # LDI
        (0xA8, 1, 16),  # LDD
        (0xB0, 2, 21),  # LDIR repeats while BC != 0
        (0xB8, 2, 21),  # LDDR repeats while BC != 0
    ],
)
def test_rust_block_transfer_matches_python(
    opcode: int, byte_count: int, expected_cycles: int
) -> None:
    RustZ80Cpu = _rust_cpu()
    program = bytes([0xED, opcode])
    python_bus = MemoryBus(program)
    rust_bus = MemoryBus(program)
    source_data = bytes(range(0x41, 0x41 + byte_count))
    python_bus.memory[0x1000 : 0x1000 + byte_count] = source_data
    rust_bus.memory[0x1000 : 0x1000 + byte_count] = source_data
    python_cpu = Z80Cpu(python_bus)
    rust_cpu = RustZ80Cpu(rust_bus, None)

    initial_bc = byte_count
    python_cpu.a = rust_cpu.a = 0x10
    python_cpu.f = rust_cpu.f = 0xC1
    python_cpu.hl = rust_cpu.hl = 0x1000
    python_cpu.de = rust_cpu.de = 0x2000
    python_cpu.bc = rust_cpu.bc = initial_bc

    python_cycles = python_cpu.step()
    rust_cycles = rust_cpu.step()

    attributes = (
        "a",
        "f",
        "b",
        "c",
        "d",
        "e",
        "h",
        "l",
        "pc",
        "r",
        "wz",
        "total_cycles",
    )
    assert python_cycles == rust_cycles == expected_cycles
    assert {name: getattr(rust_cpu, name) for name in attributes} == {
        name: getattr(python_cpu, name) for name in attributes
    }
    assert rust_cpu.bc == python_cpu.bc
    assert rust_cpu.de == python_cpu.de
    assert rust_cpu.hl == python_cpu.hl
    assert rust_bus.memory == python_bus.memory


@pytest.mark.parametrize("opcode", [0x00, 0x77, 0x7F, 0x80, 0xA4, 0xC0])
def test_rust_unassigned_ed_opcode_matches_classic_z80_nop_behavior(opcode: int) -> None:
    RustZ80Cpu = _rust_cpu()
    program = bytes([0xED, opcode])
    python_cpu = Z80Cpu(MemoryBus(program))
    rust_cpu = RustZ80Cpu(MemoryBus(program), None)

    assert python_cpu.step() == rust_cpu.step() == 8
    assert python_cpu.pc == rust_cpu.pc == 2
    assert python_cpu.r == rust_cpu.r == 2
    assert python_cpu.total_cycles == rust_cpu.total_cycles == 8


def test_rust_core_fails_explicitly_on_assigned_but_unsupported_ed_opcode() -> None:
    RustZ80Cpu = _rust_cpu()
    cpu = RustZ80Cpu(MemoryBus(bytes([0xED, 0x42])), None)
    with pytest.raises(NotImplementedError, match="does not support ED opcode 0x42"):
        cpu.step()


@pytest.mark.parametrize(
    ("opcode", "expected_cycles"),
    [
        (0x07, 4),  # RLCA
        (0x0F, 4),  # RRCA
        (0x17, 4),  # RLA
        (0x1F, 4),  # RRA
        (0x27, 4),  # DAA
        (0x2F, 4),  # CPL
        (0x37, 4),  # SCF
        (0x3F, 4),  # CCF
        (0x08, 4),  # EX AF,AF'
        (0xD9, 4),  # EXX
        (0xE3, 19), # EX (SP),HL
        (0xEB, 4),  # EX DE,HL
    ],
)
def test_rust_base_misc_instructions_match_python(opcode: int, expected_cycles: int) -> None:
    RustZ80Cpu = _rust_cpu()
    program = bytes([opcode])
    python_bus = MemoryBus(program)
    rust_bus = MemoryBus(program)
    python_bus.memory[0x3000:0x3002] = bytes([0xA5, 0x5A])
    rust_bus.memory[0x3000:0x3002] = bytes([0xA5, 0x5A])
    python_cpu = Z80Cpu(python_bus)
    rust_cpu = RustZ80Cpu(rust_bus, None)

    for cpu in (python_cpu, rust_cpu):
        cpu.a = 0x91
        cpu.f = 0x95
        cpu.b, cpu.c = 0x12, 0x34
        cpu.d, cpu.e = 0x56, 0x78
        cpu.h, cpu.l = 0x9A, 0xBC
        cpu.sp = 0x3000
        cpu.af_alt = 0x2468
        cpu.bc_alt = 0x1357
        cpu.de_alt = 0xFEDC
        cpu.hl_alt = 0xBEEF

    python_cycles = python_cpu.step()
    rust_cycles = rust_cpu.step()
    attributes = (
        "a", "f", "b", "c", "d", "e", "h", "l", "sp", "pc",
        "af_alt", "bc_alt", "de_alt", "hl_alt", "wz", "r", "total_cycles",
    )
    assert python_cycles == rust_cycles == expected_cycles
    assert {name: getattr(rust_cpu, name) for name in attributes} == {
        name: getattr(python_cpu, name) for name in attributes
    }
    assert rust_bus.memory == python_bus.memory


def test_rust_core_fails_explicitly_on_unsupported_opcode() -> None:
    RustZ80Cpu = _rust_cpu()
    bus = MemoryBus(bytes([0xDD]))
    cpu = RustZ80Cpu(bus, None)
    with pytest.raises(NotImplementedError, match="does not support opcode"):
        cpu.step()


def test_machine_can_select_rust_core_for_a_halted_test_cartridge() -> None:
    _rust_cpu()
    machine = SG1000Machine(bytes([0x76]), cpu_core="rust")
    assert machine.cpu_core == "rust"
    machine.run_frames(1)
    assert machine.vdp.frame_number == 1
    assert machine.total_tstates > 0
