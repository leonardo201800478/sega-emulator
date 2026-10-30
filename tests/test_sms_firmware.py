"""Tests for explicit SMS firmware discovery and SHA-256 identification."""

from __future__ import annotations

from sega_emulator.machine.sms_firmware import (
    SMS_BIOS_SIZE,
    SmsFirmwareDirectoryStatus,
    SmsFirmwareFileStatus,
    SmsFirmwareRegion,
    scan_sms_firmware_directory,
)

JP_SHA256 = "67846e26764bd862f19179294347f7353a4166b62ac4198a5ec32933b7da486e"
EXPORT_SHA256 = "477617917a12a30f9f43844909dc2de6e6a617430f5c9a36306c86414a670d50"


def test_unconfigured_and_missing_directories_are_reported(tmp_path):
    assert (
        scan_sms_firmware_directory(None).status
        is SmsFirmwareDirectoryStatus.NOT_CONFIGURED
    )
    missing = scan_sms_firmware_directory(tmp_path / "missing")
    assert missing.status is SmsFirmwareDirectoryStatus.DIRECTORY_MISSING
    assert missing.files == ()


def test_non_directory_path_is_rejected(tmp_path):
    file_path = tmp_path / "not-a-directory"
    file_path.write_text("x", encoding="utf-8")
    result = scan_sms_firmware_directory(file_path)
    assert result.status is SmsFirmwareDirectoryStatus.INVALID_DIRECTORY


def test_arbitrary_filename_is_scanned_but_unknown_hash_is_not_accepted(tmp_path):
    (tmp_path / "anything.dat").write_bytes(b"\x00" * SMS_BIOS_SIZE)
    result = scan_sms_firmware_directory(tmp_path)
    assert result.status is SmsFirmwareDirectoryStatus.READY
    assert len(result.files) == 1
    assert result.files[0].status is SmsFirmwareFileStatus.NOT_FOUND
    assert result.files[0].sha256 is not None
    assert result.files[0].region is None


def test_wrong_size_file_is_invalid_and_not_hashed(tmp_path):
    (tmp_path / "truncated.bin").write_bytes(b"\x00" * (SMS_BIOS_SIZE - 1))
    result = scan_sms_firmware_directory(tmp_path)
    assert result.files[0].status is SmsFirmwareFileStatus.INVALID_FILE
    assert result.files[0].sha256 is None


def test_export_and_japan_signatures_are_unique_catalog_entries():
    # The catalog intentionally contains exactly two unique signatures:
    # JP and the byte-identical US/EU export image.
    from sega_emulator.machine import sms_firmware

    assert sms_firmware._KNOWN_BIOS[JP_SHA256] is SmsFirmwareRegion.JAPAN
    assert (
        sms_firmware._KNOWN_BIOS[EXPORT_SHA256]
        is SmsFirmwareRegion.EXPORT_US_EU
    )
    assert len(sms_firmware._KNOWN_BIOS) == 2


def test_duplicate_recognized_hashes_are_not_reported_as_conflicts(tmp_path):
    # A scanner-level duplicate test uses a patched digest function so no BIOS
    # bytes are needed in the repository or test environment.
    from unittest.mock import patch

    data = b"\xA5" * SMS_BIOS_SIZE
    (tmp_path / "first.bin").write_bytes(data)
    (tmp_path / "renamed.rom").write_bytes(data)
    with patch(
        "sega_emulator.machine.sms_firmware.hashlib.sha256"
    ) as sha256:
        sha256.return_value.hexdigest.return_value = EXPORT_SHA256
        result = scan_sms_firmware_directory(tmp_path)

    assert len(result.files) == 2
    assert all(
        item.status is SmsFirmwareFileStatus.DUPLICATE_HASH
        for item in result.files
    )
    assert all(
        item.region is SmsFirmwareRegion.EXPORT_US_EU
        for item in result.files
    )
