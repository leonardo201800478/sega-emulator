"""Validação exaustiva das formas imediatas da ALU Z80 de 8 bits.

O oráculo calcula resultado e flags sem chamar helpers internos da CPU. Os
testes cobrem despacho, consumo do operando, PC e T-states nominais; não
substituem comparação com hardware real ou exercitador independente.
"""

from sega_emulator.bus.memory import RAM_SIZE, SG1000Ram
from sega_emulator.bus.memory_bus import SG1000MemoryBus
from sega_emulator.cpu.z80 import Z80Cpu


def _expected(
    operation: str, accumulator: int, operand: int, carry: int
) -> tuple[int, int]:
    if operation in ("add", "adc"):
        carry_in = carry if operation == "adc" else 0
        raw = accumulator + operand + carry_in
        result = raw & 0xFF
        flags = result & 0xA8
        if result == 0:
            flags |= 0x40
        if (accumulator & 0x0F) + (operand & 0x0F) + carry_in > 0x0F:
            flags |= 0x10
        if (~(accumulator ^ operand) & (accumulator ^ result) & 0x80) != 0:
            flags |= 0x04
        if raw > 0xFF:
            flags |= 0x01
        return result, flags

    if operation in ("sub", "sbc", "cp"):
        carry_in = carry if operation == "sbc" else 0
        raw = accumulator - operand - carry_in
        result = raw & 0xFF
        flags = (result & 0xA8) | 0x02
        if result == 0:
            flags |= 0x40
        if (accumulator & 0x0F) < ((operand & 0x0F) + carry_in):
            flags |= 0x10
        if ((accumulator ^ operand) & (accumulator ^ result) & 0x80) != 0:
            flags |= 0x04
        if raw < 0:
            flags |= 0x01
        if operation == "cp":
            flags = (flags & ~0x28) | (operand & 0x28)
        return result, flags

    if operation == "and":
        result = accumulator & operand
        half_carry = 0x10
    elif operation == "xor":
        result = accumulator ^ operand
        half_carry = 0
    else:
        result = accumulator | operand
        half_carry = 0

    flags = (result & 0xA8) | half_carry
    if result == 0:
        flags |= 0x40
    if result.bit_count() % 2 == 0:
        flags |= 0x04
    return result, flags


def test_immediate_alu_results_flags_pc_and_cycles_for_all_byte_pairs() -> None:
    """Percorre todos os pares A/n para as oito operações ALU imediatas."""
    operations = (
        ("add", 0xC6, False),
        ("adc", 0xCE, True),
        ("sub", 0xD6, False),
        ("sbc", 0xDE, True),
        ("and", 0xE6, False),
        ("xor", 0xEE, False),
        ("or", 0xF6, False),
        ("cp", 0xFE, False),
    )
    ram = SG1000Ram(bytes(RAM_SIZE))

    for operation, opcode, uses_carry in operations:
        program = bytes(byte for operand in range(0x100) for byte in (opcode, operand))
        cpu = Z80Cpu(SG1000MemoryBus(ram, program))
        carry_values = (0, 1) if uses_carry else (0,)

        for operand in range(0x100):
            instruction_address = operand * 2
            for accumulator in range(0x100):
                for carry in carry_values:
                    cpu.a = accumulator
                    cpu.f = carry
                    cpu.pc = instruction_address
                    cpu.r = 0
                    cpu.total_cycles = 0
                    expected_result, expected_flags = _expected(
                        operation, accumulator, operand, carry
                    )

                    assert cpu.step() == 7, (
                        f"{operation.upper()} immediate cycles "
                        f"A={accumulator:02X} n={operand:02X} C={carry}"
                    )
                    if operation == "cp":
                        assert cpu.a == accumulator, (
                            f"CP immediate alterou A={accumulator:02X} "
                            f"n={operand:02X}"
                        )
                    else:
                        assert cpu.a == expected_result, (
                            f"{operation.upper()} immediate "
                            f"A={accumulator:02X} n={operand:02X} C={carry}"
                        )
                    assert cpu.f == expected_flags, (
                        f"{operation.upper()} immediate flags "
                        f"A={accumulator:02X} n={operand:02X} C={carry}: "
                        f"obtido {cpu.f:02X}, esperado {expected_flags:02X}"
                    )
                    assert cpu.pc == instruction_address + 2
                    assert cpu.r == 1
                    assert cpu.total_cycles == 7
