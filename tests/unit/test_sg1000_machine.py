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
    rom = bytearray(0x2000)
    rom[:5] = bytes((0x3E, 0x42, 0xC3, 0x00, 0x00))
    machine = SG1000Machine(bytes(rom))

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


def test_cpu_program_can_configure_vdp_through_io_ports() -> None:
    # LD A,C0h; OUT (BFh),A; LD A,81h; OUT (BFh),A; JP 0008h
    rom = bytes((0x3E, 0xC0, 0xD3, 0xBF, 0x3E, 0x81, 0xD3, 0xBF, 0xC3, 0x08, 0x00))
    machine = SG1000Machine(rom)

    for _ in range(4):
        machine.step()

    assert machine.vdp.registers[1] == 0xC0
    assert machine.cpu.pc == 8
