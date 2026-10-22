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
    cpu = make_cpu(b"\xa8")  # XOR A,B
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


def test_jp_absolute_loads_little_endian_target() -> None:
    cpu = make_cpu(bytes([0xC3, 0x34, 0x12]))  # JP 1234h
    cpu.f = 0xFF

    assert cpu.step() == 10
    assert cpu.pc == 0x1234
    assert cpu.f == 0xFF
    assert cpu.total_cycles == 10


@pytest.mark.parametrize(
    ("opcode", "flags", "taken"),
    [
        (0xC2, 0x00, True),  # JP NZ,nn
        (0xC2, 0x40, False),
        (0xCA, 0x40, True),  # JP Z,nn
        (0xCA, 0x00, False),
        (0xD2, 0x00, True),  # JP NC,nn
        (0xD2, 0x01, False),
        (0xDA, 0x01, True),  # JP C,nn
        (0xDA, 0x00, False),
    ],
)
def test_jp_conditional_checks_flags(opcode: int, flags: int, taken: bool) -> None:
    cpu = make_cpu(bytes([opcode, 0x78, 0x56]))
    cpu.f = flags

    assert cpu.step() == 10
    assert cpu.pc == (0x5678 if taken else 3)
    assert cpu.f == flags


@pytest.mark.parametrize(
    ("program", "start_pc", "expected_pc"),
    [
        (bytes([0x18, 0x03]), 0, 5),  # salto para frente
        (bytes([0x18, 0xFC]), 0, 0xFFFE),  # salto para trás
    ],
)
def test_jr_applies_signed_displacement(
    program: bytes, start_pc: int, expected_pc: int
) -> None:
    cpu = make_cpu(program)
    cpu.pc = start_pc

    assert cpu.step() == 12
    assert cpu.pc == expected_pc


@pytest.mark.parametrize(
    ("opcode", "flags", "taken"),
    [
        (0x20, 0x00, True),  # JR NZ,e
        (0x20, 0x40, False),
        (0x28, 0x40, True),  # JR Z,e
        (0x28, 0x00, False),
        (0x30, 0x00, True),  # JR NC,e
        (0x30, 0x01, False),
        (0x38, 0x01, True),  # JR C,e
        (0x38, 0x00, False),
    ],
)
def test_jr_conditional_checks_flags_and_cycles(
    opcode: int, flags: int, taken: bool
) -> None:
    cpu = make_cpu(bytes([opcode, 0x02]))
    cpu.f = flags

    assert cpu.step() == (12 if taken else 7)
    assert cpu.pc == (4 if taken else 2)
    assert cpu.f == flags


def test_jr_wraps_program_counter_at_16_bits() -> None:
    class FlatMemoryBus:
        def __init__(self) -> None:
            self.memory = bytearray(0x10000)
            self.memory[0xFFFE] = 0x18  # JR e
            self.memory[0xFFFF] = 0x01

        def read(self, address: int) -> int:
            return self.memory[address]

        def write(self, address: int, value: int) -> None:
            self.memory[address] = value

    cpu = Z80Cpu(FlatMemoryBus())
    cpu.pc = 0xFFFE

    assert cpu.step() == 12
    assert cpu.pc == 1


@pytest.mark.parametrize(
    ("opcode", "pair", "value"),
    [
        (0xC5, "bc", 0x1234),
        (0xD5, "de", 0x5678),
        (0xE5, "hl", 0x9ABC),
        (0xF5, "af", 0xDEF0),
    ],
)
def test_push_stores_register_pair_in_little_endian_stack(
    opcode: int, pair: str, value: int
) -> None:
    cpu = make_cpu(bytes([opcode]))
    cpu.sp = 0xC100
    setattr(cpu, pair, value)

    assert cpu.step() == 11
    assert cpu.sp == 0xC0FE
    assert cpu.bus.read(0xC0FE) == (value & 0xFF)
    assert cpu.bus.read(0xC0FF) == (value >> 8)
    assert cpu.total_cycles == 11


@pytest.mark.parametrize(
    ("opcode", "pair", "value"),
    [
        (0xC1, "bc", 0x1234),
        (0xD1, "de", 0x5678),
        (0xE1, "hl", 0x9ABC),
        (0xF1, "af", 0xDEF0),
    ],
)
def test_pop_restores_register_pair_from_stack(
    opcode: int, pair: str, value: int
) -> None:
    cpu = make_cpu(bytes([opcode]))
    cpu.sp = 0xC0FE
    cpu.bus.write(0xC0FE, value & 0xFF)
    cpu.bus.write(0xC0FF, value >> 8)

    assert cpu.step() == 10
    assert getattr(cpu, pair) == value
    assert cpu.sp == 0xC100


def test_call_pushes_return_address_and_ret_restores_it() -> None:
    cpu = make_cpu(bytes([0xCD, 0x08, 0x00] + [0x00] * 5 + [0xC9]))
    cpu.sp = 0xC100

    assert cpu.step() == 17
    assert cpu.pc == 0x0008
    assert cpu.sp == 0xC0FE
    assert cpu.bus.read(0xC0FE) == 0x03
    assert cpu.bus.read(0xC0FF) == 0x00

    assert cpu.step() == 10
    assert cpu.pc == 0x0003
    assert cpu.sp == 0xC100
    assert cpu.total_cycles == 27


@pytest.mark.parametrize(
    ("opcode", "flags", "taken"),
    [
        (0xC4, 0x00, True),  # CALL NZ,nn
        (0xC4, 0x40, False),
        (0xCC, 0x40, True),  # CALL Z,nn
        (0xCC, 0x00, False),
        (0xD4, 0x00, True),  # CALL NC,nn
        (0xD4, 0x01, False),
        (0xDC, 0x01, True),  # CALL C,nn
        (0xDC, 0x00, False),
    ],
)
def test_call_conditional_checks_flags_and_cycles(
    opcode: int, flags: int, taken: bool
) -> None:
    cpu = make_cpu(bytes([opcode, 0x78, 0x56]))
    cpu.sp = 0xC100
    cpu.f = flags

    assert cpu.step() == (17 if taken else 10)
    assert cpu.pc == (0x5678 if taken else 3)
    assert cpu.sp == (0xC0FE if taken else 0xC100)
    assert cpu.f == flags


@pytest.mark.parametrize(
    ("opcode", "flags", "taken"),
    [
        (0xC0, 0x00, True),  # RET NZ
        (0xC0, 0x40, False),
        (0xC8, 0x40, True),  # RET Z
        (0xC8, 0x00, False),
        (0xD0, 0x00, True),  # RET NC
        (0xD0, 0x01, False),
        (0xD8, 0x01, True),  # RET C
        (0xD8, 0x00, False),
    ],
)
def test_ret_conditional_checks_flags_and_cycles(
    opcode: int, flags: int, taken: bool
) -> None:
    cpu = make_cpu(bytes([opcode]))
    cpu.sp = 0xC0FE
    cpu.bus.write(0xC0FE, 0x34)
    cpu.bus.write(0xC0FF, 0x12)
    cpu.f = flags

    assert cpu.step() == (11 if taken else 5)
    assert cpu.pc == (0x1234 if taken else 1)
    assert cpu.sp == (0xC100 if taken else 0xC0FE)
    assert cpu.f == flags


def test_stack_pointer_wraps_across_16_bit_boundary() -> None:
    class FlatMemoryBus:
        def __init__(self) -> None:
            self.memory = bytearray(0x10000)
            self.memory[0] = 0xC5  # PUSH BC

        def read(self, address: int) -> int:
            return self.memory[address]

        def write(self, address: int, value: int) -> None:
            self.memory[address] = value

    cpu = Z80Cpu(FlatMemoryBus())
    cpu.sp = 0x0000
    cpu.bc = 0x1234

    assert cpu.step() == 11
    assert cpu.sp == 0xFFFE
    assert cpu.bus.read(0xFFFE) == 0x34
    assert cpu.bus.read(0xFFFF) == 0x12


@pytest.mark.parametrize(
    ("opcode", "pair", "initial", "expected"),
    [
        (0x03, "bc", 0x12FF, 0x1300),
        (0x13, "de", 0xFFFF, 0x0000),
        (0x23, "hl", 0x0000, 0x0001),
        (0x33, "sp", 0xFFFF, 0x0000),
    ],
)
def test_inc_register_pair_wraps_and_preserves_flags(
    opcode: int, pair: str, initial: int, expected: int
) -> None:
    cpu = make_cpu(bytes([opcode]))
    cpu.f = 0xD7
    setattr(cpu, pair, initial)

    assert cpu.step() == 6
    assert getattr(cpu, pair) == expected
    assert cpu.f == 0xD7


@pytest.mark.parametrize(
    ("opcode", "pair", "initial", "expected"),
    [
        (0x0B, "bc", 0x1300, 0x12FF),
        (0x1B, "de", 0x0000, 0xFFFF),
        (0x2B, "hl", 0x0001, 0x0000),
        (0x3B, "sp", 0x0000, 0xFFFF),
    ],
)
def test_dec_register_pair_wraps_and_preserves_flags(
    opcode: int, pair: str, initial: int, expected: int
) -> None:
    cpu = make_cpu(bytes([opcode]))
    cpu.f = 0xD7
    setattr(cpu, pair, initial)

    assert cpu.step() == 6
    assert getattr(cpu, pair) == expected
    assert cpu.f == 0xD7


@pytest.mark.parametrize(
    ("opcode", "hl", "operand", "expected_hl", "expected_flags"),
    [
        (0x09, 0x1234, 0x0001, 0x1235, 0xC4),  # preserva S/Z/PV
        (0x19, 0x0FFF, 0x0001, 0x1000, 0xD4),  # half-carry
        (0x09, 0xFFFF, 0x0001, 0x0000, 0xD5),  # ADD HL,BC: carry e zero
        (0x39, 0x8000, 0x8000, 0x0000, 0xC5),  # carry e zero
    ],
)
def test_add_hl_pair_updates_result_and_documented_flags(
    opcode: int, hl: int, operand: int, expected_hl: int, expected_flags: int
) -> None:
    cpu = make_cpu(bytes([opcode]))
    cpu.hl = hl
    cpu.bc = operand
    cpu.de = operand
    cpu.sp = operand
    cpu.f = 0xC4

    assert cpu.step() == 11
    assert cpu.hl == expected_hl
    assert cpu.f == expected_flags


@pytest.mark.parametrize(
    ("accumulator", "operand", "expected_flags"),
    [
        (0x42, 0x42, 0x42),  # zero e N
        (0x10, 0x01, 0x12),
        (0x00, 0x01, 0x93),
        (0x7F, 0x80, 0x87),
    ],
)
def test_cp_register_preserves_accumulator_and_sets_flags(
    accumulator: int, operand: int, expected_flags: int
) -> None:
    cpu = make_cpu(b"\xb8")  # CP B
    cpu.a = accumulator
    cpu.b = operand

    assert cpu.step() == 4
    assert cpu.a == accumulator
    assert cpu.f == expected_flags


def test_cp_reads_operand_from_hl() -> None:
    cpu = make_cpu(b"\xbe")  # CP (HL)
    cpu.hl = 0xC020
    cpu.a = 0x20
    cpu.bus.write(0xC020, 0x20)

    assert cpu.step() == 7
    assert cpu.a == 0x20
    assert cpu.f == 0x62


@pytest.mark.parametrize(
    ("opcode", "initial", "expected", "expected_flags"),
    [
        (0x04, 0x7F, 0x80, 0x95),  # overflow; carry preservado
        (0x0C, 0x0F, 0x10, 0x11),  # half-carry
        (0x3C, 0xFF, 0x00, 0x51),  # zero, half-carry e carry preservado
    ],
)
def test_inc_register_updates_flags_and_preserves_carry(
    opcode: int, initial: int, expected: int, expected_flags: int
) -> None:
    cpu = make_cpu(bytes([opcode]))
    register = (opcode >> 3) & 0x07
    cpu._write_register(register, initial)
    cpu.f = 0x01

    assert cpu.step() == 4
    assert cpu._read_register(register) == expected
    assert cpu.f == expected_flags


@pytest.mark.parametrize(
    ("opcode", "initial", "expected", "expected_flags"),
    [
        (0x05, 0x80, 0x7F, 0x3F),  # overflow e half-borrow
        (0x0D, 0x01, 0x00, 0x43),  # zero, N e carry preservado
        (0x3D, 0x00, 0xFF, 0xBB),  # sinal, half-borrow e N
    ],
)
def test_dec_register_updates_flags_and_preserves_carry(
    opcode: int, initial: int, expected: int, expected_flags: int
) -> None:
    cpu = make_cpu(bytes([opcode]))
    cpu.b = initial
    cpu.a = initial
    cpu.f = 0x01

    assert cpu.step() == 4
    register = (opcode >> 3) & 0x07
    assert cpu._read_register(register) == expected
    assert cpu.f == expected_flags


def test_inc_and_dec_hl_indirect_update_ram() -> None:
    cpu = make_cpu(bytes([0x34, 0x35]))  # INC (HL); DEC (HL)
    cpu.hl = 0xC020
    cpu.bus.write(0xC020, 0x7F)

    assert cpu.step() == 11
    assert cpu.bus.read(0xC020) == 0x80
    assert cpu.f == 0x94

    assert cpu.step() == 11
    assert cpu.bus.read(0xC020) == 0x7F
    assert cpu.f == 0x3E


@pytest.mark.parametrize(
    ("opcode", "accumulator", "operand", "expected_a", "expected_flags"),
    [
        (0xA0, 0xF0, 0x0F, 0x00, 0x54),  # AND: zero, H e paridade
        (0xA1, 0xF3, 0x0F, 0x03, 0x14),  # AND: H e paridade par 
        (0xB0, 0x80, 0x01, 0x81, 0x84),  # OR: sinal e paridade par
        (0xB1, 0x00, 0x00, 0x00, 0x44),  # OR: zero e paridade par
    ],
)
def test_and_or_register_update_accumulator_and_flags(
    opcode: int, accumulator: int, operand: int, expected_a: int, expected_flags: int
) -> None:
    cpu = make_cpu(bytes([opcode]))
    cpu.a = accumulator
    cpu.b = operand
    cpu.f = 0xFF

    assert cpu.step() == 4
    assert cpu.a == expected_a
    assert cpu.f == expected_flags


def test_and_reads_operand_from_hl() -> None:
    cpu = make_cpu(b"\\xa6")  # AND (HL)
    cpu.hl = 0xC020
    cpu.a = 0xF3
    cpu.bus.write(0xC020, 0x0F)

    assert cpu.step() == 7
    assert cpu.a == 0x03
    assert cpu.f == 0x14


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
