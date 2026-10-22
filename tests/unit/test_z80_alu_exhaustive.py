"""Validação exaustiva das flags das operações ALU de 8 bits do Z80.

Os resultados esperados são calculados por fórmulas de referência independentes
dos helpers internos da CPU. Isso valida a semântica implementada, mas não
substitui comparação com hardware real ou um exerciser independente.
"""

from sega_emulator.bus.memory import RAM_SIZE, SG1000Ram
from sega_emulator.bus.memory_bus import SG1000MemoryBus
from sega_emulator.cpu.z80 import Z80Cpu


def _expected_alu(operation: str, accumulator: int, operand: int, carry: int) -> tuple[int, int]:
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
