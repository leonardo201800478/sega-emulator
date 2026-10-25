"""Executa ZEXDOC/ZEXALL (exercitadores Z80 de Frank Cringle) no núcleo do projeto.

O arquivo .com é um artefato EXTERNO (GPL): baixe-o, não o versione.
Mini-CP/M: carrega em 0100h, intercepta BDOS (CALL 0005h: funções 2 e 9) e
termina em PC=0000h (warm boot). Cada teste imprime "OK" ou "ERROR ... CRC".

Uso:  python run_zex.py zexall.com [--max-seconds N] [--only 'subtexto ...']
Em CPython o conjunto completo leva muitas horas; --max-seconds interrompe
de forma limpa e informa quantos testes concluíram.
"""

from __future__ import annotations

import argparse
import sys
import time
from pathlib import Path

from sega_emulator.cpu.z80 import Z80Cpu


class CpmMemory:
    def __init__(self) -> None:
        self.m = bytearray(0x10000)

    def read(self, address: int) -> int:
        return self.m[address]

    def write(self, address: int, value: int) -> None:
        self.m[address] = value


class NullIO:
    def read_port(self, address: int) -> int:
        return 0xFF

    def write_port(self, address: int, value: int) -> None:
        pass


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("com", type=Path)
    ap.add_argument("--max-seconds", type=float, default=0)
    ap.add_argument(
        "--tests",
        default="",
        help="índices (0..66) separados por vírgula; remonta a tabela de testes "
        "do ZEX (offset 003Ah no .com) para rodar só esses",
    )
    args = ap.parse_args()

    mem = CpmMemory()
    image = args.com.read_bytes()
    mem.m[0x100 : 0x100 + len(image)] = image
    if args.tests:
        table = 0x100 + 0x3A
        entries = [
            mem.m[table + 2 * i] | (mem.m[table + 2 * i + 1] << 8) for i in range(67)
        ]
        chosen = [entries[int(t)] for t in args.tests.split(",")]
        for i, addr in enumerate(chosen + [0]):
            mem.m[table + 2 * i] = addr & 0xFF
            mem.m[table + 2 * i + 1] = addr >> 8
    mem.m[0x0000] = 0x76  # HALT em warm boot (também detectado por PC==0)
    mem.m[0x0005] = 0xC9  # RET: o BDOS é tratado no laço
    mem.m[0x0006], mem.m[0x0007] = 0x00, 0xF0  # topo da TPA (SP inicial do ZEX)
    cpu = Z80Cpu(mem, NullIO())
    cpu.pc = 0x100
    cpu.sp = 0xF000
    line: list[str] = []
    tests_done = 0
    failures = 0
    start = time.perf_counter()
    last_report = start
    steps = 0

    def emit(text: str) -> None:
        nonlocal tests_done, failures
        sys.stdout.write(text)
        sys.stdout.flush()
        if "OK" in text and text.endswith("\n"):
            tests_done += 1
        if "ERROR" in text:
            failures += 1

    while True:
        pc = cpu._pc
        if pc == 0x0005:
            c = cpu.c
            if c == 2:
                ch = chr(cpu.e)
                line.append(ch)
                if ch == "\n":
                    emit("".join(line))
                    line.clear()
            elif c == 9:
                address = cpu.de
                out = []
                while mem.m[address] != 0x24:  # '$'
                    out.append(chr(mem.m[address]))
                    address += 1
                text = "".join(out)
                emit(text)
        elif pc == 0x0000 and steps:
            break
        cpu.step()
        steps += 1
        if args.max_seconds and (steps & 0xFFFF) == 0:
            now = time.perf_counter()
            if now - start > args.max_seconds:
                print(
                    f"\n[interrompido após {now - start:.0f}s; {steps:,} instruções; "
                    f"testes OK: {tests_done}; ERROR: {failures}]"
                )
                return 3
    print(
        f"\n[concluído em {time.perf_counter() - start:.0f}s; {steps:,} instruções; "
        f"testes OK: {tests_done}; ERROR: {failures}]"
    )
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())