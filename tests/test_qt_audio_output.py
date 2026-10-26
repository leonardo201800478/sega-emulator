"""Tests for deterministic, anti-aliased PCM output conversion."""

from __future__ import annotations

import math
from array import array

import pytest

from sega_emulator.frontend.qt.audio_output import PcmResampler, samples_to_pcm


def _decode(pcm: bytes) -> list[int]:
    decoded = array("h")
    decoded.frombytes(pcm)
    return list(decoded)


def test_same_rate_preserves_sample_count_and_clamps_to_int16() -> None:
    samples = [-2.0, -0.5, 0.0, 0.5, 2.0]

    decoded = _decode(samples_to_pcm(samples, 44_100))

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


def test_streaming_resampler_is_independent_of_block_boundaries() -> None:
    samples = [0.4 * math.sin(index * 0.19) for index in range(4097)]
    expected = samples_to_pcm(samples, 22_050)

    resampler = PcmResampler(22_050)
    chunks = [
        resampler.process(samples[:137]),
        resampler.process(samples[137:2048]),
        resampler.process(samples[2048:]),
        resampler.flush(),
    ]

    assert b"".join(chunks) == expected


def test_downsampling_fir_rejects_frequencies_above_output_nyquist() -> None:
    count = 8192
    low_tone = [0.5 * math.sin(2 * math.pi * 0.10 * i) for i in range(count)]
    high_tone = [0.5 * math.sin(2 * math.pi * 0.35 * i) for i in range(count)]

    low_pcm = _decode(samples_to_pcm(low_tone, 22_050))
    high_pcm = _decode(samples_to_pcm(high_tone, 22_050))
    low_tail = low_pcm[100:]
    high_tail = high_pcm[100:]
    low_rms = math.sqrt(sum(sample * sample for sample in low_tail) / len(low_tail))
    high_rms = math.sqrt(sum(sample * sample for sample in high_tail) / len(high_tail))

    assert low_rms > 8_000
    assert high_rms < low_rms * 0.03


def test_resampler_flush_is_empty_after_stream_reset() -> None:
    resampler = PcmResampler(48_000)
    assert resampler.process([0.1] * 256)
    assert resampler.flush() == b""
    assert resampler.flush() == b""
