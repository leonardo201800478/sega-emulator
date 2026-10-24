"""Perfil de diagnóstico por componente para o SG-1000.

Usa uma ROM sintética de NOPs; não representa desempenho de um jogo real.
Compara CPU/barramento, VDP isolado e integração CPU+VDP com a mesma carga
nominal de um quadro. Não altera o núcleo nem impõe limites de desempenho.
"""

from __future__ import annotations

import argparse
import cProfile
import io
import pstats
import statistics
import time
from collections.abc import Callable

from sega_emulator.machine.sg1000 import SG1000Machine
from sega_emulator.video.tms9918 import TMS9918A

Z80_CLOCK_HZ = 3_579_545
SYNTHETIC_ROM = bytes(0x4000)


def configure_graphics(machine: SG1000Machine) -> None:
    """Cria uma cena determinística Graphics I com tabela de sprites vazia."""
    vdp = machine.vdp
    vdp.registers[:] = bytes((0x00, 0x40, 0x0E, 0xFF, 0x01, 0x40, 0x00, 0xF4))
    for address in range(len(vdp.vram)):
        vdp.vram[address] = (address * 37 + 13) & 0xFF
    # Atributos em 2000h; primeiro sprite termina a lista para evitar
    # trabalho de sprites aleatórios e tornar a cena reprodutível.
    vdp.vram[0x2000] = 0xD0


def new_machine() -> SG1000Machine:
    machine = SG1000Machine(SYNTHETIC_ROM)
    configure_graphics(machine)
    return machine


def capture_one_frame_cycles() -> list[int]:
    machine = new_machine()
    start_frame = machine.vdp.frame_number
    cycles: list[int] = []
    while machine.vdp.frame_number == start_frame:
        cycles.append(machine.step())
    return cycles


def measure(operation: Callable[[], object], repeats: int) -> list[float]:
    samples: list[float] = []
    for _ in range(repeats):
        started = time.perf_counter()
        operation()
        samples.append(time.perf_counter() - started)
    return samples


def run_cpu_only(instruction_count: int) -> None:
    machine = new_machine()
    for _ in range(instruction_count):
        machine.cpu.step()


def run_vdp_only(cycles: list[int]) -> None:
    vdp = TMS9918A()
    vdp.registers[:] = bytes((0x00, 0x40, 0x0E, 0xFF, 0x01, 0x40, 0x00, 0xF4))
    for address in range(len(vdp.vram)):
        vdp.vram[address] = (address * 37 + 13) & 0xFF
    vdp.vram[0x2000] = 0xD0
    for elapsed_cycles in cycles:
        vdp.advance(elapsed_cycles)


def run_integrated_frame() -> None:
    new_machine().run_frame()


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repeats", type=int, default=5)
    parser.add_argument("--profile", type=str, default=None, help="arquivo .pstats")
    args = parser.parse_args()
    if args.repeats < 1:
        parser.error("--repeats deve ser >= 1")

    cycle_trace = capture_one_frame_cycles()
    instruction_count = len(cycle_trace)
    total_tstates = sum(cycle_trace)

    cpu_samples = measure(lambda: run_cpu_only(instruction_count), args.repeats)
    vdp_samples = measure(lambda: run_vdp_only(cycle_trace), args.repeats)
    integrated_samples = measure(run_integrated_frame, args.repeats)

    cpu_median = statistics.median(cpu_samples)
    vdp_median = statistics.median(vdp_samples)
    integrated_median = statistics.median(integrated_samples)

    print("SG-1000 synthetic performance profile")
    print("Warning: NOP ROM + synthetic Graphics I scene; not a game benchmark.")
    print(f"repeats={args.repeats}")
    print(f"instructions_per_frame={instruction_count}")
    print(f"tstates_per_frame={total_tstates}")
    print(f"cpu_only_median_ms={cpu_median * 1000:.3f}")
    print(f"cpu_only_instructions_per_second={instruction_count / cpu_median:.0f}")
    print(f"vdp_only_median_ms={vdp_median * 1000:.3f}")
    print(
        f"vdp_only_speed_percent={total_tstates / vdp_median / Z80_CLOCK_HZ * 100:.2f}"
    )
    print(f"integrated_median_ms={integrated_median * 1000:.3f}")
    print(
        "integrated_speed_percent="
        f"{total_tstates / integrated_median / Z80_CLOCK_HZ * 100:.2f}"
    )
    print(f"integrated_frames_per_second={1 / integrated_median:.2f}")
    print(f"component_time_ratio_vdp_to_cpu={vdp_median / cpu_median:.3f}")

    if args.profile:
        profiler = cProfile.Profile()
        profiler.enable()
        run_integrated_frame()
        profiler.disable()
        profiler.dump_stats(args.profile)
        output = io.StringIO()
        pstats.Stats(profiler, stream=output).strip_dirs().sort_stats(
            "cumtime"
        ).print_stats(30)
        print("\nTop 30 functions by cumulative time:")
        print(output.getvalue())


if __name__ == "__main__":
    main()
