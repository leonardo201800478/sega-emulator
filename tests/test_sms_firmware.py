"""Tests for explicit SMS firmware discovery and SHA-256 identification."""

from __future__ import annotations

from sega_emulator.machine.sms_firmware import (
    SMS_BIOS_SIZE,
    SmsFirmwareDirectoryStatus,
    SmsFirmwareFileStatus,
    SmsFirmwareRegion,
    load_sms_bios,
    scan_sms_firmware_directory,
    sms_bios_region_from_cartridge,
    sms_bios_region_from_catalog,
)
from sega_emulator.rom_catalog import RomCatalog

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


def test_known_hashes_classify_japan_and_shared_export_image(tmp_path):
    from unittest.mock import patch

    (tmp_path / "jp-with-arbitrary-name.data").write_bytes(b"J" * SMS_BIOS_SIZE)
    (tmp_path / "export.bin").write_bytes(b"E" * SMS_BIOS_SIZE)

    from types import SimpleNamespace

    with patch("sega_emulator.machine.sms_firmware.hashlib.sha256") as sha256:
        sha256.side_effect = lambda data: SimpleNamespace(
            hexdigest=lambda: JP_SHA256 if data.startswith(b"J") else EXPORT_SHA256
        )
        result = scan_sms_firmware_directory(tmp_path)

    by_name = {item.path.name: item for item in result.files}
    assert by_name["jp-with-arbitrary-name.data"].region is SmsFirmwareRegion.JAPAN
    assert (
        by_name["jp-with-arbitrary-name.data"].status
        is SmsFirmwareFileStatus.VALID_JAPAN
    )
    assert by_name["export.bin"].region is SmsFirmwareRegion.EXPORT_US_EU
    assert by_name["export.bin"].status is SmsFirmwareFileStatus.VALID_EXPORT_US_EU


def test_export_and_japan_signatures_are_unique_catalog_entries():
    # The catalog intentionally contains exactly two unique signatures:
    # JP and the byte-identical US/EU export image.
    from sega_emulator.machine import sms_firmware

    assert sms_firmware._KNOWN_BIOS[JP_SHA256] is SmsFirmwareRegion.JAPAN
    assert sms_firmware._KNOWN_BIOS[EXPORT_SHA256] is SmsFirmwareRegion.EXPORT_US_EU
    assert len(sms_firmware._KNOWN_BIOS) == 2


def test_duplicate_recognized_hashes_are_not_reported_as_conflicts(tmp_path):
    # A scanner-level duplicate test uses a patched digest function so no BIOS
    # bytes are needed in the repository or test environment.
    from unittest.mock import patch

    data = b"\xa5" * SMS_BIOS_SIZE
    (tmp_path / "first.bin").write_bytes(data)
    (tmp_path / "renamed.rom").write_bytes(data)
    with patch("sega_emulator.machine.sms_firmware.hashlib.sha256") as sha256:
        sha256.return_value.hexdigest.return_value = EXPORT_SHA256
        result = scan_sms_firmware_directory(tmp_path)

    assert len(result.files) == 2
    assert result.files[0].status is SmsFirmwareFileStatus.VALID_EXPORT_US_EU
    assert result.files[1].status is SmsFirmwareFileStatus.DUPLICATE_HASH
    assert all(item.region is SmsFirmwareRegion.EXPORT_US_EU for item in result.files)


def test_load_sms_bios_selects_only_the_active_territory_and_revalidates(tmp_path):
    from unittest.mock import patch

    (tmp_path / "japan.bin").write_bytes(b"J" * SMS_BIOS_SIZE)
    (tmp_path / "export.bin").write_bytes(b"E" * SMS_BIOS_SIZE)

    def digest(data):
        value = JP_SHA256 if data.startswith(b"J") else EXPORT_SHA256
        return type("Digest", (), {"hexdigest": lambda self: value})()

    with patch("sega_emulator.machine.sms_firmware.hashlib.sha256", side_effect=digest):
        assert load_sms_bios(tmp_path, "japan") == b"J" * SMS_BIOS_SIZE
        assert load_sms_bios(tmp_path, "usa") == b"E" * SMS_BIOS_SIZE
        assert load_sms_bios(tmp_path, "europe") == b"E" * SMS_BIOS_SIZE


def test_load_sms_bios_returns_none_when_only_incompatible_region_exists(tmp_path):
    from unittest.mock import patch

    (tmp_path / "japan.bin").write_bytes(b"J" * SMS_BIOS_SIZE)
    with patch("sega_emulator.machine.sms_firmware.hashlib.sha256") as sha256:
        sha256.return_value.hexdigest.return_value = JP_SHA256
        assert load_sms_bios(tmp_path, "usa") is None


def test_cartridge_header_selects_japan_or_export_bios_automatically():
    japanese = bytearray(0x8000)
    japanese[0x7FF0:0x7FF8] = b"TMR SEGA"
    japanese[0x7FFF] = 0x3C
    export = bytearray(0x8000)
    export[0x7FF0:0x7FF8] = b"TMR SEGA"
    export[0x7FFF] = 0x4C
    assert sms_bios_region_from_cartridge(bytes(japanese)) == "japan"
    assert sms_bios_region_from_cartridge(bytes(export)) == "usa"


def test_unknown_or_missing_cartridge_header_does_not_guess_bios_region():
    unknown = bytearray(0x8000)
    unknown[0x7FF0:0x7FF8] = b"TMR SEGA"
    unknown[0x7FFF] = 0x5C
    assert sms_bios_region_from_cartridge(bytes(unknown)) is None
    assert sms_bios_region_from_cartridge(b"\x00" * 0x8000) is None


def test_cartridge_header_locations_are_checked_from_largest_to_smallest():
    rom = bytearray(0x2000)
    rom[0x1FF0:0x1FF8] = b"TMR SEGA"
    rom[0x1FFF] = 0x3C
    assert sms_bios_region_from_cartridge(bytes(rom)) == "japan"


def _write_sms_dat(directory, payload, regions, *, game_name="Fixture"):
    import hashlib
    import zlib

    releases = "".join(
        f'<release name="Fixture" region="{region}"/>' for region in regions
    )
    (directory / "sms.dat").write_text(
        f'''<datafile><header><name>Sega - Master System - Mark III</name></header>
        <game name="{game_name}">{releases}
        <rom name="fixture.sms" size="{len(payload)}"
             crc="{zlib.crc32(payload) & 0xFFFFFFFF:08x}"
             md5="{hashlib.md5(payload).hexdigest()}"
             sha1="{hashlib.sha1(payload).hexdigest()}" status="verified"/>
        </game></datafile>''',
        encoding="utf-8",
    )


def test_dat_fallback_selects_japan_bios_for_exact_japan_only_match(tmp_path):
    payload = b"synthetic-japan-rom"
    _write_sms_dat(tmp_path, payload, ("JPN",))
    assert sms_bios_region_from_catalog(payload, RomCatalog(tmp_path)) == "japan"


def test_dat_fallback_selects_export_bios_for_exact_usa_europe_match(tmp_path):
    payload = b"synthetic-export-rom"
    _write_sms_dat(tmp_path, payload, ("USA", "EUR"))
    assert sms_bios_region_from_catalog(payload, RomCatalog(tmp_path)) == "usa"


def test_dat_fallback_refuses_mixed_japan_and_export_releases(tmp_path):
    payload = b"synthetic-mixed-region-rom"
    _write_sms_dat(tmp_path, payload, ("JPN", "USA"))
    assert sms_bios_region_from_catalog(payload, RomCatalog(tmp_path)) is None


def test_dat_fallback_refuses_regions_without_known_bios_mapping(tmp_path):
    payload = b"synthetic-korea-rom"
    _write_sms_dat(tmp_path, payload, ("KOR",))
    assert sms_bios_region_from_catalog(payload, RomCatalog(tmp_path)) is None


def test_dat_fallback_does_not_identify_uncatalogued_rom(tmp_path):
    _write_sms_dat(tmp_path, b"known", ("USA",))
    assert sms_bios_region_from_catalog(b"unknown", RomCatalog(tmp_path)) is None


def test_dat_fallback_refuses_crc32_only_identity(tmp_path):
    import zlib

    payload = b"crc-only-sms-rom"
    (tmp_path / "sms.dat").write_text(
        f'''<datafile><header><name>Sega - Master System - Mark III</name></header>
        <game name="CRC only"><release region="USA"/>
        <rom name="fixture.sms" size="{len(payload)}"
             crc="{zlib.crc32(payload) & 0xFFFFFFFF:08x}"/>
        </game></datafile>''',
        encoding="utf-8",
    )
    assert sms_bios_region_from_catalog(payload, RomCatalog(tmp_path)) is None


def _write_bios_dat(directory, payload, game_name, regions):
    import hashlib
    import zlib

    releases = "".join(
        f'<release name="Firmware" region="{region}"/>' for region in regions
    )
    (directory / "bios.dat").write_text(
        f'''<datafile><header><name>Sega - Master System - Mark III</name></header>
        <game name="{game_name}">{releases}
        <rom name="firmware.sms" size="{len(payload)}"
             crc="{zlib.crc32(payload) & 0xFFFFFFFF:08x}"
             md5="{hashlib.md5(payload).hexdigest()}"
             sha1="{hashlib.sha1(payload).hexdigest()}" status="verified"/>
        </game></datafile>''',
        encoding="utf-8",
    )


def test_firmware_scanner_accepts_verified_sms_bios_from_dat(tmp_path):
    payload = b"J" * SMS_BIOS_SIZE
    (tmp_path / "firmware.bin").write_bytes(payload)
    _write_bios_dat(
        tmp_path,
        payload,
        "[BIOS] Sega Master System (Japan) (v2.1)",
        ("JPN",),
    )

    result = scan_sms_firmware_directory(tmp_path, catalog=RomCatalog(tmp_path))

    firmware = next(item for item in result.files if item.path.name == "firmware.bin")
    assert firmware.region is SmsFirmwareRegion.JAPAN
    assert firmware.status is SmsFirmwareFileStatus.VALID_JAPAN


def test_firmware_scanner_does_not_accept_an_ordinary_game_as_bios(tmp_path):
    payload = b"G" * SMS_BIOS_SIZE
    (tmp_path / "game.bin").write_bytes(payload)
    _write_bios_dat(
        tmp_path,
        payload,
        "Example Game (Japan)",
        ("JPN",),
    )

    result = scan_sms_firmware_directory(tmp_path, catalog=RomCatalog(tmp_path))

    game = next(item for item in result.files if item.path.name == "game.bin")
    assert game.region is None
    assert game.status is SmsFirmwareFileStatus.NOT_FOUND


def test_firmware_scanner_accepts_export_bios_from_dat_and_loads_it(tmp_path):
    payload = b"E" * SMS_BIOS_SIZE
    (tmp_path / "export.bin").write_bytes(payload)
    _write_bios_dat(
        tmp_path,
        payload,
        "[BIOS] Sega Master System (Europe) (v2.0)",
        ("EUR",),
    )
    catalog = RomCatalog(tmp_path)

    assert load_sms_bios(tmp_path, "europe", catalog=catalog) == payload


def test_firmware_scanner_rejects_unverified_prototype_bios_from_dat(tmp_path):
    payload = b"P" * SMS_BIOS_SIZE
    (tmp_path / "prototype.bin").write_bytes(payload)
    _write_bios_dat(
        tmp_path,
        payload,
        "[BIOS] Sega Master System (USA) (Proto)",
        ("USA",),
    )
    dat_path = tmp_path / "bios.dat"
    dat_path.write_text(
        dat_path.read_text(encoding="utf-8").replace(' status="verified"', ""),
        encoding="utf-8",
    )

    result = scan_sms_firmware_directory(tmp_path, catalog=RomCatalog(tmp_path))
    prototype = next(item for item in result.files if item.path.name == "prototype.bin")

    assert prototype.region is None
    assert prototype.status is SmsFirmwareFileStatus.NOT_FOUND

