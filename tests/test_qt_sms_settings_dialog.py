"""Tests for the SMS-specific settings dialog."""

from __future__ import annotations

import os

import pytest

os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")
pytest.importorskip("PySide6")

from PySide6.QtWidgets import QApplication

from sega_emulator.frontend.qt.preferences import FrontendPreferences
from sega_emulator.frontend.qt.sms_settings_dialog import SmsSettingsDialog


@pytest.fixture(scope="module")
def qapplication() -> QApplication:
    return QApplication.instance() or QApplication([])


def test_sms_settings_dialog_returns_validated_audio_video_and_region_preferences(
    qapplication: QApplication,
) -> None:
    dialog = SmsSettingsDialog(
        FrontendPreferences(),
        region="brazil",
        audio_backend="wasapi",
    )
    dialog.audio_backend.setCurrentIndex(dialog.audio_backend.findData("xaudio2"))
    dialog.sample_rate.setCurrentIndex(dialog.sample_rate.findData(44_100))
    dialog.buffer_size.setCurrentIndex(dialog.buffer_size.findData(1024))
    dialog.queue_blocks.setCurrentIndex(dialog.queue_blocks.findData(8))
    dialog.volume.setValue(65)
    dialog.balance.setValue(25)
    dialog.integer_scale.setChecked(False)
    dialog.region.setCurrentIndex(dialog.region.findData("europe"))

    preferences = dialog.preferences()

    assert dialog.selected_audio_backend == "xaudio2"
    assert dialog.selected_region == "europe"
    assert preferences.audio_enabled
    assert preferences.audio_sample_rate == 44_100
    assert preferences.audio_buffer_size == 1024
    assert preferences.audio_queue_blocks == 8
    assert preferences.audio_volume == 65
    assert preferences.audio_balance == 25
    assert not preferences.integer_scale
    dialog.close()


def test_sms_settings_dialog_can_disable_audio_and_restore_defaults(
    qapplication: QApplication,
) -> None:
    dialog = SmsSettingsDialog(
        FrontendPreferences(audio_volume=20, integer_scale=False),
        region="japan",
        audio_backend="xaudio2",
    )
    dialog.audio_backend.setCurrentIndex(dialog.audio_backend.findData("disabled"))

    preferences = dialog.preferences()
    assert dialog.selected_audio_backend == "disabled"
    assert not preferences.audio_enabled

    dialog._restore_defaults()
    restored = dialog.preferences()
    assert dialog.selected_audio_backend == "wasapi"
    assert dialog.selected_region == "usa"
    assert restored.audio_enabled
    assert restored.audio_volume == 80
    assert restored.integer_scale
    dialog.close()
