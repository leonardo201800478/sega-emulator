from __future__ import annotations

from sega_emulator.frontend.qt.preferences import FrontendPreferences
from sega_emulator.frontend.qt_frontend import _audio_output_warning


def test_audio_warning_when_enabled_backend_failed() -> None:
    warning = _audio_output_warning(FrontendPreferences(audio_enabled=True), None)

    assert warning is not None
    assert "WASAPI indisponível" in warning
    assert "log" in warning


def test_no_audio_warning_when_disabled() -> None:
    warning = _audio_output_warning(FrontendPreferences(audio_enabled=False), None)

    assert warning is None


def test_no_audio_warning_when_backend_is_open() -> None:
    warning = _audio_output_warning(FrontendPreferences(audio_enabled=True), object())

    assert warning is None
