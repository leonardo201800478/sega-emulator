"""Compare SG-1000 Python and Rust backends on the same deterministic ROM.

Run from the repository root after building the optional Rust extension:
    python tools/benchmarks/benchmark_sg1000_backends.py --frames 120
"""

from __future__ import annotations

import argparse
import time

from sega_emulator.machine.sg1000 import SG1000Machine


# JP 0000: deterministic two-byte loop; no copyrighted ROM is required.
SYNTHETIC_ROM = bytes((0xC3, 0x00, 0x00))


def benchmark(backend: str, frames: int) -> dict[str, float | int | str]:
    machine = SG1000Machine(SYNTHETIC_ROM, cpu_core=backend)
    # Warm up Python import/caches and execute a few frames before timing.
    machine.run_frames(3)
    start_instructions = machine.total_instructions
    start_tstates = machine.total_tstates
    started = time.perf_counter()
    machine.run_frames(frames)
    elapsed = time.perf_counter() - started
    instructions = machine.total_instructions - start_instructions
    tstates = machine.total_tstates - start_tstates
    return {
        "backend": backend,
        "frames": frames,
        "elapsed_ms": round(elapsed * 1000, 3),
        "fps": round(frames / elapsed, 3) if elapsed else 0,
        "instructions_per_second": round(instructions / elapsed) if elapsed else 0,
        "instructions": instructions,
        "tstates": tstates,
    }


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--frames", type=int, default=120)
    args = parser.parse_args()
    if args.frames < 1:
        parser.error("--frames precisa ser maior que zero")

    results = [benchmark("python", args.frames)]
    try:
        results.append(benchmark("rust", args.frames))
    except ImportError as exc:
        raise SystemExit(
            "Backend Rust indisponível. Compile a extensão com maturin develop --release "
            "--manifest-path native/z80_core/Cargo.toml."
        ) from exc

    for result in results:
        print(" ".join(f"{key}={value}" for key, value in result.items()))

    python_result, rust_result = results
    python_elapsed = float(python_result["elapsed_ms"])
    rust_elapsed = float(rust_result["elapsed_ms"])
    if rust_elapsed > 0:
        print(f"rust_speedup={python_elapsed / rust_elapsed:.3f}x")


if __name__ == "__main__":
    main()
