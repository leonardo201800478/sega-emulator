"""Validated host-rendering preferences, independent from VDP timing."""
from __future__ import annotations

from dataclasses import dataclass
from enum import StrEnum


class VideoBackend(StrEnum):
    """GPU presentation APIs supported by the planned renderer."""

    DIRECT3D11 = "d3d11"
    DIRECT3D12 = "d3d12"
    VULKAN = "vulkan"


class ScalingFilter(StrEnum):
    """Built-in image scaling and presentation filters."""

    NEAREST = "nearest"
    BILINEAR = "bilinear"
    CRT = "crt"


class AspectRatio(StrEnum):
    """User-selected presentation geometry."""

    PIXEL_PERFECT = "pixel_perfect"
    FOUR_THREE = "4:3"
    SIXTEEN_NINE = "16:9"


class WindowMode(StrEnum):
    """Window and fullscreen presentation modes."""

    WINDOWED = "windowed"
    BORDERLESS = "borderless"
    FULLSCREEN = "fullscreen"


class PresentMode(StrEnum):
    """Presentation pacing preference, subject to API/display capabilities."""

    VSYNC = "vsync"
    VRR = "vrr"
    LOW_LATENCY = "low_latency"


class ShaderFormat(StrEnum):
    """Shader source/preset formats; compatibility varies by backend."""

    RETROARCH_SLANG = "slang"
    GLSL = "glsl"
    HLSL = "hlsl"


@dataclass(frozen=True, slots=True)
class VideoOutputConfig:
    """User preferences for host rendering; never changes emulated VDP timing."""

    backend: VideoBackend = VideoBackend.DIRECT3D11
    scaling_filter: ScalingFilter = ScalingFilter.NEAREST
    aspect_ratio: AspectRatio = AspectRatio.PIXEL_PERFECT
    window_mode: WindowMode = WindowMode.WINDOWED
    present_mode: PresentMode = PresentMode.VSYNC
    shader_format: ShaderFormat = ShaderFormat.RETROARCH_SLANG
    shaders_enabled: bool = False
    frames_in_flight: int = 1
    frame_delay_ms: int = 0
    run_ahead_frames: int = 0
    integer_scaling: bool = True

    def __post_init__(self) -> None:
        enum_fields = (
            ("backend", self.backend, VideoBackend),
            ("scaling_filter", self.scaling_filter, ScalingFilter),
            ("aspect_ratio", self.aspect_ratio, AspectRatio),
            ("window_mode", self.window_mode, WindowMode),
            ("present_mode", self.present_mode, PresentMode),
            ("shader_format", self.shader_format, ShaderFormat),
        )
        for name, value, enum_type in enum_fields:
            if not isinstance(value, enum_type):
                raise ValueError(f"{name} deve ser um {enum_type.__name__} válido.")
        if type(self.shaders_enabled) is not bool:
            raise ValueError("shaders_enabled deve ser bool.")
        if type(self.integer_scaling) is not bool:
            raise ValueError("integer_scaling deve ser bool.")
        if (
            type(self.frames_in_flight) is not int
            or not 1 <= self.frames_in_flight <= 3
        ):
            raise ValueError("frames_in_flight deve ser um inteiro entre 1 e 3.")
        if type(self.frame_delay_ms) is not int or not 0 <= self.frame_delay_ms <= 50:
            raise ValueError("frame_delay_ms deve ser um inteiro entre 0 e 50.")
        if (
            type(self.run_ahead_frames) is not int
            or not 0 <= self.run_ahead_frames <= 2
        ):
            raise ValueError("run_ahead_frames deve ser um inteiro entre 0 e 2.")
