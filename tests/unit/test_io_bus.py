"""Testes do roteador de portas de I/O."""

import pytest

from sega_emulator.bus.io_bus import SG1000IOBus, UnmappedIOError
from sega_emulator.machine.sg1000 import create_memory_system


def test_unmapped_io_port_raises() -> None:
    bus = SG1000IOBus()
    with pytest.raises(UnmappedIOError):
        bus.read_port(0x00BE)


def test_io_bus_routes_vdp_data_port() -> None:
    system = create_memory_system()
    system.vdp.write_port(0xBF, 0x00)
    system.vdp.write_port(0xBF, 0x40)
    system.io_bus.write_port(0x00BE, 0xA5)
    assert system.vram.read(0) == 0xA5


def test_io_bus_uses_low_byte_of_z80_port_address() -> None:
    system = create_memory_system()
    system.vdp.write_port(0xBF, 0x34)
    system.vdp.write_port(0xBF, 0x40)
    system.io_bus.write_port(0x12BE, 0x5A)
    assert system.vram.read(0x34) == 0x5A
