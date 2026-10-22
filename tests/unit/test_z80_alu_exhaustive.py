"""Validação exaustiva das flags das operações ALU de 8 bits do Z80.

Os resultados esperados são calculados por fórmulas de referência independentes
dos helpers internos da CPU. Isso valida a semântica implementada, mas não
substitui comparação com hardware real ou um exerciser independente.
"""

from sega_emulator.bus.memory import RAM_SIZE, SG1000Ram
from sega_emulator.bus.memory_bus import SG1000MemoryBus
from sega_emulator.cpu.z80 import Z80Cpu


def _expected_alu(
    operation: str, accumulator: int, operand: int, carry: int
) -> tuple[int, int]:
    if operation in ("add", "adc"):
        carry_in = carry if operation == "adc" else 0
        total = accumulator + operand + carry_in
        result = total & 0xFF
        flags = result & 0xA8
        if result == 0:
            flags |= 0x40
        if (accumulator & 0x0F) + (operand & 0x0F) + carry_in > 0x0F:
            flags |= 0x10
        if (~(accumulator ^ operand) & (accumulator ^ result) & 0x80) != 0:
            flags |= 0x04
        if total > 0xFF:
            flags |= 0x01
        return result, flags

    carry_in = carry if operation == "sbc" else 0
    raw_result = accumulator - operand - carry_in
    result = raw_result & 0xFF
    flags = (result & 0xA8) | 0x02
    if result == 0:
        flags |= 0x40
    if (accumulator & 0x0F) < ((operand & 0x0F) + carry_in):
        flags |= 0x10
    if ((accumulator ^ operand) & (accumulator ^ result) & 0x80) != 0:
        flags |= 0x04
    if raw_result < 0:
        flags |= 0x01
    if operation == "cp":
        flags = (flags & ~0x28) | (operand & 0x28)
    return result, flags


def test_8bit_alu_results_and_flags_for_all_operand_combinations() -> None:
    """Verifica ADD/ADC/SUB/SBC/CP para todos os pares de bytes e carry aplicável."""
    operations = (
        ("add", 0x80, False),
        ("adc", 0x88, True),
        ("sub", 0x90, False),
        ("sbc", 0x98, True),
        ("cp", 0xB8, False),
    )
    ram = SG1000Ram(bytes(RAM_SIZE))

    for operation, opcode, uses_carry in operations:
        cpu = Z80Cpu(SG1000MemoryBus(ram, bytes([opcode])))
        carry_values = (0, 1) if uses_carry else (0,)
        for accumulator in range(0x100):
            for operand in range(0x100):
                for carry in carry_values:
                    cpu.a = accumulator
                    cpu.b = operand
                    cpu.f = carry
                    cpu.pc = 0
                    expected_result, expected_flags = _expected_alu(
                        operation, accumulator, operand, carry
                    )

                    assert cpu.step() == 4, (
                        f"{operation.upper()} cycles A={accumulator:02X} "
                        f"B={operand:02X} C={carry}"
                    )
                    if operation == "cp":
                        assert cpu.a == accumulator, (
                            f"CP alterou A={accumulator:02X} B={operand:02X}"
                        )
                    else:
                        assert cpu.a == expected_result, (
                            f"{operation.upper()} A={accumulator:02X} "
                            f"B={operand:02X} C={carry}"
                        )
                    assert cpu.f == expected_flags, (
                        f"{operation.upper()} flags A={accumulator:02X} "
                        f"B={operand:02X} C={carry}: "
                        f"obtido {cpu.f:02X}, esperado {expected_flags:02X}"
                    )


def test_logic_register_operations_for_all_operand_combinations() -> None:
    """Valida AND/XOR/OR para todos os pares A,B e suas flags."""
    operations = (("and", 0xA0), ("xor", 0xA8), ("or", 0xB0))
    ram = SG1000Ram(bytes(RAM_SIZE))

    for operation, opcode in operations:
        cpu = Z80Cpu(SG1000MemoryBus(ram, bytes([opcode])))
        for accumulator in range(0x100):
            for operand in range(0x100):
                if operation == "and":
                    result = accumulator & operand
                elif operation == "xor":
                    result = accumulator ^ operand
                else:
                    result = accumulator | operand

                expected_flags = result & 0xA8
                if result == 0:
                    expected_flags |= 0x40
                if result.bit_count() % 2 == 0:
                    expected_flags |= 0x04
                if operation == "and":
                    expected_flags |= 0x10

                cpu.a = accumulator
                cpu.b = operand
                cpu.f = 0xFF
                cpu.pc = 0
                assert cpu.step() == 4
                assert cpu.a == result, (
                    f"{operation.upper()} A={accumulator:02X} B={operand:02X}"
                )
                assert cpu.f == expected_flags, (
                    f"{operation.upper()} flags A={accumulator:02X} "
                    f"B={operand:02X}: obtido {cpu.f:02X}, "
                    f"esperado {expected_flags:02X}"
                )


def test_inc_dec_register_flags_for_all_byte_values_and_carry_states() -> None:
    """Valida INC B e DEC B em todo o domínio de 8 bits, preservando apenas C."""
    ram = SG1000Ram(bytes(RAM_SIZE))

    for operation, opcode in (("inc", 0x04), ("dec", 0x05)):
        cpu = Z80Cpu(SG1000MemoryBus(ram, bytes([opcode])))
        for value in range(0x100):
            for carry in (0, 1):
                if operation == "inc":
                    result = (value + 1) & 0xFF
                    expected_flags = (result & 0xA8) | carry
                    if result == 0:
                        expected_flags |= 0x40
                    if (value & 0x0F) == 0x0F:
                        expected_flags |= 0x10
                    if value == 0x7F:
                        expected_flags |= 0x04
                else:
                    result = (value - 1) & 0xFF
                    expected_flags = (result & 0xA8) | 0x02 | carry
                    if result == 0:
                        expected_flags |= 0x40
                    if (value & 0x0F) == 0:
                        expected_flags |= 0x10
                    if value == 0x80:
                        expected_flags |= 0x04

                cpu.b = value
                cpu.f = 0xFE | carry
                cpu.pc = 0
                assert cpu.step() == 4
                assert cpu.b == result, f"{operation.upper()} B={value:02X} C={carry}"
                assert cpu.f == expected_flags, (
                    f"{operation.upper()} flags B={value:02X} C={carry}: "
                    f"obtido {cpu.f:02X}, esperado {expected_flags:02X}"
                )


def test_16bit_add_adc_sbc_results_flags_and_cycles_exhaustively() -> None:
    """Percorre todos os pares de words em ADD HL,BC/ADC HL,BC/SBC HL,BC."""
    ram = SG1000Ram(bytes(RAM_SIZE))
    operations = (
        ("add", bytes([0x09]), False, 11),
        ("adc", bytes([0xED, 0x4A]), True, 15),
        ("sbc", bytes([0xED, 0x42]), True, 15),
    )

    for operation, program, uses_carry, expected_cycles in operations:
        cpu = Z80Cpu(SG1000MemoryBus(ram, program))
        carry_values = (0, 1) if uses_carry else (0,)
        for left in range(0x10000):
            for right in range(0x10000):
                for carry in carry_values:
                    old_flags = ((left >> 8) & 0xC4) | carry
                    if operation == "add":
                        raw_result = left + right
                        result = raw_result & 0xFFFF
                        expected_flags = old_flags & 0xC4
                        expected_flags |= (result >> 8) & 0x28
                        if (left & 0x0FFF) + (right & 0x0FFF) > 0x0FFF:
                            expected_flags |= 0x10
                        if raw_result > 0xFFFF:
                            expected_flags |= 0x01
                    elif operation == "adc":
                        raw_result = left + right + carry
                        result = raw_result & 0xFFFF
                        expected_flags = (result >> 8) & 0xA8
                        if result == 0:
                            expected_flags |= 0x40
                        if (left & 0x0FFF) + (right & 0x0FFF) + carry > 0x0FFF:
                            expected_flags |= 0x10
                        if (~(left ^ right) & (left ^ result) & 0x8000) != 0:
                            expected_flags |= 0x04
                        if raw_result > 0xFFFF:
                            expected_flags |= 0x01
                    else:
                        raw_result = left - right - carry
                        result = raw_result & 0xFFFF
                        expected_flags = ((result >> 8) & 0xA8) | 0x02
                        if result == 0:
                            expected_flags |= 0x40
                        if (left & 0x0FFF) < ((right & 0x0FFF) + carry):
                            expected_flags |= 0x10
                        if ((left ^ right) & (left ^ result) & 0x8000) != 0:
                            expected_flags |= 0x04
                        if raw_result < 0:
                            expected_flags |= 0x01

                    cpu.hl = left
                    cpu.bc = right
                    cpu.f = old_flags
                    cpu.pc = 0
                    assert cpu.step() == expected_cycles, (
                        f"{operation.upper()} cycles HL={left:04X} "
                        f"BC={right:04X} C={carry}"
                    )
                    assert cpu.hl == result, (
                        f"{operation.upper()} HL={left:04X} BC={right:04X} C={carry}"
                    )
                    assert cpu.f == expected_flags, (
                        f"{operation.upper()} flags HL={left:04X} "
                        f"BC={right:04X} C={carry}: "
                        f"obtido {cpu.f:02X}, esperado {expected_flags:02X}"
                    )
