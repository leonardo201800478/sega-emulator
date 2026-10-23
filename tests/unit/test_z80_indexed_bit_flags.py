"""Validação de BIT b,(IX+d)/(IY+d), incluindo flags X/Y."""

from sega_emulator.cpu.z80 import Z80Cpu


class FlatMemoryBus:
    """Barramento de 64 KiB para testes isolados da CPU."""

    def __init__(self) -> None:
        self.memory = bytearray(0x10000)

    def read(self, address: int) -> int:
        return self.memory[address & 0xFFFF]

    def write(self, address: int, value: int) -> None:
        self.memory[address & 0xFFFF] = value & 0xFF


def _expected_bit_flags(value: int, bit_index: int, carry: int, address: int) -> int:
    """Calcula flags documentadas e X/Y a partir do endereço efetivo."""
    flags = 0x10 | carry | ((address >> 8) & 0x28)
    bit_is_clear = not (value & (1 << bit_index))
    if bit_is_clear:
        flags |= 0x44
    if bit_index == 7 and not bit_is_clear:
        flags |= 0x80
    return flags


def test_indexed_bit_flags_for_all_values_bits_carry_and_xy_patterns() -> None:
    """Testa 32.768 estados em IX/IY com quatro padrões do byte alto."""
    for prefix, index_name in ((0xDD, "ix"), (0xFD, "iy")):
        for address_high in (0x00, 0x28, 0xA8, 0xD7):
            address = (address_high << 8) | 0x40
            bus = FlatMemoryBus()
            bus.memory[0:4] = bytes((prefix, 0xCB, 0x00, 0x46))
            for value in range(0x100):
                for bit_index in range(8):
                    for carry in (0, 1):
                        bus.memory[3] = 0x46 | (bit_index << 3)
                        bus.memory[address] = value
                        cpu = Z80Cpu(bus)
                        setattr(cpu, index_name, address)
                        cpu.f = 0xFE | carry
                        cpu.pc = 0
                        cpu.r = 0
                        cpu.total_cycles = 0

                        assert cpu.step() == 20
                        assert cpu.f == _expected_bit_flags(
                            value, bit_index, carry, address
                        )
                        assert bus.memory[address] == value
                        assert cpu.pc == 4
                        assert cpu.r == 2
                        assert cpu.total_cycles == 20


def test_indexed_bit_uses_signed_displacement_and_wraps_address() -> None:
    """Verifica deslocamentos -1/+1, wrap de 16 bits e flags X/Y."""
    cases = (
        (0xDD, "ix", 0xC000, 0xFF, 0xBFFF),
        (0xFD, "iy", 0xFFFF, 0x01, 0x0000),
    )
    for prefix, index_name, index, displacement, address in cases:
        bus = FlatMemoryBus()
        instruction_address = 0x1000
        bus.memory[instruction_address : instruction_address + 4] = bytes(
            (prefix, 0xCB, displacement, 0x46)
        )
        bus.memory[address] = 0x01
        cpu = Z80Cpu(bus)
        setattr(cpu, index_name, index)
        cpu.f = 0x01
        cpu.pc = instruction_address
        cpu.r = 0
        cpu.total_cycles = 0

        assert cpu.step() == 20
        assert cpu.f == _expected_bit_flags(0x01, 0, 1, address)
        assert cpu.pc == instruction_address + 4
        assert cpu.r == 2
        assert cpu.total_cycles == 20


def test_indexed_bit_interprets_all_signed_displacements() -> None:
    """Valida os 256 bytes de deslocamento para IX e IY."""
    instruction_address = 0x1000
    for prefix, index_name in ((0xDD, "ix"), (0xFD, "iy")):
        for address_high in (0x00, 0x28, 0xA8, 0xD7):
            address = (address_high << 8) | 0x40
            for displacement in range(0x100):
                signed_displacement = (
                    displacement - 0x100 if displacement & 0x80 else displacement
                )
                index = (address - signed_displacement) & 0xFFFF
                bus = FlatMemoryBus()
                bus.memory[instruction_address : instruction_address + 4] = bytes(
                    (prefix, 0xCB, displacement, 0x7E)
                )
                bus.memory[address] = 0x80
                cpu = Z80Cpu(bus)
                setattr(cpu, index_name, index)
                cpu.f = 0x01
                cpu.pc = instruction_address
                cpu.r = 0
                cpu.total_cycles = 0

                assert cpu.step() == 20
                assert cpu.f == _expected_bit_flags(0x80, 7, 1, address)
                assert bus.memory[address] == 0x80
                assert cpu.pc == instruction_address + 4
                assert cpu.r == 2
                assert cpu.total_cycles == 20

