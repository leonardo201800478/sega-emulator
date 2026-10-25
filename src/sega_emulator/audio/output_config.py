"""Validated, backend-neutral preferences for physical audio output.

These settings describe the host output path, not the emulated console clock.
"""
from __future__ import annotations

from dataclasses import dataclass
from enum import StrEnum


class AudioBackend(StrEnum):
    """Selectable host audio APIs."""

    WASAPI = "wasapi"
    XAUDIO2 = "xaudio2"
    SDL3 = "sdl3"


class PcmFormat(StrEnum):
    """PCM formats that a backend may support."""

    S16 = "s16"
    S24 = "s24"
    F32 = "f32"


class LatencyProfile(StrEnum):
    """Target buffer policy; actual latency depends on the backend and device."""

    STABLE = "stable"
    BALANCED = "balanced"
    LOW = "low"


SUPPORTED_SAMPLE_RATES = (22_050, 32_000, 44_100, 48_000, 88_200, 96_000)


@dataclass(frozen=True, slots=True)
class AudioOutputConfig:
    """User preferences for audio output, independent from PSG timing."""

    backend: AudioBackend = AudioBackend.WASAPI
    sample_rate: int = 48_000
    pcm_format: PcmFormat = PcmFormat.S16
    output_channels: int = 2
    latency_profile: LatencyProfile = LatencyProfile.BALANCED
    buffer_ms: int | None = None
    device_id: str | None = None
    wasapi_exclusive: bool = False
    allow_fallback: bool = True

    def __post_init__(self) -> None:
        if not isinstance(self.backend, AudioBackend):
            raise ValueError("backend deve ser um AudioBackend válido.")
        if self.sample_rate not in SUPPORTED_SAMPLE_RATES:
            raise ValueError(
                f"sample_rate deve ser um destes valores: {SUPPORTED_SAMPLE_RATES}."
            )
        if not isinstance(self.pcm_format, PcmFormat):
            raise ValueError("pcm_format deve ser um PcmFormat válido.")
        if self.output_channels not in (1, 2):
            raise ValueError("output_channels deve ser 1 (mono) ou 2 (estéreo).")
        if not isinstance(self.latency_profile, LatencyProfile):
            raise ValueError("latency_profile deve ser um LatencyProfile válido.")
        if self.buffer_ms is not None and (
            type(self.buffer_ms) is not int or not 1 <= self.buffer_ms <= 200
        ):
            raise ValueError("buffer_ms deve ser None ou um inteiro entre 1 e 200.")
        if self.device_id is not None and (
            not isinstance(self.device_id, str) or not self.device_id.strip()
        ):
            raise ValueError("device_id deve ser None ou uma string não vazia.")
        if type(self.wasapi_exclusive) is not bool:
            raise ValueError("wasapi_exclusive deve ser bool.")
        if type(self.allow_fallback) is not bool:
            raise ValueError("allow_fallback deve ser bool.")
        if self.wasapi_exclusive and self.backend is not AudioBackend.WASAPI:
            raise ValueError("wasapi_exclusive só se aplica ao backend WASAPI.")
