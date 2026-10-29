"""Tests for the Master System cartridge loader."""

from __future__ import annotations

import zipfile

import pytest

from sega_emulator.machine.sms import load_sms_rom


def test_sms_loader_accepts_rom_larger_than_sg1000_limit(tmp_path):
    rom = tmp_path / "test.sms"
    rom.write_bytes(bytes([0x00]) * 0x10000)

    loaded = load_sms_rom(rom)

    assert loaded.data == bytes([0x00]) * 0x10000
    assert loaded.display_name == "test.sms"
    assert len(loaded.sha256) == 64


def test_sms_loader_rejects_empty_rom(tmp_path):
    rom = tmp_path / "empty.sms"
    rom.write_bytes(b"")

    with pytest.raises(ValueError, match="vazia"):
        load_sms_rom(rom)


def test_sms_loader_rejects_zip_with_multiple_candidates(tmp_path):
    archive_path = tmp_path / "multiple.zip"
    with zipfile.ZipFile(archive_path, "w") as archive:
        archive.writestr("one.sms", bytes([0x00]) * 16)
        archive.writestr("two.sms", bytes([0x00]) * 16)

    with pytest.raises(ValueError, match="várias ROMs"):
        load_sms_rom(archive_path)


def test_sms_loader_rejects_oversized_rom(tmp_path):
    rom = tmp_path / "oversized.sms"
    rom.write_bytes(bytes([0x00]) * (0x100000 + 1))

    with pytest.raises(ValueError, match="excede o limite"):
        load_sms_rom(rom)
