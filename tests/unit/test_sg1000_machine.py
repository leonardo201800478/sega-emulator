"""Testes de integração iniciais da máquina SG-1000."""

import pytest

from sega_emulator.machine.sg1000 import SG1000Machine


def test_machine_executes_cartridge_code_and_advances_a_frame() -> None:
    # JP 0000h: ROM homebrew sintética para validar o laço de emulação.
    machine = SG1000Machine(bytes((0xC3, 0x00, 0x00)))

    steps = machine.run_frame()

    assert steps > 0
    assert machine.vdp.frame_number == 1
    assert machine.cpu.pc == 0
    assert machine.cpu.total_cycles >= 228 * 262


def test_machine_mirrors_small_cartridge_and_maps_joysticks() -> None:
    machine = SG1000Machine(bytes((0x3E, 0x42, 0xC3, 0x00, 0x00)))

    assert machine.memory_bus.read(0x0000) == 0x3E
    assert machine.memory_bus.read(0x2000) == 0x3E
    machine.set_joystick(1, 0xEF)
    assert machine.read_port(0x01DC) == 0xEF
    assert machine.read_port(0x01DD) == 0xFF


def test_machine_rejects_invalid_rom_sizes_and_joystick_values() -> None:
    with pytest.raises(ValueError):
        SG1000Machine(b"")
    machine = SG1000Machine(bytes((0x00,)))
    with pytest.raises(ValueError):
        machine.set_joystick(1, 0x100)
