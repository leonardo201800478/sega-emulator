"""Regression tests for CLI diagnostics and machine-speed summaries."""

from __future__ import annotations

import json
import zipfile
from pathlib import Path
from types import SimpleNamespace

import pytest

import sega_emulator.__main__ as cli


class FakeMachine:
    def __init__(self, data: bytes, *, cpu_core: str) -> None:
        assert cpu_core == "rust"
        self.cpu = SimpleNamespace(pc=0x1234)
        self.vdp = SimpleNamespace(frame_number=0, scanline=0, status=0)
        self.total_instructions = 0
        self.total_tstates = 0

    def run_frames(self, frames: int) -> None:
        self.vdp.frame_number += frames
        self.total_instructions += frames * 10
        self.total_tstates += frames * 59_736


def test_cli_help_and_version(capsys: pytest.CaptureFixture[str]) -> None:
    with pytest.raises(SystemExit) as help_exit:
        cli.main(["--help"])
    assert help_exit.value.code == 0
    assert "--summary-json" in capsys.readouterr().out

    with pytest.raises(SystemExit) as version_exit:
        cli.main(["--version"])
    assert version_exit.value.code == 0
    assert "SG-1000" in capsys.readouterr().out


def test_headless_frames_must_be_positive() -> None:
    with pytest.raises(SystemExit) as result:
        cli.main(["--headless-frames", "0"])
    assert result.value.code == 2


def test_list_roms_prints_supported_zip_members(
    tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    archive_path = tmp_path / "games.zip"
    with zipfile.ZipFile(archive_path, "w") as archive:
        archive.writestr("games/test.sg", b"\x00\xC3\x00\x00")
        archive.writestr("readme.txt", "not a ROM")

    assert cli.main(["--list-roms", str(archive_path)]) == 0
    payload = json.loads(capsys.readouterr().out)
    assert payload["roms"] == ["games/test.sg"]


def test_headless_summary_contains_measured_metrics(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, capsys: pytest.CaptureFixture[str]
) -> None:
    rom_path = tmp_path / "test.sg"
    rom_path.write_bytes(b"\x00\xC3\x00\x00")
    report_path = tmp_path / "nested" / "summary.json"
    loaded = SimpleNamespace(
        data=rom_path.read_bytes(),
        source_path=rom_path,
        member_name=None,
        sha256="a" * 64,
        display_name=rom_path.name,
    )
    monkeypatch.setattr(cli, "configure_session_logging", lambda *args, **kwargs: None)
    monkeypatch.setattr(cli, "load_rom", lambda path: loaded)
    monkeypatch.setattr(cli, "SG1000Machine", FakeMachine)

    result = cli.main([
        str(rom_path), "--ui", "pygame", "--headless-frames", "2",
        "--summary-json", str(report_path), "--log-dir", str(tmp_path / "logs"),
    ])

    assert result == 0
    report = json.loads(report_path.read_text(encoding="utf-8"))
    assert report["schema_version"] == 1
    assert report["rom"]["sha256"] == "a" * 64
    assert report["measurement"]["frames"] == 2
    assert report["measurement"]["instructions"] == 20
    assert report["measurement"]["tstates"] == 119_472
    assert report["measurement"]["elapsed_seconds"] > 0
    assert report["measurement"]["speed_percent_of_nominal_z80_clock"] > 0
    assert "velocidade=" in capsys.readouterr().out
