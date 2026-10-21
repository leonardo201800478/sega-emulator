"""Testes unitários do barramento de memória do SG-1000."""

import pytest

from sega_emulator.bus.memory import RAM_SIZE, SG1000Ram
from sega_emulator.bus.memory_bus import (
    ReadOnlyMemoryError,
    SG1000MemoryBus,
    UnmappedMemoryError,
)


@pytest.fixture
def ram() -> SG1000Ram:
    """Fornece RAM com estado inicial determinístico."""
    return SG1000Ram(bytes(RAM_SIZE))


def test_bus_reads_cartridge_rom_without_modifying_it(ram: SG1000Ram) -> None:
    rom = bytes([0x3E, 0x42, 0x00])
    bus = SG1000MemoryBus(ram, rom)

    assert bus.read(0x0000) == 0x3E
    assert bus.read(0x0001) == 0x42
    assert bus.read(0x0002) == 0x00


def test_bus_rejects_writes_to_mapped_rom(ram: SG1000Ram) -> None:
    bus = SG1000MemoryBus(ram, b"\x00\x01")

    with pytest.raises(ReadOnlyMemoryError):
        bus.write(0x0001, 0xFF)

    assert bus.read(0x0001) == 0x01


def test_bus_routes_reads_and_writes_to_internal_ram(ram: SG1000Ram) -> None:
    bus = SG1000MemoryBus(ram)

    bus.write(0xC123, 0xA5)

    assert bus.read(0xC123) == 0xA5
    assert bus.read(0xC523) == 0xA5


def test_bus_rejects_access_without_a_cartridge(ram: SG1000Ram) -> None:
    bus = SG1000MemoryBus(ram)

    with pytest.raises(UnmappedMemoryError):
        bus.read(0x0000)

    with pytest.raises(UnmappedMemoryError):
        bus.write(0x0000, 0x12)


def test_bus_rejects_unmapped_gap_after_rom_image(ram: SG1000Ram) -> None:
    bus = SG1000MemoryBus(ram, b"\xAA\xBB")

    with pytest.raises(UnmappedMemoryError):
        bus.read(0x0002)

    with pytest.raises(UnmappedMemoryError):
        bus.write(0x0002, 0x12)


@pytest.mark.parametrize("size", [0, 0xC001])
def test_bus_rejects_invalid_rom_size(ram: SG1000Ram, size: int) -> None:
    with pytest.raises(ValueError):
        SG1000MemoryBus(ram, bytes(size))


def test_bus_rejects_non_bytes_rom(ram: SG1000Ram) -> None:
    with pytest.raises(TypeError):
        SG1000MemoryBus(ram, bytearray(b"ROM"))  # type: ignore[arg-type]


@pytest.mark.parametrize("address", [-1, 0x10000])
def test_bus_rejects_out_of_range_addresses(
    ram: SG1000Ram, address: int
) -> None:
    bus = SG1000MemoryBus(ram, b"ROM")

    with pytest.raises(ValueError):
        bus.read(address)


def test_bus_rejects_non_integer_address(ram: SG1000Ram) -> None:
    bus = SG1000MemoryBus(ram, b"ROM")

    with pytest.raises(TypeError):
        bus.read("0x0000")  # type: ignore[arg-type]


@pytest.mark.parametrize("value", [-1, 0x100])
def test_bus_rejects_values_outside_byte_range(
    ram: SG1000Ram, value: int
) -> None:
    bus = SG1000MemoryBus(ram)

    with pytest.raises(ValueError):
        bus.write(0xC000, value)


def test_bus_rejects_non_integer_value(ram: SG1000Ram) -> None:
    bus = SG1000MemoryBus(ram)

    with pytest.raises(TypeError):
        bus.write(0xC000, "255")  # type: ignore[arg-type]
