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
