"""Testes determinísticos do VDP TMS9918A."""

import pytest

from sega_emulator.video.tms9918 import (
    LINES_PER_FRAME,
    SCREEN_HEIGHT,
    SCREEN_WIDTH,
    TSTATES_PER_LINE,
    TMS9918A,
)


def _write_register(vdp: TMS9918A, register: int, value: int) -> None:
    vdp.write_port(0xBF, value)
    vdp.write_port(0xBF, 0x80 | register)


def _set_address(vdp: TMS9918A, address: int, write: bool) -> None:
    vdp.write_port(0xBF, address & 0xFF)
    vdp.write_port(0xBF, ((address >> 8) & 0x3F) | (0x40 if write else 0))


def test_vdp_data_port_writes_and_increments_vram_address() -> None:
    vdp = TMS9918A()
    _set_address(vdp, 0x3FFE, write=True)
    vdp.write_port(0xBE, 0x12)
    vdp.write_port(0xBE, 0x34)
    assert vdp.vram[0x3FFE] == 0x12
    assert vdp.vram[0x3FFF] == 0x34
    assert vdp.address == 0


def test_vdp_read_port_uses_buffered_read_and_wraps() -> None:
    vdp = TMS9918A()
    vdp.vram[0x3FFF] = 0xA5
    vdp.vram[0] = 0x5A
    _set_address(vdp, 0x3FFF, write=False)
    assert vdp.read_port(0xBE) == 0
    assert vdp.read_port(0xBE) == 0xA5
    assert vdp.read_port(0xBE) == 0x5A


def test_vdp_register_write_and_status_read_clear_flags() -> None:
    vdp = TMS9918A()
    _write_register(vdp, 1, 0x24)
    assert vdp.registers[1] == 0x24
    vdp.status |= 0xA0
    assert vdp.read_port(0xBF) == 0xBF
    assert vdp.status == 0x1F
    assert not vdp.interrupt_pending


def test_vblank_and_frame_clock_use_documented_line_count() -> None:
    vdp = TMS9918A()
    vdp.advance(TSTATES_PER_LINE * 192)
    assert vdp.scanline == 192
    assert vdp.status & 0x80
    assert vdp.consume_frame_ready()
    vdp.advance(TSTATES_PER_LINE * (LINES_PER_FRAME - 192))
    assert vdp.scanline == 0
    assert vdp.frame_number == 1


def test_framebuffer_has_rgb24_dimensions_and_blank_backdrop() -> None:
    vdp = TMS9918A()
    _write_register(vdp, 7, 0x04)
    frame = vdp.render_frame()
    assert len(frame) == SCREEN_WIDTH * SCREEN_HEIGHT * 3
    assert frame[:3] == bytes((84, 85, 237))


def test_graphics_i_pattern_and_display_enable_render_foreground() -> None:
    vdp = TMS9918A()
    _write_register(vdp, 1, 0xC2)  # 16 KiB VRAM, display and 16x16 sprites
    _write_register(vdp, 2, 0x0E)  # name table at 3800h
    _write_register(vdp, 3, 0xFF)  # color table at 3FC0h
    _write_register(vdp, 4, 0x01)  # pattern table at 0800h
    vdp.vram[0x3800] = 1
    vdp.vram[0x0808] = 0x80
    vdp.vram[0x3FC0] = 0x20  # green foreground, black background
    frame = vdp.render_frame()
    assert frame[:3] == bytes((33, 200, 66))
    assert frame[3:6] == bytes((0, 0, 0))


def test_negative_or_non_integer_clock_advance_is_rejected() -> None:
    vdp = TMS9918A()
    with pytest.raises(ValueError):
        vdp.advance(-1)
    with pytest.raises(ValueError):
        vdp.advance(1.5)  # type: ignore[arg-type]
