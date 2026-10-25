"""Unit tests for the modular Qt frontend preference model."""

from __future__ import annotations

import pytest

from sega_emulator.frontend.qt.preferences import (
    MAX_RECENT_ROMS,
    FrontendPreferences,
)


class MemorySettings:
    def __init__(self, values: dict[str, object] | None = None) -> None:
        self.values = dict(values or {})

    def value(self, key: str, defaultValue: object = None, **kwargs: object) -> object:
        return self.values.get(key, defaultValue)

    def setValue(self, key: str, value: object) -> None:
        self.values[key] = value


def test_defaults_are_valid_and_stable() -> None:
    preferences = FrontendPreferences()

    assert preferences.window_width == 1024
    assert preferences.window_height == 768
    assert preferences.video_scale == 3
    assert preferences.recent_roms == ()
    assert preferences.audio_enabled is True


@pytest.mark.parametrize(
    ("field", "value"),
    [
        ("window_width", 200),
        ("window_height", 200),
        ("video_scale", 5),
    ],
)
def test_rejects_invalid_explicit_preferences(field: str, value: int) -> None:
    with pytest.raises(ValueError):
        FrontendPreferences(**{field: value})


def test_load_recovers_from_corrupt_or_legacy_values() -> None:
    settings = MemorySettings(
        {
            "window_width": "bad",
            "window_height": 12000,
            "video_scale": 5,
            "recent_roms": [" game.sg ", "", "game.sg", None],
            "audio_enabled": "off",
        }
    )

    preferences = FrontendPreferences.from_store(settings)

    assert preferences.window_width == 1024
    assert preferences.window_height == 768
    assert preferences.video_scale == 3
    assert preferences.recent_roms == ("game.sg",)
    assert preferences.audio_enabled is False


def test_save_and_load_round_trip() -> None:
    original = FrontendPreferences(
        window_width=1280,
        window_height=720,
        video_scale=4,
        recent_roms=("one.sg", "two.zip"),
        controller_config='{"players": {}}',
        audio_enabled=False,
        diagnostics_enabled=False,
    )
    settings = MemorySettings()

    original.save_to_store(settings)

    assert FrontendPreferences.from_store(settings) == original


def test_recent_rom_moves_to_front_without_case_duplicates() -> None:
    preferences = FrontendPreferences(recent_roms=("one.sg", "two.zip"))

    updated = preferences.with_recent_rom("ONE.SG")

    assert updated.recent_roms == ("ONE.SG", "two.zip")


def test_recent_rom_history_is_bounded() -> None:
    preferences = FrontendPreferences(
        recent_roms=tuple(f"{index}.sg" for index in range(MAX_RECENT_ROMS))
    )

    updated = preferences.with_recent_rom("new.sg")

    assert len(updated.recent_roms) == MAX_RECENT_ROMS
    assert updated.recent_roms[0] == "new.sg"
