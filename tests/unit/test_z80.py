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
        (0x00, 0x00, 0x00, 0x44),  # zero e paridade par
    ],
)
def test_daa_adjusts_accumulator_and_flags(
    accumulator: int,
    initial_flags: int,
    expected_a: int,
    expected_flags: int,
) -> None:
    cpu = make_cpu(bytes([0x27]))  # DAA
    cpu.a = accumulator
    cpu.f = initial_flags

    assert cpu.step() == 4
    assert cpu.a == expected_a
    assert cpu.f == expected_flags
    assert cpu.pc == 1
    assert cpu.total_cycles == 4


@pytest.mark.parametrize(
    ("opcode", "accumulator", "expected_a", "expected_flags"),
    [
        (0x07, 0x94, 0x29, 0xED),  # RLCA: bit 7 volta ao bit 0
        (0x0F, 0x29, 0x94, 0xC5),  # RRCA: bit 0 volta ao bit 7
        (0x17, 0x94, 0x28, 0xED),  # RLA: carry anterior entra no bit 0
        (0x1F, 0x29, 0x14, 0xC5),  # RRA: carry anterior entra no bit 7
    ],
)
def test_accumulator_rotates_update_carry_and_preserve_selected_flags(
    opcode: int, accumulator: int, expected_a: int, expected_flags: int
) -> None:
    cpu = make_cpu(bytes([opcode]))
    cpu.a = accumulator
    cpu.f = 0xD6  # S, Z e P/V definidos; carry anterior limpo

    assert cpu.step() == 4
    assert cpu.a == expected_a
    assert cpu.f == expected_flags
    assert cpu.pc == 1
    assert cpu.total_cycles == 4


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
    ("accumulator", "operand", "carry_in", "expected_a", "expected_flags"),
    [
        (0x7F, 0x00, 1, 0x80, 0x94),  # signed overflow e half-carry
        (0xFF, 0x00, 1, 0x00, 0x51),  # zero, half-carry e carry
        (0x10, 0x01, 0, 0x11, 0x00),
    ],
)
def test_adc_register_includes_carry_and_sets_flags(
    accumulator: int,
    operand: int,
    carry_in: int,
    expected_a: int,
    expected_flags: int,
) -> None:
    cpu = make_cpu(bytes([0x88]))  # ADC A,B
    cpu.a = accumulator
    cpu.b = operand
    cpu.f = carry_in

    assert cpu.step() == 4
    assert cpu.a == expected_a
    assert cpu.f == expected_flags


@pytest.mark.parametrize(
    ("accumulator", "operand", "carry_in", "expected_a", "expected_flags"),
    [
        (0x00, 0x00, 1, 0xFF, 0xBB),  # borrow, half-borrow e sinal
        (0x80, 0x00, 1, 0x7F, 0x3E),  # signed overflow e half-borrow
        (0x42, 0x42, 0, 0x00, 0x42),  # resultado zero
    ],
)
def test_sbc_register_includes_carry_and_sets_flags(
    accumulator: int,
    operand: int,
    carry_in: int,
    expected_a: int,
    expected_flags: int,
) -> None:
    cpu = make_cpu(bytes([0x98]))  # SBC A,B
    cpu.a = accumulator
    cpu.b = operand
    cpu.f = carry_in

    assert cpu.step() == 4
    assert cpu.a == expected_a
    assert cpu.f == expected_flags


def test_adc_and_sbc_read_operand_from_hl() -> None:
    cpu = make_cpu(bytes([0x8E, 0x9E]))  # ADC A,(HL); SBC A,(HL)
    cpu.hl = 0xC020
    cpu.a = 0x10
    cpu.bus.write(0xC020, 0x01)
    cpu.f = 1

    assert cpu.step() == 7
    assert cpu.a == 0x12
    assert cpu.f == 0x00

    cpu.f = 1
    assert cpu.step() == 7
    assert cpu.a == 0x10
    assert cpu.f == 0x02


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
    ("condition_code", "taken_flags", "not_taken_flags"),
    CONDITIONAL_FLAG_CASES,
)
def test_jp_conditional_supports_all_eight_conditions(
    condition_code: int, taken_flags: int, not_taken_flags: int
) -> None:
    opcode = 0xC2 + (condition_code << 3)
    for flags, taken in ((taken_flags, True), (not_taken_flags, False)):
        cpu = make_cpu(bytes([opcode, 0x78, 0x56]))
        cpu.f = flags

        assert cpu.step() == 10
        assert cpu.pc == (0x5678 if taken else 3)
        assert cpu.f == flags


@pytest.mark.parametrize(
    ("accumulator", "flags", "expected_a", "expected_flags"),
    [
        (0x00, 0xC5, 0xFF, 0xFF),  # S/Y/X e H/N; S/Z/PV/C preservadas
        (0x28, 0x45, 0xD7, 0x57),
        (0xFF, 0x00, 0x00, 0x12),  # CPL preserva Z/PV, aqui ambos já começam zerados
    ],
)
def test_cpl_complements_accumulator_and_updates_only_documented_flags(
    accumulator: int, flags: int, expected_a: int, expected_flags: int
) -> None:
    cpu = make_cpu(bytes([0x2F]))  # CPL
    cpu.a = accumulator
    cpu.f = flags

    assert cpu.step() == 4
    assert cpu.a == expected_a
    assert cpu.f == expected_flags
    assert cpu.pc == 1
    assert cpu.total_cycles == 4


@pytest.mark.parametrize(
    ("accumulator", "flags", "expected_flags"),
    [
        (0x00, 0xD6, 0xC5),  # S/Z/PV preservados, XY do A, carry definido
        (0x28, 0x01, 0x29),
        (0xD7, 0xFF, 0xC5),
    ],
)
def test_scf_sets_carry_and_preserves_sign_zero_parity(
    accumulator: int, flags: int, expected_flags: int
) -> None:
    cpu = make_cpu(bytes([0x37]))  # SCF
    cpu.a = accumulator
    cpu.f = flags

    assert cpu.step() == 4
    assert cpu.f == expected_flags
    assert cpu.a == accumulator
    assert cpu.pc == 1
    assert cpu.total_cycles == 4


@pytest.mark.parametrize(
    ("accumulator", "flags", "expected_flags"),
    [
        (0x00, 0xC5, 0xD4),  # carry anterior vira half-carry
        (0x28, 0x00, 0x29),  # carry anterior zero vira carry
        (0xD7, 0xFF, 0xD4),  # carry anterior um vira H; X/Y vêm de A
    ],
)
def test_ccf_complements_carry_and_updates_half_carry(
    accumulator: int, flags: int, expected_flags: int
) -> None:
    cpu = make_cpu(bytes([0x3F]))  # CCF
    cpu.a = accumulator
    cpu.f = flags

    assert cpu.step() == 4
    assert cpu.f == expected_flags
    assert cpu.a == accumulator
    assert cpu.pc == 1
    assert cpu.total_cycles == 4


@pytest.mark.parametrize(
    ("initial_b", "displacement", "expected_b", "expected_pc", "expected_cycles"),
    [
        (0x02, 0x02, 0x01, 0x0004, 13),  # B não zero: desvia para frente
        (0x01, 0xFE, 0x00, 0x0002, 8),  # B chega a zero: não desvia
        (0x00, 0x01, 0xFF, 0x0003, 13),  # decremento com wrap para FFh
        (0x02, 0xFE, 0x01, 0x0000, 13),  # deslocamento negativo
    ],
)
def test_djnz_decrements_b_branches_and_preserves_flags(
    initial_b: int,
    displacement: int,
    expected_b: int,
    expected_pc: int,
    expected_cycles: int,
) -> None:
    cpu = make_cpu(bytes([0x10, displacement]))
    cpu.b = initial_b
    cpu.f = 0xD7

    assert cpu.step() == expected_cycles
    assert cpu.b == expected_b
    assert cpu.pc == expected_pc
    assert cpu.f == 0xD7
    assert cpu.total_cycles == expected_cycles


def test_djnz_relative_branch_wraps_program_counter() -> None:
    class FlatMemoryBus:
        def __init__(self) -> None:
            self.memory = bytearray(0x10000)
            self.memory[0xFFFE] = 0x10  # DJNZ e
            self.memory[0xFFFF] = 0xFE  # deslocamento -2

        def read(self, address: int) -> int:
            return self.memory[address]

        def write(self, address: int, value: int) -> None:
            self.memory[address] = value

    cpu = Z80Cpu(FlatMemoryBus())
    cpu.pc = 0xFFFE
    cpu.b = 0x02
    cpu.f = 0xA5

    assert cpu.step() == 13
    assert cpu.b == 0x01
    assert cpu.pc == 0xFFFE
    assert cpu.f == 0xA5


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
    ("condition_code", "taken_flags", "not_taken_flags"),
    CONDITIONAL_FLAG_CASES,
)
def test_call_conditional_supports_all_eight_conditions_and_cycles(
    condition_code: int, taken_flags: int, not_taken_flags: int
) -> None:
    opcode = 0xC4 + (condition_code << 3)
    for flags, taken in ((taken_flags, True), (not_taken_flags, False)):
        cpu = make_cpu(bytes([opcode, 0x78, 0x56]))
        cpu.sp = 0xC100
        cpu.f = flags

        assert cpu.step() == (17 if taken else 10)
        assert cpu.pc == (0x5678 if taken else 3)
        assert cpu.sp == (0xC0FE if taken else 0xC100)
        assert cpu.f == flags
        if taken:
            assert cpu.bus.read(0xC0FE) == 0x03
            assert cpu.bus.read(0xC0FF) == 0x00


@pytest.mark.parametrize(
    ("condition_code", "taken_flags", "not_taken_flags"),
    CONDITIONAL_FLAG_CASES,
)
def test_ret_conditional_supports_all_eight_conditions_and_cycles(
    condition_code: int, taken_flags: int, not_taken_flags: int
) -> None:
    opcode = 0xC0 + (condition_code << 3)
    for flags, taken in ((taken_flags, True), (not_taken_flags, False)):
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
    ("opcode", "hl", "operand", "carry", "expected_hl", "expected_flags"),
    [
        (0x4A, 0x1234, 0x0001, 0, 0x1235, 0x00),
        (0x5A, 0x0FFF, 0x0000, 1, 0x1000, 0x10),
        (0x6A, 0x7FFF, 0x0000, 1, 0xFFFF, 0xBC),  # ADC HL,HL uses HL as both operands
        (0x7A, 0xFFFF, 0x0000, 1, 0x0000, 0x51),
        (0x42, 0x1234, 0x0001, 0, 0x1233, 0x02),
        (0x52, 0x1000, 0x0001, 1, 0x0FFE, 0x1A),
        (0x62, 0x8000, 0x0001, 0, 0x0000, 0x42),  # SBC HL,HL subtracts HL from itself
        (0x72, 0x0000, 0x0000, 1, 0xFFFF, 0xBB),
    ],
)
def test_ed_adc_sbc_hl_pair_results_flags_and_cycles(
    opcode: int,
    hl: int,
    operand: int,
    carry: int,
    expected_hl: int,
    expected_flags: int,
) -> None:
    cpu = make_cpu(bytes([0xED, opcode]))
    cpu.hl = hl
    cpu.bc = operand
    cpu.de = operand
    cpu.sp = operand
    cpu.f = carry

    assert cpu.step() == 15
    assert cpu.hl == expected_hl
    assert cpu.f == expected_flags
    assert cpu.pc == 2
    assert cpu.total_cycles == 15


@pytest.mark.parametrize(
    ("opcode", "hl", "operand", "carry", "expected_hl", "expected_flags"),
    [
        (0x4A, 0x7FFF, 0x0000, 1, 0x8000, 0x94),
        (0x4A, 0xFFFF, 0x0000, 1, 0x0000, 0x51),
        (0x42, 0x0000, 0x0000, 1, 0xFFFF, 0xBB),
        (0x42, 0x8000, 0x0001, 0, 0x7FFF, 0x3E),
    ],
)
def test_ed_adc_sbc_hl_overflow_and_boundary_flags(
    opcode: int,
    hl: int,
    operand: int,
    carry: int,
    expected_hl: int,
    expected_flags: int,
) -> None:
    cpu = make_cpu(bytes([0xED, opcode]))
    cpu.hl = hl
    cpu.bc = operand
    cpu.f = carry

    assert cpu.step() == 15
    assert cpu.hl == expected_hl
    assert cpu.f == expected_flags


def test_unsupported_ed_opcode_reports_prefixed_opcode() -> None:
    cpu = make_cpu(bytes([0xED, 0x00]))

    with pytest.raises(UnsupportedOpcodeError, match="ED 00h"):
        cpu.step()


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
    register = (opcode >> 3) & 0x07
    cpu._write_register(register, initial)
    cpu.f = 0x01

    assert cpu.step() == 4
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
    cpu._write_register(opcode & 0x07, operand)
    cpu.f = 0xFF

    assert cpu.step() == 4
    assert cpu.a == expected_a
    assert cpu.f == expected_flags


def test_and_reads_operand_from_hl() -> None:
    cpu = make_cpu(b"\xa6")  # AND (HL)
    cpu.hl = 0xC020
    cpu.a = 0xF3
    cpu.bus.write(0xC020, 0x0F)

    assert cpu.step() == 7
    assert cpu.a == 0x03
    assert cpu.f == 0x14


@pytest.mark.parametrize(
    ("opcode", "accumulator", "operand", "carry_in", "expected_a", "expected_flags"),
    [
        (0xC6, 0x0F, 0x01, 0, 0x10, 0x10),  # ADD A,n: half-carry
        (0xCE, 0x7F, 0x00, 1, 0x80, 0x94),  # ADC A,n: overflow
        (0xD6, 0x00, 0x01, 0, 0xFF, 0xBB),  # SUB n: borrow
        (0xDE, 0x00, 0x00, 1, 0xFF, 0xBB),  # SBC A,n: borrow de entrada
        (0xE6, 0xF0, 0x0F, 0, 0x00, 0x54),  # AND n: zero, H e paridade
        (0xEE, 0xF0, 0x0F, 0, 0xFF, 0xAC),  # XOR n: sinal, XY e paridade
        (0xF6, 0x80, 0x01, 0, 0x81, 0x84),  # OR n: sinal e paridade
        (0xFE, 0x42, 0x42, 0, 0x42, 0x42),  # CP n: zero sem alterar A
    ],
)
def test_immediate_alu_instructions(
    opcode: int,
    accumulator: int,
    operand: int,
    carry_in: int,
    expected_a: int,
    expected_flags: int,
) -> None:
    cpu = make_cpu(bytes([opcode, operand]))
    cpu.a = accumulator
    cpu.f = carry_in

    assert cpu.step() == 7
    assert cpu.pc == 2
    assert cpu.a == expected_a
    assert cpu.f == expected_flags
    assert cpu.total_cycles == 7


@pytest.mark.parametrize(
    ("target", "expected_cycles"), [(0x0000, 4), (0xC123, 4), (0xFFFF, 4)]
)
def test_jp_hl_jumps_to_register_pair_and_preserves_flags(
    target: int, expected_cycles: int
) -> None:
    cpu = make_cpu(bytes([0xE9]))  # JP (HL)
    cpu.hl = target
    cpu.f = 0xD7

    assert cpu.step() == expected_cycles
    assert cpu.pc == target
    assert cpu.hl == target
    assert cpu.f == 0xD7
    assert cpu.total_cycles == expected_cycles


@pytest.mark.parametrize("value", [0x0000, 0xC123, 0xFFFF])
def test_ld_sp_hl_copies_pair_and_preserves_flags(value: int) -> None:
    cpu = make_cpu(bytes([0xF9]))  # LD SP,HL
    cpu.hl = value
    cpu.sp = 0x1234
    cpu.f = 0xD7

    assert cpu.step() == 6
    assert cpu.sp == value
    assert cpu.hl == value
    assert cpu.f == 0xD7
    assert cpu.pc == 1
    assert cpu.total_cycles == 6


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


@pytest.mark.parametrize(
    ("opcode", "pair", "expected"),
    [
        (0x01, "bc", 0x1234),
        (0x11, "de", 0xABCD),
        (0x21, "hl", 0xC020),
        (0x31, "sp", 0xFFFF),
    ],
)
def test_ld_register_pair_immediate_reads_little_endian_and_preserves_flags(
    opcode: int, pair: str, expected: int
) -> None:
    cpu = make_cpu(bytes([opcode, expected & 0xFF, expected >> 8]))
    cpu.f = 0xD7

    assert cpu.step() == 10
    assert getattr(cpu, pair) == expected
    assert cpu.pc == 3
    assert cpu.f == 0xD7
    assert cpu.total_cycles == 10


def test_ld_register_pair_immediate_wraps_program_counter() -> None:
    class FlatMemoryBus:
        def __init__(self) -> None:
            self.memory = bytearray(0x10000)
            self.memory[0xFFFE:0x10000] = bytes([0x01, 0x34])
            self.memory[0x0000] = 0x12

        def read(self, address: int) -> int:
            return self.memory[address]

        def write(self, address: int, value: int) -> None:
            self.memory[address] = value

    cpu = Z80Cpu(FlatMemoryBus())
    cpu.pc = 0xFFFE

    assert cpu.step() == 10
    assert cpu.bc == 0x1234
    assert cpu.pc == 0x0001


@pytest.mark.parametrize(
    ("opcode", "pair", "address", "value"),
    [
        (0x02, "bc", 0xC020, 0xA5),
        (0x12, "de", 0xC021, 0x5A),
    ],
)
def test_ld_indirect_pair_from_accumulator_writes_ram_and_preserves_flags(
    opcode: int, pair: str, address: int, value: int
) -> None:
    cpu = make_cpu(bytes([opcode]))
    setattr(cpu, pair, address)
    cpu.a = value
    cpu.f = 0xD7

    assert cpu.step() == 7
    assert cpu.bus.read(address) == value
    assert cpu.f == 0xD7
    assert cpu.pc == 1
    assert cpu.total_cycles == 7


@pytest.mark.parametrize(
    ("opcode", "pair", "address", "value"),
    [
        (0x0A, "bc", 0xC020, 0xA5),
        (0x1A, "de", 0xC021, 0x5A),
    ],
)
def test_ld_accumulator_from_indirect_pair_reads_ram_and_preserves_flags(
    opcode: int, pair: str, address: int, value: int
) -> None:
    cpu = make_cpu(bytes([opcode]))
    setattr(cpu, pair, address)
    cpu.bus.write(address, value)
    cpu.a = 0x11
    cpu.f = 0xD7

    assert cpu.step() == 7
    assert cpu.a == value
    assert cpu.f == 0xD7
    assert cpu.pc == 1
    assert cpu.total_cycles == 7


def test_ld_absolute_hl_writes_little_endian_and_preserves_flags() -> None:
    cpu = make_cpu(bytes([0x22, 0x20, 0xC0]))  # LD (C020h),HL
    cpu.hl = 0xA55A
    cpu.f = 0xD7

    assert cpu.step() == 16
    assert cpu.bus.read(0xC020) == 0x5A
    assert cpu.bus.read(0xC021) == 0xA5
    assert cpu.hl == 0xA55A
    assert cpu.f == 0xD7
    assert cpu.pc == 3
    assert cpu.total_cycles == 16


def test_ld_hl_from_absolute_reads_little_endian_and_preserves_flags() -> None:
    cpu = make_cpu(bytes([0x2A, 0x20, 0xC0]))  # LD HL,(C020h)
    cpu.bus.write(0xC020, 0x5A)
    cpu.bus.write(0xC021, 0xA5)
    cpu.hl = 0x1234
    cpu.f = 0xD7

    assert cpu.step() == 16
    assert cpu.hl == 0xA55A
    assert cpu.f == 0xD7
    assert cpu.pc == 3
    assert cpu.total_cycles == 16


def test_ld_absolute_hl_wraps_second_memory_access_at_ffff() -> None:
    class FlatMemoryBus:
        def __init__(self) -> None:
            self.memory = bytearray(0x10000)
            self.memory[0x0100:0x0103] = bytes([0x22, 0xFF, 0xFF])

        def read(self, address: int) -> int:
            return self.memory[address]

        def write(self, address: int, value: int) -> None:
            self.memory[address] = value

    bus = FlatMemoryBus()
    cpu = Z80Cpu(bus)
    cpu.pc = 0x0100
    cpu.hl = 0x1234

    assert cpu.step() == 16
    assert bus.memory[0xFFFF] == 0x34
    assert bus.memory[0x0000] == 0x12
    assert cpu.pc == 0x0103


def test_ld_hl_from_absolute_wraps_second_memory_access_at_ffff() -> None:
    class FlatMemoryBus:
        def __init__(self) -> None:
            self.memory = bytearray(0x10000)
            self.memory[0x0100:0x0103] = bytes([0x2A, 0xFF, 0xFF])
            self.memory[0xFFFF] = 0xCD
            self.memory[0x0000] = 0xAB

        def read(self, address: int) -> int:
            return self.memory[address]

        def write(self, address: int, value: int) -> None:
            self.memory[address] = value

    cpu = Z80Cpu(FlatMemoryBus())
    cpu.pc = 0x0100
    cpu.f = 0xD7

    assert cpu.step() == 16
    assert cpu.hl == 0xABCD
    assert cpu.f == 0xD7
    assert cpu.pc == 0x0103


def test_ld_absolute_from_accumulator_writes_ram_and_preserves_flags() -> None:
    cpu = make_cpu(bytes([0x32, 0x20, 0xC0]))  # LD (C020h),A
    cpu.a = 0xA5
    cpu.f = 0xD7

    assert cpu.step() == 13
    assert cpu.bus.read(0xC020) == 0xA5
    assert cpu.f == 0xD7
    assert cpu.pc == 3
    assert cpu.total_cycles == 13


def test_ld_accumulator_from_absolute_reads_ram_and_preserves_flags() -> None:
    cpu = make_cpu(bytes([0x3A, 0x21, 0xC0]))  # LD A,(C021h)
    cpu.bus.write(0xC021, 0x5A)
    cpu.a = 0x11
    cpu.f = 0xD7

    assert cpu.step() == 13
    assert cpu.a == 0x5A
    assert cpu.f == 0xD7
    assert cpu.pc == 3
    assert cpu.total_cycles == 13


@pytest.mark.parametrize(
    ("main_pair", "alternate_pair", "initial_main", "initial_alt"),
    [
        ("af", "af_alt", 0x1234, 0xABCD),
        ("bc", "bc_alt", 0x1234, 0xABCD),
        ("de", "de_alt", 0x1234, 0xABCD),
        ("hl", "hl_alt", 0x1234, 0xABCD),
    ],
)
def test_exchange_register_pair_swaps_both_values(
    main_pair: str, alternate_pair: str, initial_main: int, initial_alt: int
) -> None:
    opcode = {"af": 0x08, "bc": 0xD9, "de": 0xD9, "hl": 0xD9}[main_pair]
    cpu = make_cpu(bytes([opcode]))
    setattr(cpu, main_pair, initial_main)
    setattr(cpu, alternate_pair, initial_alt)

    assert cpu.step() == 4
    if main_pair == "af":
        assert cpu.af == initial_alt
        assert cpu.af_alt == initial_main
    else:
        # EXX swaps BC, DE and HL together; validate the selected pair and
        # ensure the other two pairs are swapped in the same instruction.
        assert getattr(cpu, main_pair) == initial_alt
        assert getattr(cpu, alternate_pair) == initial_main
        assert cpu.bc == (initial_alt if main_pair == "bc" else 0)
        assert cpu.de == (initial_alt if main_pair == "de" else 0)
        assert cpu.hl == (initial_alt if main_pair == "hl" else 0)
    assert cpu.pc == 1
    assert cpu.total_cycles == 4


def test_exx_swaps_all_three_register_pairs_without_touching_af_or_flags() -> None:
    cpu = make_cpu(bytes([0xD9]))
    cpu.af = 0x12D7
    cpu.bc, cpu.de, cpu.hl = 0x1234, 0x5678, 0x9ABC
    cpu.af_alt = 0xABCD
    cpu.bc_alt, cpu.de_alt, cpu.hl_alt = 0xCDEF, 0x9876, 0x5432

    assert cpu.step() == 4
    assert (cpu.bc, cpu.de, cpu.hl) == (0xCDEF, 0x9876, 0x5432)
    assert (cpu.bc_alt, cpu.de_alt, cpu.hl_alt) == (0x1234, 0x5678, 0x9ABC)
    assert cpu.af == 0x12D7
    assert cpu.af_alt == 0xABCD


def test_ex_de_hl_swaps_pairs_without_touching_flags() -> None:
    cpu = make_cpu(bytes([0xEB]))
    cpu.de, cpu.hl, cpu.f = 0x1234, 0xABCD, 0xD7

    assert cpu.step() == 4
    assert (cpu.de, cpu.hl, cpu.f) == (0xABCD, 0x1234, 0xD7)
    assert cpu.pc == 1


@pytest.mark.parametrize(
    ("opcode", "vector"),
    [
        (0xC7, 0x00),
        (0xCF, 0x08),
        (0xD7, 0x10),
        (0xDF, 0x18),
        (0xE7, 0x20),
        (0xEF, 0x28),
        (0xF7, 0x30),
        (0xFF, 0x38),
    ],
)
def test_rst_pushes_return_address_and_jumps_to_vector(
    opcode: int, vector: int
) -> None:
    cpu = make_cpu(bytes([opcode]))
    cpu.sp = 0xC100
    cpu.f = 0xD7

    assert cpu.step() == 11
    assert cpu.pc == vector
    assert cpu.sp == 0xC0FE
    assert cpu.bus.read(0xC0FE) == 0x01
    assert cpu.bus.read(0xC0FF) == 0x00
    assert cpu.f == 0xD7
    assert cpu.total_cycles == 11


def test_rst_stack_wraps_at_zero() -> None:
    class FlatMemoryBus:
        def __init__(self) -> None:
            self.memory = bytearray(0x10000)
            self.memory[0x0100] = 0xFF

        def read(self, address: int) -> int:
            return self.memory[address]

        def write(self, address: int, value: int) -> None:
            self.memory[address] = value

    bus = FlatMemoryBus()
    cpu = Z80Cpu(bus)
    cpu.pc = 0x0100
    cpu.sp = 0x0001

    assert cpu.step() == 11
    assert cpu.pc == 0x38
    assert cpu.sp == 0xFFFF
    assert bus.memory[0xFFFF] == 0x01
    assert bus.memory[0x0000] == 0x01


@pytest.mark.parametrize(
    ("opcode", "special_register"),
    [(0x47, "i"), (0x4F, "r")],
)
def test_ed_load_special_register_from_accumulator(
    opcode: int, special_register: str
) -> None:
    cpu = make_cpu(bytes([0xED, opcode]))
    cpu.a = 0xA5
    cpu.f = 0xD7
    cpu.r = 0x10

    assert cpu.step() == 9
    assert getattr(cpu, special_register) == 0xA5
    assert cpu.f == 0xD7
    assert cpu.pc == 2
    assert cpu.total_cycles == 9
    if special_register == "r":
        # R increments for both M1 fetches, then LD R,A overwrites it.
        assert cpu.r == 0xA5
    else:
        assert cpu.r == 0x12


@pytest.mark.parametrize(
    ("initial_r", "expected_r"),
    [(0x00, 0x01), (0x7F, 0x00), (0x80, 0x81), (0xFF, 0x80)],
)
def test_refresh_register_increments_low_seven_bits_on_opcode_fetch(
    initial_r: int, expected_r: int
) -> None:
    cpu = make_cpu(bytes([0x00]))  # NOP
    cpu.r = initial_r

    assert cpu.step() == 4
    assert cpu.r == expected_r


@pytest.mark.parametrize(
    ("value", "iff2", "initial_flags", "expected_flags"),
    [
        (0x00, False, 0xD5, 0x41),  # zero and carry; H/N/PV reset
        (0xA8, True, 0x02, 0xAC),  # sign, undocumented X/Y and IFF2 in PV
        (0x28, False, 0x01, 0x29),  # X/Y and carry preserved
        (0x7F, True, 0x00, 0x2C),  # Y/X and IFF2
    ],
)
def test_ed_load_accumulator_from_i_updates_flags(
    value: int, iff2: bool, initial_flags: int, expected_flags: int
) -> None:
    cpu = make_cpu(bytes([0xED, 0x57]))  # LD A,I
    cpu.i = value
    cpu.iff2 = iff2
    cpu.f = initial_flags

    assert cpu.step() == 9
    assert cpu.a == value
    assert cpu.f == expected_flags
    assert cpu.pc == 2
    assert cpu.total_cycles == 9


@pytest.mark.parametrize(
    ("initial_r", "iff2", "initial_flags", "expected_a", "expected_flags"),
    [
        (0x05, True, 0x01, 0x07, 0x05),
        (0x7E, False, 0x00, 0x00, 0x40),
        (0xFF, True, 0x02, 0x81, 0x84),
    ],
)
def test_ed_load_accumulator_from_r_includes_opcode_refreshes_and_flags(
    initial_r: int,
    iff2: bool,
    initial_flags: int,
    expected_a: int,
    expected_flags: int,
) -> None:
    cpu = make_cpu(bytes([0xED, 0x5F]))  # LD A,R
    cpu.r = initial_r
    cpu.iff2 = iff2
    cpu.f = initial_flags

    assert cpu.step() == 9
    assert cpu.a == expected_a
    assert cpu.f == expected_flags
    assert cpu.pc == 2
    assert cpu.total_cycles == 9


@pytest.mark.parametrize(
    ("opcode", "pair", "value"),
    [
        (0x43, "bc", 0x1234),
        (0x53, "de", 0x5678),
        (0x63, "hl", 0x9ABC),
        (0x73, "sp", 0xDEF0),
    ],
)
def test_ed_store_register_pair_to_absolute_address_preserves_flags(
    opcode: int, pair: str, value: int
) -> None:
    cpu = make_cpu(bytes([0xED, opcode, 0x20, 0xC0]))
    setattr(cpu, pair, value)
    cpu.f = 0xD7

    assert cpu.step() == 20
    assert cpu.bus.read(0xC020) == (value & 0xFF)
    assert cpu.bus.read(0xC021) == (value >> 8)
    assert getattr(cpu, pair) == value
    assert cpu.f == 0xD7
    assert cpu.pc == 4
    assert cpu.total_cycles == 20


@pytest.mark.parametrize(
    ("opcode", "pair", "value"),
    [
        (0x4B, "bc", 0x1234),
        (0x5B, "de", 0x5678),
        (0x6B, "hl", 0x9ABC),
        (0x7B, "sp", 0xDEF0),
    ],
)
def test_ed_load_register_pair_from_absolute_address_preserves_flags(
    opcode: int, pair: str, value: int
) -> None:
    cpu = make_cpu(bytes([0xED, opcode, 0x20, 0xC0]))
    cpu.bus.write(0xC020, value & 0xFF)
    cpu.bus.write(0xC021, value >> 8)
    cpu.f = 0xD7

    assert cpu.step() == 20
    assert getattr(cpu, pair) == value
    assert cpu.f == 0xD7
    assert cpu.pc == 4
    assert cpu.total_cycles == 20


@pytest.mark.parametrize(
    ("opcode", "pair", "value"),
    [(0x43, "bc", 0x1234), (0x73, "sp", 0xABCD)],
)
def test_ed_store_register_pair_wraps_second_write_at_ffff(
    opcode: int, pair: str, value: int
) -> None:
    class FlatMemoryBus:
        def __init__(self) -> None:
            self.memory = bytearray(0x10000)
            self.memory[0x0100:0x0104] = bytes([0xED, opcode, 0xFF, 0xFF])

        def read(self, address: int) -> int:
            return self.memory[address]

        def write(self, address: int, value: int) -> None:
            self.memory[address] = value

    bus = FlatMemoryBus()
    cpu = Z80Cpu(bus)
    cpu.pc = 0x0100
    setattr(cpu, pair, value)

    assert cpu.step() == 20
    assert bus.memory[0xFFFF] == (value & 0xFF)
    assert bus.memory[0x0000] == (value >> 8)
    assert cpu.pc == 0x0104


@pytest.mark.parametrize(
    ("opcode", "pair", "value"),
    [(0x4B, "bc", 0x1234), (0x7B, "sp", 0xABCD)],
)
def test_ed_load_register_pair_wraps_second_read_at_ffff(
    opcode: int, pair: str, value: int
) -> None:
    class FlatMemoryBus:
        def __init__(self) -> None:
            self.memory = bytearray(0x10000)
            self.memory[0x0100:0x0104] = bytes([0xED, opcode, 0xFF, 0xFF])
            self.memory[0xFFFF] = value & 0xFF
            self.memory[0x0000] = value >> 8

        def read(self, address: int) -> int:
            return self.memory[address]

        def write(self, address: int, value: int) -> None:
            self.memory[address] = value

    cpu = Z80Cpu(FlatMemoryBus())
    cpu.pc = 0x0100
    cpu.f = 0xD7

    assert cpu.step() == 20
    assert getattr(cpu, pair) == value
    assert cpu.f == 0xD7
    assert cpu.pc == 0x0104
    assert cpu.total_cycles == 20


def test_ex_sp_hl_exchanges_word_preserves_flags_and_stack_pointer() -> None:
    cpu = make_cpu(bytes([0xE3]))  # EX (SP),HL
    cpu.sp = 0xC020
    cpu.hl = 0x7012
    cpu.bus.write(0xC020, 0x11)
    cpu.bus.write(0xC021, 0x22)
    cpu.f = 0xD7

    assert cpu.step() == 19
    assert cpu.hl == 0x2211
    assert cpu.bus.read(0xC020) == 0x12
    assert cpu.bus.read(0xC021) == 0x70
    assert cpu.sp == 0xC020
    assert cpu.f == 0xD7
    assert cpu.pc == 1
    assert cpu.total_cycles == 19


def test_ex_sp_hl_wraps_second_address_from_ffff_to_0000() -> None:
    class FlatMemoryBus:
        def __init__(self) -> None:
            self.memory = bytearray(0x10000)
            self.memory[0x0100] = 0xE3
            self.memory[0xFFFF] = 0x34
            self.memory[0x0000] = 0x12

        def read(self, address: int) -> int:
            return self.memory[address]

        def write(self, address: int, value: int) -> None:
            self.memory[address] = value

    bus = FlatMemoryBus()
    cpu = Z80Cpu(bus)
    cpu.pc = 0x0100
    cpu.sp = 0xFFFF
    cpu.hl = 0xABCD
    cpu.f = 0xA5

    assert cpu.step() == 19
    assert cpu.hl == 0x1234
    assert bus.memory[0xFFFF] == 0xCD
    assert bus.memory[0x0000] == 0xAB
    assert cpu.sp == 0xFFFF
    assert cpu.f == 0xA5
    assert cpu.pc == 0x0101
    assert cpu.total_cycles == 19


def test_ex_sp_hl_reads_both_bytes_before_writing_stack() -> None:
    class TracingMemoryBus:
        def __init__(self) -> None:
            self.memory = bytearray(0x10000)
            self.memory[0x0100] = 0xE3
            self.memory[0xC000:0xC002] = bytes([0x34, 0x12])
            self.accesses: list[tuple[str, int, int | None]] = []

        def read(self, address: int) -> int:
            self.accesses.append(("read", address, None))
            return self.memory[address]

        def write(self, address: int, value: int) -> None:
            self.accesses.append(("write", address, value))
            self.memory[address] = value

    bus = TracingMemoryBus()
    cpu = Z80Cpu(bus)
    cpu.pc = 0x0100
    cpu.sp = 0xC000
    cpu.hl = 0xABCD

    assert cpu.step() == 19
    assert cpu.hl == 0x1234
    assert bus.accesses == [
        ("read", 0x0100, None),
        ("read", 0xC000, None),
        ("read", 0xC001, None),
        ("write", 0xC001, 0xAB),
        ("write", 0xC000, 0xCD),
    ]


@pytest.mark.parametrize(
    ("opcode", "initial", "expected", "expected_flags"),
    [
        (0x00, 0x81, 0x03, 0x05),  # RLC B
        (0x08, 0x01, 0x80, 0x85),  # RRC B
        (0x10, 0x80, 0x00, 0x45),  # RL B
        (0x18, 0x01, 0x00, 0x45),  # RR B
        (0x20, 0x80, 0x00, 0x45),  # SLA B
        (0x28, 0x81, 0xC0, 0x84),  # SRA B
        (0x38, 0x01, 0x00, 0x45),  # SRL B
    ],
)
def test_cb_rotate_and_shift_registers(
    opcode: int, initial: int, expected: int, expected_flags: int
) -> None:
    cpu = make_cpu(bytes([0xCB, opcode]))
    cpu.b = initial
    assert cpu.step() == 8
    assert cpu.b == expected
    assert cpu.f == expected_flags
    assert cpu.pc == 2


@pytest.mark.parametrize(
    ("opcode", "initial", "expected"),
    [(0x40, 0x01, 0x00), (0x80, 0xFF, 0xFE), (0xC0, 0x00, 0x01)],
)
def test_cb_bit_res_set_registers(opcode: int, initial: int, expected: int) -> None:
    cpu = make_cpu(bytes([0xCB, opcode]))
    cpu.b = initial
    cpu.f = 0x01
    assert cpu.step() == 8
    if opcode >> 6 == 1:
        assert cpu.b == initial
        assert cpu.f & 0x11 == 0x11
    else:
        assert cpu.b == expected
        assert cpu.f == 0x01


def test_cb_operations_on_hl_memory_have_expected_nominal_cycles() -> None:
    cpu = make_cpu(bytes([0xCB, 0x06, 0xCB, 0x46, 0xCB, 0x86]))
    cpu.hl = 0xC020
    cpu.bus.write(cpu.hl, 0x80)
    assert cpu.step() == 15
    assert cpu.bus.read(cpu.hl) == 0x01
    assert cpu.step() == 12
    assert cpu.step() == 15
    assert cpu.bus.read(cpu.hl) == 0x00


def test_halt_repeats_four_tstates_until_interrupt() -> None:
    cpu = make_cpu(bytes([0x76, 0x00]))
    assert cpu.step() == 4
    assert cpu.halted
    halted_pc = cpu.pc
    assert cpu.step() == 4
    assert cpu.pc == halted_pc
    assert cpu.total_cycles == 8


def test_di_ei_and_nmi_service() -> None:
    cpu = make_cpu(bytes([0xF3, 0xFB, 0x00]))
    cpu.sp = 0xC100
    cpu.iff1 = cpu.iff2 = True
    assert cpu.step() == 4
    assert not cpu.iff1 and not cpu.iff2
    assert cpu.step() == 4
    cpu.step()
    cpu.request_nmi()
    assert cpu.step() == 11
    assert cpu.pc == 0x0066
    assert cpu.sp == 0xC0FE
