"""Testes da VRAM dedicada do TMS9918."""

import pytest

from sega_emulator.bus.vram import TMS9918VRAM, VRAM_SIZE


def test_vram_has_16_kibibytes() -> None:
    vram = TMS9918VRAM()
    assert len(vram.snapshot()) == VRAM_SIZE


def test_vram_reads_and_writes_all_boundaries() -> None:
    vram = TMS9918VRAM()
    for address in (0x0000, 0x0001, 0x1FFF, 0x3FFE, 0x3FFF):
        vram.write(address, address & 0xFF)
        assert vram.read(address) == (address & 0xFF)


def test_vram_is_independent_of_cpu_ram() -> None:
    vram = TMS9918VRAM()
    vram.write(0, 0xA5)
    assert vram.read(0) == 0xA5


@pytest.mark.parametrize("address", [-1, 0x4000])
def test_vram_rejects_invalid_address(address: int) -> None:
    with pytest.raises(ValueError):
        TMS9918VRAM().read(address)


def test_vram_rejects_wrong_initial_size() -> None:
    with pytest.raises(ValueError):
        TMS9918VRAM(bytes(VRAM_SIZE - 1))
