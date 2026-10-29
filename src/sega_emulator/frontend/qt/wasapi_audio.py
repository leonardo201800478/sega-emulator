"""Thin Qt lifecycle facade over the native Rust WASAPI output."""

from __future__ import annotations


def _native_type():
    try:
        from sega_z80_core import NativeWasapiOutput
    except ImportError as exc:
        raise RuntimeError(
            "A saída WASAPI Rust não está instalada. Compile native/z80_core "
            "com Maturin e a feature python-extension."
        ) from exc
    return NativeWasapiOutput


class WasapiAudioOutput:
    """Expose start/write/close and diagnostics without Python audio processing."""

    def __init__(
        self,
        sample_rate: int,
        buffer_size: int,
        *,
        queue_blocks: int = 8,
    ) -> None:
        if type(sample_rate) is not int or sample_rate <= 0:
            raise ValueError("sample_rate deve ser um inteiro positivo")
        if type(buffer_size) is not int or buffer_size <= 0:
            raise ValueError("buffer_size deve ser um inteiro positivo")
        if type(queue_blocks) is not int or queue_blocks < 1:
            raise ValueError("queue_blocks deve ser um inteiro positivo")
        self._native = _native_type()(sample_rate, buffer_size, queue_blocks)

    @property
    def sample_rate(self) -> int:
        return int(self._native.sample_rate)

    @property
    def buffer_size(self) -> int:
        return int(self._native.buffer_size)

    @property
    def underrun_callbacks(self) -> int:
        return int(self._native.underrun_count)

    @property
    def dropped_blocks(self) -> int:
        return int(self._native.dropped_block_count)

    def write_pcm(self, pcm: bytes) -> None:
        if not isinstance(pcm, bytes):
            raise TypeError("pcm deve ser bytes PCM16")
        self._native.write_pcm(pcm)

    def close(self) -> None:
        self._native.close()

    def __enter__(self) -> WasapiAudioOutput:
        return self

    def __exit__(self, exc_type, exc, traceback) -> None:
        self.close()
