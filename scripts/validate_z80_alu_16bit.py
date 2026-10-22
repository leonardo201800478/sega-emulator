"""Validador opcional da ALU Z80 de 16 bits com progresso e multiprocessing.

O modo completo verifica todos os pares HL/BC (2**32 pares por operação).
É deliberadamente separado da suíte pytest padrão, pois envolve cerca de
21,47 bilhões de execuções de instruções e pode demorar bastante mesmo em
máquinas com vários núcleos.

Exemplos:
    python scripts/validate_z80_alu_16bit.py --left-count 16
    python scripts/validate_z80_alu_16bit.py --left-count 1024 --sampling distributed
    python scripts/validate_z80_alu_16bit.py --full
"""

from __future__ import annotations

import argparse
import multiprocessing
import os
import sys
import time
from collections.abc import Iterable
from multiprocessing.pool import Pool

from sega_emulator.bus.memory import RAM_SIZE, SG1000Ram
from sega_emulator.bus.memory_bus import SG1000MemoryBus
from sega_emulator.cpu.z80 import Z80Cpu

MASK16 = 0xFFFF
TOTAL_LEFT_VALUES = 0x10000
OPERATIONS = (
    ("ADD HL,BC", bytes((0x09,)), False, 11),
    ("ADC HL,BC", bytes((0xED, 0x4A)), True, 15),
    ("SBC HL,BC", bytes((0xED, 0x42)), True, 15),
)


def _expected(
    operation: str, left: int, right: int, carry: int
) -> tuple[int, int]:
    if operation == "ADD HL,BC":
        raw = left + right
        result = raw & MASK16
        flags = (left >> 8) & 0xC4
        flags |= (result >> 8) & 0x28
        if (left & 0x0FFF) + (right & 0x0FFF) > 0x0FFF:
            flags |= 0x10
        if raw > MASK16:
            flags |= 0x01
        return result, flags

    if operation == "ADC HL,BC":
        raw = left + right + carry
        result = raw & MASK16
        flags = (result >> 8) & 0xA8
        if result == 0:
            flags |= 0x40
        if (left & 0x0FFF) + (right & 0x0FFF) + carry > 0x0FFF:
            flags |= 0x10
        if (~(left ^ right) & (left ^ result) & 0x8000) != 0:
            flags |= 0x04
        if raw > MASK16:
            flags |= 0x01
        return result, flags

    raw = left - right - carry
    result = raw & MASK16
    flags = ((result >> 8) & 0xA8) | 0x02
    if result == 0:
        flags |= 0x40
    if (left & 0x0FFF) < ((right & 0x0FFF) + carry):
        flags |= 0x10
    if ((left ^ right) & (left ^ result) & 0x8000) != 0:
        flags |= 0x04
    if raw < 0:
        flags |= 0x01
    return result, flags


def _check_left_chunk(left_values: tuple[int, ...]) -> int:
    """Valida todos os valores BC para cada HL atribuído a este processo."""
    ram = SG1000Ram(bytes(RAM_SIZE))
    checked = 0

    for operation, program, uses_carry, expected_cycles in OPERATIONS:
        cpu = Z80Cpu(SG1000MemoryBus(ram, program))
        carry_values = (0, 1) if uses_carry else (0,)

        for left in left_values:
            for right in range(TOTAL_LEFT_VALUES):
                for carry in carry_values:
                    old_flags = ((left >> 8) & 0xC4) | carry
                    expected_result, expected_flags = _expected(
                        operation, left, right, carry
                    )
                    cpu.hl = left
                    cpu.bc = right
                    cpu.f = old_flags
                    cpu.pc = 0

                    actual_cycles = cpu.step()
                    if actual_cycles != expected_cycles:
                        raise AssertionError(
                            f"{operation} ciclos: HL={left:04X} BC={right:04X} "
                            f"C={carry}; obtido={actual_cycles}, "
                            f"esperado={expected_cycles}"
                        )
                    if cpu.hl != expected_result:
                        raise AssertionError(
                            f"{operation} resultado: HL={left:04X} BC={right:04X} "
                            f"C={carry}; obtido={cpu.hl:04X}, "
                            f"esperado={expected_result:04X}"
                        )
                    if cpu.f != expected_flags:
                        raise AssertionError(
                            f"{operation} flags: HL={left:04X} BC={right:04X} "
                            f"C={carry}; obtido={cpu.f:02X}, "
                            f"esperado={expected_flags:02X}"
                        )
                    checked += 1

    return len(left_values)


def _chunks(values: Iterable[int], size: int) -> Iterable[tuple[int, ...]]:
    chunk: list[int] = []
    for value in values:
        chunk.append(value)
        if len(chunk) == size:
            yield tuple(chunk)
            chunk.clear()
    if chunk:
        yield tuple(chunk)


def _select_left_values(count: int, sampling: str) -> tuple[int, ...]:
    """Seleciona HL em ordem determinística, sequencial ou distribuída."""
    if sampling == "sequential":
        return tuple(range(count))

    if count == 1:
        return (0x8000,)
    if count < 13:
        return tuple(round(index * MASK16 / (count - 1)) for index in range(count))

    # Inclui fronteiras importantes para carry, half-carry, sinal e wraparound.
    boundary_values = (
        0x0000,
        0x0001,
        0x000F,
        0x0010,
        0x00FF,
        0x0100,
        0x0FFF,
        0x1000,
        0x7FFF,
        0x8000,
        0x8001,
        0xFFFE,
        0xFFFF,
    )
    selected = {
        round(index * MASK16 / (count - 1)) for index in range(count)
    }
    required = set(boundary_values)

    # Substitui os quantis mais próximos pelas fronteiras ainda ausentes,
    # preservando o tamanho da amostra e a distribuição pelo domínio inteiro.
    for boundary in boundary_values:
        if boundary in selected:
            continue
        replaceable = selected - required
        if replaceable:
            nearest = min(replaceable, key=lambda value: abs(value - boundary))
            selected.remove(nearest)
            selected.add(boundary)

    return tuple(sorted(selected))

def _format_duration(seconds: float) -> str:
    seconds = max(0, int(seconds))
    hours, remainder = divmod(seconds, 3600)
    minutes, seconds = divmod(remainder, 60)
    if hours:
        return f"{hours}h{minutes:02d}m{seconds:02d}s"
    if minutes:
        return f"{minutes}m{seconds:02d}s"
    return f"{seconds}s"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument(
        "--full",
        action="store_true",
        help="verifica todos os 65.536 valores de HL (teste muito demorado)",
    )
    mode.add_argument(
        "--left-count",
        type=int,
        default=16,
        help="verifica os primeiros N valores de HL contra todos os valores de BC "
        "(padrão: 16; use 65536 para cobertura completa)",
    )
    parser.add_argument(
        "--sampling",
        choices=("sequential", "distributed"),
        default="sequential",
        help=(
            "seleção de HL para --left-count (padrão: sequencial; "
            "distributed cobre todo o domínio)"
        ),
    )
    parser.add_argument(
        "--workers",
        type=int,
        default=min(8, max(1, (os.cpu_count() or 2) - 1)),
        help="processos paralelos (padrão: núcleos lógicos menos um)",
    )
    parser.add_argument(
        "--chunk-size",
        type=int,
        default=4,
        help="quantos valores HL cada tarefa processa (padrão: 4)",
    )
    args = parser.parse_args()

    left_count = TOTAL_LEFT_VALUES if args.full else args.left_count
    if not 1 <= left_count <= TOTAL_LEFT_VALUES:
        parser.error("--left-count deve estar entre 1 e 65536")
    if args.workers < 1:
        parser.error("--workers deve ser pelo menos 1")
    if args.chunk_size < 1:
        parser.error("--chunk-size deve ser pelo menos 1")

    left_values = _select_left_values(left_count, args.sampling)
    tasks = list(_chunks(left_values, args.chunk_size))
    workers = min(args.workers, len(tasks))
    total_cases = left_count * 0x10000 * 5
    print(
        f"ALU Z80 16-bit: {left_count:,}/65,536 valores HL; "
        f"até {total_cases:,} verificações; {workers} processos; "
        f"amostragem {args.sampling}.",
        flush=True,
    )
    if args.full:
        print(
            "Modo completo: estimativa de duração depende do processador; "
            "o teste pode levar horas.",
            flush=True,
        )

    started = time.monotonic()
    completed_left = 0
    pool = multiprocessing.get_context("spawn").Pool(processes=workers)
    iterator = pool.imap_unordered(_check_left_chunk, tasks)
    try:
        while completed_left < left_count:
            try:
                count = iterator.next(timeout=0.25)
                completed_left += count
            except multiprocessing.TimeoutError:
                pass

            elapsed = time.monotonic() - started
            percent = completed_left * 100 / left_count
            rate = completed_left / elapsed if elapsed > 0 else 0
            remaining = (
                (left_count - completed_left) / rate if rate > 0 else 0
            )
            bar_width = 24
            filled = int(bar_width * percent / 100)
            bar = "#" * filled + "-" * (bar_width - filled)
            print(
                f"\r[{bar}] {percent:6.2f}% "
                f"HL {completed_left:,}/{left_count:,} "
                f"| decorrido {_format_duration(elapsed)} "
                f"| restante ~{_format_duration(remaining)}",
                end="",
                flush=True,
            )
        pool.close()
        pool.join()
    except BaseException:
        pool.terminate()
        pool.join()
        raise

    elapsed = time.monotonic() - started
    print(
        f"\nAPROVADO: {left_count:,} valores HL verificados em "
        f"{_format_duration(elapsed)} ({total_cases:,} verificações)."
    )
    return 0


if __name__ == "__main__":
    multiprocessing.freeze_support()
    try:
        raise SystemExit(main())
    except AssertionError as error:
        print(f"FALHA: {error}", file=sys.stderr)
        raise SystemExit(1) from error
