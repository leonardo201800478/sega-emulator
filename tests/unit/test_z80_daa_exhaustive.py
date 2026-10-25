"""Validação exaustiva de DAA para todas as combinações de A, N, H e C.

O oráculo implementa a regra de ajuste decimal a partir do estado de entrada,
sem chamar o helper da CPU. Os casos sintéticos não substituem comparação com
hardware real ou um exercitador Z80 independente.
"""

from sega_emulator.bus.memory import RAM_SIZE, SG1000Ram
from sega_emulator.bus.memory_bus import SG1000MemoryBus
from sega_emulator.cpu.z80 import Z80Cpu


def _expected_daa(
    accumulator: int, subtract: int, half_carry: int, carry: int
) -> tuple[int, int]:
    correction = 0
    if half_carry or (accumulator & 0x0F) > 9:
        correction |= 0x06
    if carry or accumulator > 0x99:
        correction |= 0x60

    if subtract:
        result = (accumulator - correction) & 0xFF
    else:
        result = (accumulator + correction) & 0xFF

    carry_out = carry or (not subtract and accumulator > 0x99)
    flags = (result & 0xA8) | (0x02 if subtract else 0)
    if result == 0:
        flags |= 0x40
    if result.bit_count() % 2 == 0:
        flags |= 0x04
    if (accumulator ^ result) & 0x10:
        flags |= 0x10
    if carry_out:
        flags |= 0x01
    return result, flags


def test_daa_subtract_applies_a_thresholds_when_h_and_c_are_clear() -> None:
    ram = SG1000Ram(bytes(RAM_SIZE))
    cpu = Z80Cpu(SG1000MemoryBus(ram, bytes([0x27])))
    cpu.a = 0x9C
    cpu.f = 0x22  # N=1, H=0, C=0

    assert cpu.step() == 4
    assert (cpu.a, cpu.f, cpu.q) == (0x36, 0x27, 0x27)


def test_daa_results_flags_pc_r_and_cycles_for_all_input_combinations() -> None:
    """Verifica os 2.048 estados de entrada A/N/H/C da instrução DAA."""
    ram = SG1000Ram(bytes(RAM_SIZE))
    cpu = Z80Cpu(SG1000MemoryBus(ram, bytes([0x27])))

    for accumulator in range(0x100):
        for subtract in range(2):
            for half_carry in range(2):
                for carry in range(2):
                    # Semântica DAA depende de N/H/C; os outros flags de entrada
                    # são deliberadamente definidos para confirmar que são refeitos.
                    cpu.a = accumulator
                    cpu.f = 0xEC | (subtract << 1) | (half_carry << 4) | carry
                    cpu.pc = 0
                    cpu.r = 0xFE
                    cpu.total_cycles = 0
                    expected_a, expected_f = _expected_daa(
                        accumulator, subtract, half_carry, carry
                    )

                    assert cpu.step() == 4, (
                        f"DAA cycles A={accumulator:02X} N={subtract} "
                        f"H={half_carry} C={carry}"
                    )
                    assert cpu.a == expected_a, (
                        f"DAA A A={accumulator:02X} N={subtract} "
                        f"H={half_carry} C={carry}: "
                        f"obtido {cpu.a:02X}, esperado {expected_a:02X}"
                    )
                    assert cpu.f == expected_f, (
                        f"DAA flags A={accumulator:02X} N={subtract} "
                        f"H={half_carry} C={carry}: "
                        f"obtido {cpu.f:02X}, esperado {expected_f:02X}"
                    )
                    assert cpu.pc == 1
                    assert cpu.r == 0xFF
                    assert cpu.total_cycles == 4
