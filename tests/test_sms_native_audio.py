"""Integration tests for native Rust SMS PSG audio exposed through PyO3."""

from __future__ import annotations

import pytest

pytest.importorskip("sega_z80_core")

from sega_z80_core import NativePcmResampler, NativeSMSMachine

from sega_emulator.frontend.qt.audio_output import PcmResampler
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
    return NativeSMSMachine(rom, False)


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


def test_sms_audio_pcm_is_deterministic_and_drain_does_not_change_timing() -> None:
    left = _machine()
    right = _machine()

    for _ in range(10_000):
        left.step()
        right.step()

    left_tstates = left.total_tstates
    right_tstates = right.total_tstates
    left_pcm = left.drain_audio_pcm(NativePcmResampler(48_000, 100))
    right_pcm = right.drain_audio_pcm(NativePcmResampler(48_000, 100))

    assert left_pcm
    assert len(left_pcm) % 2 == 0
    assert left_pcm == right_pcm
    assert left.total_tstates == left_tstates
    assert right.total_tstates == right_tstates


def test_sms_pal_profile_selects_pal_psg_clock() -> None:
    machine = NativeSMSMachine(bytes([0x00]), True)
    assert machine.psg_sample_rate == 44_100
    assert machine.psg_clock_hz == 3_546_895


def test_sms_python_adapter_drains_pcm_without_changing_tstates() -> None:
    machine = SMSMachine(
        bytes(
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
    )
    resampler = PcmResampler(48_000, 100)

    for _ in range(10_000):
        machine.step()

    before = machine.total_tstates
    pcm = machine.drain_audio_pcm(resampler)

    assert pcm
    assert len(pcm) % 2 == 0
    assert machine.psg_sample_rate == 44_100
    assert machine.psg_clock_hz == 3_579_545
    assert machine.total_tstates == before


def test_sms_frame_runner_exposes_native_framebuffer_and_controller() -> None:
    machine = NativeSMSMachine(bytes([0x00]) * 16, False)
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
