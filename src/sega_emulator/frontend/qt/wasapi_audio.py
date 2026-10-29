"""Thin Qt lifecycle facade over native Rust Windows audio outputs."""
from __future__ import annotations


def _native_type(backend: str):
    try:
        import sega_z80_core
    except ImportError as exc:
        raise RuntimeError(
            "O runtime de áudio Rust não está instalado. Compile native/z80_core "
            "com Maturin e a feature python-extension."
        ) from exc

    native_name = {
        "wasapi": "NativeWasapiOutput",
        "xaudio2": "NativeXAudio2Output",
    }.get(backend)
    if native_name is None:
        raise ValueError(f"Backend de áudio inválido: {backend}")
    try:
        return getattr(sega_z80_core, native_name)
    except AttributeError as exc:
        raise RuntimeError(
            f"O backend nativo {backend} não está presente na extensão Rust."
        ) from exc


class NativeAudioOutput:
    """Control a Rust-owned output stream; Python does no audio processing."""

    def __init__(
        self,
        sample_rate: int,
        buffer_size: int,
        *,
        backend: str = "wasapi",
        queue_blocks: int = 4,
    ) -> None:
        if type(sample_rate) is not int or sample_rate <= 0:
            raise ValueError("sample_rate deve ser um inteiro positivo")
        if type(buffer_size) is not int or buffer_size <= 0:
            raise ValueError("buffer_size deve ser um inteiro positivo")
        if type(queue_blocks) is not int or queue_blocks < 1:
            raise ValueError("queue_blocks deve ser um inteiro positivo")
        if backend not in {"wasapi", "xaudio2"}:
            raise ValueError(f"Backend de áudio inválido: {backend}")
        self.backend = backend
        self._native = _native_type(backend)(sample_rate, buffer_size, queue_blocks)

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

    def __enter__(self) -> NativeAudioOutput:
        return self

    def __exit__(self, exc_type, exc, traceback) -> None:
        self.close()


# Backward-compatible import name for callers that only use the default backend.
WasapiAudioOutput = NativeAudioOutput
