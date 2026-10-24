"""Benchmark real SG-1000 ROM execution without adding the ROM to the repository.

The ROM is supplied by the caller (the workflow downloads Super Uwol from its
homebrew publisher/archive page). Results are relative to the GitHub runner and
must not be treated as a Windows/Ryzen benchmark.
"""

from __future__ import annotations

import argparse
import cProfile
import hashlib
import pstats
import statistics
import time
from pathlib import Path

from sega_emulator.machine.sg1000 import SG1000Machine

Z80_CLOCK_HZ = 3_579_545
TSTATES_PER_FRAME = 228 * 262
NOMINAL_FPS = Z80_CLOCK_HZ / TSTATES_PER_FRAME


def run_frames(
    machine: SG1000Machine, frame_count: int, max_steps_per_frame: int
) -> tuple[int, int]:
    """Run a fixed number of complete VDP frames; return steps and T-states."""
    start_frame = machine.vdp.frame_number
    steps_total = 0
    tstates_total = 0
    for _ in range(frame_count):
        target_frame = machine.vdp.frame_number + 1
        steps_this_frame = 0
        while machine.vdp.frame_number < target_frame:
            cycles = machine.step()
            steps_total += 1
            steps_this_frame += 1
            tstates_total += cycles
            if steps_this_frame >= max_steps_per_frame:
                raise RuntimeError(
                    f"frame {machine.vdp.frame_number - start_frame}: "
                    f"exceeded {max_steps_per_frame} CPU steps without a frame"
                )
    return steps_total, tstates_total


def new_machine(rom: bytes) -> SG1000Machine:
    machine = SG1000Machine(rom)
    # Keep both joysticks released; this benchmarks deterministic boot/title
    # behavior and avoids inventing undocumented input-bit semantics.
    machine.joystick1 = 0xFF
    machine.joystick2 = 0xFF
    return machine


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--rom", type=Path, required=True, help="SG-1000 ROM file")
    parser.add_argument("--frames", type=int, default=120, help="measured frames")
    parser.add_argument("--warmup-frames", type=int, default=30)
    parser.add_argument("--repeats", type=int, default=3)
    parser.add_argument("--max-steps-per-frame", type=int, default=200_000)
    parser.add_argument(
        "--profile", type=Path, default=None, help="write cProfile .pstats"
    )
    args = parser.parse_args()
    if args.frames < 1 or args.warmup_frames < 0 or args.repeats < 1:
        parser.error("frames/repeats must be positive and warmup-frames nonnegative")
    if args.max_steps_per_frame < 1:
        parser.error("max-steps-per-frame must be positive")

    try:
        rom = args.rom.read_bytes()
    except OSError as exc:
        print(f"ERROR: cannot read ROM: {exc}")
        return 2
    if not rom:
        print("ERROR: ROM file is empty")
        return 2

    print("SG-1000 real-ROM performance benchmark")
    print(f"rom={args.rom.name}")
    print(f"rom_size_bytes={len(rom)}")
    print(f"rom_sha256={hashlib.sha256(rom).hexdigest()}")
    print(f"warmup_frames={args.warmup_frames}")
    print(f"measured_frames={args.frames}")
    print(f"repeats={args.repeats}")
    print(f"nominal_hardware_fps={NOMINAL_FPS:.3f}")
    print("input=both joysticks released (0xFF)")
    samples: list[float] = []
    instructions_per_frame: list[float] = []
    cycles_per_frame: list[float] = []

    try:
        for repeat in range(args.repeats):
            machine = new_machine(rom)
            run_frames(machine, args.warmup_frames, args.max_steps_per_frame)
            started = time.perf_counter()
            steps, tstates = run_frames(
                machine, args.frames, args.max_steps_per_frame
            )
            elapsed = time.perf_counter() - started
            fps = args.frames / elapsed
            samples.append(elapsed)
            instructions_per_frame.append(steps / args.frames)
            cycles_per_frame.append(tstates / args.frames)
            print(
                f"repeat_{repeat + 1}_seconds={elapsed:.4f} "
                f"fps={fps:.3f} "
                f"speed_percent={fps / NOMINAL_FPS * 100:.2f} "
                f"instructions_per_frame={steps / args.frames:.1f} "
                f"tstates_per_frame={tstates / args.frames:.1f}"
            )
    except Exception as exc:
        print(f"BENCHMARK_STOPPED: {type(exc).__name__}: {exc}")
        print("The ROM reached an unsupported instruction or another runtime error.")
        return 2

    median_seconds = statistics.median(samples)
    median_fps = args.frames / median_seconds
    print(f"median_seconds_for_{args.frames}_frames={median_seconds:.4f}")
    print(f"median_fps={median_fps:.3f}")
    print(f"median_speed_percent={median_fps / NOMINAL_FPS * 100:.2f}")
    print(f"median_instructions_per_frame={statistics.median(instructions_per_frame):.1f}")
    print(f"median_tstates_per_frame={statistics.median(cycles_per_frame):.1f}")

    if args.profile:
        machine = new_machine(rom)
        run_frames(machine, args.warmup_frames, args.max_steps_per_frame)
        profiler = cProfile.Profile()
        profiler.runcall(
            run_frames, machine, args.frames, args.max_steps_per_frame
        )
        profiler.dump_stats(str(args.profile))
        stats = pstats.Stats(profiler).strip_dirs().sort_stats("cumtime")
        print(f"profile_file={args.profile}")
        print("Top 30 functions by cumulative time:")
        stats.print_stats(30)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
