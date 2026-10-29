"""Thin Python facade for native Rust PCM processing.

The resampler, filtering, gain and PCM16 conversion are implemented in the
Rust audio runtime and exposed by the PyO3 extension. This module contains no
DSP implementation.
"""

from __future__ import annotations

from collections.abc import Sequence

PSG_SAMPLE_RATE = 44_100


def _native_type():
    try:
        from sega_z80_core import NativePcmResampler
    except ImportError as exc:
        raise RuntimeError(
            "O runtime PCM Rust não está instalado. Compile native/z80_core "
            "com Maturin e a feature python-extension."
        ) from exc
    return NativePcmResampler


class PcmResampler:
    """Python lifecycle adapter around the native Rust streaming resampler."""

    def __init__(self, output_rate: int, volume: int = 100) -> None:
        if type(output_rate) is not int or output_rate <= 0:
            raise ValueError("output_rate deve ser um inteiro positivo")
        if type(volume) is not int or not 0 <= volume <= 100:
            raise ValueError("volume deve ser um inteiro entre 0 e 100")
        self._native = _native_type()(output_rate, volume)

    @property
    def output_rate(self) -> int:
        return int(self._native.output_rate)

    def set_volume(self, volume: int) -> None:
        if type(volume) is not int or not 0 <= volume <= 100:
            raise ValueError("volume deve ser um inteiro entre 0 e 100")
        self._native.set_volume(volume)

    def reset(self) -> None:
        self._native.reset()

    def process(self, samples: Sequence[float]) -> bytes:
        return bytes(self._native.process(samples))

    def flush(self) -> bytes:
        return bytes(self._native.flush())


def samples_to_pcm(samples: Sequence[float], output_rate: int) -> bytes:
    """Convert a finite block through the native Rust PCM pipeline."""
    if type(output_rate) is not int or output_rate <= 0:
        raise ValueError("output_rate deve ser um inteiro positivo")
    if not samples:
        return b""
    resampler = PcmResampler(output_rate)
    return resampler.process(samples) + resampler.flush()
