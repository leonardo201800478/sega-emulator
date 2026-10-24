"""Long-running, deterministic Z80 stress audit for GitHub Actions.

This is a robustness/invariant audit, not an independent hardware oracle.
Unsupported opcodes and runtime exceptions are recorded as findings instead of
being silently treated as correct behavior. Run with --hours 5.5 to fit within
the extended workflow's six-hour job limit.
"""
from __future__ import annotations

import argparse
import json
import random
import time
import traceback
from collections import Counter
from pathlib import Path
from typing import Any

from sega_emulator.bus.memory import RAM_SIZE, SG1000Ram
from sega_emulator.bus.memory_bus import SG1000MemoryBus
from sega_emulator.cpu.z80 import Z80Cpu


def make_cpu(program: bytes) -> Z80Cpu:
    return Z80Cpu(SG1000MemoryBus(SG1000Ram(bytes(RAM_SIZE)), program))


def initial_state(cpu: Z80Cpu) -> dict[str, Any]:
    names = (
        "a", "f", "b", "c", "d", "e", "h", "l", "ix", "iy", "i", "r",
        "wz", "q", "af_alt", "bc_alt", "de_alt", "hl_alt", "sp", "pc",
        "iff1", "iff2", "interrupt_mode", "halted", "total_cycles",
    )
    return {name: getattr(cpu, name) for name in names if hasattr(cpu, name)}


def invariant_errors(cpu: Z80Cpu) -> list[str]:
    errors: list[str] = []
    byte_registers = ("a", "f", "b", "c", "d", "e", "h", "l", "i", "r", "q")
    word_registers = ("ix", "iy", "wz", "sp", "pc", "af_alt", "bc_alt", "de_alt", "hl_alt")
    for name in byte_registers:
        if hasattr(cpu, name):
            value = getattr(cpu, name)
            if type(value) is not int or not 0 <= value <= 0xFF:
                errors.append(f"{name} fora de 8 bits: {value!r}")
    for name in word_registers:
        if hasattr(cpu, name):
            value = getattr(cpu, name)
            if type(value) is not int or not 0 <= value <= 0xFFFF:
                errors.append(f"{name} fora de 16 bits: {value!r}")
    if type(cpu.total_cycles) is not int or cpu.total_cycles < 0:
        errors.append(f"total_cycles inválido: {cpu.total_cycles!r}")
    return errors


def program_for(vector: int, rng: random.Random) -> tuple[str, bytes]:
    # All base opcodes, then all second bytes behind CB/ED/DD/FD prefixes.
    if vector < 256:
        return f"base_{vector:02X}", bytes([vector, 0, 0, 0, 0, 0])
    prefix_index, opcode = divmod(vector - 256, 256)
    prefixes = (0xCB, 0xED, 0xDD, 0xFD)
    if prefix_index < len(prefixes):
        return f"prefix_{prefixes[prefix_index]:02X}_{opcode:02X}", bytes(
            [prefixes[prefix_index], opcode, 0, 0, 0, 0]
        )
    # Deterministic pseudo-random instruction streams with fixed seed.
    return f"random_{vector:08X}", bytes(rng.randrange(256) for _ in range(16))


def execute_case(
    case_id: str, program: bytes, case_seed: int
) -> tuple[str, dict[str, Any]]:
    cpu = make_cpu(program)
    state_rng = random.Random(case_seed)
    cpu.af = state_rng.randrange(0x10000)
    cpu.bc = state_rng.randrange(0x10000)
    cpu.de = state_rng.randrange(0x10000)
    cpu.hl = state_rng.randrange(0x10000)
    cpu.ix = state_rng.randrange(0x10000)
    cpu.iy = state_rng.randrange(0x10000)
    cpu.sp = state_rng.randrange(0x10000)
    cpu.i = state_rng.randrange(0x100)
    cpu.r = state_rng.randrange(0x100)
    cpu.wz = state_rng.randrange(0x10000)
    cpu.q = state_rng.randrange(0x100)
    cpu.iff1 = bool(state_rng.randrange(2))
    cpu.iff2 = bool(state_rng.randrange(2))
    before = initial_state(cpu)
    try:
        cycles = cpu.step()
        if type(cycles) is not int or cycles <= 0:
            return "invalid_cycle_count", {
                "case_id": case_id, "program": program.hex(), "cycles": repr(cycles)
            }
        errors = invariant_errors(cpu)
        if errors:
            return "invariant_violation", {
                "case_id": case_id, "program": program.hex(), "errors": errors,
                "before": before, "after": initial_state(cpu), "cycles": cycles,
            }
        return "executed", {
            "case_id": case_id, "program": program.hex(), "before": before,
            "after": initial_state(cpu), "cycles": cycles,
        }
    except NotImplementedError as exc:
        return "unsupported_opcode", {
            "case_id": case_id, "program": program.hex(), "error": str(exc),
            "before": before,
        }
    except Exception as exc:  # the point is to capture, not hide, robustness failures
        return "runtime_exception", {
            "case_id": case_id, "program": program.hex(),
            "error_type": type(exc).__name__, "error": str(exc),
            "traceback": traceback.format_exc(limit=4), "before": before,
        }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--hours", type=float, default=5.5)
    parser.add_argument("--seed", type=int, default=0x5E6A1000)
    parser.add_argument("--report", type=Path, default=Path("artifacts/z80-long-stress-report.json"))
    parser.add_argument("--checkpoint-seconds", type=float, default=60.0)
    args = parser.parse_args()
    if not 0 < args.hours <= 5.75:
        parser.error("--hours deve ser > 0 e <= 5.75 para caber no limite do workflow")

    rng = random.Random(args.seed)
    deadline = time.monotonic() + args.hours * 3600
    counts: Counter[str] = Counter()
    findings: list[dict[str, Any]] = []
    executed_vectors: set[str] = set()
    vector = 0
    attempts = 0
    started_wall = time.time()
    last_checkpoint = time.monotonic()
    total = 256 + 4 * 256

    def checkpoint(final: bool = False) -> None:
        report = {
            "status": "completed" if final else "running",
            "seed": args.seed,
            "requested_hours": args.hours,
            "started_epoch": started_wall,
            "elapsed_seconds": round(time.time() - started_wall, 3),
            "cases": attempts,
            "counts": dict(counts),
            "distinct_vectors": len(executed_vectors),
            "findings_retained": len(findings),
            "findings": findings,
        }
        args.report.parent.mkdir(parents=True, exist_ok=True)
        args.report.write_text(json.dumps(report, indent=2, sort_keys=True), encoding="utf-8")
        print(
            f"[{'FINAL' if final else 'CHECKPOINT'}] cases={report['cases']:,} "
            f"elapsed={report['elapsed_seconds']:.0f}s counts={dict(counts)}",
            flush=True,
        )

    while time.monotonic() < deadline:
        if vector < total:
            case_id, program = program_for(vector, rng)
        else:
            case_id = f"random_{vector:08X}"
            program = bytes(rng.randrange(256) for _ in range(16))
        case_seed = args.seed ^ vector
        status, result = execute_case(case_id, program, case_seed)
        attempts += 1
        counts[status] += 1
        executed_vectors.add(case_id if vector < total else "random_stream")
        if status != "executed" and len(findings) < 2000:
            findings.append({"status": status, **result})

        # Repeat each deterministic vector from the same initial conditions.
        if status == "executed":
            repeat_status, repeat = execute_case(case_id, program, case_seed)
            counts["determinism_checks"] += 1
            if repeat_status != "executed" or result["after"] != repeat["after"] or result["cycles"] != repeat["cycles"]:
                counts["determinism_divergence"] += 1
                if len(findings) < 2000:
                    findings.append({
                        "status": "determinism_divergence", "case_id": case_id,
                        "program": program.hex(), "first": result, "repeat": repeat,
                    })

        vector += 1
        now = time.monotonic()
        if now - last_checkpoint >= args.checkpoint_seconds:
            checkpoint()
            last_checkpoint = now

    checkpoint(final=True)
    # The audit is considered failed only for robustness/invariant/determinism
    # regressions; unsupported instructions are reported separately as coverage gaps.
    fatal = counts["invariant_violation"] + counts["runtime_exception"] + counts["invalid_cycle_count"] + counts["determinism_divergence"]
    print(f"Robustness findings: {fatal}; unsupported opcodes recorded: {counts['unsupported_opcode']}", flush=True)
    return 1 if fatal else 0


if __name__ == "__main__":
    raise SystemExit(main())
