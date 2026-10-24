"""Regressões para avaliação por scanline e renderização de sprites TMS9918A."""

from sega_emulator.video.tms9918 import TMS9918A


def _write_register(vdp: TMS9918A, register: int, value: int) -> None:
    vdp.write_port(0xBF, value)
    vdp.write_port(0xBF, 0x80 | register)


def _setup(vdp: TMS9918A, *, size16: bool = False, magnified: bool = False) -> None:
    mode = 0x40 | (0x02 if size16 else 0) | (0x01 if magnified else 0)
    _write_register(vdp, 1, mode)
    _write_register(vdp, 5, 0x40)  # atributos em 0x2000
    _write_register(vdp, 6, 0x00)  # padrões em 0x0000
    vdp.vram[0:0x400] = b"\xff" * 0x400


def _sprite(
    vdp: TMS9918A, index: int, y: int, x: int, pattern: int = 0, color: int = 1
) -> None:
    offset = 0x2000 + index * 4
    vdp.vram[offset : offset + 4] = bytes((y & 0xFF, x & 0xFF, pattern, color))


def _pixel(frame: bytes, x: int, y: int) -> bytes:
    offset = (y * 256 + x) * 3
    return frame[offset : offset + 3]


def test_opaque_overlap_sets_collision_and_preserves_first_sprite_priority() -> None:
    vdp = TMS9918A()
    _setup(vdp)
    _sprite(vdp, 0, 0xFF, 20, color=2)
    _sprite(vdp, 1, 0xFF, 20, color=4)
    _sprite(vdp, 2, 0xD0, 0)

    frame = vdp.render_frame()

    assert vdp.status & 0x20
    assert _pixel(frame, 20, 0) == bytes((33, 200, 66))


def test_fifth_sprite_sets_overflow_flag_and_fifth_sprite_index() -> None:
    vdp = TMS9918A()
    _setup(vdp)
    for index in range(5):
        _sprite(vdp, index, 0xFF, index * 12)
    _sprite(vdp, 5, 0xD0, 0)

    vdp.render_frame()

    assert vdp.status & 0x40
    assert vdp.status & 0x1F == 4


def test_transparent_sprite_pixels_do_not_collide() -> None:
    vdp = TMS9918A()
    _setup(vdp)
    vdp.vram[0:8] = b"\x00" * 8
    _sprite(vdp, 0, 0xFF, 20, color=1)
    _sprite(vdp, 1, 0xFF, 20, color=2)
    _sprite(vdp, 2, 0xD0, 0)

    vdp.render_frame()

    assert not (vdp.status & 0x20)


def test_transparent_sprite_still_counts_toward_scanline_limit() -> None:
    vdp = TMS9918A()
    _setup(vdp)
    for index in range(4):
        _sprite(vdp, index, 0xFF, index * 12, color=0)
    _sprite(vdp, 4, 0xFF, 60, color=1)
    _sprite(vdp, 5, 0xD0, 0)

    vdp.render_frame()

    assert vdp.status & 0x40
    assert vdp.status & 0x1F == 4


def test_early_sprite_keeps_priority_when_later_sprite_overlaps() -> None:
    vdp = TMS9918A()
    _setup(vdp)
    _sprite(vdp, 0, 0xFF, 40, color=2)
    _sprite(vdp, 1, 0xFF, 44, color=4)
    _sprite(vdp, 2, 0xD0, 0)

    frame = vdp.render_frame()

    assert _pixel(frame, 44, 0) == bytes((33, 200, 66))
    assert vdp.status & 0x20


def test_sprite_can_be_shifted_left_by_32_pixels() -> None:
    vdp = TMS9918A()
    _setup(vdp)
    _sprite(vdp, 0, 0xFF, 32, color=2 | 0x80)
    _sprite(vdp, 1, 0xD0, 0)

    frame = vdp.render_frame()

    assert _pixel(frame, 0, 0) == bytes((33, 200, 66))
    assert _pixel(frame, 20, 0) == bytes((0, 0, 0))


def test_16x16_sprite_uses_aligned_four_pattern_block() -> None:
    vdp = TMS9918A()
    _setup(vdp, size16=True)
    _sprite(vdp, 0, 0xFF, 10, pattern=3, color=2)
    _sprite(vdp, 1, 0xD0, 0)
    # Em 16x16, os dois bits inferiores do número do padrão são ignorados.
    vdp.vram[0x00:0x08] = b"\x00" * 8
    vdp.vram[0x08:0x10] = b"\x00" * 8
    vdp.vram[0x10:0x18] = b"\x00" * 8
    vdp.vram[0x18:0x20] = b"\x80" * 8

    frame = vdp.render_frame()

    assert _pixel(frame, 10, 0) == bytes((0, 0, 0))
    assert _pixel(frame, 10, 8) == bytes((33, 200, 66))


def test_status_read_clears_sprite_flags() -> None:
    vdp = TMS9918A()
    vdp.status = 0x7F

    assert vdp.read_port(0xBF) == 0x7F
    assert vdp.status == 0x1F
