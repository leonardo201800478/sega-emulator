"""Compare Python and Rust Z80 execution paths with a deterministic workload.

Run after building/installing the optional extension:
    python tools/benchmarks/benchmark_z80_backends.py
The script measures CPU.step() separately from SG1000Machine.step(), which also
advances the VDP and audio synchronizer. It reports measurements; it does not
assume that Rust must be faster.
"""

from __future__ import annotations

import argparse
import json
import platform
import statistics
import sys
import time
from collections.abc import Callable

from sega_emulator.cpu.z80 import Z80Cpu
from sega_emulator.machine.sg1000 import SG1000Machine


ROM = b"\xC3\x00\x00"  # JP 0000h: deterministic three-byte loop.


def make_machine(core: str) -> SG1000Machine:
    machine = SG1000Machine(ROM, cpu_core=core)
    if core == "python" and not isinstance(machine.cpu, Z80Cpu):
        raise RuntimeError("Python backend selection did not instantiate Z80Cpu")
    if core == "rust":
        try:
            from sega_z80_core import RustZ80Cpu
        except ImportError as exc:
            raise RuntimeError(
                "Rust extension is unavailable. Build it with "
                "'maturin develop --manifest-path native/z80_core/Cargo.toml "
                "--features python-extension'."
            ) from exc
        if not isinstance(machine.cpu, RustZ80Cpu):
            raise RuntimeError("Rust backend selection did not instantiate RustZ80Cpu")
    return machine


def measure(
    core: str,
    step: Callable[[SG1000Machine], int],
    instructions: int,
    warmups: int,
    repeats: int,
) -> dict[str, object]:
    def run_once() -> float:
        machine = make_machine(core)
        started = time.perf_counter()
        for _ in range(instructions):
            step(machine)
        elapsed = time.perf_counter() - started
        if machine.cpu.pc != 0:
            raise RuntimeError(
                f"{core} execution did not finish on expected loop boundary: "
                f"PC={machine.cpu.pc:#06x}"
            )
        return elapsed

    for _ in range(warmups):
        run_once()
    samples = [run_once() for _ in range(repeats)]
    median_seconds = statistics.median(samples)
    probe = make_machine(core)
    return {
        "backend": core,
        "cpu_type": f"{type(probe.cpu).__module__}.{type(probe.cpu).__qualname__}",
        "instructions": instructions,
        "samples_seconds": [round(sample, 6) for sample in samples],
        "median_seconds": round(median_seconds, 6),
        "instructions_per_second": round(instructions / median_seconds, 2),
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--instructions", type=int, default=100_000)
    parser.add_argument("--warmups", type=int, default=1)
    parser.add_argument("--repeats", type=int, default=5)
    args = parser.parse_args()
    if args.instructions < 1 or args.warmups < 0 or args.repeats < 1:
        parser.error("instructions/repeats must be positive and warmups non-negative")

    results: dict[str, object] = {
        "python_version": sys.version.split()[0],
        "platform": platform.platform(),
        "workload": "JP 0000h loop",
        "note": (
            "CPU-only timing excludes machine/VDP/audio stepping; full-machine timing "
            "includes SG1000Machine.step(). Compare within each workload."
        ),
    }
    workloads = {
        "cpu_only": lambda machine: machine.cpu.step(),
        "full_machine": lambda machine: machine.step(),
    }
    for workload_name, step in workloads.items():
        rows = [
            measure(core, step, args.instructions, args.warmups, args.repeats)
            for core in ("python", "rust")
        ]
        python_rate = float(rows[0]["instructions_per_second"])
        rust_rate = float(rows[1]["instructions_per_second"])
        results[workload_name] = {
            "results": rows,
            "rust_speedup_vs_python": round(rust_rate / python_rate, 3)
            if python_rate
            else None,
        }
    print(json.dumps(results, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
