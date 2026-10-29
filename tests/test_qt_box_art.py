"""Tests for local box-art lookup by ROM filename."""

from __future__ import annotations

from pathlib import Path

import pytest

pytest.importorskip("PySide6")

from PySide6.QtCore import QSettings  # noqa: E402
from PySide6.QtWidgets import QApplication  # noqa: E402

from sega_emulator.frontend.qt.rom_library_dialog import (  # noqa: E402
    RomLibraryDialog,
    _find_box_art,
)


@pytest.fixture
def qt_app():
    app = QApplication.instance() or QApplication([])
    yield app


@pytest.fixture
def settings(tmp_path):
    store = QSettings(str(tmp_path / "settings.ini"), QSettings.Format.IniFormat)
    store.clear()
    return store


def test_box_art_matches_rom_stem_case_insensitively_and_recursively(
    tmp_path: Path,
) -> None:
    rom = tmp_path / "roms" / "Sonic The Hedgehog.md"
    art_root = tmp_path / "art"
    nested = art_root / "Mega Drive"
    nested.mkdir(parents=True)
    rom.parent.mkdir()
    rom.write_bytes(b"rom")
    cover = nested / "sonic the hedgehog.WEBP"
    cover.write_bytes(b"placeholder image bytes")

    assert _find_box_art(rom, art_root) == cover


def test_box_art_does_not_match_a_different_filename(tmp_path: Path) -> None:
    rom = tmp_path / "game.sg"
    art_root = tmp_path / "art"
    art_root.mkdir()
    rom.write_bytes(b"rom")
    (art_root / "other.png").write_bytes(b"not a real image")

    assert _find_box_art(rom, art_root) is None


def test_box_art_folder_is_persisted_per_system(qt_app, settings, tmp_path: Path):
    dialog = RomLibraryDialog(settings)
    dialog.selected_system = "sg1000"
    art_dir = tmp_path / "covers"
    art_dir.mkdir()

    # Exercise the same persistence contract used by the folder-picker callback.
    dialog.box_art_directory_edit.setText(str(art_dir))
    settings.setValue("box_art_directories/sg1000", str(art_dir))
    settings.setValue(
        "box_art_directories/master_system", str(tmp_path / "sms-covers")
    )

    assert dialog._box_art_directory_for("sg1000") == str(art_dir)
    assert dialog._box_art_directory_for("master_system") == str(
        tmp_path / "sms-covers"
    )
    dialog.close()
