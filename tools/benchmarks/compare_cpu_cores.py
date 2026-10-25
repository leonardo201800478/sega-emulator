from __future__ import annotations

import argparse
import hashlib
import json
import statistics
import struct
import time
from pathlib import Path
from typing import Any

from sega_emulator.machine.sg1000 import SG1000Machine
from sega_emulator.rom_loader import load_rom


def _audio_hash(samples: list[float]) -> str:
    digest = hashlib.sha256()
    for sample in samples:
        digest.update(struct.pack("<f", sample))
    return digest.hexdigest()


def _run_once(rom: bytes, core: str, warmup_frames: int, frames: int) -> dict[str, Any]:
    machine = SG1000Machine(rom, cpu_core=core)
    machine.run_frames(warmup_frames)
    machine.drain_audio_samples()

    instructions_before = machine.total_instructions
    tstates_before = machine.total_tstates
    started = time.perf_counter()
    machine.run_frames(frames)
    elapsed = time.perf_counter() - started
    audio = machine.drain_audio_samples()

    return {
        "core": core,
        "frames": frames,
        "elapsed_seconds": elapsed,
        "fps": frames / elapsed if elapsed else None,
        "instructions": machine.total_instructions - instructions_before,
        "instructions_per_second": (
            (machine.total_instructions - instructions_before) / elapsed
            if elapsed
            else None
        ),
        "tstates": machine.total_tstates - tstates_before,
        "final_pc": machine.cpu.pc,
        "final_registers": {
            name: getattr(machine.cpu, name)
            for name in (
                "a",
                "f",
                "b",
                "c",
                "d",
                "e",
                "h",
                "l",
                "sp",
                "pc",
                "ix",
                "iy",
                "i",
                "r",
                "wz",
                "iff1",
                "iff2",
                "interrupt_mode",
                "halted",
            )
        },
        "framebuffer_sha256": hashlib.sha256(machine.vdp.framebuffer).hexdigest(),
        "audio_sha256": _audio_hash(audio),
        "audio_samples": len(audio),
    }


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Compare Python and experimental Rust Z80 cores on the same ROM."
    )
    source = parser.add_mutually_exclusive_group(required=True)
    source.add_argument("--rom", type=Path, help="ROM .sg/.rom/.bin or ZIP")
    source.add_argument(
        "--synthetic-loop",
        action="store_true",
        help="use a deterministic synthetic ROM containing JP 0000h",
    )
    parser.add_argument("--warmup-frames", type=int, default=30)
    parser.add_argument("--frames", type=int, default=120)
    parser.add_argument("--repeats", type=int, default=3)
    parser.add_argument(
        "--output", type=Path, default=None, help="optional JSON report path"
    )
    args = parser.parse_args()

    if args.warmup_frames < 0 or args.frames < 1 or args.repeats < 1:
        parser.error("warmup-frames deve ser >= 0; frames e repeats devem ser >= 1")

    if args.synthetic_loop:
        rom = bytes((0xC3, 0x00, 0x00))
        rom_name = "synthetic-jp-0000-loop"
        rom_sha256 = hashlib.sha256(rom).hexdigest()
    else:
        loaded = load_rom(args.rom)
        rom = loaded.data
        rom_name = loaded.display_name
        rom_sha256 = loaded.sha256
    report: dict[str, Any] = {
        "rom_name": rom_name,
        "rom_sha256": rom_sha256,
        "warmup_frames": args.warmup_frames,
        "frames_per_repeat": args.frames,
        "repeats": args.repeats,
        "cores": {},
    }

    for core in ("python", "rust"):
        runs: list[dict[str, Any]] = []
        try:
            for _ in range(args.repeats):
                runs.append(
                    _run_once(rom, core, args.warmup_frames, args.frames)
                )
        except Exception as exc:
            report["cores"][core] = {
                "status": "failed",
                "error": f"{type(exc).__name__}: {exc}",
            }
            if core == "python":
                print(json.dumps(report, indent=2, ensure_ascii=False))
                return 2
            break

        median_seconds = statistics.median(run["elapsed_seconds"] for run in runs)
        report["cores"][core] = {
            "status": "ok",
            "median_elapsed_seconds": median_seconds,
            "median_fps": args.frames / median_seconds,
            "median_instructions_per_second": statistics.median(
                run["instructions_per_second"] for run in runs
            ),
            "runs": runs,
        }

    python_result = report["cores"].get("python", {})
    rust_result = report["cores"].get("rust", {})
    if python_result.get("status") == "ok" and rust_result.get("status") == "ok":
        python_signature = python_result["runs"][-1]
        rust_signature = rust_result["runs"][-1]
        equivalent = all(
            python_signature[key] == rust_signature[key]
            for key in (
                "instructions",
                "tstates",
                "final_pc",
                "final_registers",
                "framebuffer_sha256",
                "audio_sha256",
                "audio_samples",
            )
        )
        report["equivalence"] = {
            "same_execution_signature": equivalent,
            "compared_fields": [
                "instructions",
                "tstates",
                "final_pc",
                "final_registers",
                "framebuffer_sha256",
                "audio_sha256",
                "audio_samples",
            ],
        }
        if equivalent:
            report["rust_speedup_vs_python"] = (
                python_result["median_elapsed_seconds"]
                / rust_result["median_elapsed_seconds"]
            )
    else:
        report["equivalence"] = {
            "same_execution_signature": False,
            "note": (
                "Não foi possível comparar os dois núcleos; "
                "consulte o erro do núcleo Rust."
            ),
        }

    rendered = json.dumps(report, indent=2, ensure_ascii=False)
    print(rendered)
    if args.output is not None:
        args.output.write_text(rendered + "\n", encoding="utf-8")
    if rust_result.get("status") != "ok":
        return 2
    return 0 if report.get("equivalence", {}).get("same_execution_signature") else 1


if __name__ == "__main__":
    raise SystemExit(main())
