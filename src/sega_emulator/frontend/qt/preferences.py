"""Validated, backend-independent Qt frontend preferences.

This module deliberately has no PySide6 dependency so configuration rules can be
unit-tested without creating a QApplication.
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Protocol

MIN_WINDOW_WIDTH = 640
MIN_WINDOW_HEIGHT = 480
MAX_WINDOW_WIDTH = 7680
MAX_WINDOW_HEIGHT = 4320
SUPPORTED_VIDEO_SCALES = (2, 3, 4)
MAX_RECENT_ROMS = 12


class SettingsStore(Protocol):
    """Small subset of QSettings used by the preferences adapter."""

    def value(self, key: str, defaultValue: Any = None, **kwargs: Any) -> Any: ...

    def setValue(self, key: str, value: Any) -> None: ...


@dataclass(frozen=True, slots=True)
class FrontendPreferences:
    """User-visible settings with defensive validation and stable defaults."""

    window_width: int = 1024
    window_height: int = 768
    video_scale: int = 3
    recent_roms: tuple[str, ...] = ()
    controller_config: str = ""
    firmware_directory: str = ""
    audio_enabled: bool = True
    diagnostics_enabled: bool = False
    cpu_core: str = "rust"
    audio_sample_rate: int = 48_000
    audio_buffer_size: int = 512
    audio_volume: int = 80
    audio_balance: int = 0
    audio_queue_blocks: int = 4
    video_filter: str = "nearest"
    integer_scale: bool = True
    sprite_limit_enabled: bool = True
    vsync_mode: str = "on"
    vrr_mode: str = "driver"
    av_sync_mode: str = "low_latency"
    crt_scanline_intensity: int = 40
    crt_scanline_thickness: int = 50
    crt_phosphor_mask: int = 25
    crt_curvature: int = 0
    crt_signal_mode: str = "rgb"

    def __post_init__(self) -> None:
        if not MIN_WINDOW_WIDTH <= self.window_width <= MAX_WINDOW_WIDTH:
            raise ValueError("window_width is outside the supported range")
        if not MIN_WINDOW_HEIGHT <= self.window_height <= MAX_WINDOW_HEIGHT:
            raise ValueError("window_height is outside the supported range")
        if self.video_scale not in SUPPORTED_VIDEO_SCALES:
            raise ValueError("video_scale must be one of 2, 3, or 4")
        if len(self.recent_roms) > MAX_RECENT_ROMS:
            raise ValueError(f"recent_roms may contain at most {MAX_RECENT_ROMS} items")
        if any(
            not isinstance(path, str) or not path.strip() for path in self.recent_roms
        ):
            raise ValueError("recent_roms must contain non-empty path strings")
        if not isinstance(self.controller_config, str):
            raise ValueError("controller_config must be serialized as text")
        if not isinstance(self.firmware_directory, str):
            raise ValueError("firmware_directory must be a path string")
        if self.cpu_core != "rust":
            raise ValueError("cpu_core must be 'rust'; the Qt runtime is Rust-only")
        if self.audio_sample_rate not in (22_050, 44_100, 48_000):
            raise ValueError("audio_sample_rate must be 22050, 44100, or 48000")
        if self.audio_buffer_size not in (256, 512, 1024, 2048):
            raise ValueError("audio_buffer_size must be 256, 512, 1024, or 2048")
        if type(self.audio_volume) is not int or not 0 <= self.audio_volume <= 100:
            raise ValueError("audio_volume must be an integer from 0 to 100")
        if type(self.audio_balance) is not int or not -100 <= self.audio_balance <= 100:
            raise ValueError("audio_balance must be an integer from -100 to 100")
        if type(self.audio_queue_blocks) is not int or self.audio_queue_blocks not in (
            2,
            4,
            8,
        ):
            raise ValueError("audio_queue_blocks must be 2, 4, or 8")
        if self.video_filter not in (
            "nearest",
            "bilinear",
            "dither_blending",
            "crt_ntsc",
        ):
            raise ValueError(
                "video_filter must be nearest, bilinear, dither_blending, or crt_ntsc"
            )
        if type(self.integer_scale) is not bool:
            raise ValueError("integer_scale must be a bool")
        if type(self.sprite_limit_enabled) is not bool:
            raise ValueError("sprite_limit_enabled must be a bool")
        if self.vsync_mode not in ("on", "off", "adaptive"):
            raise ValueError("vsync_mode must be on, off, or adaptive")
        if self.vrr_mode not in ("driver", "prefer", "off"):
            raise ValueError("vrr_mode must be driver, prefer, or off")
        if self.av_sync_mode not in ("low_latency", "audio_clock"):
            raise ValueError("av_sync_mode must be low_latency or audio_clock")
        if self.crt_signal_mode not in ("rgb", "composite", "rf"):
            raise ValueError("crt_signal_mode must be rgb, composite, or rf")
        for field_name in (
            "crt_scanline_intensity",
            "crt_scanline_thickness",
            "crt_phosphor_mask",
            "crt_curvature",
        ):
            value = getattr(self, field_name)
            if type(value) is not int or not 0 <= value <= 100:
                raise ValueError(f"{field_name} must be an integer from 0 to 100")

    @classmethod
    def from_store(cls, store: SettingsStore) -> FrontendPreferences:
        """Read persisted values, falling back safely on invalid or legacy values."""
        defaults = cls()
        width = _bounded_int(
            store.value("window_width", defaults.window_width),
            defaults.window_width,
            MIN_WINDOW_WIDTH,
            MAX_WINDOW_WIDTH,
        )
        height = _bounded_int(
            store.value("window_height", defaults.window_height),
            defaults.window_height,
            MIN_WINDOW_HEIGHT,
            MAX_WINDOW_HEIGHT,
        )
        scale = _bounded_choice(
            store.value("video_scale", defaults.video_scale),
            defaults.video_scale,
            SUPPORTED_VIDEO_SCALES,
        )
        raw_recent = store.value("recent_roms", list(defaults.recent_roms))
        if isinstance(raw_recent, str):
            raw_recent = [raw_recent] if raw_recent else []
        if not isinstance(raw_recent, (list, tuple)):
            raw_recent = []
        recent = tuple(
            dict.fromkeys(
                item.strip()
                for item in raw_recent
                if isinstance(item, str) and item.strip()
            )
        )[:MAX_RECENT_ROMS]
        controller_config = store.value("controller_config", "")
        if not isinstance(controller_config, str):
            controller_config = ""
        firmware_directory = store.value("firmware_directory", "")
        if not isinstance(firmware_directory, str):
            firmware_directory = ""
        firmware_directory = firmware_directory.strip()
        audio_enabled = _bool_value(
            store.value("audio_enabled", defaults.audio_enabled),
            defaults.audio_enabled,
        )
        # Detailed logging is session-only and opt-in from the Debug tab.
        # Do not silently re-enable heavy logs on the next application launch.
        diagnostics_enabled = False
        # Migrate legacy profiles that persisted the removed Python runtime.
        cpu_core = "rust"
        if store.value("cpu_core", defaults.cpu_core) != "rust":
            store.setValue("cpu_core", "rust")
        sample_rate = _bounded_choice(
            store.value("audio_sample_rate", defaults.audio_sample_rate),
            defaults.audio_sample_rate,
            (22_050, 44_100, 48_000),
        )
        buffer_size = _bounded_choice(
            store.value("audio_buffer_size", defaults.audio_buffer_size),
            defaults.audio_buffer_size,
            (256, 512, 1024, 2048),
        )
        volume = _bounded_int(
            store.value("audio_volume", defaults.audio_volume),
            defaults.audio_volume,
            0,
            100,
        )
        balance = _bounded_int(
            store.value("audio_balance", defaults.audio_balance),
            defaults.audio_balance,
            -100,
            100,
        )
        queue_blocks = _bounded_choice(
            store.value("audio_queue_blocks", defaults.audio_queue_blocks),
            defaults.audio_queue_blocks,
            (2, 4, 8),
        )
        video_filter = store.value("video_filter", defaults.video_filter)
        if video_filter not in ("nearest", "bilinear", "dither_blending", "crt_ntsc"):
            video_filter = defaults.video_filter
        crt_signal_mode = store.value("crt_signal_mode", defaults.crt_signal_mode)
        if crt_signal_mode not in ("rgb", "composite", "rf"):
            crt_signal_mode = defaults.crt_signal_mode
        integer_scale = _bool_value(
            store.value("integer_scale", defaults.integer_scale), defaults.integer_scale
        )
        sprite_limit_enabled = _bool_value(
            store.value("sprite_limit_enabled", defaults.sprite_limit_enabled),
            defaults.sprite_limit_enabled,
        )
        vsync_mode = store.value("vsync_mode", defaults.vsync_mode)
        if vsync_mode not in ("on", "off", "adaptive"):
            vsync_mode = defaults.vsync_mode
        vrr_mode = store.value("vrr_mode", defaults.vrr_mode)
        if vrr_mode not in ("driver", "prefer", "off"):
            vrr_mode = defaults.vrr_mode
        av_sync_mode = store.value("av_sync_mode", defaults.av_sync_mode)
        if av_sync_mode not in ("low_latency", "audio_clock"):
            av_sync_mode = defaults.av_sync_mode
        crt_values = {
            name: _bounded_int(
                store.value(name, getattr(defaults, name)),
                getattr(defaults, name),
                0,
                100,
            )
            for name in (
                "crt_scanline_intensity",
                "crt_scanline_thickness",
                "crt_phosphor_mask",
                "crt_curvature",
            )
        }
        return cls(
            window_width=width,
            window_height=height,
            video_scale=scale,
            recent_roms=recent,
            controller_config=controller_config,
            firmware_directory=firmware_directory,
            audio_enabled=audio_enabled,
            diagnostics_enabled=diagnostics_enabled,
            cpu_core=cpu_core,
            audio_sample_rate=sample_rate,
            audio_buffer_size=buffer_size,
            audio_volume=volume,
            audio_balance=balance,
            audio_queue_blocks=queue_blocks,
            video_filter=video_filter,
            integer_scale=integer_scale,
            sprite_limit_enabled=sprite_limit_enabled,
            vsync_mode=vsync_mode,
            vrr_mode=vrr_mode,
            av_sync_mode=av_sync_mode,
            crt_signal_mode=crt_signal_mode,
            **crt_values,
        )

    def save_to_store(self, store: SettingsStore) -> None:
        """Persist values using stable, explicit keys."""
        store.setValue("window_width", self.window_width)
        store.setValue("window_height", self.window_height)
        store.setValue("video_scale", self.video_scale)
        store.setValue("recent_roms", list(self.recent_roms))
        store.setValue("controller_config", self.controller_config)
        store.setValue("firmware_directory", self.firmware_directory)
        store.setValue("audio_enabled", self.audio_enabled)
        store.setValue("diagnostics_enabled", False)
        store.setValue("cpu_core", self.cpu_core)
        store.setValue("audio_sample_rate", self.audio_sample_rate)
        store.setValue("audio_buffer_size", self.audio_buffer_size)
        store.setValue("audio_volume", self.audio_volume)
        store.setValue("audio_balance", self.audio_balance)
        store.setValue("audio_queue_blocks", self.audio_queue_blocks)
        store.setValue("video_filter", self.video_filter)
        store.setValue("integer_scale", self.integer_scale)
        store.setValue("sprite_limit_enabled", self.sprite_limit_enabled)
        store.setValue("vsync_mode", self.vsync_mode)
        store.setValue("vrr_mode", self.vrr_mode)
        store.setValue("av_sync_mode", self.av_sync_mode)
        store.setValue("crt_scanline_intensity", self.crt_scanline_intensity)
        store.setValue("crt_scanline_thickness", self.crt_scanline_thickness)
        store.setValue("crt_phosphor_mask", self.crt_phosphor_mask)
        store.setValue("crt_curvature", self.crt_curvature)
        store.setValue("crt_signal_mode", self.crt_signal_mode)

    def with_recent_rom(self, path: str) -> FrontendPreferences:
        """Return preferences with a ROM moved to the front of the history."""
        normalized = path.strip()
        if not normalized:
            return self
        recent = (normalized,) + tuple(
            item
            for item in self.recent_roms
            if item.casefold() != normalized.casefold()
        )
        return FrontendPreferences(
            window_width=self.window_width,
            window_height=self.window_height,
            video_scale=self.video_scale,
            recent_roms=recent[:MAX_RECENT_ROMS],
            controller_config=self.controller_config,
            firmware_directory=self.firmware_directory,
            audio_enabled=self.audio_enabled,
            diagnostics_enabled=self.diagnostics_enabled,
            cpu_core=self.cpu_core,
            audio_sample_rate=self.audio_sample_rate,
            audio_buffer_size=self.audio_buffer_size,
            audio_volume=self.audio_volume,
            audio_balance=self.audio_balance,
            audio_queue_blocks=self.audio_queue_blocks,
            video_filter=self.video_filter,
            integer_scale=self.integer_scale,
            sprite_limit_enabled=self.sprite_limit_enabled,
            vsync_mode=self.vsync_mode,
            vrr_mode=self.vrr_mode,
            av_sync_mode=self.av_sync_mode,
            crt_scanline_intensity=self.crt_scanline_intensity,
            crt_scanline_thickness=self.crt_scanline_thickness,
            crt_phosphor_mask=self.crt_phosphor_mask,
            crt_curvature=self.crt_curvature,
            crt_signal_mode=self.crt_signal_mode,
        )


def _bounded_int(value: Any, default: int, minimum: int, maximum: int) -> int:
    try:
        result = int(value)
    except TypeError:
        return default
    except ValueError:
        return default
    except OverflowError:
        return default
    return result if minimum <= result <= maximum else default


def _bounded_choice(value: Any, default: int, choices: tuple[int, ...]) -> int:
    result = _bounded_int(value, default, min(choices), max(choices))
    return result if result in choices else default


def _bool_value(value: Any, default: bool) -> bool:
    if isinstance(value, bool):
        return value
    if isinstance(value, int) and value in (0, 1):
        return bool(value)
    if isinstance(value, str):
        normalized = value.strip().casefold()
        if normalized in {"true", "1", "yes", "on"}:
            return True
        if normalized in {"false", "0", "no", "off"}:
            return False
    return default
