"""Benchmark reproduzível do custo do PSG isolado e na máquina SG-1000.

Executar em CI para comparar commits; os números são diagnósticos de runner,
não metas absolutas de desempenho entre máquinas diferentes.
"""

from __future__ import annotations

import cProfile
import pstats
from time import perf_counter

from sega_emulator.audio.sn76489 import DEFAULT_CLOCK_HZ, SN76489
from sega_emulator.machine.sg1000 import SG1000Machine


def _disable_psg_clock(clocks: int) -> None:
    """Usado apenas como controle experimental do custo de integração do PSG."""


def benchmark_psg(seconds: int = 2) -> None:
    psg = SN76489()
    psg.write(0x90)
    psg.write(0x8E)
    psg.write(0x0F)
    start = perf_counter()
    psg.advance_clocks(DEFAULT_CLOCK_HZ * seconds)
    elapsed = perf_counter() - start
    print(
        "PSG isolado: "
        f"{seconds} s simulados em {elapsed:.4f} s reais "
        f"({seconds / elapsed:.2f}x tempo real)"
    )
    print(f"Amostras produzidas: {len(psg.drain_samples())}")


def benchmark_machine(frames: int = 3) -> None:
    results: dict[str, float] = {}
    for label, enabled in (("PSG integrado", True), ("PSG clock desativado", False)):
        machine = SG1000Machine(bytes((0xC3, 0x00, 0x00)))
        if not enabled:
            machine.psg.advance_clocks = _disable_psg_clock
        start = perf_counter()
        machine.run_frames(frames)
        elapsed = perf_counter() - start
        results[label] = elapsed
        print(
            f"{label}: {frames} quadros em {elapsed:.4f} s "
            f"({frames / elapsed:.2f} quadros/s; "
            f"{machine.total_instructions / elapsed:.0f} instruções/s)"
        )

    baseline = results["PSG clock desativado"]
    integrated = results["PSG integrado"]
    print(
        "Custo relativo observado do PSG: "
        f"{((integrated / baseline) - 1) * 100:.1f}% "
        "(mesmo runner, ROM sintética JP 0000h)"
    )


def profile_integrated_machine() -> None:
    machine = SG1000Machine(bytes((0xC3, 0x00, 0x00)))
    profiler = cProfile.Profile()
    profiler.enable()
    machine.run_frame()
    profiler.disable()
    print("Top 20 funções por tempo cumulativo (1 quadro, PSG integrado):")
    pstats.Stats(profiler).strip_dirs().sort_stats("cumulative").print_stats(20)


if __name__ == "__main__":
    benchmark_psg()
    benchmark_machine()
    profile_integrated_machine()
