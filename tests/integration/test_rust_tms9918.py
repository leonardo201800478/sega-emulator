from __future__ import annotations

import pytest

from sega_emulator.video.tms9918 import TMS9918A


def _rust_vdp():
    native = pytest.importorskip("sega_z80_core")
    return native.RustTMS9918A


def test_rust_tms9918a_port_protocol_matches_python() -> None:
    RustTMS9918A = _rust_vdp()
    reference = TMS9918A()
    native = RustTMS9918A()

    for vdp in (reference, native):
        # Set VRAM write address 0x1234, then write across the 16 KiB boundary.
        vdp.write_port(0x81, 0x34)
        vdp.write_port(0x81, 0x52 | 0x40)
        for value in (0xA5, 0x5A, 0xC3):
            vdp.write_port(0x80, value)
        vdp.write_port(0x81, 0x34)
        vdp.write_port(0x81, 0x12)
    assert bytes(native.vram) == bytes(reference.vram)
    assert native.address == reference.address
    assert native.read_buffer == reference.read_buffer
    for _ in range(3):
        assert native.read_port(0x80) == reference.read_port(0x80)
    assert native.address == reference.address
    assert native.read_buffer == reference.read_buffer


@pytest.mark.parametrize("mode", [0, 1, 2, 4])
def test_rust_tms9918a_framebuffer_matches_python_for_video_modes(mode: int) -> None:
    RustTMS9918A = _rust_vdp()
    reference = TMS9918A()
    native = RustTMS9918A()

    # Deterministic, nontrivial tile/pattern/color data; terminate sprite list
    # immediately so this test isolates background/text rendering.
    vram = bytearray(((i * 37 + (i >> 5) * 13 + 7) & 0xFF) for i in range(0x4000))
    vram[0] = 0xD0
    registers = bytearray(8)
    registers[1] = 0x40 | (0x10 if mode == 1 else 0x08 if mode == 2 else 0)
    registers[0] = 0x02 if mode == 4 else 0
    registers[2] = 0x0E
    registers[3] = 0xFF
    registers[4] = 0x07
    registers[5] = 0
    registers[6] = 0x07
    registers[7] = 0xF4

    for vdp in (reference, native):
        vdp.vram[:] = vram
        vdp.registers[:] = registers

    assert native.render_frame() == reference.render_frame()


def test_rust_tms9918a_raster_and_vblank_match_python() -> None:
    RustTMS9918A = _rust_vdp()
    reference = TMS9918A()
    native = RustTMS9918A()

    # Advance across partial and exact scanline/frame boundaries.
    for cycles in (1, 227, 228, 192 * 228 - 456, 70 * 228, 17):
        reference.advance(cycles)
        native.advance(cycles)
        assert native.scanline == reference.scanline
        assert native.frame_number == reference.frame_number
        assert native.status == reference.status
        assert native.consume_frame_ready() == reference.consume_frame_ready()
        assert native.framebuffer == reference.framebuffer
