"""Security and semantic tests for the SMS ROM audit service."""

from __future__ import annotations

import json
import zipfile

import pytest

from sega_emulator import sms_rom_audit as audit


class FakeMachine:
    def __init__(self):
        self.vdp = type(
            "Vdp",
            (),
            {
                "frame_number": 0,
                "visible_height": 192,
                "framebuffer": bytes([0, 0, 0, 255, 255, 255]) * (256 * 192 // 2),
            },
        )()
        self.total_instructions = 0
        self.total_tstates = 0
        self.region = "usa"
        self.bios_enabled = False
        self.cpu = type("Cpu", (), {"pc": 0x1234})()

    def run_frame(self, *, max_steps):
        assert max_steps > 0
        self.vdp.frame_number += 1
        self.total_instructions += 10
        self.total_tstates += 40


def test_file_read_is_bounded_even_when_file_grows_after_discovery(
    tmp_path, monkeypatch
):
    path = tmp_path / "growing.sms"
    path.write_bytes(b"x" * 32)
    target = audit.SmsRomAuditTarget(path, path.name, declared_size=32)
    monkeypatch.setattr(audit, "MAX_SMS_ROM_SIZE", 64)
    path.write_bytes(b"x" * 65)

    with pytest.raises(ValueError, match="excede"):
        audit.read_sms_rom_target(target)


def test_zip_member_read_is_bounded(tmp_path, monkeypatch):
    path = tmp_path / "oversized.zip"
    with zipfile.ZipFile(path, "w") as archive:
        archive.writestr("game.sms", b"x" * 65)
    target = audit.discover_sms_rom_targets(tmp_path)[0]
    monkeypatch.setattr(audit, "MAX_SMS_ROM_SIZE", 64)

    with pytest.raises(ValueError, match="excede"):
        audit.read_sms_rom_target(target)


def test_discovery_rejects_too_many_files(tmp_path, monkeypatch):
    for index in range(3):
        (tmp_path / f"{index}.sms").write_bytes(b"rom")
    monkeypatch.setattr(audit, "MAX_DISCOVERED_FILES", 2)

    with pytest.raises(audit.SmsRomAuditLimitError, match="arquivos"):
        audit.discover_sms_rom_targets(tmp_path)


def test_discovery_rejects_zip_member_count_over_limit(tmp_path, monkeypatch):
    path = tmp_path / "many.zip"
    with zipfile.ZipFile(path, "w") as archive:
        archive.writestr("one.sms", b"1")
        archive.writestr("two.sms", b"2")
    monkeypatch.setattr(audit, "MAX_ZIP_MEMBERS", 1)

    with pytest.raises(audit.SmsRomAuditLimitError, match="membros"):
        audit.discover_sms_rom_targets(tmp_path)


def test_discovery_rejects_oversized_zip_before_metadata_parse(tmp_path, monkeypatch):
    path = tmp_path / "large-container.zip"
    with zipfile.ZipFile(path, "w") as archive:
        archive.writestr("game.sms", b"rom")
    monkeypatch.setattr(audit, "MAX_ZIP_ARCHIVE_SIZE", 1)

    with pytest.raises(audit.SmsRomAuditLimitError, match="ZIP excede"):
        audit.discover_sms_rom_targets(tmp_path)


def test_visual_activity_is_not_claimed_as_confirmed_boot(tmp_path):
    path = tmp_path / "homebrew.sms"
    path.write_bytes(b"legal test fixture")
    targets = audit.discover_sms_rom_targets(tmp_path)

    report = audit.run_sms_rom_audit(
        targets,
        lambda _data: FakeMachine(),
        root_directory=tmp_path,
        frames_per_rom=3,
    )

    assert report.counts == {"EXECUTED_VISUAL_ACTIVITY": 1}
    assert "BOOT_CONFIRMED" not in report.to_json()
    payload = json.loads(report.to_json())
    assert "legal test fixture" not in json.dumps(payload)
    assert payload["schema_version"] == 2


def test_uniform_framebuffer_is_inconclusive(tmp_path):
    path = tmp_path / "uniform.sms"
    path.write_bytes(b"rom")

    class UniformMachine(FakeMachine):
        def __init__(self):
            super().__init__()
            self.vdp.framebuffer = bytes([0, 0, 0]) * (256 * 192)

    report = audit.run_sms_rom_audit(
        audit.discover_sms_rom_targets(tmp_path),
        lambda _data: UniformMachine(),
        root_directory=tmp_path,
        frames_per_rom=2,
    )

    assert report.counts == {"EXECUTED_INCONCLUSIVE": 1}
