"""Tests for explicit SRAM configuration at the Python/Rust SMS boundary."""

from __future__ import annotations

import sys
from pathlib import Path
from types import ModuleType

import pytest

from sega_emulator.machine.sms import SMSMachine


class FakeNativeSMSMachine:
    def __init__(self, rom: bytes, **kwargs: object) -> None:
        self.rom = rom
        self.kwargs = kwargs

    def save_cartridge_ram(self) -> bool:
        return True


def install_fake_native(monkeypatch: pytest.MonkeyPatch) -> None:
    module = ModuleType("sega_z80_core")
    module.NativeSMSMachine = FakeNativeSMSMachine
    monkeypatch.setitem(sys.modules, "sega_z80_core", module)


def test_sms_adapter_forwards_explicit_sram_capacity_and_path(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path
) -> None:
    install_fake_native(monkeypatch)
    save_path = tmp_path / "sonic.srm"

    machine = SMSMachine(
        bytes([0x00]),
        cartridge_ram_size=0x2000,
        sram_path=save_path,
    )

    assert machine.native_machine.kwargs["cartridge_ram_size"] == 0x2000
    assert machine.native_machine.kwargs["sram_path"] == str(save_path)
    assert machine.save_cartridge_ram() is True


@pytest.mark.parametrize("size", [0, 0x1000, 0x10000, True])
def test_sms_adapter_rejects_unsupported_sram_capacity(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path, size: int
) -> None:
    install_fake_native(monkeypatch)

    with pytest.raises(ValueError, match="cartridge_ram_size"):
        SMSMachine(
            bytes([0x00]),
            cartridge_ram_size=size,
            sram_path=tmp_path / "game.srm",
        )


def test_sms_adapter_requires_capacity_and_path_together(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path
) -> None:
    install_fake_native(monkeypatch)

    with pytest.raises(ValueError, match="informados juntos"):
        SMSMachine(bytes([0x00]), cartridge_ram_size=0x2000)

    with pytest.raises(ValueError, match="informados juntos"):
        SMSMachine(bytes([0x00]), sram_path=tmp_path / "game.srm")
