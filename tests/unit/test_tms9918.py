"""Testes determinísticos do VDP TMS9918A."""

import pytest

from sega_emulator.video.tms9918 import (
    LINES_PER_FRAME,
    SCREEN_HEIGHT,
    SCREEN_WIDTH,
    TMS9918A,
    TSTATES_PER_LINE,
    VRAM_SIZE,
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
    assert vdp.read_port(0xBE) == 0xA5
    assert vdp.read_port(0xBE) == 0x5A
    assert vdp.read_port(0xBE) == 0


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


def test_graphics_ii_mode_uses_r0_bit_one_and_r1_bit_three() -> None:
    vdp = TMS9918A()
    _write_register(vdp, 0, 0x02)
    _write_register(vdp, 1, 0xC0)
    _write_register(vdp, 2, 0x0E)
    _write_register(vdp, 3, 0xFF)
    _write_register(vdp, 4, 0x04)
    vdp.vram[0x3800] = 0
    vdp.vram[0x2000] = 0x80
    frame = vdp.render_frame()
    assert frame[:3] == bytes((252, 85, 84))


def test_negative_or_non_integer_clock_advance_is_rejected() -> None:
    vdp = TMS9918A()
    with pytest.raises(ValueError):
        vdp.advance(-1)
    with pytest.raises(ValueError):
        vdp.advance(1.5)  # type: ignore[arg-type]


def test_vdp_data_write_updates_read_buffer_and_wraps_address() -> None:
    vdp = TMS9918A()
    _set_address(vdp, 0x3FFF, write=True)
    vdp.write_port(0xBE, 0xA6)
    assert vdp.vram[0x3FFF] == 0xA6
    assert vdp.read_buffer == 0xA6
    assert vdp.address == 0


def test_vdp_ports_decode_hardware_aliases() -> None:
    vdp = TMS9918A()
    vdp.vram[0x1234] = 0x6D
    _set_address(vdp, 0x1234, write=False)
    assert vdp.read_port(0x80) == 0x6D
    assert vdp.read_port(0xA0) == 0
    assert vdp.address == 0x1237


def test_vdp_vram_is_exactly_16_kib_and_address_wraps() -> None:
    vdp = TMS9918A()
    assert len(vdp.vram) == VRAM_SIZE
    _set_address(vdp, 0xFFFF, write=True)
    assert vdp.address == 0x3FFF
    vdp.write_port(0xBE, 0x72)
    assert vdp.address == 0
    assert vdp.vram[0x3FFF] == 0x72


def test_vdp_status_read_cancels_partial_control_command() -> None:
    vdp = TMS9918A()
    vdp.write_port(0xBF, 0x34)
    assert vdp._control_first == 0x34
    vdp.read_port(0xBF)
    assert vdp._control_first is None
    vdp.write_port(0xBF, 0x80)
    assert vdp.address == 0
    assert vdp.registers[0] == 0


def _configure_sprite(
    vdp: TMS9918A,
    index: int,
    *,
    y: int,
    x: int,
    pattern: int,
    color: int,
) -> None:
    """Configura uma entrada da tabela de atributos de sprites."""
    base = (vdp.registers[5] & 0x7F) << 7
    offset = base + index * 4
    vdp.vram[offset : offset + 4] = bytes((y, x, pattern, color))


def test_transparent_sprite_pixels_still_trigger_collision_without_painting() -> None:
    vdp = TMS9918A()
    _write_register(vdp, 1, 0x40)  # display enabled, sprites 8x8
    _write_register(vdp, 5, 0x04)  # sprite attributes at 0200h
    _write_register(vdp, 6, 0x01)  # sprite patterns at 0800h
    _write_register(vdp, 7, 0x04)
    # First sprite uses transparent colour 0; second sprite is red.
    _configure_sprite(vdp, 0, y=0xFF, x=20, pattern=1, color=0)
    _configure_sprite(vdp, 1, y=0xFF, x=20, pattern=2, color=8)
    _configure_sprite(vdp, 2, y=0xD0, x=0, pattern=0, color=0)
    vdp.vram[0x808:0x810] = bytes([0x80] * 8)
    vdp.vram[0x810:0x818] = bytes([0x80] * 8)

    vdp.advance(TSTATES_PER_LINE)
    assert vdp.status & 0x20
    frame = vdp.render_frame()

    pixel = (0 * SCREEN_WIDTH + 20) * 3
    assert frame[pixel : pixel + 3] == bytes(PALETTE[8])


def test_fifth_sprite_status_uses_first_overflow_in_raster_order() -> None:
    vdp = TMS9918A()
    _write_register(vdp, 1, 0x40)
    _write_register(vdp, 5, 0x04)
    _write_register(vdp, 6, 0x01)
    # Indices 0-4 overflow on line 41 (fifth index 4); indices 5-9
    # overflow earlier on line 1 (fifth index 9), which must win.
    for index in range(10):
        y = 40 if index < 5 else 0
        _configure_sprite(vdp, index, y=y, x=index * 8, pattern=0, color=1)
    _configure_sprite(vdp, 10, y=0xD0, x=0, pattern=0, color=0)

    vdp.advance(TSTATES_PER_LINE * 2)  # process lines 0 and 1
    vdp.render_frame()

    assert vdp.status & 0x40
    assert (vdp.status & 0x1F) == 9


def test_sprite_collision_status_is_visible_and_clearable_per_scanline() -> None:
    vdp = TMS9918A()
    _write_register(vdp, 1, 0x40)
    _write_register(vdp, 5, 0x04)
    _write_register(vdp, 6, 0x01)
    _configure_sprite(vdp, 0, y=0xFF, x=12, pattern=1, color=0)
    _configure_sprite(vdp, 1, y=0xFF, x=12, pattern=2, color=8)
    _configure_sprite(vdp, 2, y=0xD0, x=0, pattern=0, color=0)
    vdp.vram[0x808:0x810] = bytes([0x80] * 8)
    vdp.vram[0x810:0x818] = bytes([0x80] * 8)

    vdp.advance(TSTATES_PER_LINE)
    assert vdp.status & 0x20

    status = vdp.read_port(0xBF)
    assert status & 0x20
    assert not (vdp.status & 0x20)

    vdp.advance(TSTATES_PER_LINE)
    assert vdp.status & 0x20


def test_multicolor_mode_selects_pattern_bytes_by_tile_row() -> None:
    vdp = TMS9918A()
    _write_register(vdp, 1, 0x48)  # display enabled, Multicolor mode
    _write_register(vdp, 2, 0x0E)  # name table at 3800h
    _write_register(vdp, 4, 0x01)  # pattern generator at 0800h
    vdp.vram[0x3800] = 1
    vdp.vram[0x3820] = 1  # same pattern name in the next tile row
    vdp.vram[0x808:0x810] = bytes((0x23, 0x45, 0x67, 0x89, 0xAB, 0xCD, 0xEF, 0x10))

    frame = vdp.render_frame()

    top_left = 0
    second_tile_row_left = (8 * SCREEN_WIDTH) * 3
    assert frame[top_left : top_left + 3] == bytes(PALETTE[2])
    assert frame[second_tile_row_left : second_tile_row_left + 3] == bytes(PALETTE[6])
