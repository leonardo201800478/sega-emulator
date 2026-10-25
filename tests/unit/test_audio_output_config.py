import pytest

from sega_emulator.audio.output_config import (
    SUPPORTED_SAMPLE_RATES,
    AudioBackend,
    AudioOutputConfig,
    LatencyProfile,
    PcmFormat,
)


def test_default_audio_output_preferences() -> None:
    config = AudioOutputConfig()
    assert config.backend is AudioBackend.WASAPI
    assert config.sample_rate == 48_000
    assert config.pcm_format is PcmFormat.S16
    assert config.output_channels == 2
    assert config.latency_profile is LatencyProfile.BALANCED
    assert config.wasapi_exclusive is False
    assert config.allow_fallback is True


@pytest.mark.parametrize("rate", SUPPORTED_SAMPLE_RATES)
def test_supported_rates(rate: int) -> None:
    assert AudioOutputConfig(sample_rate=rate).sample_rate == rate


@pytest.mark.parametrize("backend", list(AudioBackend))
def test_backends_are_selectable(backend: AudioBackend) -> None:
    assert AudioOutputConfig(backend=backend).backend is backend


@pytest.mark.parametrize("pcm_format", list(PcmFormat))
def test_pcm_formats_are_representable(pcm_format: PcmFormat) -> None:
    assert AudioOutputConfig(pcm_format=pcm_format).pcm_format is pcm_format


@pytest.mark.parametrize("profile", list(LatencyProfile))
def test_latency_profiles_are_representable(profile: LatencyProfile) -> None:
    assert AudioOutputConfig(latency_profile=profile).latency_profile is profile


@pytest.mark.parametrize("rate", [0, -1, 48001, 48000.0, True])
def test_invalid_rates_are_rejected(rate: object) -> None:
    with pytest.raises(ValueError, match="sample_rate"):
        AudioOutputConfig(sample_rate=rate)  # type: ignore[arg-type]


@pytest.mark.parametrize("channels", [0, 3, 1.0, True])
def test_invalid_channel_counts_are_rejected(channels: object) -> None:
    with pytest.raises(ValueError, match="output_channels"):
        AudioOutputConfig(output_channels=channels)  # type: ignore[arg-type]


@pytest.mark.parametrize("buffer_ms", [0, -1, 201, 1.5, True])
def test_invalid_buffers_are_rejected(buffer_ms: object) -> None:
    with pytest.raises(ValueError, match="buffer_ms"):
        AudioOutputConfig(buffer_ms=buffer_ms)  # type: ignore[arg-type]


def test_device_and_manual_buffer_are_configurable() -> None:
    config = AudioOutputConfig(buffer_ms=8, device_id="endpoint-1")
    assert config.buffer_ms == 8
    assert config.device_id == "endpoint-1"


def test_exclusive_mode_is_wasapi_only() -> None:
    with pytest.raises(ValueError, match="WASAPI"):
        AudioOutputConfig(backend=AudioBackend.SDL3, wasapi_exclusive=True)


@pytest.mark.parametrize("field", ["wasapi_exclusive", "allow_fallback"])
def test_boolean_options_reject_integers(field: str) -> None:
    with pytest.raises(ValueError):
        AudioOutputConfig(**{field: 1})
