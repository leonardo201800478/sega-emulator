"""Testes dos perfis conhecidos de hardware de cartuchos SG-1000."""

import zlib

from sega_emulator.machine.cartridge_profiles import (
    SG1000_II_RAM_EXT_TYPE_B_CRC32,
    cartridge_profile,
    cartridge_ram_size,
)
from sega_emulator.machine.sg1000 import SG1000Machine


def test_all_known_type_b_crcs_are_registered() -> None:
    assert SG1000_II_RAM_EXT_TYPE_B_CRC32 == {
        0x69FC1494,
        0xFFC4EE3F,
        0x2E366CCF,
        0xAAAC12CF,
        0xD2EDD329,
    }


def test_known_type_b_crc_selects_8k_ram(monkeypatch) -> None:
    monkeypatch.setattr(zlib, "crc32", lambda data: 0x69FC1494)

    assert cartridge_ram_size(b"synthetic test ROM") == 0x2000


def test_unknown_cartridge_keeps_standard_1k_ram(monkeypatch) -> None:
    monkeypatch.setattr(zlib, "crc32", lambda data: 0x12345678)

    assert cartridge_ram_size(b"synthetic test ROM") == 0x0400


def test_type_b_machine_maps_8k_ram_and_mirrors_second_8k(monkeypatch) -> None:
    monkeypatch.setattr(zlib, "crc32", lambda data: 0x69FC1494)
    machine = SG1000Machine(b"synthetic test ROM")

    assert machine.ram_size == 0x2000
    machine.memory_bus.write(0xC000, 0xA5)
    machine.memory_bus.write(0xC800, 0x5A)

    assert machine.memory_bus.read(0xC000) == 0xA5
    assert machine.memory_bus.read(0xC800) == 0x5A
    assert machine.memory_bus.read(0xE000) == 0xA5
    assert machine.memory_bus.read(0xE800) == 0x5A


def test_dahjee_type_a_maps_expansion_ram_and_keeps_internal_ram(monkeypatch) -> None:
    monkeypatch.setattr(zlib, "crc32", lambda data: 0x223397A1)
    machine = SG1000Machine(b"synthetic King's Valley ROM")

    assert machine.ram_size == 0x0400
    assert machine.cartridge_ram_size == 0x2000
    machine.memory_bus.write(0x2000, 0xA5)
    machine.memory_bus.write(0x3FFF, 0x5A)
    assert machine.memory_bus.read(0x2000) == 0xA5
    assert machine.memory_bus.read(0x3FFF) == 0x5A
    machine.memory_bus.write(0xC000, 0x3C)
    assert machine.memory_bus.read(0xC000) == 0x3C
    assert machine.memory_bus.read(0x2000) == 0xA5


def test_othello_profile_maps_2k_ram(monkeypatch) -> None:
    monkeypatch.setattr(zlib, "crc32", lambda data: 0xAF4F14BC)
    machine = SG1000Machine(b"synthetic Othello ROM")

    assert machine.cartridge_ram_size == 0x0800
    machine.memory_bus.write(0x8000, 0xA5)
    machine.memory_bus.write(0x87FF, 0x5A)
    assert machine.memory_bus.read(0x8000) == 0xA5
    assert machine.memory_bus.read(0x8800) == 0xA5
    assert machine.memory_bus.read(0x8FFF) == 0x5A


def test_standard_machine_keeps_1k_ram_mirroring(monkeypatch) -> None:
    monkeypatch.setattr(zlib, "crc32", lambda data: 0x12345678)
    machine = SG1000Machine(b"synthetic test ROM")

    assert machine.ram_size == 0x0400
    machine.memory_bus.write(0xC000, 0xA5)

    assert machine.memory_bus.read(0xC000) == 0xA5
    assert machine.memory_bus.read(0xC400) == 0xA5
    assert machine.memory_bus.read(0xC800) == 0xA5


def test_castle_japan_uses_cartridge_ram_at_8000(monkeypatch) -> None:
    monkeypatch.setattr(zlib, "crc32", lambda data: 0x092F29D6)
    machine = SG1000Machine(b"synthetic Castle ROM")

    assert machine.ram_size == 0x0400
    assert machine.cartridge_ram_size == 0x2000
    machine.memory_bus.write(0x8000, 0xA5)
    machine.memory_bus.write(0x8001, 0x5A)

    # The 16 KiB CPU window mirrors the physical 8 KiB cartridge RAM twice.
    assert machine.memory_bus.read(0x8000) == 0xA5
    assert machine.memory_bus.read(0x8001) == 0x5A
    assert machine.memory_bus._read_for_cpu(0x8000) == 0xA5
    assert machine.memory_bus._read_for_cpu(0x8001) == 0x5A
    assert machine.memory_bus.read(0xA000) == 0xA5
    assert machine.memory_bus.read(0xA001) == 0x5A

    # The system's internal RAM remains separate at C000h–FFFFh.
    machine.memory_bus.write(0xC000, 0x3C)
    assert machine.memory_bus.read(0xC000) == 0x3C
    assert machine.memory_bus.read(0x8000) == 0xA5


def test_dahjee_type_a_maps_expansion_ram_at_2000(monkeypatch) -> None:
    monkeypatch.setattr(zlib, "crc32", lambda data: 0x223397A1)
    profile = cartridge_profile(b"synthetic King's Valley ROM")

    assert profile.internal_ram_size == 0x0400
    assert profile.cartridge_ram_size == 0x2000
    assert profile.cartridge_ram_start == 0x2000
    assert profile.cartridge_ram_end == 0x3FFF


def test_omv_othello_maps_2k_ram_at_8000(monkeypatch) -> None:
    monkeypatch.setattr(zlib, "crc32", lambda data: 0xAF4F14BC)
    profile = cartridge_profile(b"synthetic Othello ROM")

    assert profile.internal_ram_size == 0x0400
    assert profile.cartridge_ram_size == 0x0800
    assert profile.cartridge_ram_start == 0x8000
    assert profile.cartridge_ram_end == 0xBFFF


def test_english_logo_bomberman_sha256_selects_type_b_ram(monkeypatch) -> None:
    class FakeDigest:
        def hexdigest(self) -> str:
            return "3eff3d6f1f74041f7b732455799d0978ab49724552ff2985f34b76478cd91721"

    monkeypatch.setattr(zlib, "crc32", lambda data: 0x12345678)
    monkeypatch.setattr("hashlib.sha256", lambda data: FakeDigest())

    assert cartridge_ram_size(b"English-logo test ROM") == 0x2000
