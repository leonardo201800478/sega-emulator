"""Testes do caminho CPU–VDP–VRAM, sem alegar renderização completa."""

from sega_emulator.machine.sg1000 import create_memory_system
from sega_emulator.video.tms9918 import VDP_CONTROL_PORT, VDP_DATA_PORT


def set_vdp_address(system, address: int, *, write: bool) -> None:
    system.io_bus.write_port(VDP_CONTROL_PORT, address & 0xFF)
    command = ((address >> 8) & 0x3F) | (0x40 if write else 0)
    system.io_bus.write_port(VDP_CONTROL_PORT, command)


def test_cpu_io_write_reaches_vram_and_increments_address() -> None:
    system = create_memory_system()
    set_vdp_address(system, 0x1234, write=True)
    system.io_bus.write_port(VDP_DATA_PORT, 0xA1)
    system.io_bus.write_port(VDP_DATA_PORT, 0xB2)

    assert system.vram.read(0x1234) == 0xA1
    assert system.vram.read(0x1235) == 0xB2
    assert system.vdp.address == 0x1236


def test_vdp_read_command_uses_buffered_vram_access() -> None:
    system = create_memory_system()
    system.vram.write(0x2345, 0x67)
    system.vram.write(0x2346, 0x89)
    set_vdp_address(system, 0x2345, write=False)

    assert system.io_bus.read_port(VDP_DATA_PORT) == 0x67
    assert system.io_bus.read_port(VDP_DATA_PORT) == 0x89


def test_register_write_uses_two_byte_control_sequence() -> None:
    system = create_memory_system()
    system.io_bus.write_port(VDP_CONTROL_PORT, 0x02)
    system.io_bus.write_port(VDP_CONTROL_PORT, 0x81)

    assert system.vdp.registers[1] == 0x02


def test_status_read_clears_vblank_and_pending_control_latch() -> None:
    system = create_memory_system()
    system.vdp.set_vblank(True)
    system.io_bus.write_port(VDP_CONTROL_PORT, 0x12)

    assert system.io_bus.read_port(VDP_CONTROL_PORT) & 0x80
    assert not (system.vdp.status & 0x80)
    assert system.vdp._control_latch is None


def test_address_wraps_at_16_kibibytes() -> None:
    system = create_memory_system()
    set_vdp_address(system, 0x3FFF, write=True)
    system.io_bus.write_port(VDP_DATA_PORT, 0xCC)
    assert system.vram.read(0x3FFF) == 0xCC
    assert system.vdp.address == 0
