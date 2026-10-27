"""Testes do carregador de ROM e da instrumentação de diagnóstico."""

import json
import zipfile

import pytest

from sega_emulator.diagnostics import configure_session_logging
from sega_emulator.rom_loader import list_rom_members, load_rom


def test_load_sg_rom_and_compute_sha256(tmp_path):
    rom = bytes([0x00, 0xC3, 0x00, 0x00])
    path = tmp_path / "SuperUwol-SG.sg"
    path.write_bytes(rom)

    loaded = load_rom(path)

    assert loaded.data == rom
    assert loaded.display_name == "SuperUwol-SG.sg"
    assert loaded.sha256 == __import__("hashlib").sha256(rom).hexdigest()
    assert loaded.member_name is None


def test_load_single_rom_from_zip_without_extracting(tmp_path):
    rom = bytes([0x00, 0xC3, 0x00, 0x00])
    path = tmp_path / "SuperUwol-SG.zip"
    with zipfile.ZipFile(path, "w") as archive:
        archive.writestr("roms/SuperUwol-SG.sg", rom)
        archive.writestr("readme.txt", "test")

    loaded = load_rom(path)

    assert loaded.data == rom
    assert loaded.member_name == "roms/SuperUwol-SG.sg"
    assert list_rom_members(path) == ["roms/SuperUwol-SG.sg"]
    assert not (tmp_path / "roms").exists()


def test_zip_with_multiple_roms_requires_explicit_selection(tmp_path):
    path = tmp_path / "multi.zip"
    with zipfile.ZipFile(path, "w") as archive:
        archive.writestr("one.sg", b"\x00")
        archive.writestr("two.rom", b"\x01")

    with pytest.raises(ValueError, match="várias ROMs"):
        load_rom(path)


def test_invalid_extension_and_corrupt_zip_are_reported(tmp_path):
    path = tmp_path / "game.txt"
    path.write_text("not a rom", encoding="utf-8")
    with pytest.raises(ValueError, match="Formato não suportado"):
        load_rom(path)

    corrupt = tmp_path / "bad.zip"
    corrupt.write_bytes(b"not zip")
    with pytest.raises(ValueError, match="ZIP válido"):
        load_rom(corrupt)


def test_rom_size_limit_is_enforced(tmp_path):
    path = tmp_path / "oversize.sg"
    path.write_bytes(bytes(0xC001))
    with pytest.raises(ValueError, match="até 49152 bytes"):
        load_rom(path)


def test_session_logging_preserves_unrelated_root_handlers(tmp_path):
    import logging

    root = logging.getLogger()
    sentinel = logging.NullHandler()
    root.addHandler(sentinel)
    try:
        configure_session_logging(tmp_path)
        assert sentinel in root.handlers
    finally:
        root.removeHandler(sentinel)


def test_session_logging_writes_json_lines(tmp_path):
    import logging

    json_path = configure_session_logging(tmp_path)
    logging.getLogger("test.emulator").info(
        "ROM carregada",
        extra={"event": "rom_loaded", "rom_sha256": "abc123", "frame": 4},
    )
    for handler in logging.getLogger().handlers:
        handler.flush()

    records = [
        json.loads(line) for line in json_path.read_text(encoding="utf-8").splitlines()
    ]
    assert records[0]["event"] == "session_start"
    assert records[1]["event"] == "rom_loaded"
    assert records[1]["rom_sha256"] == "abc123"
    assert records[1]["frame"] == 4


def test_cli_help_exposes_diagnostic_options(capsys):
    import sega_emulator.__main__ as cli

    with pytest.raises(SystemExit) as result:
        cli.main(["--help"])
    assert result.value.code == 0
    assert "--summary-json" in capsys.readouterr().out


def test_cli_rejects_zero_headless_frames():
    import sega_emulator.__main__ as cli

    with pytest.raises(SystemExit) as result:
        cli.main(["--headless-frames", "0"])
    assert result.value.code == 2


def test_cli_lists_roms_from_zip(tmp_path, capsys):
    import sega_emulator.__main__ as cli

    archive_path = tmp_path / "games.zip"
    with zipfile.ZipFile(archive_path, "w") as archive:
        archive.writestr("games/test.sg", bytes([0, 195, 0, 0]))
        archive.writestr("readme.txt", "not a ROM")

    assert cli.main(["--list-roms", str(archive_path)]) == 0
    payload = json.loads(capsys.readouterr().out)
    assert payload["roms"] == ["games/test.sg"]
