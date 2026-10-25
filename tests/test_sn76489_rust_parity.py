"""Differential tests for native SN76489AN and audio synchronization."""

from __future__ import annotations

import pytest

from sega_emulator.audio.sn76489 import SN76489
from sega_emulator.audio.synchronization import AudioSynchronizer


def _configured_pair(
    native_type, *, clock_hz: int = 3_579_545, sample_rate: int = 44_100
):
    python_psg = SN76489(clock_hz=clock_hz, sample_rate=sample_rate)
    rust_psg = native_type(clock_hz=clock_hz, sample_rate=sample_rate)
    writes = (
        0x85,  # Tone 0 period low nibble
        0x12,  # Tone 0 period high bits
        0x90,  # Tone 0 attenuation
        0xA4,  # Tone 1 period low nibble
        0x23,  # Tone 1 period high bits
        0xB2,  # Tone 1 attenuation
        0xC7,  # Tone 2 period low nibble
        0x11,  # Tone 2 period high bits
        0xD3,  # Tone 2 attenuation
        0xE4,  # White noise, rate 0
        0xF0,  # Noise attenuation
    )
    for device in (python_psg, rust_psg):
        for value in writes:
            device.write(value)
    return python_psg, rust_psg


def _assert_psg_state_matches(python_psg, rust_psg) -> None:
    for name in (
        "clock_hz",
        "sample_rate",
        "tone_periods",
        "attenuations",
        "noise_control",
        "dropped_samples",
        "_latched_register",
        "_tone_counters",
        "_tone_outputs",
        "_noise_counter",
        "_noise_counter_phase",
        "_lfsr",
        "_clock_divider_phase",
        "_output_phase",
        "_noise_period",
    ):
        assert getattr(rust_psg, name) == getattr(python_psg, name), name


def test_native_psg_clock_advancement_and_samples_match_python() -> None:
    native_core = pytest.importorskip("sega_z80_core")
    python_psg, rust_psg = _configured_pair(native_core.NativeSn76489)

    python_psg.advance_clocks(75_000)
    rust_psg.advance_clocks(75_000)

    _assert_psg_state_matches(python_psg, rust_psg)
    assert rust_psg.drain_samples() == python_psg.drain_samples()


def test_native_psg_render_and_bounded_buffer_match_python() -> None:
    native_core = pytest.importorskip("sega_z80_core")
    python_psg, rust_psg = _configured_pair(native_core.NativeSn76489)
    assert rust_psg.render(257) == python_psg.render(257)
    _assert_psg_state_matches(python_psg, rust_psg)

    python_buffered, rust_buffered = _configured_pair(
        native_core.NativeSn76489,
        clock_hz=1_000,
        sample_rate=100,
    )
    python_buffered.advance_clocks(16_000)
    rust_buffered.advance_clocks(16_000)
    _assert_psg_state_matches(python_buffered, rust_buffered)
    assert rust_buffered.drain_samples() == python_buffered.drain_samples()


def test_native_audio_synchronizer_preserves_timestamp_and_write_order() -> None:
    native_core = pytest.importorskip("sega_z80_core")
    python_psg, rust_psg = _configured_pair(native_core.NativeSn76489)
    python_sync = AudioSynchronizer(python_psg)
    rust_sync = native_core.NativeAudioSynchronizer(rust_psg)
    events = (
        (0, 0x85),
        (17, 0x12),
        (17, 0x90),
        (100, 0xE4),
        (230, 0xF0),
        (700, 0xA7),
    )
    for tstate, value in events:
        python_sync.schedule_write(tstate, value)
        rust_sync.schedule_write(tstate, value)

    python_sync.advance_to(500)
    rust_sync.advance_to(500)

    _assert_psg_state_matches(python_psg, rust_psg)
    assert rust_sync.current_tstate == python_sync.current_tstate
    assert tuple(rust_sync.pending_events) == tuple(
        (event.tstate, event.value) for event in python_sync.pending_events
    )
    assert rust_psg.drain_samples() == python_psg.drain_samples()
