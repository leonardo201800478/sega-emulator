"""Testes dos perfis conhecidos de hardware de cartuchos SG-1000."""

import zlib

from sega_emulator.machine.cartridge_profiles import (
    SG1000_II_RAM_EXT_TYPE_B_CRC32,
    cartridge_ram_size,
)
from sega_emulator.machine.sg1000 import SG1000Machine


def test_bomberman_special_crc_is_registered_as_type_b() -> None:
    assert 0x69FC1494 in SG1000_II_RAM_EXT_TYPE_B_CRC32


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


def test_standard_machine_keeps_1k_ram_mirroring(monkeypatch) -> None:
    monkeypatch.setattr(zlib, "crc32", lambda data: 0x12345678)
    machine = SG1000Machine(b"synthetic test ROM")

    assert machine.ram_size == 0x0400
    machine.memory_bus.write(0xC000, 0xA5)

    assert machine.memory_bus.read(0xC000) == 0xA5
    assert machine.memory_bus.read(0xC400) == 0xA5
    assert machine.memory_bus.read(0xC800) == 0xA5


def test_english_logo_bomberman_sha256_selects_type_b_ram(monkeypatch) -> None:
    class FakeDigest:
        def hexdigest(self) -> str:
            return (
                "3eff3d6f1f74041f7b732455799d0978ab49724552ff2985f34b76478cd91721"
            )

    monkeypatch.setattr(zlib, "crc32", lambda data: 0x12345678)
    monkeypatch.setattr("hashlib.sha256", lambda data: FakeDigest())

    assert cartridge_ram_size(b"English-logo test ROM") == 0x2000
