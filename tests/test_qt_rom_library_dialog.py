"""Offscreen tests for ROM-library window controls and grid layout."""

from __future__ import annotations

import os

os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")

import pytest

pytest.importorskip("PySide6")

from PySide6.QtCore import QSettings, QSize, Qt  # noqa: E402
from PySide6.QtWidgets import QApplication  # noqa: E402

from sega_emulator.frontend.qt.rom_library_dialog import RomLibraryDialog  # noqa: E402


@pytest.fixture
def qt_app():
    app = QApplication.instance() or QApplication([])
    yield app


@pytest.fixture
def settings(tmp_path):
    store = QSettings(str(tmp_path / "settings.ini"), QSettings.Format.IniFormat)
    store.clear()
    return store


def test_library_window_has_standard_window_controls(qt_app, settings):
    dialog = RomLibraryDialog(settings)
    flags = dialog.windowFlags()

    assert flags & Qt.WindowType.WindowMinimizeButtonHint
    assert flags & Qt.WindowType.WindowMaximizeButtonHint
    assert flags & Qt.WindowType.WindowCloseButtonHint
    assert dialog.isFullScreen() is False
    dialog.close()


def test_rom_browser_uses_fixed_wrapping_grid_cells(qt_app, settings):
    dialog = RomLibraryDialog(settings)

    assert dialog.rom_list.isWrapping() is True
    assert dialog.rom_list.gridSize() == QSize(184, 244)
    assert dialog.rom_list.iconSize() == QSize(148, 184)
    assert dialog.rom_list.horizontalScrollBarPolicy() == Qt.ScrollBarPolicy.ScrollBarAlwaysOff
    assert dialog.rom_list.verticalScrollBarPolicy() == Qt.ScrollBarPolicy.ScrollBarAsNeeded
    dialog.close()
