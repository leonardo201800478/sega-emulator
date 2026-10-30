"""Differential tests for the native TMS9918A port."""

from __future__ import annotations

import pytest

from sega_emulator.video.tms9918 import TMS9918A


def _write_register(vdp, index: int, value: int) -> None:
    vdp.write_port(0x81, value)
    vdp.write_port(0x81, 0x80 | index)


def _write_vram(vdp, address: int, data: bytes) -> None:
    vdp.write_port(0x81, address & 0xFF)
    vdp.write_port(0x81, ((address >> 8) & 0x3F) | 0x40)
    for value in data:
        vdp.write_port(0x80, value)


def _configured_pair(native_type):
    python_vdp = TMS9918A()
    rust_vdp = native_type()
    vram = bytes((index * 37 + 11) & 0xFF for index in range(0x4000))
    for vdp in (python_vdp, rust_vdp):
        _write_vram(vdp, 0, vram)
        # End the sprite list deterministically at sprite zero.
        _write_vram(vdp, 0x0F80, b"\xd0")
    return python_vdp, rust_vdp


@pytest.mark.parametrize(
    ("register_zero", "register_one"),
    [
        (0x00, 0x40),  # Graphics I
        (0x00, 0x50),  # Text
        (0x00, 0x48),  # Multicolor
        (0x02, 0x40),  # Graphics II
    ],
)
def test_native_vdp_frame_rendering_matches_python(
    register_zero: int, register_one: int
) -> None:
    native_core = pytest.importorskip("sega_z80_core")
    python_vdp, rust_vdp = _configured_pair(native_core.NativeTms9918A)
    registers = {
        0: register_zero,
        1: register_one,
        2: 0,
        3: 0,
        4: 0,
        5: 0x1F,
        6: 0,
        7: 0xF1,
    }
    for vdp in (python_vdp, rust_vdp):
        for index, value in registers.items():
            _write_register(vdp, index, value)

    assert rust_vdp.render_frame() == python_vdp.render_frame()


def test_native_vdp_raster_and_vblank_match_python() -> None:
    native_core = pytest.importorskip("sega_z80_core")
    python_vdp, rust_vdp = _configured_pair(native_core.NativeTms9918A)
    for vdp in (python_vdp, rust_vdp):
        _write_register(vdp, 0, 0x00)
        _write_register(vdp, 1, 0x40)
        _write_register(vdp, 5, 0x1F)
        _write_register(vdp, 7, 0x03)
        vdp.advance(228 * 192)

    assert rust_vdp.scanline == python_vdp.scanline
    assert rust_vdp.frame_number == python_vdp.frame_number
    assert rust_vdp.status == python_vdp.status
    assert rust_vdp.interrupt_pending == python_vdp.interrupt_pending
    assert rust_vdp.framebuffer == python_vdp.framebuffer
    assert rust_vdp.consume_frame_ready() is python_vdp.consume_frame_ready()


def test_native_vdp_sprite_composition_and_status_match_python() -> None:
    native_core = pytest.importorskip("sega_z80_core")
    python_vdp, rust_vdp = _configured_pair(native_core.NativeTms9918A)
    sprites = b"".join(bytes((0, x, 1, 0x0F)) for x in (20, 22, 24, 26, 28))
    sprites += b"\xd0\x00\x00\x00"

    for vdp in (python_vdp, rust_vdp):
        _write_register(vdp, 0, 0x00)
        _write_register(vdp, 1, 0x40)
        _write_register(vdp, 5, 0x1F)
        _write_register(vdp, 6, 0x01)
        _write_vram(vdp, 0x0808, b"\xff" * 8)
        _write_vram(vdp, 0x0F80, sprites)
        vdp.advance(228 * 2)

    assert rust_vdp.status == python_vdp.status
    assert rust_vdp.status & 0x40
    assert rust_vdp.status & 0x20
    assert rust_vdp.render_frame() == python_vdp.render_frame()
