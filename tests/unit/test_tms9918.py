"""Testes determinísticos do VDP TMS9918A."""

import pytest

from sega_emulator.video.tms9918 import (
    LINES_PER_FRAME,
    PALETTE,
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


def _prepare_sprite_table(vdp: TMS9918A) -> None:
    # Place the attribute table away from sprite patterns at address zero.
    _write_register(vdp, 1, 0x40)  # Display enabled; 8x8 sprites, no magnification.
    _write_register(vdp, 5, 0x40)  # Attribute table at 2000h.
    _write_register(vdp, 6, 0x00)  # Sprite patterns at 0000h.
    for index in range(32):
        offset = 0x2000 + index * 4
        vdp.vram[offset : offset + 4] = bytes((0xD1, 0, 0, 0))
    # raw Y=FFh maps to the first visible scanline; bit 7 is the left shift.
    vdp.vram[0x0008] = 0x80
    vdp.vram[0x0010] = 0x80


def _set_sprite(
    vdp: TMS9918A,
    index: int,
    *,
    x: int,
    pattern: int,
    color: int,
    raw_y: int = 0xFF,
) -> None:
    offset = 0x2000 + index * 4
    vdp.vram[offset : offset + 4] = bytes((raw_y, x, pattern, color))


def test_sprite_priority_keeps_first_opaque_sprite_and_sets_collision() -> None:
    vdp = TMS9918A()
    _prepare_sprite_table(vdp)
    _set_sprite(vdp, 0, x=10, pattern=1, color=2)
    _set_sprite(vdp, 1, x=10, pattern=2, color=4)
    pixels = bytearray(PALETTE[0] * (SCREEN_WIDTH * SCREEN_HEIGHT))

    vdp._render_sprites(pixels)

    offset = 10 * 3
    assert pixels[offset : offset + 3] == bytes(PALETTE[2])
    assert vdp.status & 0x20


def test_transparent_sprite_can_collide_without_hiding_visible_sprite() -> None:
    vdp = TMS9918A()
    _prepare_sprite_table(vdp)
    _set_sprite(vdp, 0, x=10, pattern=1, color=0)
    _set_sprite(vdp, 1, x=10, pattern=2, color=4)
    pixels = bytearray(PALETTE[0] * (SCREEN_WIDTH * SCREEN_HEIGHT))

    vdp._render_sprites(pixels)

    assert pixels[10 * 3 : 10 * 3 + 3] == bytes(PALETTE[4])
    assert vdp.status & 0x20


def test_offscreen_sprite_pixels_still_participate_in_collision() -> None:
    vdp = TMS9918A()
    _prepare_sprite_table(vdp)
    # Attribute bit 7 shifts both sprites 32 pixels left; their opaque pixels
    # overlap at x=-32, outside the visible framebuffer.
    _set_sprite(vdp, 0, x=0, pattern=1, color=0x82)
    _set_sprite(vdp, 1, x=0, pattern=2, color=0x84)
    pixels = bytearray(PALETTE[0] * (SCREEN_WIDTH * SCREEN_HEIGHT))

    vdp._render_sprites(pixels)

    assert vdp.status & 0x20
    assert pixels[:3] == bytes(PALETTE[0])


def test_fifth_active_sprite_sets_index_and_is_not_drawn() -> None:
    vdp = TMS9918A()
    _prepare_sprite_table(vdp)
    for index in range(5):
        _set_sprite(vdp, index, x=index * 10, pattern=1, color=2)
    pixels = bytearray(PALETTE[0] * (SCREEN_WIDTH * SCREEN_HEIGHT))

    vdp._render_sprites(pixels)

    assert vdp.status & 0x40
    assert vdp.status & 0x1F == 4
    assert pixels[40 * 3 : 40 * 3 + 3] == bytes(PALETTE[0])


def test_fifth_sprite_flag_is_not_raised_during_vblank() -> None:
    vdp = TMS9918A()
    _prepare_sprite_table(vdp)
    for index in range(5):
        _set_sprite(vdp, index, x=index * 10, pattern=1, color=2)
    vdp.status |= 0x80
    pixels = bytearray(PALETTE[0] * (SCREEN_WIDTH * SCREEN_HEIGHT))

    vdp._render_sprites(pixels)

    assert vdp.status & 0x80
    assert not (vdp.status & 0x40)


def test_reset_preserves_vram_unless_explicitly_requested() -> None:
    vdp = TMS9918A()
    vdp.vram[0x1234] = 0xA5
    _write_register(vdp, 1, 0x60)
    vdp.advance(TSTATES_PER_LINE * 10)

    vdp.reset()

    assert vdp.vram[0x1234] == 0xA5
    assert vdp.registers == bytes(8)
    assert vdp.status == 0x1F
    assert vdp.scanline == 0
    assert vdp.frame_number == 0
    vdp.reset(clear_vram=True)
    assert not any(vdp.vram)


def test_fifth_sprite_status_is_generated_at_raster_line_boundary() -> None:
    vdp = TMS9918A()
    _prepare_sprite_table(vdp)
    for index in range(5):
        _set_sprite(vdp, index, x=index * 10, pattern=1, color=2)

    vdp.advance(TSTATES_PER_LINE)

    assert vdp.scanline == 1
    assert vdp.status & 0x40
    assert vdp.status & 0x1F == 4

