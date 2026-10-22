"""Testes sintéticos do subconjunto inicial da CPU Z80."""

import pytest

from sega_emulator.bus.memory import RAM_SIZE, SG1000Ram
from sega_emulator.bus.memory_bus import SG1000MemoryBus
from sega_emulator.cpu.z80 import UnsupportedOpcodeError, Z80Cpu


def make_cpu(program: bytes) -> Z80Cpu:
    """Cria CPU e barramento com programa sintético em ROM."""
    ram = SG1000Ram(bytes(RAM_SIZE))
    return Z80Cpu(SG1000MemoryBus(ram, program))


CONDITIONAL_FLAG_CASES = [
    (0, 0x00, 0x40),  # NZ: Z=0 / Z=1
    (1, 0x40, 0x00),  # Z: Z=1 / Z=0
    (2, 0x00, 0x01),  # NC: C=0 / C=1
    (3, 0x01, 0x00),  # C: C=1 / C=0
    (4, 0x00, 0x04),  # PO: PV=0 / PV=1
    (5, 0x04, 0x00),  # PE: PV=1 / PV=0
    (6, 0x00, 0x80),  # P: S=0 / S=1
    (7, 0x80, 0x00),  # M: S=1 / S=0
]


@pytest.mark.parametrize(
    ("accumulator", "initial_flags", "expected_a", "expected_flags"),
    [
        (0x3C, 0x10, 0x42, 0x14),  # soma BCD: corrige o nibble baixo
        (0x9A, 0x00, 0x00, 0x55),  # soma: ajuste alto gera carry
        (0x15, 0x00, 0x15, 0x00),  # valor BCD já válido
        (0x1B, 0x12, 0x15, 0x02),  # subtração: corrige sem alterar N
        (0x00, 0x03, 0xA0, 0xA7),  # subtração com carry/borrow anterior
    ],
)
def test_daa_adjusts_bcd_and_sets_documented_flags(
    accumulator: int, initial_flags: int, expected_a: int, expected_flags: int
) -> None:
    cpu = make_cpu(bytes([0x27]))
    cpu.a = accumulator
    cpu.f = initial_flags

    assert cpu.step() == 4
    assert cpu.a == expected_a
    assert cpu.f == expected_flags


@pytest.mark.parametrize(
    ("opcode", "accumulator", "initial_flags", "expected_a", "expected_flags"),
    [
        (0x07, 0x81, 0xD7, 0x03, 0xC5),  # RLCA
        (0x0F, 0x81, 0xD7, 0xC0, 0xD5),  # RRCA
        (0x17, 0x94, 0xED, 0x29, 0xE5),  # RLA
        (0x1F, 0x29, 0x94, 0x94, 0x85),  # RRA
    ],
    # current test content continues
