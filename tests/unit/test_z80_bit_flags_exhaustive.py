"""Varredura exaustiva das flags da instrução BIT no prefixo CB."""

from sega_emulator.cpu.z80 import Z80Cpu


class FlatMemoryBus:
    """Barramento de 64 KiB para testes isolados da CPU."""

    def __init__(self) -> None:
        self.memory = bytearray(0x10000)

    def read(self, address: int) -> int:
        return self.memory[address & 0xFFFF]

    def write(self, address: int, value: int) -> None:
        self.memory[address & 0xFFFF] = value & 0xFF


def _expected_bit_flags(value: int, bit_index: int, carry: int, xy_source: int) -> int:
    """Calcula flags de BIT e aplica a hipótese testada para os bits X/Y."""
    flags = 0x10 | carry | (xy_source & 0x28)
    bit_is_clear = not (value & (1 << bit_index))
    if bit_is_clear:
        flags |= 0x44  # Z e P/V
    if bit_index == 7 and not bit_is_clear:
        flags |= 0x80  # S só é afetado por BIT 7
    return flags


def test_bit_register_flags_for_all_values_bits_and_carry_states() -> None:
    """Testa 2.048 estados em B, incluindo flags, PC, R e ciclos nominais."""
    bus = FlatMemoryBus()
    bus.memory[0:2] = bytes((0xCB, 0x40))
    for value in range(0x100):
        for bit_index in range(8):
            for carry in (0, 1):
                bus.memory[1] = 0x40 | (bit_index << 3)
                cpu = Z80Cpu(bus)
                cpu.b = value
                cpu.f = 0xFE | carry
                cpu.pc = 0
                cpu.r = 0
                cpu.total_cycles = 0

                assert cpu.step() == 8
                assert cpu.f == _expected_bit_flags(value, bit_index, carry, value)
                assert cpu.b == value
                assert cpu.pc == 2
                assert cpu.r == 2
                assert cpu.total_cycles == 8


def test_bit_hl_flags_for_all_values_bits_carry_and_wz_patterns() -> None:
    """Testa 8.192 estados de BIT (HL), variando X/Y via WZ/MEMPTR."""
    address = 0xC040
    for wz_high in (0x00, 0x28, 0xA8, 0xD7):
        xy_source = wz_high
        bus = FlatMemoryBus()
        bus.memory[0:2] = bytes((0xCB, 0x46))
        for value in range(0x100):
            for bit_index in range(8):
                for carry in (0, 1):
                    bus.memory[1] = 0x46 | (bit_index << 3)
                    bus.memory[address] = value
                    cpu = Z80Cpu(bus)
                    cpu.hl = address
                    cpu.wz = wz_high << 8
                    cpu.f = 0xFE | carry
                    cpu.pc = 0
                    cpu.r = 0
                    cpu.total_cycles = 0

                    assert cpu.step() == 12
                    assert cpu.f == _expected_bit_flags(
                        value, bit_index, carry, xy_source
                    )
                    assert bus.memory[address] == value
                    assert cpu.pc == 2
                    assert cpu.r == 2
                    assert cpu.total_cycles == 12
