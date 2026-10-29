"""Offscreen tests for ROM-library window controls and grid layout."""

from __future__ import annotations

import os

os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")

import pytest

pytest.importorskip("PySide6")

from PySide6.QtCore import QSettings, QSize, Qt  # noqa: E402
from PySide6.QtWidgets import QApplication, QListWidgetItem  # noqa: E402

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
    assert (
        dialog.rom_list.horizontalScrollBarPolicy()
        == Qt.ScrollBarPolicy.ScrollBarAlwaysOff
    )
    assert (
        dialog.rom_list.verticalScrollBarPolicy()
        == Qt.ScrollBarPolicy.ScrollBarAsNeeded
    )
    dialog.close()


def test_master_system_is_available_but_unimplemented_systems_remain_disabled(
    qt_app, settings
):
    dialog = RomLibraryDialog(settings)

    assert dialog.system_buttons["sg1000"].isEnabled()
    assert dialog.system_buttons["master_system"].isEnabled()
    assert dialog.system_buttons["master_system"].text() == "Master System"
    assert not dialog.system_buttons["game_gear"].isEnabled()
    assert not dialog.system_buttons["mega_drive"].isEnabled()
    dialog.close()


@pytest.mark.parametrize("system", ["sg1000", "master_system"])
def test_supported_system_rom_can_be_launched_from_library(
    qt_app, settings, tmp_path, system
):
    dialog = RomLibraryDialog(settings)
    rom_path = tmp_path / ("game.sms" if system == "master_system" else "game.sg")
    rom_path.write_bytes(b"test rom")
    dialog.selected_system = system
    item = QListWidgetItem(rom_path.name)
    item.setData(Qt.ItemDataRole.UserRole, str(rom_path))
    dialog.rom_list.addItem(item)
    item.setSelected(True)
    dialog._update_launch_state()

    assert dialog.launch_button.isEnabled()

    dialog._launch_selected()

    assert dialog.selected_rom == rom_path
    assert dialog.selected_system == system
    assert dialog.result() == dialog.DialogCode.Accepted
    dialog.close()


def test_rom_library_search_filters_by_name_and_path(qt_app, settings, tmp_path):
    dialog = RomLibraryDialog(settings)
    for name in ("Sonic.sms", "Alex Kidd.sms", "Phantasy Star.sms"):
        item = QListWidgetItem(name)
        item.setData(Qt.ItemDataRole.UserRole, str(tmp_path / name))
        dialog.rom_list.addItem(item)

    dialog.search_edit.setText("sonic")

    assert not dialog.rom_list.item(0).isHidden()
    assert dialog.rom_list.item(1).isHidden()
    assert dialog.rom_list.item(2).isHidden()

    dialog.search_edit.setText(str(tmp_path / "Alex"))

    assert dialog.rom_list.item(0).isHidden()
    assert not dialog.rom_list.item(1).isHidden()
    assert dialog.rom_list.item(2).isHidden()
    dialog.close()


def test_open_library_uses_cached_index_without_starting_scan(
    qt_app, settings, tmp_path, monkeypatch
):
    dialog = RomLibraryDialog(settings)
    dialog._index_path = tmp_path / "library-index.json"
    rom_path = tmp_path / "cached.sms"
    rom_path.write_bytes(b"cached rom")
    from sega_emulator.rom_library import RomEntry, RomLibrary

    library = RomLibrary(dialog._index_path)
    library._entries["master_system"][str(rom_path)] = RomEntry(
        system="master_system",
        path=str(rom_path),
        name=rom_path.name,
        size=10,
        mtime_ns=1,
        sha256="0" * 64,
        suffix=".sms",
    )
    library._save_index()

    def fail_if_scanned(*args, **kwargs):
        raise AssertionError("Opening a system must not start a filesystem scan")

    monkeypatch.setattr(dialog, "_start_scan", fail_if_scanned)
    dialog._open_library("master_system")

    assert dialog.rom_list.count() == 1
    assert dialog.rom_list.item(0).text().startswith("cached.sms")
    assert "Atualizar biblioteca" in dialog.status_label.text()
    dialog.close()
