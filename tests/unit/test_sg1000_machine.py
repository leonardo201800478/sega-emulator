"""Testes de integração iniciais da máquina SG-1000."""

import pytest

from sega_emulator.devtools.demo_rom import build_demo_rom
from sega_emulator.machine.sg1000 import SG1000Machine


def test_machine_executes_cartridge_code_and_advances_a_frame() -> None:
    # JP 0000h: ROM homebrew sintética para validar o laço de emulação.
    machine = SG1000Machine(bytes((0xC3, 0x00, 0x00)))

    steps = machine.run_frame()

    assert steps > 0
    assert machine.vdp.frame_number == 1
    assert machine.cpu.pc == 0
    assert machine.cpu.total_cycles >= 228 * 262
    assert machine.total_instructions == steps
    assert machine.total_tstates == machine.cpu.total_cycles


def test_machine_mirrors_small_cartridge_and_maps_joysticks() -> None:
    rom = bytearray(0x2000)
    rom[:5] = bytes((0x3E, 0x42, 0xC3, 0x00, 0x00))
    machine = SG1000Machine(bytes(rom))

    assert machine.memory_bus.read(0x0000) == 0x3E
    assert machine.memory_bus.read(0x2000) == 0x3E
    machine.set_joystick(1, 0xEF)
    machine.set_joystick(2, 0xDF)
    assert machine.read_port(0x01DC) == 0xEF
    assert machine.read_port(0x01DD) == 0xDF
    assert machine.read_port(0x0000) == 0xEF
    assert machine.read_port(0x0001) == 0xDF
    machine.memory_bus.write(0x0000, 0x00)
    assert machine.memory_bus.read(0x0000) == 0x3E


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


def test_generated_homebrew_rom_initializes_vdp_and_renders_pattern() -> None:
    machine = SG1000Machine(build_demo_rom())

    # The first frame includes startup-time VRAM initialization; the second
    # frame validates the stable raster output after the display is configured.
    machine.run_frames(2)

    assert machine.vdp.registers[1] == 0xC0
    assert machine.vdp.vram[0x3800] == 1
    assert machine.vdp.vram[0x3B00] == 0xD0
    assert machine.vdp.framebuffer[:3] == bytes((33, 200, 66))


def test_machine_decodes_vdp_port_aliases() -> None:
    machine = SG1000Machine(bytes((0x00,)))
    machine.vdp.vram[0x0123] = 0xA9
    machine.vdp.write_port(0xBF, 0x23)
    machine.vdp.write_port(0xBF, 0x01)
    assert machine.read_port(0x009E) == 0xA9
    assert machine.read_port(0x00BF) == machine.vdp.status


def test_machine_writes_vdp_through_aliased_data_port() -> None:
    machine = SG1000Machine(bytes((0x00,)))
    machine.write_port(0x009F, 0x45)
    machine.write_port(0x009F, 0x40)
    machine.write_port(0x009E, 0xBC)
    assert machine.vdp.vram[0x0045] == 0xBC


def test_machine_routes_psg_writes_from_sg1000_port_range() -> None:
    machine = SG1000Machine(bytes((0x00,)))

    machine.write_port(0x007F, 0x90)
    machine.write_port(0x0040, 0x85)
    machine.write_port(0x0040, 0x12)

    assert machine.psg.attenuations[0] == 0
    assert machine.psg.tone_periods[0] == 0x125
    assert machine.vdp.registers[0] == 0


def test_machine_clocks_psg_and_produces_frame_audio() -> None:
    machine = SG1000Machine(bytes((0xC3, 0x00, 0x00)))
    machine.write_port(0x007F, 0x8E)
    machine.write_port(0x007F, 0x0F)
    machine.write_port(0x007F, 0x90)
    machine.run_frame()

    samples = machine.drain_audio_samples()
    assert 700 <= len(samples) <= 760
    assert all(-1.0 <= sample <= 1.0 for sample in samples)
    assert machine.drain_audio_samples() == []
