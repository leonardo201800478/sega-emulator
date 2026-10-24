"""Reproducible SG-1000 runtime benchmark for local ROMs and CI smoke runs.

The default workload is a tiny synthetic Z80 loop; it is a harness check, not
a representative game-performance result. Supply --rom to benchmark a legally
obtained local ROM. Timings are informational and should only be compared on
the same machine, Python build, power mode, ROM and arguments.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import platform
import statistics
import sys
import time
import zlib
from pathlib import Path
from typing import Any

from sega_emulator.machine.sg1000 import SG1000Machine

# JP 0000: deterministic, valid Z80 loop. No commercial ROM is required in CI.
SYNTHETIC_ROM = bytes((0xC3, 0x00, 0x00))


def run_step_path(machine: SG1000Machine, frames: int) -> int:
    """Run through the public per-instruction machine.step() path."""
    target = machine.vdp.frame_number + frames
    steps = 0
    while machine.vdp.frame_number < target:
        machine.step()
        steps += 1
    return steps


def percentile95(values: list[float]) -> float:
    ordered = sorted(values)
    return ordered[min(len(ordered) - 1, int((len(ordered) - 1) * 0.95))]


def run_trial(rom: bytes, path: str, warmup_frames: int, frames: int) -> dict[str, Any]:
    machine = SG1000Machine(rom)
    machine.run_frames(warmup_frames)

    started = time.perf_counter_ns()
    if path == "run_frame":
        steps = machine.run_frames(frames)
    else:
        steps = run_step_path(machine, frames)
    elapsed_ns = time.perf_counter_ns() - started

    audio = machine.drain_audio_samples()
    audio_bytes = b"".join(
        int(round(sample * 1_000_000)).to_bytes(4, "little", signed=True)
        for sample in audio
    )
    return {
        "elapsed_seconds": elapsed_ns / 1_000_000_000,
        "fps": frames / (elapsed_ns / 1_000_000_000),
        "instructions": steps,
        "total_instructions": machine.total_instructions,
        "total_tstates": machine.total_tstates,
        "final_frame": machine.vdp.frame_number,
        "framebuffer_sha256": hashlib.sha256(machine.vdp.framebuffer).hexdigest(),
        "audio_sha256": hashlib.sha256(audio_bytes).hexdigest(),
        "audio_samples_drained": len(audio),
    }


def summarize(results: list[dict[str, Any]]) -> dict[str, Any]:
    elapsed = [item["elapsed_seconds"] for item in results]
    fps = [item["fps"] for item in results]
    return {
        "trials": len(results),
        "elapsed_seconds_median": statistics.median(elapsed),
        "elapsed_seconds_p95": percentile95(elapsed),
        "fps_median": statistics.median(fps),
        "fps_p05": percentile95([-value for value in fps]) * -1,
        "instructions_per_trial": results[0]["instructions"],
        "total_tstates": results[0]["total_tstates"],
        "framebuffer_sha256": results[0]["framebuffer_sha256"],
        "audio_sha256": results[0]["audio_sha256"],
        "deterministic": all(
            item["framebuffer_sha256"] == results[0]["framebuffer_sha256"]
            and item["audio_sha256"] == results[0]["audio_sha256"]
            and item["total_tstates"] == results[0]["total_tstates"]
            for item in results
        ),
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--rom", type=Path, help="caminho para ROM SG-1000 local/legal")
    parser.add_argument("--frames", type=int, default=120)
    parser.add_argument("--warmup-frames", type=int, default=30)
    parser.add_argument("--trials", type=int, default=5)
    parser.add_argument("--output", type=Path, default=Path("runtime-benchmark.json"))
    args = parser.parse_args()
    if args.frames < 1 or args.warmup_frames < 0 or args.trials < 2:
        parser.error("--frames deve ser positivo, warmup >= 0 e trials >= 2")

    try:
        rom = args.rom.read_bytes() if args.rom else SYNTHETIC_ROM
    except OSError as exc:
        parser.error(f"não foi possível ler a ROM: {exc}")
    if not rom:
        parser.error("a ROM está vazia")

    paths: dict[str, list[dict[str, Any]]] = {"run_frame": [], "step": []}
    # Alternate order to reduce systematic warm-cache/order bias.
    order = ("run_frame", "step")
    for trial in range(args.trials):
        for path in order if trial % 2 == 0 else reversed(order):
            paths[path].append(
                run_trial(rom, path, args.warmup_frames, args.frames)
            )

    result = {
        "schema_version": 1,
        "environment": {
            "python": sys.version,
            "implementation": platform.python_implementation(),
            "platform": platform.platform(),
            "machine": platform.machine(),
        },
        "workload": {
            "kind": "local_rom" if args.rom else "synthetic_jp_loop",
            "rom_name": args.rom.name if args.rom else "synthetic-jp-0000.rom",
            "size_bytes": len(rom),
            "crc32": f"{zlib.crc32(rom):08X}",
            "sha256": hashlib.sha256(rom).hexdigest(),
            "warmup_frames": args.warmup_frames,
            "measured_frames_per_trial": args.frames,
        },
        "results": {path: summarize(samples) for path, samples in paths.items()},
        "comparison": {
            "speedup_run_frame_vs_step": (
                statistics.median(item["fps"] for item in paths["run_frame"])
                / statistics.median(item["fps"] for item in paths["step"])
            ),
            "cross_path_framebuffer_match": (
                paths["run_frame"][0]["framebuffer_sha256"]
                == paths["step"][0]["framebuffer_sha256"]
            ),
            "cross_path_audio_match": (
                paths["run_frame"][0]["audio_sha256"]
                == paths["step"][0]["audio_sha256"]
            ),
            "cross_path_tstates_match": (
                paths["run_frame"][0]["total_tstates"]
                == paths["step"][0]["total_tstates"]
            ),
        },
        "notes": [
            "CI synthetic workload validates the harness, not game compatibility.",
            "Compare timings only on the same machine and configuration.",
            "The two execution paths must match observable hashes and T-state totals.",
        ],
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(
        json.dumps(result, indent=2, ensure_ascii=False) + "\n", encoding="utf-8"
    )
    print(f"Workload: {result['workload']['kind']} / SHA-256 {result['workload']['sha256']}")
    for path, summary in result["results"].items():
        print(
            f"{path}: median={summary['fps_median']:.2f} FPS, "
            f"p95 elapsed={summary['elapsed_seconds_p95']:.4f}s, "
            f"deterministic={summary['deterministic']}"
        )
    print(f"run_frame/step speed ratio: {result['comparison']['speedup_run_frame_vs_step']:.3f}x")
    print(
        "cross-path equivalence: "
        f"framebuffer={result['comparison']['cross_path_framebuffer_match']}, "
        f"audio={result['comparison']['cross_path_audio_match']}, "
        f"T-states={result['comparison']['cross_path_tstates_match']}"
    )
    print(f"JSON: {args.output.resolve()}")
    if not all(summary["deterministic"] for summary in result["results"].values()):
        return 2
    if not all(result["comparison"][key] for key in (
        "cross_path_framebuffer_match", "cross_path_audio_match", "cross_path_tstates_match"
    )):
        return 3
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
