"""Integration tests for the Rust-only SG-1000 runtime."""

from __future__ import annotations

import pytest

from sega_emulator.machine.sg1000 import SG1000Machine



def _machine(rom: bytes = b"\xc3\x00\x00") -> SG1000Machine:
    pytest.importorskip("sega_z80_core")
    return SG1000Machine(rom)



def test_machine_uses_only_native_rust_runtime() -> None:
    machine = _machine()
    assert machine.cpu_core == "rust"
    assert machine.native_machine is not None
    assert machine.cpu.pc == 0
    assert machine.total_instructions == 0
    machine.step()
    assert machine.total_instructions == 1
    assert machine.total_tstates > 0


def test_native_machine_routes_joystick_and_psg_events() -> None:
    machine = _machine(b"\x00")
    machine.set_joystick(1, 0xFE)
    assert machine.read_port(0x00) == 0xFE
    assert machine.read_port(0xDC) == 0xFE

    machine.write_port_at(0x7F, 0x90, 123)
    machine.write_port_at(0x40, 0x9F, 456)
    assert machine.drain_psg_write_events() == [(123, 0x90), (456, 0x9F)]
    assert machine.drain_psg_write_events() == []


def test_native_machine_exposes_sprite_limit_control() -> None:
    machine = _machine()
    assert machine.vdp.sprite_limit_enabled is True
    machine.set_sprite_limit_enabled(False)
    assert machine.vdp.sprite_limit_enabled is False
    machine.set_sprite_limit_enabled(True)
    assert machine.vdp.sprite_limit_enabled is True


def test_native_machine_runs_a_frame_and_rejects_invalid_limit() -> None:
    machine = _machine()
    with pytest.raises(ValueError, match="max_steps"):
        machine.run_frame(0)
    steps = machine.run_frame()
    assert steps > 0
    assert machine.vdp.frame_number == 1


def test_machine_rejects_unknown_native_audio_backend() -> None:
    machine = _machine()
    with pytest.raises(RuntimeError, match="unsupported native audio backend"):
        machine.configure_audio_output("invalid")
    assert machine.native_machine.audio_output_metrics == (0, 0)

