"""Integration tests for native Rust SMS PSG audio exposed through PyO3."""

from __future__ import annotations

import pytest

pytest.importorskip("sega_z80_core")

from sega_z80_core import NativeSMSMachine

from sega_emulator.frontend.qt.audio_output import PcmResampler


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
    left_pcm = left.drain_audio_pcm(PcmResampler(48_000, 100))
    right_pcm = right.drain_audio_pcm(PcmResampler(48_000, 100))

    assert left_pcm
    assert len(left_pcm) % 2 == 0
    assert left_pcm == right_pcm
    assert left.total_tstates == left_tstates
    assert right.total_tstates == right_tstates


def test_sms_pal_profile_selects_pal_psg_clock() -> None:
    machine = NativeSMSMachine(bytes([0x00]), True)
    assert machine.psg_clock_hz == 3_546_895
