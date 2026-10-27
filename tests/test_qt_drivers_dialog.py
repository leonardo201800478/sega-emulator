"""Tests for the global host-driver selector."""
from __future__ import annotations

import os

os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")

import pytest  # noqa: E402

pytest.importorskip("PySide6")

from PySide6.QtWidgets import QApplication  # noqa: E402

from sega_emulator.frontend.qt.drivers_dialog import DriversDialog  # noqa: E402


@pytest.fixture
def qt_app():
    app = QApplication.instance() or QApplication([])
    yield app


def test_drivers_dialog_exposes_only_implemented_api_choices(qt_app) -> None:
    dialog = DriversDialog()
    assert [
        dialog.input_combo.itemData(i) for i in range(dialog.input_combo.count())
    ] == ["auto", "xinput", "sdl3"]
    assert [
        dialog.audio_combo.itemData(i) for i in range(dialog.audio_combo.count())
    ] == ["wasapi", "disabled"]
    assert [
        dialog.video_combo.itemData(i) for i in range(dialog.video_combo.count())
    ] == ["auto", "vulkan", "raster"]
    assert dialog.drivers() == ("auto", "wasapi", "auto")
    dialog.close()


def test_drivers_dialog_restores_global_defaults(qt_app) -> None:
    dialog = DriversDialog(
        input_driver="sdl3", audio_driver="disabled", video_driver="raster"
    )
    dialog._restore_defaults()
    assert dialog.drivers() == ("auto", "wasapi", "auto")
    dialog.close()
