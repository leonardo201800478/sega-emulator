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


def test_box_art_matches_launchbox_front_art_with_approximate_name(
    tmp_path: Path,
) -> None:
    rom = tmp_path / "roms" / "Sonic The Hedgehog (Japan).zip"
    art_root = tmp_path / "art"
    nested = art_root / "Box - Front" / "Mega Drive"
    nested.mkdir(parents=True)
    rom.parent.mkdir()
    rom.write_bytes(b"rom")
    cover = nested / "Sonic Hedgehog - Box - Front.png"
    cover.write_bytes(b"placeholder image bytes")

    assert _find_box_art(rom, art_root) == cover


def test_manual_box_art_override_is_saved_per_rom_and_system(
    qt_app, settings, tmp_path: Path
) -> None:
    dialog = RomLibraryDialog(settings)
    rom = tmp_path / "Sonic The Hedgehog (Japan).zip"
    cover = tmp_path / "cover.png"
    rom.write_bytes(b"rom")
    cover.write_bytes(b"image")
    dialog.selected_system = "sg1000"

    settings.setValue(dialog._manual_box_art_key(rom), str(cover))

    assert dialog._manual_box_art_for(rom) == cover
    dialog.selected_system = "master_system"
    assert dialog._manual_box_art_for(rom) is None
    dialog.close()


def test_box_art_matches_launchbox_numbered_filename_and_prefers_first_variant(
    tmp_path: Path,
) -> None:
    rom = tmp_path / "roms" / "007 James Bond (Japan) (En,Ja).zip"
    art_root = tmp_path / "art" / "Box - Front" / "Japan"
    art_root.mkdir(parents=True)
    rom.parent.mkdir()
    rom.write_bytes(b"rom")
    first = art_root / "007 James Bond-01.jpg"
    second = art_root / "007 James Bond-02.jpg"
    first.write_bytes(b"first")
    second.write_bytes(b"second")

    assert _find_box_art(rom, art_root.parent) == first


def test_box_art_matches_punctuation_variants(tmp_path: Path) -> None:
    rom = tmp_path / "roms" / "C_Sol! (Japan).zip"
    art_root = tmp_path / "art" / "Japan"
    art_root.mkdir(parents=True)
    rom.parent.mkdir()
    rom.write_bytes(b"rom")
    cover = art_root / "C So!-01.jpg"
    cover.write_bytes(b"image")

    assert _find_box_art(rom, art_root.parent) == cover


def test_box_art_does_not_match_a_different_filename(tmp_path: Path) -> None:
    rom = tmp_path / "game.sg"
    art_root = tmp_path / "art"
    art_root.mkdir()
    rom.write_bytes(b"rom")
    (art_root / "other.png").write_bytes(b"not a real image")

    assert _find_box_art(rom, art_root) is None


def test_system_selector_uses_packaged_artwork_and_only_enables_sg1000(
    qt_app, settings
) -> None:
    dialog = RomLibraryDialog(settings)

    assert dialog.system_buttons["sg1000"].isEnabled()
    assert not dialog.system_buttons["master_system"].isEnabled()
    for system, button in dialog.system_buttons.items():
        assert not button.icon().isNull(), f"Missing system artwork: {system}"

    dialog.close()


def test_box_art_folder_is_persisted_per_system(qt_app, settings, tmp_path: Path):
    dialog = RomLibraryDialog(settings)
    dialog.selected_system = "sg1000"
    art_dir = tmp_path / "covers"
    art_dir.mkdir()

    # Exercise the same persistence contract used by the folder-picker callback.
    dialog.box_art_directory_edit.setText(str(art_dir))
    settings.setValue("box_art_directories/sg1000", str(art_dir))
    settings.setValue("box_art_directories/master_system", str(tmp_path / "sms-covers"))

    assert dialog._box_art_directory_for("sg1000") == str(art_dir)
    assert dialog._box_art_directory_for("master_system") == str(
        tmp_path / "sms-covers"
    )
    dialog.close()
