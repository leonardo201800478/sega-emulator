"""Smoke tests for SMS machine state exposed by the native Rust binding."""

from __future__ import annotations

import pytest

pytest.importorskip("sega_z80_core")

from sega_z80_core import NativeSMSMachine
from sega_emulator.machine import SMSMachine



def _machine() -> NativeSMSMachine:
    # Program: set tone channel 0 and loop forever.
    rom = bytes(
        [
            0x3E,
            0x90,
            0xD3,
            0x7F,
            0x3E,
            0x85,
            0xD3,
            0x7F,
            0x3E,
            0x12,
            0xD3,
            0x7F,
            0xC3,
            0x00,
            0x00,
        ]
    )
    return NativeSMSMachine(rom, region="usa")



def test_sms_binding_uses_native_psg_and_ntsc_clock() -> None:
    machine = _machine()

    assert machine.psg_sample_rate == 44_100
    assert machine.psg_clock_hz == 3_579_545
    assert machine.total_tstates == 0

    for _ in range(6):
        machine.step()

    assert machine.psg_tone_periods[0] == 0x125
    assert machine.psg_attenuations[0] == 0
    assert machine.total_tstates > 0


def test_sms_machine_rejects_unknown_native_audio_backend() -> None:
    machine = _machine()
    with pytest.raises(RuntimeError, match="unsupported native audio backend"):
        machine.configure_audio_output("invalid")
    assert machine.audio_output_metrics == (0, 0)


def test_sms_pal_profile_selects_pal_psg_clock() -> None:
    machine = NativeSMSMachine(bytes([0x00]), region="europe")
    assert machine.psg_sample_rate == 44_100
    assert machine.psg_clock_hz == 3_546_895


def test_sms_brazil_pal_m_profile_uses_ntsc_like_clock() -> None:
    native = NativeSMSMachine(bytes([0x00]), region="brazil")
    adapter = SMSMachine(bytes([0x00]), region="brazil")

    assert native.psg_clock_hz == 3_579_545
    assert adapter.region == "brazil"
    assert adapter.psg_clock_hz == 3_579_545


def test_sms_adapter_routes_console_pause_to_z80_nmi() -> None:
    machine = SMSMachine(bytes([0x31, 0xF0, 0xDF, 0x00]))
    machine.step()
    machine.request_nmi()

    assert machine.step() == 11
    assert machine.cpu.pc == 0x0066


def test_sms_frame_runner_exposes_native_framebuffer_and_controller() -> None:
    machine = NativeSMSMachine(bytes([0x00]) * 16, region="usa")
    assert machine.frame_number == 0
    steps = machine.run_frame(100_000)
    assert 0 < steps <= 100_000
    assert machine.frame_number == 1
    assert machine.visible_height == 192
    assert len(machine.framebuffer) == 256 * 192 * 3
    assert machine.total_tstates > 0
    machine.set_controller(1, True, False, False, False, True, False)
    with pytest.raises(ValueError):
        machine.set_controller(3, False, False, False, False, False, False)


def test_sms_adapter_exposes_qt_frame_and_controller_contract() -> None:
    machine = SMSMachine(bytes([0x00]) * 16)
    machine.run_frame()
    assert len(machine.vdp.framebuffer) == 256 * 192 * 3
    assert machine.vdp.frame_number == 1
    assert machine.vdp.visible_height == 192
    machine.set_controller_actions(1, {"up", "button1"})
