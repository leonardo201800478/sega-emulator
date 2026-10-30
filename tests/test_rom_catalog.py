from __future__ import annotations

import hashlib
import zlib
from pathlib import Path

import pytest

from sega_emulator.rom_catalog import RomCatalog, inspect_disc_media


def _write_dat(directory: Path, data: bytes, *, regions: str = "USA") -> Path:
    path = directory / "fixture.dat"
    path.write_text(
        f"""<?xml version="1.0"?>
<datafile>
  <header><name>Sega - Master System - Mark III (Parent-Clone)</name></header>
  <game name="Example Game (USA)">
    <description>Example Game (USA)</description>
    <release name="Example Game" region="{regions}"/>
    <rom name="example.sms" size="{len(data)}"
         crc="{zlib.crc32(data) & 0xFFFFFFFF:08x}"
         md5="{hashlib.md5(data).hexdigest()}"
         sha1="{hashlib.sha1(data).hexdigest()}"/>
  </game>
</datafile>
""",
        encoding="utf-8",
    )
    return path


def test_catalog_identifies_exact_rom_and_region(tmp_path: Path) -> None:
    payload = b"synthetic sms fixture"
    _write_dat(tmp_path, payload, regions="EUR")
    result = RomCatalog(tmp_path).identify_bytes(payload, suffix=".sms")

    assert result.status == "matched"
    assert result.system == "master_system"
    assert result.game_name == "Example Game (USA)"
    assert result.regions == ("EUR",)
    assert result.matched_by == "sha1"


def test_unknown_rom_is_not_guessed(tmp_path: Path) -> None:
    _write_dat(tmp_path, b"known")
    result = RomCatalog(tmp_path).identify_bytes(b"unlisted hack")

    assert result.status == "not_catalogued"
    assert result.system is None
    assert result.regions == ()


def test_crc32_plus_size_is_supported_when_stronger_hashes_missing(
    tmp_path: Path,
) -> None:
    payload = b"legacy dat entry"
    path = tmp_path / "legacy.dat"
    path.write_text(
        f"""<datafile><header><name>Sega - Master System - Mark III</name></header>
<game name="Legacy"><release region="JPN"/>
<rom name="legacy.sms" size="{len(payload)}"
     crc="{zlib.crc32(payload) & 0xFFFFFFFF:08x}"/>
</game></datafile>""",
        encoding="utf-8",
    )
    result = RomCatalog(tmp_path).identify_bytes(payload)

    assert result.status == "matched"
    assert result.matched_by == "crc32+size"


def test_cue_resolves_relative_bin_and_reports_missing_tracks(tmp_path: Path) -> None:
    track = tmp_path / "track 01.bin"
    track.write_bytes(b"synthetic disc track")
    cue = tmp_path / "game.cue"
    cue.write_text(
        'FILE "track 01.bin" BINARY\n  TRACK 01 MODE1/2352\n',
        encoding="utf-8",
    )

    descriptor = inspect_disc_media(cue)
    assert descriptor.system == "sega_cd"
    assert descriptor.kind == "cue_bin"
    assert descriptor.complete
    assert descriptor.data_files == (track.resolve(),)

    track.unlink()
    missing = inspect_disc_media(cue)
    assert not missing.complete
    assert "Faixa referenciada ausente" in missing.problems[0]


@pytest.mark.parametrize(
    ("suffix", "kind"),
    [(".iso", "iso"), (".chd", "chd")],
)
def test_single_file_disc_formats_are_classified(
    tmp_path: Path, suffix: str, kind: str
) -> None:
    image = tmp_path / f"disc{suffix}"
    image.write_bytes(b"synthetic image")
    descriptor = inspect_disc_media(image)

    assert descriptor.system == "sega_cd"
    assert descriptor.kind == kind
    assert descriptor.complete


def test_bin_without_cue_is_not_assumed_to_be_a_disc(tmp_path: Path) -> None:
    image = tmp_path / "cart.bin"
    image.write_bytes(b"could be a cartridge")

    with pytest.raises(ValueError, match="Arquivo .bin isolado"):
        inspect_disc_media(image)


def test_catalog_uses_no_intro_title_region_when_release_tags_are_absent(
    tmp_path: Path,
) -> None:
    payload = b"synthetic title-region fixture"
    path = tmp_path / "title-region.dat"
    path.write_text(
        f'''<datafile><header><name>Sega - Master System - Mark III</name></header>
        <game name="Example Game (Japan, Europe) (En)">
        <rom name="example.sms" size="{len(payload)}"
             crc="{zlib.crc32(payload) & 0xFFFFFFFF:08x}"
             md5="{hashlib.md5(payload).hexdigest()}"
             sha1="{hashlib.sha1(payload).hexdigest()}"/>
        </game></datafile>''',
        encoding="utf-8",
    )

    result = RomCatalog(tmp_path).identify_bytes(payload)

    assert result.status == "matched"
    assert result.regions == ("EUR", "JPN")


def test_default_dat_directory_supports_source_and_installed_layouts(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    from sega_emulator import rom_catalog

    package_file = tmp_path / "source" / "src" / "sega_emulator" / "rom_catalog.py"
    package_file.parent.mkdir(parents=True)
    package_file.write_text("", encoding="utf-8")
    source_dats = tmp_path / "source" / "dats"
    source_dats.mkdir()
    installed_dats = tmp_path / "prefix" / "share" / "sega-emulator" / "dats"
    installed_dats.mkdir(parents=True)

    monkeypatch.setattr(rom_catalog, "__file__", str(package_file))
    monkeypatch.setattr(rom_catalog.sys, "prefix", str(tmp_path / "prefix"))

    assert rom_catalog.default_dat_directory() == source_dats
    source_dats.rmdir()
    assert rom_catalog.default_dat_directory() == installed_dats


def test_catalog_extracts_brazil_and_europe_from_no_intro_title(tmp_path: Path) -> None:
    payload = b"synthetic brazil-europe title fixture"
    path = tmp_path / "brazil-europe.dat"
    path.write_text(
        f'''<datafile><header><name>Sega - Master System - Mark III</name></header>
        <game name="Example Game (Europe, Brazil) (En)">
        <rom name="example.sms" size="{len(payload)}"
             crc="{zlib.crc32(payload) & 0xFFFFFFFF:08x}"
             md5="{hashlib.md5(payload).hexdigest()}"
             sha1="{hashlib.sha1(payload).hexdigest()}"/>
        </game></datafile>''',
        encoding="utf-8",
    )

    result = RomCatalog(tmp_path).identify_bytes(payload, suffix=".sms")

    assert result.status == "matched"
    assert result.regions == ("BRA", "EUR")
