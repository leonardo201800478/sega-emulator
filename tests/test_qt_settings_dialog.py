"""Offscreen tests for the Qt audio/video settings dialog."""

from __future__ import annotations

import os

os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")

import pytest  # noqa: E402

pytest.importorskip("PySide6")

from PySide6.QtWidgets import QApplication, QTabWidget  # noqa: E402

from sega_emulator.frontend.qt.preferences import FrontendPreferences  # noqa: E402
from sega_emulator.frontend.qt.settings_dialog import SettingsDialog  # noqa: E402


@pytest.fixture
def qt_app():
    app = QApplication.instance() or QApplication([])
    yield app


def test_dialog_loads_and_returns_audio_video_preferences(qt_app) -> None:
    original = FrontendPreferences(
        audio_enabled=False,
        audio_sample_rate=48_000,
        audio_buffer_size=1024,
        audio_volume=65,
        video_filter="bilinear",
        integer_scale=False,
        sprite_limit_enabled=False,
    )
    dialog = SettingsDialog(original)

    tabs = dialog.findChild(QTabWidget)
    assert tabs is not None
    assert [tabs.tabText(index) for index in range(tabs.count())] == [
        "Saída de áudio",
        "PSG / Modelo",
        "Vídeo",
        "CRT / NTSC",
    ]
    assert dialog.audio_enabled.isChecked() is False
    assert dialog.sample_rate.currentData() == 48_000
    assert dialog.buffer_size.currentData() == 1024
    assert dialog.volume.value() == 65
    assert dialog.video_filter.currentData() == "bilinear"
    assert dialog.integer_scale.isChecked() is False
    assert dialog.sprite_limit_unlocked.isChecked() is True
    assert dialog.vsync_mode.currentData() == "on"
    assert dialog.vrr_mode.currentData() == "driver"
    assert dialog.av_sync_mode.currentData() == "low_latency"
    assert dialog.crt_signal_mode.currentData() == "rgb"
    assert dialog.video_filter.findData("dither_blending") >= 0
    assert dialog.video_filter.findData("crt_ntsc") >= 0

    dialog.audio_enabled.setChecked(True)
    dialog.sample_rate.setCurrentIndex(dialog.sample_rate.findData(22_050))
    dialog.buffer_size.setCurrentIndex(dialog.buffer_size.findData(256))
    dialog.volume.setValue(40)
    dialog.video_filter.setCurrentIndex(dialog.video_filter.findData("dither_blending"))
    dialog.integer_scale.setChecked(True)
    dialog.sprite_limit_unlocked.setChecked(False)
    dialog.vsync_mode.setCurrentIndex(dialog.vsync_mode.findData("off"))
    dialog.vrr_mode.setCurrentIndex(dialog.vrr_mode.findData("prefer"))
    dialog.av_sync_mode.setCurrentIndex(dialog.av_sync_mode.findData("audio_clock"))
    dialog.crt_scanline_intensity.setValue(55)
    dialog.crt_signal_mode.setCurrentIndex(dialog.crt_signal_mode.findData("composite"))

    updated = dialog.preferences()
    assert updated.audio_enabled is True
    assert updated.audio_sample_rate == 22_050
    assert updated.audio_buffer_size == 256
    assert updated.audio_volume == 40
    assert updated.video_filter == "dither_blending"
    assert updated.integer_scale is True
    assert updated.sprite_limit_enabled is True
    assert updated.vsync_mode == "off"
    assert updated.vrr_mode == "prefer"
    assert updated.av_sync_mode == "audio_clock"
    assert updated.crt_scanline_intensity == 55
    assert updated.crt_signal_mode == "composite"
    # Editing presentation settings must not switch the selected emulation core.
    assert updated.cpu_core == original.cpu_core
    dialog.close()


def test_restore_defaults_resets_audio_video_controls(qt_app) -> None:
    dialog = SettingsDialog(
        FrontendPreferences(
            audio_enabled=False,
            audio_sample_rate=48_000,
            audio_buffer_size=2048,
            audio_volume=10,
            video_filter="bilinear",
            integer_scale=False,
            sprite_limit_enabled=False,
            vsync_mode="off",
            vrr_mode="prefer",
            av_sync_mode="audio_clock",
            crt_scanline_intensity=80,
            crt_signal_mode="rf",
        )
    )

    dialog._restore_defaults()

    defaults = FrontendPreferences()
    assert dialog.audio_enabled.isChecked() is defaults.audio_enabled
    assert dialog.sample_rate.currentData() == defaults.audio_sample_rate
    assert dialog.buffer_size.currentData() == defaults.audio_buffer_size
    assert dialog.volume.value() == defaults.audio_volume
    assert dialog.video_filter.currentData() == defaults.video_filter
    assert dialog.integer_scale.isChecked() is defaults.integer_scale
    assert dialog.sprite_limit_unlocked.isChecked() is (
        not defaults.sprite_limit_enabled
    )
    assert dialog.vsync_mode.currentData() == defaults.vsync_mode
    assert dialog.vrr_mode.currentData() == defaults.vrr_mode
    assert dialog.av_sync_mode.currentData() == defaults.av_sync_mode
    assert dialog.crt_scanline_intensity.value() == defaults.crt_scanline_intensity
    assert dialog.crt_signal_mode.currentData() == defaults.crt_signal_mode
    dialog.close()
