"""Validated, backend-neutral preferences for physical audio output.

These settings describe the host output path, not the emulated console clock.
"""

from __future__ import annotations

from collections.abc import Mapping
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
        if (
            type(self.sample_rate) is not int
            or self.sample_rate not in SUPPORTED_SAMPLE_RATES
        ):
            raise ValueError(
                f"sample_rate deve ser um destes valores: {SUPPORTED_SAMPLE_RATES}."
            )
        if not isinstance(self.pcm_format, PcmFormat):
            raise ValueError("pcm_format deve ser um PcmFormat válido.")
        if type(self.output_channels) is not int or self.output_channels not in (1, 2):
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

    def to_dict(self) -> dict[str, object]:
        """Serializa preferências para JSON sem incluir estado do dispositivo."""
        return {
            "backend": self.backend.value,
            "sample_rate": self.sample_rate,
            "pcm_format": self.pcm_format.value,
            "output_channels": self.output_channels,
            "latency_profile": self.latency_profile.value,
            "buffer_ms": self.buffer_ms,
            "device_id": self.device_id,
            "wasapi_exclusive": self.wasapi_exclusive,
            "allow_fallback": self.allow_fallback,
        }

    @classmethod
    def from_dict(cls, values: Mapping[str, object]) -> AudioOutputConfig:
        """Carrega preferências persistidas; rejeita campos desconhecidos."""
        if not isinstance(values, Mapping):
            raise ValueError("A configuração de áudio deve ser um mapeamento.")

        allowed = {
            "backend",
            "sample_rate",
            "pcm_format",
            "output_channels",
            "latency_profile",
            "buffer_ms",
            "device_id",
            "wasapi_exclusive",
            "allow_fallback",
        }
        unknown = set(values) - allowed
        if unknown:
            names = ", ".join(sorted(str(key) for key in unknown))
            raise ValueError(f"Campos de configuração de áudio desconhecidos: {names}.")

        parsed = dict(values)
        for key, enum_type in (
            ("backend", AudioBackend),
            ("pcm_format", PcmFormat),
            ("latency_profile", LatencyProfile),
        ):
            if key in parsed and not isinstance(parsed[key], enum_type):
                try:
                    parsed[key] = enum_type(parsed[key])
                except (TypeError, ValueError) as exc:
                    raise ValueError(f"{key} contém uma opção desconhecida.") from exc

        return cls(**parsed)
