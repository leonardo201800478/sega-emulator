"""Tests for cartridge-profile metadata; hardware mapping is tested in Rust."""

from __future__ import annotations

import zlib

from sega_emulator.machine.cartridge_profiles import (
    SG1000_DAHJEE_TYPE_A_CRC32,
    SG1000_DAHJEE_TYPE_A_RAM_END,
    SG1000_DAHJEE_TYPE_A_RAM_SIZE,
    SG1000_DAHJEE_TYPE_A_RAM_START,
    SG1000_II_RAM_EXT_TYPE_B_CRC32,
    SG1000_II_RAM_EXT_TYPE_B_SIZE,
    SG1000_INTERNAL_RAM_SIZE,
    SG1000_OTHELLO_CRC32,
    SG1000_THE_CASTLE_JAPAN_CRC32,
    cartridge_profile,
    cartridge_ram_size,
)


def test_all_known_type_b_crcs_are_registered() -> None:
    assert SG1000_II_RAM_EXT_TYPE_B_CRC32 == {
        0x69FC1494,
        0xFFC4EE3F,
        0x2E366CCF,
        0xAAAC12CF,
        0xD2EDD329,
    }


def test_known_type_b_crc_selects_profile_metadata(monkeypatch) -> None:
    monkeypatch.setattr(zlib, "crc32", lambda data: 0x69FC1494)
    profile = cartridge_profile(b"synthetic test ROM")
    assert profile.internal_ram_size == SG1000_II_RAM_EXT_TYPE_B_SIZE
    assert profile.cartridge_ram_size == 0
    assert cartridge_ram_size(b"synthetic test ROM") == 0x2000


def test_dahjee_type_a_crcs_select_external_ram_window(monkeypatch) -> None:
    assert SG1000_DAHJEE_TYPE_A_CRC32 == {
        0x223397A1,
        0x281D2888,
        0x2E7166D5,
        0xDF7CBFA5,
        0xE0816BB7,
        0x5CBD1163,
        0xC550B4F0,
    }
    monkeypatch.setattr(zlib, "crc32", lambda data: 0x223397A1)
    profile = cartridge_profile(b"synthetic King's Valley ROM")
    assert profile.internal_ram_size == SG1000_INTERNAL_RAM_SIZE
    assert profile.cartridge_ram_size == SG1000_DAHJEE_TYPE_A_RAM_SIZE
    assert profile.cartridge_ram_size == 0x2000
    assert profile.cartridge_ram_start == SG1000_DAHJEE_TYPE_A_RAM_START == 0x2000
    assert profile.cartridge_ram_end == SG1000_DAHJEE_TYPE_A_RAM_END == 0x3FFF


def test_othello_catalog_crc_selects_2k_cartridge_ram(monkeypatch) -> None:
    assert SG1000_OTHELLO_CRC32 == {0xAF4F14BC, 0x1D1A0CA3}
    monkeypatch.setattr(zlib, "crc32", lambda data: 0xAF4F14BC)
    profile = cartridge_profile(b"synthetic Othello ROM")
    assert profile.internal_ram_size == SG1000_INTERNAL_RAM_SIZE
    assert profile.cartridge_ram_size == 0x0800
    assert profile.cartridge_ram_start == 0x8000
    assert profile.cartridge_ram_end == 0xBFFF


def test_unknown_cartridge_keeps_standard_profile(monkeypatch) -> None:
    monkeypatch.setattr(zlib, "crc32", lambda data: 0x12345678)
    profile = cartridge_profile(b"synthetic test ROM")
    assert profile.internal_ram_size == SG1000_INTERNAL_RAM_SIZE
    assert profile.cartridge_ram_size == 0
    assert profile.cartridge_ram_start == 0
    assert profile.cartridge_ram_end == -1
    assert cartridge_ram_size(b"synthetic test ROM") == 0x0400


def test_castle_japan_profile_selects_cartridge_ram_window(monkeypatch) -> None:
    monkeypatch.setattr(zlib, "crc32", lambda data: SG1000_THE_CASTLE_JAPAN_CRC32)
    profile = cartridge_profile(b"synthetic Castle ROM")
    assert profile.internal_ram_size == SG1000_INTERNAL_RAM_SIZE
    assert profile.cartridge_ram_size == SG1000_II_RAM_EXT_TYPE_B_SIZE
    assert profile.cartridge_ram_start == 0x8000
    assert profile.cartridge_ram_end == 0xBFFF


def test_english_logo_bomberman_sha256_selects_type_b_ram(monkeypatch) -> None:
    class FakeDigest:
        def hexdigest(self) -> str:
            return "3eff3d6f1f74041f7b732455799d0978ab49724552ff2985f34b76478cd91721"

    monkeypatch.setattr(zlib, "crc32", lambda data: 0x12345678)
    monkeypatch.setattr("hashlib.sha256", lambda data: FakeDigest())
    assert cartridge_profile(b"English-logo test ROM").internal_ram_size == 0x2000
