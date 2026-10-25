"""Testes unitários da RAM do SG-1000."""

import pytest

from sega_emulator.bus.memory import RAM_SIZE, SG1000Ram

@pytest.fixture
def ram() -> SG1000Ram:
    """Fornece RAM com estado inicial determinístico."""
    return SG1000Ram(bytes(RAM_SIZE))

def test_ram_has_one_kibibyte_of_physical_storage() -> None:
    ram = SG1000Ram(bytes(RAM_SIZE))

    assert len(ram._data) == RAM_SIZE

def test_ram_reads_and_writes_bytes(ram: SG1000Ram) -> None:
    ram.write(0xC000, 0xA5)

    assert ram.read(0xC000) == 0xA5

def test_ram_mirrors_every_1024_bytes(ram: SG1000Ram) -> None:
    ram.write(0xC000, 0x12)

    assert ram.read(0xC400) == 0x12
    assert ram.read(0xC800) == 0x12
    assert ram.read(0xCC00) == 0x12
    assert ram.read(0xFFFF) == 0

def test_last_ram_byte_is_mirrored(ram: SG1000Ram) -> None:
    ram.write(0xFFFF, 0xAB)

    assert ram.read(0xC3FF) == 0xAB
    assert ram.read(0xFFFF) == 0xAB

def test_ram_rejects_incorrect_initial_size() -> None:
    with pytest.raises(ValueError):
        SG1000Ram(bytes(RAM_SIZE - 1))

@pytest.mark.parametrize("address", [0x0000, 0xBFFF])
def test_ram_rejects_addresses_outside_ram(address: int) -> None:
    ram = SG1000Ram(bytes(RAM_SIZE))

    with pytest.raises(ValueError):
        ram.read(address)

@pytest.mark.parametrize("value", [-1, 0x100])
def test_ram_rejects_values_outside_byte_range(ram: SG1000Ram, value: int) -> None:
    with pytest.raises(ValueError):
        ram.write(0xC000, value)

def test_ram_rejects_non_integer_address(ram: SG1000Ram) -> None:
    with pytest.raises(TypeError):
        ram.read("0xC000")  # type: ignore[arg-type]

def test_ram_rejects_non_integer_value(ram: SG1000Ram) -> None:
    with pytest.raises(TypeError):
        ram.write(0xC000, "255")  # type: ignore[arg-type]

def test_every_address_in_ram_window_maps_to_low_ten_bits(ram: SG1000Ram) -> None:
    """Os 16 KiB do intervalo usam apenas os dez bits inferiores."""
    for address in range(0xC000, 0x10000):
        assert ram._offset(address) == (address & 0x03FF)

def test_all_ram_mirrors_share_the_same_physical_byte(ram: SG1000Ram) -> None:
    """Uma escrita em qualquer espelho deve ser visível em todos os outros."""
    for offset in range(RAM_SIZE):
        address = 0xC000 + offset
        value = (offset * 37 + 0x5A) & 0xFF
        ram.write(address, value)

        for mirror in range(0xC000 + offset, 0x10000, RAM_SIZE):
            assert ram.read(mirror) == value

def test_ram_write_does_not_alias_adjacent_physical_bytes(ram: SG1000Ram) -> None:
    ram.write(0xC000, 0x11)
    ram.write(0xC001, 0x22)
    ram.write(0xC3FF, 0x33)

    assert ram.read(0xC000) == 0x11
    assert ram.read(0xC001) == 0x22
    assert ram.read(0xC3FF) == 0x33
    assert ram.read(0xC400) == 0x11

def test_ram_rejects_addresses_above_16_bit_range(ram: SG1000Ram) -> None:
    for address in (-1, 0x10000, 0x1C000):
        with pytest.raises(ValueError):
            ram.read(address)

def test_ram_expansion_has_eight_kibibytes_and_mirrors_at_e000() -> None:
    ram = SG1000Ram(bytes(0x2000))

    assert ram.size == 0x2000
    ram.write(0xC000, 0xA5)
    ram.write(0xDFFF, 0x5A)

    assert ram.read(0xC000) == 0xA5
    assert ram.read(0xE000) == 0xA5
    assert ram.read(0xDFFF) == 0x5A
    assert ram.read(0xFFFF) == 0x5A
    assert ram.read(0xC800) == 0

def test_ram_rejects_unsupported_physical_sizes() -> None:
    with pytest.raises(ValueError):
        SG1000Ram(bytes(0x0800))
    with pytest.raises(ValueError):
        SG1000Ram(bytes(0x4000))
