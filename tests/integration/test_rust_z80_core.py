from __future__ import annotations

import pytest

from sega_emulator.cpu.z80 import Z80Cpu


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
            0x31, 0x00, 0xF0,  # LD SP, F000
            0x21, 0x00, 0x20,  # LD HL, 2000
            0x3E, 0x7F,        # LD A, 7F
            0x06, 0x01,        # LD B, 01
            0x80,              # ADD A, B
            0x77,              # LD (HL), A
            0x7E,              # LD A, (HL)
            0xFE, 0x80,        # CP 80
            0x20, 0x02,        # JR NZ, +2 (not taken)
            0x76,              # HALT
        ]
    )
    python_bus = MemoryBus(program)
    rust_bus = MemoryBus(program)
    python_cpu = Z80Cpu(python_bus)
    rust_cpu = RustZ80Cpu(rust_bus, None)

    attributes = (
        "a", "f", "b", "c", "d", "e", "h", "l", "sp", "pc",
        "ix", "iy", "i", "r", "iff1", "iff2", "interrupt_mode",
        "halted", "total_cycles",
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


def test_rust_core_fails_explicitly_on_unsupported_opcode() -> None:
    RustZ80Cpu = _rust_cpu()
    bus = MemoryBus(bytes([0xDD]))
    cpu = RustZ80Cpu(bus, None)
    with pytest.raises(NotImplementedError, match="does not support opcode"):
        cpu.step()
