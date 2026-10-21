"""Testes sintéticos do subconjunto inicial da CPU Z80."""

import pytest

from sega_emulator.bus.memory import RAM_SIZE, SG1000Ram
from sega_emulator.bus.memory_bus import SG1000MemoryBus
from sega_emulator.cpu.z80 import UnsupportedOpcodeError, Z80Cpu


def make_cpu(program: bytes) -> Z80Cpu:
    """Cria CPU e barramento com programa sintético em ROM."""
    ram = SG1000Ram(bytes(RAM_SIZE))
    return Z80Cpu(SG1000MemoryBus(ram, program))


def test_ld_immediate_and_register_transfer_preserve_flags() -> None:
    cpu = make_cpu(bytes([0x06, 0x42, 0x78]))  # LD B,42h; LD A,B
    cpu.f = 0xD7

    assert cpu.step() == 7
    assert cpu.b == 0x42
    assert cpu.pc == 2

    assert cpu.step() == 4
    assert cpu.a == 0x42
    assert cpu.f == 0xD7
    assert cpu.pc == 3
    assert cpu.total_cycles == 11


def test_ld_hl_indirect_reads_and_writes_ram() -> None:
    program = bytes(
        [
            0x26,
            0xC0,  # LD H,C0h
            0x2E,
            0x10,  # LD L,10h
            0x36,
            0xA5,  # LD (HL),A5h
            0x7E,  # LD A,(HL)
            0x47,  # LD B,A
        ]
    )
    cpu = make_cpu(program)

    assert cpu.step() == 7
    assert cpu.step() == 7
    assert cpu.hl == 0xC010

    assert cpu.step() == 10
    assert cpu.step() == 7
    assert cpu.step() == 4
    assert cpu.a == 0xA5
    assert cpu.b == 0xA5
    assert cpu.total_cycles == 35


@pytest.mark.parametrize(
    ("accumulator", "operand", "expected_a", "expected_flags"),
    [
        (0x7F, 0x01, 0x80, 0x94),  # sinal, half-carry e overflow
        (0xFF, 0x01, 0x00, 0x51),  # zero, half-carry e carry
    ],
)
def test_add_a_register_sets_result_and_flags(
    accumulator: int, operand: int, expected_a: int, expected_flags: int
) -> None:
    cpu = make_cpu(b"\x80")  # ADD A,B
    cpu.a = accumulator
    cpu.b = operand

    assert cpu.step() == 4
    assert cpu.a == expected_a
    assert cpu.f == expected_flags


@pytest.mark.parametrize(
    ("accumulator", "operand", "expected_a", "expected_flags"),
    [
        (0x00, 0x01, 0xFF, 0xBB),  # sinal, half-borrow, subtração e borrow
        (0x80, 0x01, 0x7F, 0x3E),  # overflow sem borrow
    ],
)
def test_sub_register_sets_result_and_flags(
    accumulator: int, operand: int, expected_a: int, expected_flags: int
) -> None:
    cpu = make_cpu(b"\x90")  # SUB B
    cpu.a = accumulator
    cpu.b = operand

    assert cpu.step() == 4
    assert cpu.a == expected_a
    assert cpu.f == expected_flags


def test_xor_register_sets_zero_and_even_parity_flags() -> None:
    cpu = make_cpu(b"\xA8")  # XOR A,B
    cpu.a = 0x55
    cpu.b = 0x55

    assert cpu.step() == 4
    assert cpu.a == 0
    assert cpu.f == 0x44


def test_alu_operation_reads_value_from_hl() -> None:
    cpu = make_cpu(bytes([0x86]))  # ADD A,(HL)
    cpu.hl = 0xC020
    cpu.a = 0x10
    cpu.bus.write(0xC020, 0x22)

    assert cpu.step() == 7
    assert cpu.a == 0x32
    assert cpu.total_cycles == 7


def test_nop_advances_program_counter_and_cycles() -> None:
    cpu = make_cpu(b"\x00")

    assert cpu.step() == 4
    assert cpu.pc == 1
    assert cpu.total_cycles == 4


def test_unsupported_opcode_is_explicit() -> None:
    cpu = make_cpu(b"\x76")  # HALT ainda não foi implementado.

    with pytest.raises(UnsupportedOpcodeError, match="76h"):
        cpu.step()

    assert cpu.pc == 1
    assert cpu.total_cycles == 0


@pytest.mark.parametrize(
    ("pair", "expected_high", "expected_low"),
    [
        ("af", 0x12, 0x34),
        ("bc", 0x56, 0x78),
        ("de", 0x9A, 0xBC),
        ("hl", 0xDE, 0xF0),
    ],
)
def test_register_pair_round_trip(
    pair: str, expected_high: int, expected_low: int
) -> None:
    cpu = make_cpu(b"\x00")

    setattr(cpu, pair, (expected_high << 8) | expected_low)

    assert getattr(cpu, pair) == (expected_high << 8) | expected_low


@pytest.mark.parametrize("value", [-1, 0x10000])
def test_register_pair_rejects_out_of_range_values(value: int) -> None:
    cpu = make_cpu(b"\x00")

    with pytest.raises(ValueError):
        cpu.hl = value
