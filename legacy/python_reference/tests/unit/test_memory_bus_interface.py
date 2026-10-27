"""Testes do contrato de barramento consumido por componentes."""

from sega_emulator.bus.interfaces import MemoryBus
from sega_emulator.bus.memory import RAM_SIZE, SG1000Ram
from sega_emulator.bus.memory_bus import SG1000MemoryBus


def _write_then_read(bus: MemoryBus, address: int, value: int) -> int:
    """Simula um consumidor que depende somente do contrato público."""
    bus.write(address, value)
    return bus.read(address)


def test_memory_bus_implements_minimum_byte_access_contract() -> None:
    ram = SG1000Ram(bytes(RAM_SIZE))
    bus = SG1000MemoryBus(ram)

    assert _write_then_read(bus, 0xC123, 0xA5) == 0xA5
