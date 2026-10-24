"""Executa ZEXDOC/ZEXALL externos contra o núcleo Z80 do projeto.

O arquivo .COM é externo e distribuído separadamente sob GPL; não o versione.
O harness prepara uma mini-CP/M e intercepta BDOS (CALL 0005h, funções 2 e 9).

Exemplos:
  python run_zex.py zexall.com --max-seconds 300
  python run_zex.py zexall.com --tests 0,1,2
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
        self.m[address] = value & 0xFF


class NullIO:
    def read_port(self, address: int) -> int:
        return 0xFF

    def write_port(self, address: int, value: int) -> None:
        pass


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("com", type=Path)
    parser.add_argument("--max-seconds", type=float, default=0)
    parser.add_argument(
        "--tests",
        default="",
        help="índices de teste (0..66), separados por vírgula",
    )
    args = parser.parse_args()
    if args.max_seconds < 0:
        parser.error("--max-seconds deve ser >= 0")

    image = args.com.read_bytes()
    if not image or len(image) > 0xFF00:
        parser.error("arquivo .COM vazio ou grande demais para a TPA CP/M")
    memory = CpmMemory()
    memory.m[0x100 : 0x100 + len(image)] = image
    if args.tests:
        try:
            indices = [int(value) for value in args.tests.split(",")]
        except ValueError:
            parser.error("--tests aceita apenas índices inteiros separados por vírgula")
        if not indices or any(index < 0 or index > 66 for index in indices):
            parser.error("índices de --tests devem estar entre 0 e 66")
        table = 0x100 + 0x3A
        entries = [
            memory.m[table + 2 * index] | (memory.m[table + 2 * index + 1] << 8)
            for index in range(67)
        ]
        for index, address in enumerate([entries[i] for i in indices] + [0]):
            memory.m[table + 2 * index] = address & 0xFF
            memory.m[table + 2 * index + 1] = (address >> 8) & 0xFF

    memory.m[0x0000] = 0x76
    memory.m[0x0005] = 0xC9
    memory.m[0x0006], memory.m[0x0007] = 0x00, 0xF0
    cpu = Z80Cpu(memory, NullIO())
    cpu.pc = 0x100
    cpu.sp = 0xF000
    line: list[str] = []
    tests_done = 0
    failures = 0
    started = time.perf_counter()
    steps = 0

    def emit(text: str) -> None:
        nonlocal tests_done, failures
        sys.stdout.write(text)
        sys.stdout.flush()
        if "ERROR" not in text and text.rstrip().endswith("OK"):
            tests_done += 1
        if "ERROR" in text:
            failures += 1

    while True:
        pc = cpu._pc
        if pc == 0x0005:
            if cpu.c == 2:
                character = chr(cpu.e)
                line.append(character)
                if character == "\n":
                    emit("".join(line))
                    line.clear()
            elif cpu.c == 9:
                address = cpu.de
                output: list[str] = []
                while address < 0x10000 and memory.m[address] != 0x24:
                    output.append(chr(memory.m[address]))
                    address += 1
                if address >= 0x10000:
                    print(
                        "\nERRO: string BDOS sem terminador '$'",
                        file=sys.stderr,
                    )
                    return 2
                emit("".join(output))
        elif pc == 0x0000 and steps:
            break

        cpu.step()
        steps += 1
        if args.max_seconds and (steps & 0xFFFF) == 0:
            elapsed = time.perf_counter() - started
            if elapsed > args.max_seconds:
                print(
                    f"\n[interrompido após {elapsed:.0f}s; {steps:,} instruções; "
                    f"testes OK: {tests_done}; ERROR: {failures}]"
                )
                return 3

    elapsed = time.perf_counter() - started
    print(
        f"\n[concluído em {elapsed:.0f}s; {steps:,} instruções; "
        f"testes OK: {tests_done}; ERROR: {failures}]"
    )
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
