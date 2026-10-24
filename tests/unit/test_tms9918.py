"""Testes determinísticos do VDP TMS9918A."""

import pytest

from sega_emulator.video.tms9918 import (
    LINES_PER_FRAME,
    PALETTE,
    SCREEN_HEIGHT,
    SCREEN_WIDTH,
    TMS9918A,
    TSTATES_PER_LINE,
    VBLANK_LINE,
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


def test_vblank_line_does_not_set_fifth_sprite_overflow_after_status_read() -> None:
    vdp = TMS9918A()
    _prepare_sprite_table(vdp)
    for index in range(5):
        _set_sprite(vdp, index, x=index * 10, pattern=1, color=2, raw_y=0xBF)

    vdp.advance(VBLANK_LINE * TSTATES_PER_LINE)
    assert vdp.status & 0x80

    # Reading status clears F. That must not re-enable 5S on line 192.
    vdp.read_port(0x81)
    vdp.advance(TSTATES_PER_LINE)

    assert not (vdp.status & 0x40)
    assert not (vdp.status & 0x80)


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


def test_raster_rendering_preserves_emitted_lines_after_vram_changes() -> None:
    vdp = TMS9918A()
    _write_register(vdp, 1, 0x40)
    _write_register(vdp, 2, 0x01)  # Name table at 0400h.
    _write_register(vdp, 3, 0x80)  # Color table at 2000h.
    _write_register(vdp, 4, 0x01)  # Pattern table at 0800h.
    _write_register(vdp, 7, 0xF1)  # Foreground 15, background 1.
    vdp.vram[0x0400 : 0x0400 + 768] = bytes(768)
    vdp.vram[0x0800:0x0808] = bytes((0x80, 0, 0, 0, 0, 0, 0, 0))
    vdp.vram[0x2000] = 0xF1

    vdp.advance(TSTATES_PER_LINE)
    assert vdp._working_framebuffer[:3] == bytes(PALETTE[15])

    # Line zero has already been emitted. Later changes must affect later lines,
    # not retroactively repaint the line that the raster has passed.
    vdp.vram[0x0800] = 0x00
    vdp.advance((LINES_PER_FRAME - 1) * TSTATES_PER_LINE)

    assert vdp.frame_number == 1
    assert vdp.framebuffer[:3] == bytes(PALETTE[15])
    line_eight_pixel_zero = 8 * SCREEN_WIDTH * 3
    assert vdp.framebuffer[line_eight_pixel_zero : line_eight_pixel_zero + 3] == bytes(
        PALETTE[1]
    )


@pytest.mark.parametrize(
    ("r0", "r1"),
    (
        (0x00, 0x40),  # Graphics I
        (0x00, 0x50),  # Text
        (0x00, 0x48),  # Multicolor
        (0x02, 0x40),  # Graphics II
    ),
)
def test_incremental_scanlines_match_full_frame_renderer(r0: int, r1: int) -> None:
    vdp = TMS9918A()
    vdp.vram[:] = bytes((index * 29 + 7) & 0xFF for index in range(VRAM_SIZE))
    _write_register(vdp, 0, r0)
    _write_register(vdp, 1, r1)

    vdp.advance(LINES_PER_FRAME * TSTATES_PER_LINE)

    assert vdp.frame_number == 1
    assert vdp.framebuffer == vdp.render_frame()


def test_frame_is_published_when_vblank_begins() -> None:
    vdp = TMS9918A()
    _write_register(vdp, 1, 0x40)
    _write_register(vdp, 7, 0x0E)

    vdp.advance(VBLANK_LINE * TSTATES_PER_LINE)

    assert vdp.scanline == VBLANK_LINE
    assert vdp.frame_number == 0
    assert vdp.framebuffer[:3] == bytes(PALETTE[14])
    assert vdp.consume_frame_ready()
    assert not vdp.consume_frame_ready()


def test_text_mode_renders_six_pixel_glyphs_with_eight_pixel_left_border() -> None:
    vdp = TMS9918A()
    _write_register(vdp, 1, 0x50)
    _write_register(vdp, 2, 0x01)
    _write_register(vdp, 4, 0x01)
    _write_register(vdp, 7, 0xF1)
    vdp.vram[0x0400 : 0x0400 + 960] = bytes(960)
    vdp.vram[0x0800:0x0808] = bytes((0xA8, 0x40, 0, 0, 0, 0, 0, 0))

    pixels = vdp.render_frame()

    assert pixels[7 * 3 : 7 * 3 + 3] == bytes(PALETTE[1])
    assert pixels[8 * 3 : 8 * 3 + 3] == bytes(PALETTE[15])
    assert pixels[9 * 3 : 9 * 3 + 3] == bytes(PALETTE[1])
    assert pixels[10 * 3 : 10 * 3 + 3] == bytes(PALETTE[15])
    assert pixels[248 * 3 : 248 * 3 + 3] == bytes(PALETTE[1])


def test_multicolor_mode_reuses_each_color_pair_for_four_scanlines() -> None:
    vdp = TMS9918A()
    _write_register(vdp, 1, 0x48)
    _write_register(vdp, 2, 0x01)
    _write_register(vdp, 4, 0x01)
    _write_register(vdp, 7, 0x01)
    vdp.vram[0x0400 : 0x0400 + 768] = bytes(768)
    vdp.vram[0x0800] = 0xF2
    vdp.vram[0x0801] = 0x34

    pixels = vdp.render_frame()

    assert pixels[0:3] == bytes(PALETTE[15])
    assert pixels[3 * 3 : 3 * 3 + 3] == bytes(PALETTE[15])
    assert pixels[4 * 3 : 4 * 3 + 3] == bytes(PALETTE[2])
    row_three = 3 * SCREEN_WIDTH * 3
    assert pixels[row_three : row_three + 3] == bytes(PALETTE[15])
    row_four = 4 * SCREEN_WIDTH * 3
    assert pixels[row_four : row_four + 3] == bytes(PALETTE[3])
    assert pixels[row_four + 4 * 3 : row_four + 4 * 3 + 3] == bytes(PALETTE[4])


def test_interrupt_output_tracks_vblank_and_register_enable() -> None:
    vdp = TMS9918A()
    _write_register(vdp, 1, 0x20)

    assert not vdp.interrupt_pending
    vdp.advance(VBLANK_LINE * TSTATES_PER_LINE)
    assert vdp.interrupt_pending

    status = vdp.read_port(0x81)
    assert status & 0x80
    assert not vdp.interrupt_pending


def test_sprite_magnification_and_16_by_16_pattern_layout() -> None:
    magnified = TMS9918A()
    _prepare_sprite_table(magnified)
    _write_register(magnified, 1, 0x41)
    _set_sprite(magnified, 0, x=10, pattern=1, color=2)
    pixels = bytearray(PALETTE[0] * (SCREEN_WIDTH * SCREEN_HEIGHT))
    magnified._render_sprites(pixels)
    assert pixels[10 * 3 : 10 * 3 + 3] == bytes(PALETTE[2])
    assert pixels[11 * 3 : 11 * 3 + 3] == bytes(PALETTE[2])
    assert pixels[(SCREEN_WIDTH + 10) * 3 : (SCREEN_WIDTH + 10) * 3 + 3] == bytes(
        PALETTE[2]
    )

    large = TMS9918A()
    _prepare_sprite_table(large)
    _write_register(large, 1, 0x42)
    _set_sprite(large, 0, x=10, pattern=5, color=2)
    large.vram[0x20] = 0x80  # top-left tile, row zero
    large.vram[0x28] = 0x80  # top-right tile, row zero
    pixels = bytearray(PALETTE[0] * (SCREEN_WIDTH * SCREEN_HEIGHT))
    large._render_sprites(pixels)
    assert pixels[10 * 3 : 10 * 3 + 3] == bytes(PALETTE[2])
    assert pixels[18 * 3 : 18 * 3 + 3] == bytes(PALETTE[2])


def test_offscreen_vertical_sprites_can_set_collision_during_vblank() -> None:
    vdp = TMS9918A()
    _prepare_sprite_table(vdp)
    _set_sprite(vdp, 0, x=10, pattern=1, color=2, raw_y=0xD1)
    _set_sprite(vdp, 1, x=10, pattern=2, color=4, raw_y=0xD1)

    vdp.advance(211 * TSTATES_PER_LINE)

    assert vdp.status & 0x80
    assert vdp.status & 0x20


def test_fifth_sprite_status_uses_first_raster_overflow() -> None:
    vdp = TMS9918A()
    _prepare_sprite_table(vdp)
    # Sprites 0-4 overflow on line 41; sprites 5-9 overflow earlier, on line 1.
    for index in range(10):
        raw_y = 40 if index < 5 else 0
        _set_sprite(vdp, index, x=index * 8, pattern=0, color=1, raw_y=raw_y)
    _set_sprite(vdp, 10, x=0, pattern=0, color=0, raw_y=0xD0)

    vdp.advance(2 * TSTATES_PER_LINE)

    assert vdp.status & 0x40
    assert vdp.status & 0x1F == 9


def test_sprite_collision_status_can_be_cleared_and_raised_on_later_scanline() -> None:
    vdp = TMS9918A()
    _prepare_sprite_table(vdp)
    _set_sprite(vdp, 0, x=12, pattern=1, color=0)
    _set_sprite(vdp, 1, x=12, pattern=2, color=8)
    vdp.vram[0x0008:0x0010] = bytes([0x80] * 8)
    vdp.vram[0x0010:0x0018] = bytes([0x80] * 8)

    vdp.advance(TSTATES_PER_LINE)
    assert vdp.status & 0x20

    status = vdp.read_port(0x81)
    assert status & 0x20
    assert not (vdp.status & 0x20)

    vdp.advance(TSTATES_PER_LINE)
    assert vdp.status & 0x20


def test_multicolor_pattern_row_advances_across_character_rows() -> None:
    vdp = TMS9918A()
    _write_register(vdp, 1, 0x48)  # Display enabled, Multicolor mode.
    _write_register(vdp, 2, 0x0E)  # Name table at 3800h.
    _write_register(vdp, 4, 0x01)  # Pattern table at 0800h.
    vdp.vram[0x3800] = 1
    vdp.vram[0x3820] = 1  # Reuse character 1 in the next 8-pixel row.
    vdp.vram[0x0808:0x0810] = bytes(
        (0x23, 0x45, 0x67, 0x89, 0xAB, 0xCD, 0xEF, 0x10)
    )

    full_frame = vdp.render_frame()
    top_pixel = full_frame[:3]
    second_character_row_pixel = full_frame[8 * SCREEN_WIDTH * 3 :][:3]

    assert top_pixel == bytes(PALETTE[2])
    assert second_character_row_pixel == bytes(PALETTE[6])

    vdp.advance(9 * TSTATES_PER_LINE)
    raster_pixel = vdp._working_framebuffer[8 * SCREEN_WIDTH * 3 :][:3]
    assert raster_pixel == bytes(PALETTE[6])
