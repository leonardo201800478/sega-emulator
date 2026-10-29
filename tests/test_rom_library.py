"""Tests for the non-destructive ROM library index."""

from __future__ import annotations

from pathlib import Path
from threading import Event

import pytest

from sega_emulator.rom_library import RomLibrary


def test_recursive_scan_indexes_candidates_and_duplicate_hashes(tmp_path: Path) -> None:
    root = tmp_path / "Mega Drive"
    nested = root / "nested"
    nested.mkdir(parents=True)
    first = nested / "game.md"
    second = root / "copy.bin"
    first.write_bytes(b"same data")
    second.write_bytes(b"same data")
    library = RomLibrary(tmp_path / "cache" / "library.json")

    result = library.scan("mega_drive", [root])

    assert len(result.entries) == 2
    assert result.added == 2
    assert result.duplicate_groups == (tuple(sorted((str(first.resolve()), str(second.resolve()))),),)
    assert library.entries("mega_drive") == result.entries


def test_incremental_scan_reuses_unchanged_entries_and_detects_changes(
    tmp_path: Path,
) -> None:
    root = tmp_path / "roms"
    root.mkdir()
    rom = root / "game.sg"
    rom.write_bytes(b"first")
    library = RomLibrary(tmp_path / "index.json")

    initial = library.scan("sg1000", [root])
    unchanged = library.scan("sg1000", [root])
    rom.write_bytes(b"changed content")
    updated = library.scan("sg1000", [root])

    assert initial.added == 1
    assert unchanged.unchanged == 1
    assert updated.updated == 1
    assert updated.entries[0].sha256 != initial.entries[0].sha256


def test_full_scan_rehashes_and_removed_files_are_reconciled(tmp_path: Path) -> None:
    root = tmp_path / "roms"
    root.mkdir()
    rom = root / "game.sg"
    rom.write_bytes(b"rom")
    library = RomLibrary(tmp_path / "index.json")
    library.scan("sg1000", [root])
    rom.unlink()

    result = library.scan("sg1000", [root], full=True)

    assert result.removed == 1
    assert result.entries == ()
    assert result.full_scan is True


def test_cancelled_scan_preserves_unvisited_cached_entries(tmp_path: Path) -> None:
    root = tmp_path / "roms"
    root.mkdir()
    for name in ("a.sg", "b.sg", "c.sg"):
        (root / name).write_bytes(name.encode())
    library = RomLibrary(tmp_path / "index.json")
    library.scan("sg1000", [root])

    cancel = Event()
    cancel.set()
    result = library.scan("sg1000", [root], full=True, cancel=cancel)

    assert result.cancelled is True
    assert len(result.entries) == 3
    assert result.removed == 0


def test_missing_root_is_reported_and_other_roots_are_scanned(tmp_path: Path) -> None:
    valid = tmp_path / "valid"
    valid.mkdir()
    (valid / "game.sms").write_bytes(b"candidate")
    library = RomLibrary(tmp_path / "index.json")

    result = library.scan("master_system", [tmp_path / "missing", valid])

    assert len(result.entries) == 1
    assert any("não encontrado" in error.message for error in result.errors)


def test_index_survives_restart_and_is_rebuilt_if_corrupt(tmp_path: Path) -> None:
    root = tmp_path / "roms"
    root.mkdir()
    (root / "game.gg").write_bytes(b"candidate")
    index = tmp_path / "index.json"
    RomLibrary(index).scan("game_gear", [root])

    restored = RomLibrary(index)
    assert len(restored.entries("game_gear")) == 1

    index.write_text("{invalid json", encoding="utf-8")
    rebuilt = RomLibrary(index)
    assert rebuilt.entries("game_gear") == ()


def test_unknown_system_is_rejected(tmp_path: Path) -> None:
    library = RomLibrary(tmp_path / "index.json")
    with pytest.raises(ValueError, match="Sistema desconhecido"):
        library.entries("dreamcast")


def test_default_directories_are_separate_per_system(tmp_path: Path) -> None:
    directories = RomLibrary.default_directories(tmp_path / "ROMs")

    assert directories["sg1000"] == (tmp_path / "ROMs" / "SG-1000",)
    assert directories["master_system"] == (tmp_path / "ROMs" / "Master System",)
    assert len(set(directories.values())) == 6
