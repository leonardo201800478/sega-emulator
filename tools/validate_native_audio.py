"""Physical smoke test for native Rust WASAPI and XAudio2 outputs.

Run from the repository root after installing the Qt/native extension:
    python tools/validate_native_audio.py --backend both
    python tools/validate_native_audio.py --backend both --trace-csv timing.csv

The low-volume tone validates host output only; it does not execute or modify
emulated hardware state. Timing rows measure the Python PCM producer cadence and
write-call duration, not device presentation latency or emulated CPU T-states.
"""

from __future__ import annotations

import argparse
import csv
import math
import statistics
import struct
import sys
import time
from pathlib import Path

from sega_emulator.frontend.qt.wasapi_audio import NativeAudioOutput

# Original short chiptune motif. Frequencies are equal-tempered notes; None is
# a rest. This is a host-output listening test, not a Master System PSG test.
_MELODY_HZ: tuple[float | None, ...] = (
    523.25,
    659.25,
    783.99,
    659.25,
    587.33,
    698.46,
    880.00,
    698.46,
    523.25,
    659.25,
    783.99,
    1046.50,
    783.99,
    659.25,
    587.33,
    None,
)
_MELODY_BPM = 120.0


def _pcm_block(
    sample_rate: int,
    frequency: float,
    start_sample: int,
    count: int,
    signal: str,
) -> bytes:
    amplitude = 0.10 * 32767
    samples: list[int] = []
    samples_per_note = sample_rate * 60.0 / _MELODY_BPM
    for offset in range(count):
        absolute_sample = start_sample + offset
        if signal == "melody":
            note_index = int(absolute_sample / samples_per_note) % len(_MELODY_HZ)
            current_frequency = _MELODY_HZ[note_index]
            note_start = int(note_index * samples_per_note)
            cycle_sample = absolute_sample - note_start
        else:
            current_frequency = frequency
            cycle_sample = absolute_sample

        if current_frequency is None:
            samples.append(0)
            continue
        phase = (2.0 * math.pi * current_frequency * cycle_sample / sample_rate) % (
            2.0 * math.pi
        )
        samples.append(round(amplitude * math.sin(phase)))

    return struct.pack(f"<{count}h", *samples)


def _percentile(values: list[float], percentile: float) -> float:
    if not values:
        return 0.0
    ordered = sorted(values)
    index = min(len(ordered) - 1, math.ceil(percentile * len(ordered)) - 1)
    return ordered[max(0, index)]


def _write_trace(path: Path, rows: list[dict[str, int | float | str]]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    fields = (
        "backend",
        "cycle",
        "sample_index",
        "scheduled_start_s",
        "actual_start_s",
        "start_interval_ms",
        "schedule_lateness_ms",
        "pcm_generation_ms",
        "write_pcm_ms",
        "producer_work_ms",
    )
    with path.open("w", newline="", encoding="utf-8") as trace_file:
        writer = csv.DictWriter(trace_file, fieldnames=fields)
        writer.writeheader()
        writer.writerows(rows)


def _run_backend(
    backend: str,
    sample_rate: int,
    buffer_size: int,
    queue_blocks: int,
    seconds: int,
    frequency: float,
    signal: str,
    balance: int,
    trace_csv: Path | None,
) -> bool:
    block_period = buffer_size / sample_rate
    print(
        f"\nBackend={backend}; taxa solicitada={sample_rate} Hz; "
        f"bloco={buffer_size} amostras ({block_period * 1000:.3f} ms); "
        f"fila={queue_blocks}; duração={seconds}s; sinal={signal}; "
        f"balanço={balance}"
    )
    try:
        output = NativeAudioOutput(
            sample_rate,
            buffer_size,
            backend=backend,
            queue_blocks=queue_blocks,
            balance=balance,
        )
    except Exception as exc:
        print(f"FAIL: inicialização: {type(exc).__name__}: {exc}")
        return False

    started = time.monotonic()
    sample_index = 0
    cycle = 0
    previous_cycle_start: float | None = None
    failure: str | None = None
    underruns = 0
    dropped = 0
    rows: list[dict[str, int | float | str]] = []
    intervals_ms: list[float] = []
    lateness_ms: list[float] = []
    generation_ms: list[float] = []
    write_ms: list[float] = []
    work_ms: list[float] = []
    late_cycles = 0

    try:
        while True:
            scheduled_start = started + sample_index / sample_rate
            now = time.monotonic()
            if now < scheduled_start:
                time.sleep(scheduled_start - now)
            cycle_start = time.monotonic()
            if cycle_start - started >= seconds:
                break

            lateness = max(0.0, cycle_start - scheduled_start)
            if lateness > block_period:
                late_cycles += 1
            if previous_cycle_start is not None:
                intervals_ms.append((cycle_start - previous_cycle_start) * 1000.0)

            generation_started = time.monotonic()
            pcm = _pcm_block(sample_rate, frequency, sample_index, buffer_size, signal)
            generation_duration = time.monotonic() - generation_started

            write_started = time.monotonic()
            output.write_pcm(pcm)
            write_duration = time.monotonic() - write_started
            work_duration = generation_duration + write_duration

            generation_ms.append(generation_duration * 1000.0)
            write_ms.append(write_duration * 1000.0)
            work_ms.append(work_duration * 1000.0)
            lateness_ms.append(lateness * 1000.0)
            rows.append(
                {
                    "backend": backend,
                    "cycle": cycle,
                    "sample_index": sample_index,
                    "scheduled_start_s": scheduled_start - started,
                    "actual_start_s": cycle_start - started,
                    "start_interval_ms": (
                        (cycle_start - previous_cycle_start) * 1000.0
                        if previous_cycle_start is not None
                        else 0.0
                    ),
                    "schedule_lateness_ms": lateness * 1000.0,
                    "pcm_generation_ms": generation_duration * 1000.0,
                    "write_pcm_ms": write_duration * 1000.0,
                    "producer_work_ms": work_duration * 1000.0,
                }
            )
            previous_cycle_start = cycle_start
            sample_index += buffer_size
            cycle += 1

        underruns = output.underrun_callbacks
        dropped = output.dropped_blocks
    except Exception as exc:
        failure = f"FAIL: escrita PCM: {type(exc).__name__}: {exc}"
    try:
        output.close()
    except Exception as exc:
        close_failure = f"FAIL: encerramento: {type(exc).__name__}: {exc}"
        failure = f"{failure}\n{close_failure}" if failure else close_failure

    elapsed = time.monotonic() - started
    if trace_csv is not None and rows:
        try:
            trace_path = trace_csv.with_name(
                f"{trace_csv.stem}_{backend}{trace_csv.suffix}"
            )
            _write_trace(trace_path, rows)
            print(f"Trace de ciclos salvo em: {trace_path}")
        except OSError as exc:
            failure = (
                f"{failure}\nFAIL: gravação do trace CSV: {exc}"
                if failure
                else f"FAIL: gravação do trace CSV: {exc}"
            )

    if failure:
        print(failure)
        return False

    effective_rate = sample_index / elapsed if elapsed > 0 else 0.0
    print(
        f"Saída solicitada: {output.sample_rate} Hz / {output.buffer_size} amostras; "
        f"tempo decorrido={elapsed:.2f}s; ciclos={cycle}; "
        f"amostras produzidas={sample_index}; "
        f"taxa média do produtor={effective_rate:.1f} amostras/s"
    )
    print(
        f"Underruns={underruns}; blocos descartados={dropped}; "
        f"ciclos atrasados >1 bloco={late_cycles}/{cycle}"
    )
    print(
        "Intervalo entre ciclos (ms): "
        f"mediana={statistics.median(intervals_ms) if intervals_ms else 0.0:.3f}; "
        f"p95={_percentile(intervals_ms, 0.95):.3f}; "
        f"p99={_percentile(intervals_ms, 0.99):.3f}; "
        f"máximo={max(intervals_ms, default=0.0):.3f}"
    )
    print(
        "Atraso de início vs. agenda (ms): "
        f"mediana={statistics.median(lateness_ms) if lateness_ms else 0.0:.3f}; "
        f"p95={_percentile(lateness_ms, 0.95):.3f}; "
        f"p99={_percentile(lateness_ms, 0.99):.3f}; "
        f"máximo={max(lateness_ms, default=0.0):.3f}"
    )
    print(
        "Tempo por ciclo (ms): "
        f"geração PCM p95={_percentile(generation_ms, 0.95):.3f}; "
        f"write_pcm p95={_percentile(write_ms, 0.95):.3f}; "
        f"trabalho total p95={_percentile(work_ms, 0.95):.3f}; "
        f"máximo total={max(work_ms, default=0.0):.3f}; "
        f"período-alvo={block_period * 1000:.3f}"
    )
    print(
        "Confirme auditivamente a melodia/sinal sem cortes, estalos ou distorção; "
        "este teste não executa o PSG emulado."
    )
    return True


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--backend", choices=("wasapi", "xaudio2", "both"), default="both"
    )
    parser.add_argument("--seconds", type=int, default=8)
    parser.add_argument("--sample-rate", type=int, default=48_000)
    parser.add_argument("--buffer-size", type=int, default=512)
    parser.add_argument("--queue-blocks", type=int, default=4)
    parser.add_argument("--frequency", type=float, default=440.0)
    parser.add_argument(
        "--balance",
        type=int,
        default=0,
        help="balanço de saída: -100 (esquerda), 0 (centro), 100 (direita)",
    )
    parser.add_argument(
        "--signal",
        choices=("tone", "melody"),
        default="tone",
        help="tom senoidal de referência ou melodia chiptune original",
    )
    parser.add_argument(
        "--trace-csv",
        type=Path,
        default=None,
        help=(
            "salva os tempos por ciclo em CSV; com backend=both cria "
            "um arquivo por backend"
        ),
    )
    args = parser.parse_args()

    if not 1 <= args.seconds <= 300:
        parser.error("--seconds deve estar entre 1 e 300")
    if args.sample_rate not in {22_050, 44_100, 48_000}:
        parser.error("--sample-rate deve ser 22050, 44100 ou 48000")
    if args.buffer_size not in {256, 512, 1024, 2048}:
        parser.error("--buffer-size deve ser 256, 512, 1024 ou 2048")
    if args.queue_blocks not in {2, 4, 8}:
        parser.error("--queue-blocks deve ser 2, 4 ou 8")
    if not 20.0 <= args.frequency <= 10_000.0:
        parser.error("--frequency deve estar entre 20 e 10000 Hz")
    if not -100 <= args.balance <= 100:
        parser.error("--balance deve estar entre -100 e 100")

    backends = ("wasapi", "xaudio2") if args.backend == "both" else (args.backend,)
    results = [
        _run_backend(
            backend,
            args.sample_rate,
            args.buffer_size,
            args.queue_blocks,
            args.seconds,
            args.frequency,
            args.signal,
            args.balance,
            args.trace_csv,
        )
        for backend in backends
    ]
    return 0 if all(results) else 1


if __name__ == "__main__":
    sys.exit(main())
