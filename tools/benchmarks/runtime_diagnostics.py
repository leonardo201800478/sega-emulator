"""Diagnóstico por fase da execução real SG-1000; gera JSON para análise.

Os tempos da passagem instrumentada incluem overhead de medição e não devem ser
comparados diretamente ao FPS da passagem-base. Tempos de fases internas são
inclusivos: não some CPU/VDP com scanline/pixel, pois há chamadas aninhadas.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import statistics
import time
from collections import Counter
from pathlib import Path
from typing import Any, Callable

from sega_emulator.machine.sg1000 import SG1000Machine

MODE_NAMES = {
    0: "graphics_i",
    1: "text",
    2: "multicolor",
    3: "invalid_3",
    4: "graphics_ii",
}


def run_frames(machine: SG1000Machine, count: int, max_steps: int) -> int:
    """Executa quadros completos com limite de segurança e retorna passos."""
    steps_total = 0
    for _ in range(count):
        target = machine.vdp.frame_number + 1
        steps = 0
        while machine.vdp.frame_number < target:
            machine.step()
            steps += 1
            steps_total += 1
            if steps >= max_steps and machine.vdp.frame_number < target:
                raise RuntimeError(
                    f"Limite de {max_steps} instruções atingido no quadro "
                    f"{machine.vdp.frame_number}"
                )
    return steps_total


def timed_call(
    original: Callable[..., Any], key: str, metrics: dict[str, Any]
) -> Callable[..., Any]:
    """Cronometra uma chamada sem alterar argumentos, retorno ou exceções."""
    def wrapper(*args: Any, **kwargs: Any) -> Any:
        started = time.perf_counter_ns()
        metrics["calls"][key] += 1
        try:
            return original(*args, **kwargs)
        finally:
            metrics["time_ns"][key] += time.perf_counter_ns() - started

    return wrapper


def instrument(machine: SG1000Machine, metrics: dict[str, Any]) -> None:
    """Instrumenta CPU, VDP, rasterização e pixel apenas nesta instância."""
    cpu = machine.cpu
    vdp = machine.vdp
    cpu.step = timed_call(cpu.step, "cpu.step", metrics)
    vdp.advance = timed_call(vdp.advance, "vdp.advance", metrics)
    vdp._put_pixel = timed_call(vdp._put_pixel, "vdp._put_pixel", metrics)

    original_scanline = vdp._render_scanline

    def scanline_wrapper(y: int, *, update_sprite_status: bool = False) -> None:
        mode = MODE_NAMES.get(vdp._display_mode, f"mode_{vdp._display_mode}")
        enabled = bool(vdp.registers[1] & 0x40)
        metrics["scanlines"][f"{mode}:{'enabled' if enabled else 'disabled'}"] += 1
        original_scanline(y, update_sprite_status=update_sprite_status)

    vdp._render_scanline = timed_call(scanline_wrapper, "vdp._render_scanline", metrics)

    vdp._render_sprite_line = timed_call(
        vdp._render_sprite_line, "vdp._render_sprite_line", metrics
    )


def new_metrics() -> dict[str, Any]:
    return {
        "calls": Counter(),
        "time_ns": Counter(),
        "scanlines": Counter(),
    }


def profile_frames(
    machine: SG1000Machine, count: int, max_steps: int, metrics: dict[str, Any]
) -> list[dict[str, Any]]:
    """Registra tempo e estado observável ao concluir cada quadro."""
    records: list[dict[str, Any]] = []
    for _ in range(count):
        target = machine.vdp.frame_number + 1
        start_ns = time.perf_counter_ns()
        start_instructions = machine.total_instructions
        start_tstates = machine.total_tstates
        steps = 0
        while machine.vdp.frame_number < target:
            machine.step()
            steps += 1
            if steps >= max_steps and machine.vdp.frame_number < target:
                raise RuntimeError(
                    f"Limite de {max_steps} instruções atingido no quadro "
                    f"{machine.vdp.frame_number}"
                )
        elapsed_ns = time.perf_counter_ns() - start_ns
        framebuffer = machine.vdp.framebuffer
        records.append({
            "frame": machine.vdp.frame_number,
            "elapsed_ms_instrumented": round(elapsed_ns / 1_000_000, 3),
            "instructions": machine.total_instructions - start_instructions,
            "tstates": machine.total_tstates - start_tstates,
            "pc": f"{machine.cpu.pc:04X}",
            "scanline": machine.vdp.scanline,
            "display_mode": MODE_NAMES.get(
                machine.vdp._display_mode, f"mode_{machine.vdp._display_mode}"
            ),
            "display_enabled": bool(machine.vdp.registers[1] & 0x40),
            "vdp_registers_hex": bytes(machine.vdp.registers).hex(),
            "vdp_status_hex": f"{machine.vdp.status:02X}",
            "framebuffer_sha256": hashlib.sha256(framebuffer).hexdigest(),
        })
    return records


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--rom", type=Path, required=True, help="ROM SG-1000 local")
    parser.add_argument("--frames", type=int, default=120)
    parser.add_argument("--warmup-frames", type=int, default=30)
    parser.add_argument("--max-steps-per-frame", type=int, default=200_000)
    parser.add_argument("--output", type=Path, default=Path("runtime-diagnostic.json"))
    args = parser.parse_args()
    if args.frames < 1 or args.warmup_frames < 0 or args.max_steps_per_frame < 1:
        parser.error("frames/max-steps devem ser positivos; warmup não pode ser negativo")

    try:
        rom = args.rom.read_bytes()
        if not rom:
            raise ValueError("arquivo ROM vazio")
        # Baseline separado, sem monkeypatch ou profiler.
        baseline = SG1000Machine(rom)
        run_frames(baseline, args.warmup_frames, args.max_steps_per_frame)
        baseline_started = time.perf_counter_ns()
        baseline_steps = run_frames(baseline, args.frames, args.max_steps_per_frame)
        baseline_elapsed_ns = time.perf_counter_ns() - baseline_started

        # Nova instância: instrumentação não contamina a medição-base.
        machine = SG1000Machine(rom)
        run_frames(machine, args.warmup_frames, args.max_steps_per_frame)
        metrics = new_metrics()
        instrument(machine, metrics)
        frames = profile_frames(machine, args.frames, args.max_steps_per_frame, metrics)
    except (OSError, ValueError, RuntimeError, NotImplementedError) as exc:
        print(f"DIAGNOSTIC_FAILED: {type(exc).__name__}: {exc}")
        return 2

    baseline_seconds = baseline_elapsed_ns / 1_000_000_000
    frame_ms = [item["elapsed_ms_instrumented"] for item in frames]
    result = {
        "schema_version": 1,
        "rom": {
            "path": str(args.rom),
            "size_bytes": len(rom),
            "sha256": hashlib.sha256(rom).hexdigest(),
        },
        "configuration": {
            "warmup_frames": args.warmup_frames,
            "measured_frames": args.frames,
            "max_steps_per_frame": args.max_steps_per_frame,
            "timing_note": (
                "Baseline FPS is uninstrumented. Per-phase timings and per-frame "
                "times are instrumented; nested phase times are inclusive and "
                "must not be added together."
            ),
        },
        "baseline": {
            "elapsed_seconds": round(baseline_seconds, 6),
            "fps": round(args.frames / baseline_seconds, 3),
            "speed_percent_of_59_923_fps": round(
                (args.frames / baseline_seconds) / 59.923 * 100, 2
            ),
            "instructions": baseline_steps,
            "instructions_per_frame": round(baseline_steps / args.frames, 2),
        },
        "instrumented": {
            "frame_time_ms_median": round(statistics.median(frame_ms), 3),
            "frame_time_ms_p95": round(sorted(frame_ms)[
                min(len(frame_ms) - 1, int(len(frame_ms) * 0.95))
            ], 3),
            "calls": dict(metrics["calls"]),
            "time_ms_inclusive": {
                key: round(value / 1_000_000, 3)
                for key, value in metrics["time_ns"].items()
            },
            "scanlines_by_mode_and_display": dict(metrics["scanlines"]),
            "frames": frames,
        },
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(
        json.dumps(result, indent=2, ensure_ascii=False) + "\n", encoding="utf-8"
    )
    print(f"ROM SHA-256: {result['rom']['sha256']}")
    print(f"Baseline sem instrumentação: {result['baseline']['fps']:.3f} FPS")
    print(
        "Quadro instrumentado mediano/P95: "
        f"{result['instrumented']['frame_time_ms_median']:.3f}/"
        f"{result['instrumented']['frame_time_ms_p95']:.3f} ms"
    )
    print("Tempos inclusivos por fase (não somar fases aninhadas):")
    for key, elapsed_ms in sorted(
        result["instrumented"]["time_ms_inclusive"].items(),
        key=lambda item: item[1],
        reverse=True,
    ):
        print(
            f"  {key}: {elapsed_ms:.3f} ms "
            f"({result['instrumented']['calls'].get(key, 0)} chamadas)"
        )
    print(f"Relatório JSON: {args.output.resolve()}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
