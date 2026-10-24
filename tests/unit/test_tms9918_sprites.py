"""Regressões de status e prioridade dos sprites TMS9918A."""

from sega_emulator.video.tms9918 import TMS9918A


def _write_register(vdp: TMS9918A, register: int, value: int) -> None:
    vdp.write_port(0xBF, value)
    vdp.write_port(0xBF, 0x80 | register)


def _setup_sprites(vdp: TMS9918A) -> None:
    _write_register(vdp, 1, 0x40)  # display enabled; sprites 8x8
    _write_register(vdp, 5, 0x40)  # sprite attributes at 0x2000
    _write_register(vdp, 6, 0x00)  # sprite patterns at 0x0000
    vdp.vram[0:8] = b"\xff" * 8


def _sprite(vdp: TMS9918A, index: int, y: int, x: int, color: int = 1) -> None:
    offset = 0x2000 + index * 4
    vdp.vram[offset : offset + 4] = bytes((y, x, 0, color))


def test_overlapping_opaque_sprites_set_collision_flag_and_keep_first_sprite() -> None:
    vdp = TMS9918A()
    _setup_sprites(vdp)
    _sprite(vdp, 0, 0xFF, 20, 2)  # top line is y=0
    _sprite(vdp, 1, 0xFF, 20, 4)
    _sprite(vdp, 2, 0xD0, 0)

    frame = vdp.render_frame()
    assert vdp.status & 0x20
    assert frame[(20 * 3) : (20 * 3 + 3)] == bytes((212, 82, 77))


def test_fifth_sprite_on_a_scanline_sets_overflow_and_sprite_index() -> None:
    vdp = TMS9918A()
    _setup_sprites(vdp)
    for index in range(5):
        _sprite(vdp, index, 0xFF, index * 12, 1)
    _sprite(vdp, 5, 0xD0, 0)

    vdp.render_frame()
    assert vdp.status & 0x40
    assert vdp.status & 0x1F == 4


def test_transparent_sprite_pixels_do_not_trigger_collision() -> None:
    vdp = TMS9918A()
    _setup_sprites(vdp)
    vdp.vram[0:8] = b"\x00" * 8
    _sprite(vdp, 0, 0xFF, 20, 1)
    _sprite(vdp, 1, 0xFF, 20, 2)
    _sprite(vdp, 2, 0xD0, 0)

    vdp.render_frame()
    assert not (vdp.status & 0x20)


def test_status_read_clears_collision_and_overflow_flags() -> None:
    vdp = TMS9918A()
    vdp.status = 0x7F
    assert vdp.read_port(0xBF) == 0x7F
    assert vdp.status == 0x1F
