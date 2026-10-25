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
    audio_enabled: bool = True
    diagnostics_enabled: bool = True

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
        audio_enabled = _bool_value(
            store.value("audio_enabled", defaults.audio_enabled),
            defaults.audio_enabled,
        )
        diagnostics_enabled = _bool_value(
            store.value("diagnostics_enabled", defaults.diagnostics_enabled),
            defaults.diagnostics_enabled,
        )
        return cls(
            window_width=width,
            window_height=height,
            video_scale=scale,
            recent_roms=recent,
            controller_config=controller_config,
            audio_enabled=audio_enabled,
            diagnostics_enabled=diagnostics_enabled,
        )

    def save_to_store(self, store: SettingsStore) -> None:
        """Persist values using stable, explicit keys."""
        store.setValue("window_width", self.window_width)
        store.setValue("window_height", self.window_height)
        store.setValue("video_scale", self.video_scale)
        store.setValue("recent_roms", list(self.recent_roms))
        store.setValue("controller_config", self.controller_config)
        store.setValue("audio_enabled", self.audio_enabled)
        store.setValue("diagnostics_enabled", self.diagnostics_enabled)

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
            audio_enabled=self.audio_enabled,
            diagnostics_enabled=self.diagnostics_enabled,
        )


def _bounded_int(value: Any, default: int, minimum: int, maximum: int) -> int:
    try:
        result = int(value)
    except TypeError, ValueError, OverflowError:
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
