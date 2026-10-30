"""Tests for the SMS batch audit service."""

from __future__ import annotations

import json
import zipfile
from threading import Event

from sega_emulator.sms_rom_audit import (
    discover_sms_rom_targets,
    run_sms_rom_audit,
)


class _FakeVdp:
    def __init__(self, *, uniform: bool = False) -> None:
        self.frame_number = 0
        self.visible_height = 192
        self.framebuffer = (
            bytes([0, 0, 0]) * (256 * 192)
            if uniform
            else bytes([0, 0, 0, 255, 255, 255]) * (256 * 192 // 2)
        )


class _FakeMachine:
    def __init__(self, *, uniform: bool = False) -> None:
        self.vdp = _FakeVdp(uniform=uniform)
        self.total_instructions = 0
        self.total_tstates = 0
        self.cpu = type("Cpu", (), {"pc": 0x1234})()

    def run_frame(self, *, max_steps: int) -> int:
        assert max_steps > 0
        self.vdp.frame_number += 1
        self.total_instructions += 100
        self.total_tstates += 400
        return 400


def test_discovery_finds_loose_roms_and_every_supported_zip_member(tmp_path):
    (tmp_path / "nested").mkdir()
    (tmp_path / "nested" / "one.sms").write_bytes(b"rom-one")
    with zipfile.ZipFile(tmp_path / "collection.zip", "w") as archive:
        archive.writestr("folder/two.sms", b"rom-two")
        archive.writestr("three.bin", b"rom-three")
        archive.writestr("readme.txt", "not a ROM")

    targets = discover_sms_rom_targets(tmp_path)

    assert len(targets) == 3
    assert {target.display_name for target in targets} == {
        "one.sms",
        "two.sms",
        "three.bin",
    }
    assert sum(target.member_name is not None for target in targets) == 2


def test_audit_runs_native_machine_factory_and_confirms_framebuffer(tmp_path):
    rom = tmp_path / "homebrew.sms"
    rom.write_bytes(b"arbitrary SMS image without mandatory header")
    targets = discover_sms_rom_targets(tmp_path)
    created: list[bytes] = []

    def factory(data: bytes) -> _FakeMachine:
        created.append(data)
        return _FakeMachine()

    report = run_sms_rom_audit(
        targets,
        factory,
        root_directory=tmp_path,
        frames_per_rom=4,
        timeout_seconds=2.0,
    )

    assert created == [rom.read_bytes()]
    assert report.counts == {"BOOT_CONFIRMED": 1}
    result = report.results[0]
    assert result.frames_advanced == 4
    assert result.instructions == 400
    assert result.tstates == 1600
    assert result.pc == 0x1234
    assert result.sha256 is not None
    payload = json.loads(report.to_json())
    assert payload["results"][0]["status"] == "BOOT_CONFIRMED"
    assert "arbitrary SMS image" not in report.to_json()


def test_uniform_framebuffer_is_inconclusive_not_reported_as_failure(tmp_path):
    (tmp_path / "plain.sms").write_bytes(b"rom")
    report = run_sms_rom_audit(
        discover_sms_rom_targets(tmp_path),
        lambda _data: _FakeMachine(uniform=True),
        root_directory=tmp_path,
        frames_per_rom=2,
    )

    assert report.counts == {"EXECUTED_NO_VISUAL_VARIATION": 1}
    assert report.results[0].details is not None


def test_bad_zip_without_sms_member_is_counted_as_read_failure(tmp_path):
    archive_path = tmp_path / "empty.zip"
    with zipfile.ZipFile(archive_path, "w") as archive:
        archive.writestr("notes.txt", "not a ROM")

    report = run_sms_rom_audit(
        discover_sms_rom_targets(tmp_path),
        lambda _data: _FakeMachine(),
        root_directory=tmp_path,
    )

    assert report.counts == {"READ_OR_EXTRACTION_FAILED": 1}
    assert "não contém" in (report.results[0].details or "").casefold()


def test_cancel_before_first_rom_returns_explicit_cancelled_report(tmp_path):
    (tmp_path / "one.sms").write_bytes(b"rom")
    cancel = Event()
    cancel.set()

    report = run_sms_rom_audit(
        discover_sms_rom_targets(tmp_path),
        lambda _data: _FakeMachine(),
        root_directory=tmp_path,
        cancel_event=cancel,
    )

    assert report.cancelled
    assert report.total_discovered == 1
    assert report.results == ()
