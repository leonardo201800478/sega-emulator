"""Differential tests for the native SG-1000 machine loop and I/O."""

from __future__ import annotations

import pytest

from sega_emulator.machine.sg1000 import SG1000Machine


def _assert_cpu_matches(reference, native) -> None:
    for name in (
        "a",
        "f",
        "b",
        "c",
        "d",
        "e",
        "h",
        "l",
        "pc",
        "sp",
        "ix",
        "iy",
        "i",
        "r",
        "iff1",
        "iff2",
        "interrupt_mode",
        "halted",
        "wz",
        "q",
        "p",
    ):
        assert getattr(native.cpu, name) == getattr(reference.cpu, name), name
    assert native.total_instructions == reference.total_instructions
    assert native.total_tstates == reference.total_tstates


def test_native_machine_routes_ports_and_steps_like_python() -> None:
    pytest.importorskip("sega_z80_core")
    rom = bytes(
        (
            0x3E,
            0x40,
            0xD3,
            0x81,
            0x3E,
            0x81,
            0xD3,
            0x81,
            0x3E,
            0x85,
            0xD3,
            0x7F,
            0x3E,
            0x12,
            0xD3,
            0x7F,
            0x3E,
            0x90,
            0xD3,
            0x7F,
            0xC3,
            0x00,
            0x00,
        )
    )
    reference = SG1000Machine(rom)
    native = SG1000Machine(rom, cpu_core="rust")

    native.set_joystick(1, 0xFE)
    reference.set_joystick(1, 0xFE)
    assert native.read_port(0x00) == reference.read_port(0x00) == 0xFE
    assert native.read_port(0xDC) == reference.read_port(0xDC) == 0xFE

    for _ in range(150):
        assert native.step() == reference.step()
        _assert_cpu_matches(reference, native)
        assert native.vdp.status == reference.vdp.status
        assert native.vdp.scanline == reference.vdp.scanline
        assert native.psg.tone_periods == reference.psg.tone_periods
        assert native.psg.attenuations == reference.psg.attenuations
        assert native.psg._tone_outputs == reference.psg._tone_outputs
    assert native.drain_audio_samples() == pytest.approx(
        reference.drain_audio_samples(), abs=1e-12
    )


def test_native_machine_run_frame_matches_python_frame_loop() -> None:
    pytest.importorskip("sega_z80_core")
    rom = b"\xc3\x00\x00"
    reference = SG1000Machine(rom)
    native = SG1000Machine(rom, cpu_core="rust")

    reference_steps = reference.run_frame()
    native_steps = native.run_frame()

    assert native_steps == reference_steps
    _assert_cpu_matches(reference, native)
    assert native.vdp.frame_number == reference.vdp.frame_number == 1
    assert native.vdp.scanline == reference.vdp.scanline
    assert native.vdp.status == reference.vdp.status
    assert native.vdp.framebuffer == reference.vdp.framebuffer
    assert native.drain_audio_samples() == pytest.approx(
        reference.drain_audio_samples(), abs=1e-12
    )


def test_native_machine_rejects_invalid_frame_step_limit() -> None:
    pytest.importorskip("sega_z80_core")
    machine = SG1000Machine(b"\xc3\x00\x00", cpu_core="rust")
    with pytest.raises(ValueError, match="max_steps"):
        machine.run_frame(0)
