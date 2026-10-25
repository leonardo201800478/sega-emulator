import pytest

from sega_emulator.video.output_config import (
    AspectRatio,
    PresentMode,
    ScalingFilter,
    ShaderFormat,
    VideoBackend,
    VideoOutputConfig,
    WindowMode,
)


def test_default_video_output_is_deterministic_and_conservative() -> None:
    config = VideoOutputConfig()
    assert config.backend is VideoBackend.DIRECT3D11
    assert config.scaling_filter is ScalingFilter.NEAREST
    assert config.aspect_ratio is AspectRatio.PIXEL_PERFECT
    assert config.window_mode is WindowMode.WINDOWED
    assert config.present_mode is PresentMode.VSYNC
    assert config.shader_format is ShaderFormat.RETROARCH_SLANG
    assert config.shaders_enabled is False
    assert config.frames_in_flight == 1
    assert config.frame_delay_ms == 0
    assert config.run_ahead_frames == 0
    assert config.integer_scaling is True


@pytest.mark.parametrize("backend", list(VideoBackend))
def test_all_video_backends_are_representable(backend: VideoBackend) -> None:
    assert VideoOutputConfig(backend=backend).backend is backend


@pytest.mark.parametrize("scaling_filter", list(ScalingFilter))
def test_all_scaling_filters_are_representable(scaling_filter: ScalingFilter) -> None:
    assert VideoOutputConfig(scaling_filter=scaling_filter).scaling_filter is scaling_filter


@pytest.mark.parametrize("aspect_ratio", list(AspectRatio))
def test_aspect_ratio_modes_are_representable(aspect_ratio: AspectRatio) -> None:
    assert VideoOutputConfig(aspect_ratio=aspect_ratio).aspect_ratio is aspect_ratio


@pytest.mark.parametrize("window_mode", list(WindowMode))
def test_window_modes_are_representable(window_mode: WindowMode) -> None:
    assert VideoOutputConfig(window_mode=window_mode).window_mode is window_mode


@pytest.mark.parametrize("present_mode", list(PresentMode))
def test_present_modes_are_representable(present_mode: PresentMode) -> None:
    assert VideoOutputConfig(present_mode=present_mode).present_mode is present_mode


@pytest.mark.parametrize("shader_format", list(ShaderFormat))
def test_shader_formats_are_representable(shader_format: ShaderFormat) -> None:
    assert VideoOutputConfig(shader_format=shader_format).shader_format is shader_format


@pytest.mark.parametrize("frames", [0, 4, -1, 1.0, True])
def test_invalid_frames_in_flight_are_rejected(frames: object) -> None:
    with pytest.raises(ValueError, match="frames_in_flight"):
        VideoOutputConfig(frames_in_flight=frames)  # type: ignore[arg-type]


@pytest.mark.parametrize("delay", [-1, 51, 1.5, True])
def test_invalid_frame_delay_is_rejected(delay: object) -> None:
    with pytest.raises(ValueError, match="frame_delay_ms"):
        VideoOutputConfig(frame_delay_ms=delay)  # type: ignore[arg-type]


@pytest.mark.parametrize("frames", [-1, 3, 1.5, True])
def test_invalid_run_ahead_is_rejected(frames: object) -> None:
    with pytest.raises(ValueError, match="run_ahead_frames"):
        VideoOutputConfig(run_ahead_frames=frames)  # type: ignore[arg-type]


@pytest.mark.parametrize("field", ["shaders_enabled", "integer_scaling"])
def test_boolean_settings_reject_integer_values(field: str) -> None:
    with pytest.raises(ValueError):
        VideoOutputConfig(**{field: 1})
