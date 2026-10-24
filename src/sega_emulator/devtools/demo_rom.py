"""Gera uma ROM homebrew de diagnóstico visual para o VDP do SG-1000.

A ROM exibe um padrão de tabuleiro em Graphics I. Ela não é um jogo comercial
nem contém código/ativos de terceiros; serve para testar CPU, OUT, VDP e frontend.
"""

from __future__ import annotations

import argparse
import hashlib
from pathlib import Path


def _out(value: int, port: int) -> bytes:
    return bytes((0x3E, value & 0xFF, 0xD3, port & 0xFF))


def _register_write(register: int, value: int) -> bytes:
    return _out(value, 0xBF) + _out(0x80 | (register & 7), 0xBF)


def _set_vram_address(address: int, write: bool = True) -> bytes:
    command = ((address >> 8) & 0x3F) | (0x40 if write else 0)
    return _out(address & 0xFF, 0xBF) + _out(command, 0xBF)


def build_demo_rom() -> bytes:
    code = bytearray()
    # Register setup: 16 KiB VRAM, display on, Graphics I and table bases.
    for register, value in (
        (1, 0xC0),
        (2, 0x0E),
        (3, 0xFF),
        (4, 0x01),
        (5, 0x76),
        (6, 0x00),
        (7, 0x04),
    ):
        code.extend(_register_write(register, value))

    # Sprite table terminator prevents zero-filled VRAM from enabling sprites.
    code.extend(_set_vram_address(0x3B00))
    code.extend(_out(0xD0, 0xBE))

    # Pattern 1: alternating rows. Graphics I pattern table starts at 0800h.
    code.extend(_set_vram_address(0x0808))
    for value in (0xAA, 0x55) * 4:
        code.extend(_out(value, 0xBE))

    # Color group 0: green foreground and gray background.
    code.extend(_set_vram_address(0x3FC0))
    code.extend(_out(0x2E, 0xBE))

    # Name table at 3800h: 768 copies of tile 1, in three 256-byte loops.
    code.extend(_set_vram_address(0x3800))
    code.extend((0x3E, 0x01))  # LD A,1
    for _ in range(3):
        code.extend((0x06, 0x00))  # LD B,0 means 256 iterations with DJNZ.
        loop_address = len(code)
        code.extend((0xD3, 0xBE, 0x10, 0xFC))  # OUT (BE),A; DJNZ loop
        assert len(code) == loop_address + 4

    loop_address = len(code)
    code.extend((0xC3, loop_address & 0xFF, (loop_address >> 8) & 0xFF))
    if len(code) > 0x2000:
        raise RuntimeError("A ROM de diagnóstico excedeu 8 KiB.")
    return bytes(code).ljust(0x2000, b"\x00")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--output",
        type=Path,
        default=Path("sg1000-demo.rom"),
        help="arquivo ROM de saída (padrão: sg1000-demo.rom)",
    )
    args = parser.parse_args()
    rom = build_demo_rom()
    args.output.write_bytes(rom)
    print(f"ROM homebrew criada: {args.output.resolve()}")
    print(f"Tamanho: {len(rom)} bytes")
    print(f"SHA-256: {hashlib.sha256(rom).hexdigest()}")


if __name__ == "__main__":
    main()
