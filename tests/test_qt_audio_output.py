"""Tests for PCM output conversion used by the Qt audio backend."""

from __future__ import annotations

from array import array

import pytest

from sega_emulator.frontend.qt.audio_output import samples_to_pcm


def test_same_rate_preserves_sample_count_and_clamps_to_int16() -> None:
    samples = [-2.0, -0.5, 0.0, 0.5, 2.0]

    pcm = samples_to_pcm(samples, 44_100)

    decoded = array("h")
    decoded.frombytes(pcm)
    assert len(decoded) == len(samples)
    assert decoded[0] == -32768
    assert decoded[1] < 0
    assert decoded[2] == 0
    assert decoded[3] > 0
    assert decoded[4] == 32767


@pytest.mark.parametrize(("rate", "expected"), [(22_050, 2), (44_100, 4), (48_000, 4)])
def test_resampling_returns_signed_16_bit_mono(rate: int, expected: int) -> None:
    pcm = samples_to_pcm([0.0, 0.25, -0.25, 0.0], rate)

    assert len(pcm) == expected * 2


def test_empty_samples_return_empty_pcm() -> None:
    assert samples_to_pcm([], 48_000) == b""


@pytest.mark.parametrize("rate", [0, -1, 44_100.0, True])
def test_invalid_output_rate_is_rejected(rate: int) -> None:
    with pytest.raises(ValueError, match="output_rate"):
        samples_to_pcm([0.0], rate)
