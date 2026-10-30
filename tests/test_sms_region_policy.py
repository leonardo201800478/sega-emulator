"""SMS region resolution, metadata precedence and nominal frame pacing tests."""

from __future__ import annotations

import hashlib
import zlib

import pytest

from sega_emulator.machine.sms_region import (
    normalize_sms_region_preference,
    resolve_sms_region_profile,
    sms_frame_period_seconds,
    sms_region_candidates,
    sms_region_candidates_for_rom,
)
from sega_emulator.rom_catalog import RomCatalog


def _sms_header(region_code: int) -> bytes:
    rom = bytearray(0x8000)
    rom[0x7FF0:0x7FF8] = b"TMR SEGA"
    rom[0x7FFF] = (region_code << 4) | 0x0C
    return bytes(rom)


def _write_sms_dat(tmp_path, payload: bytes, title: str, *, strong_hashes=True):
    sha1 = f'<rom name="fixture.sms" size="{len(payload)}"'
    if strong_hashes:
        sha1 += (
            f' sha1="{hashlib.sha1(payload).hexdigest()}"'
            f' md5="{hashlib.md5(payload).hexdigest()}"'
        )
    sha1 += f' crc="{zlib.crc32(payload) & 0xFFFFFFFF:08x}"/>'
    (tmp_path / "sms.dat").write_text(
        f'''<datafile><header><name>Sega - Master System - Mark III</name></header>
        <game name="{title}">{sha1}</game></datafile>''',
        encoding="utf-8",
    )


@pytest.mark.parametrize(
    ("preference", "expected"),
    [
        ("auto", ("usa",)),
        ("usa", ("usa",)),
        ("europe", ("europe",)),
        ("japan", ("japan",)),
        ("brazil", ("brazil",)),
        ("invalid", ("usa",)),
    ],
)
def test_region_candidates_follow_manual_or_deterministic_default(
    preference, expected
):
    assert sms_region_candidates(preference) == expected


def test_invalid_persisted_region_defaults_to_auto():
    assert normalize_sms_region_preference(None) == "auto"
    assert normalize_sms_region_preference("invalid") == "auto"


def test_pal_m_frame_pacing_matches_ntsc_like_60hz_profile():
    assert sms_frame_period_seconds("brazil") == sms_frame_period_seconds("usa")
    assert sms_frame_period_seconds("europe") > sms_frame_period_seconds("usa")
    assert 0.019 < sms_frame_period_seconds("europe") < 0.021
    assert 0.016 < sms_frame_period_seconds("usa") < 0.018


def test_unknown_concrete_region_is_rejected():
    with pytest.raises(ValueError, match="region"):
        sms_frame_period_seconds("auto")


def test_header_japan_selects_japan_profile():
    result = resolve_sms_region_profile("auto", _sms_header(0x3))
    assert result.region == "japan"
    assert result.source == "header"
    assert result.header_family == "japan"


def test_header_export_without_exact_dat_uses_usa_default():
    result = resolve_sms_region_profile("auto", _sms_header(0x4))
    assert result.region == "usa"
    assert result.source == "header_export_default"
    assert result.header_family == "export"


@pytest.mark.parametrize(
    ("title", "expected"),
    [
        ("Synthetic SMS (USA)", "usa"),
        ("Synthetic SMS (Europe)", "europe"),
        ("Synthetic SMS (Japan)", "japan"),
        ("Synthetic SMS (Brazil)", "brazil"),
    ],
)
def test_exact_dat_refines_profile_when_header_is_absent(tmp_path, title, expected):
    payload = f"synthetic {expected} release".encode()
    _write_sms_dat(tmp_path, payload, title)

    result = resolve_sms_region_profile("auto", payload, RomCatalog(tmp_path))

    assert result.region == expected
    assert result.source == "dat"
    assert not result.ambiguous
    assert not result.conflict


@pytest.mark.parametrize(
    ("title", "expected"),
    [
        ("Synthetic SMS (USA)", "usa"),
        ("Synthetic SMS (Europe)", "europe"),
        ("Synthetic SMS (Brazil)", "brazil"),
    ],
)
def test_export_header_is_refined_by_exact_dat(tmp_path, title, expected):
    rom = _sms_header(0x4)
    _write_sms_dat(tmp_path, rom, title)

    result = resolve_sms_region_profile("auto", rom, RomCatalog(tmp_path))

    assert result.region == expected
    assert result.source == "dat_refinement"
    assert result.header_family == "export"


def test_brazil_and_europe_multi_region_release_is_ambiguous(tmp_path):
    payload = b"synthetic brazil europe shared release"
    _write_sms_dat(tmp_path, payload, "Synthetic SMS (Europe, Brazil)")

    result = resolve_sms_region_profile("auto", payload, RomCatalog(tmp_path))

    assert result.region == "usa"
    assert result.source == "ambiguous_fallback"
    assert result.release_regions == ("BRA", "EUR")
    assert result.ambiguous
    assert not result.conflict


def test_export_header_does_not_guess_between_brazil_and_europe(tmp_path):
    rom = _sms_header(0x4)
    _write_sms_dat(tmp_path, rom, "Synthetic SMS (Europe, Brazil)")

    result = resolve_sms_region_profile("auto", rom, RomCatalog(tmp_path))

    assert result.region == "usa"
    assert result.source == "ambiguous_fallback"
    assert result.ambiguous


def test_header_has_priority_but_conflicting_dat_is_reported(tmp_path):
    rom = _sms_header(0x3)
    _write_sms_dat(tmp_path, rom, "Synthetic SMS (Brazil)")

    result = resolve_sms_region_profile("auto", rom, RomCatalog(tmp_path))

    assert result.region == "japan"
    assert result.source == "header"
    assert result.conflict


def test_export_header_conflicting_with_japan_dat_uses_safe_fallback(tmp_path):
    rom = _sms_header(0x4)
    _write_sms_dat(tmp_path, rom, "Synthetic SMS (Japan)")

    result = resolve_sms_region_profile("auto", rom, RomCatalog(tmp_path))

    assert result.region == "usa"
    assert result.source == "conflict_fallback"
    assert result.conflict


@pytest.mark.parametrize("region_code", [0x3, 0x4])
def test_korean_and_taiwan_releases_are_not_collapsed_into_japan_or_export(
    tmp_path, region_code
):
    rom = _sms_header(region_code)
    _write_sms_dat(tmp_path, rom, "Synthetic SMS (Korea)")

    result = resolve_sms_region_profile("auto", rom, RomCatalog(tmp_path))

    assert result.region == "usa"
    assert result.source == "conflict_fallback"
    assert result.conflict
    assert result.ambiguous


def test_unknown_rom_uses_usa_without_bios_region_guess():
    result = resolve_sms_region_profile("auto", b"\x00" * 0x8000)

    assert result.region == "usa"
    assert result.source == "fallback"
    assert result.header_family is None
    assert result.release_regions == ()


def test_crc_only_dat_does_not_choose_video_profile(tmp_path):
    payload = b"synthetic crc only"
    _write_sms_dat(
        tmp_path,
        payload,
        "Synthetic SMS (Europe)",
        strong_hashes=False,
    )

    result = resolve_sms_region_profile("auto", payload, RomCatalog(tmp_path))

    assert result.region == "usa"
    assert result.source == "ambiguous_fallback"
    assert result.ambiguous


def test_manual_profile_overrides_header_and_dat(tmp_path):
    rom = _sms_header(0x3)
    _write_sms_dat(tmp_path, rom, "Synthetic SMS (Japan)")

    result = resolve_sms_region_profile("brazil", rom, RomCatalog(tmp_path))

    assert result.region == "brazil"
    assert result.source == "manual"
    assert result.header_family == "japan"


def test_automatic_candidates_are_single_profile_and_use_catalog(tmp_path):
    rom = b"synthetic brazil rom"
    _write_sms_dat(tmp_path, rom, "Synthetic SMS (Brazil)")

    assert sms_region_candidates_for_rom(
        "auto", rom, RomCatalog(tmp_path)
    ) == ("brazil",)
    assert sms_region_candidates_for_rom("auto", b"unknown") == ("usa",)
    assert sms_region_candidates_for_rom("europe", _sms_header(0x3)) == ("europe",)
