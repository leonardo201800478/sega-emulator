"""Physical smoke test for native Rust WASAPI and XAudio2 outputs.

Run from the repository root after installing the Qt/native extension:
    python tools/validate_native_audio.py --backend both

The low-volume tone validates host output only; it does not execute or modify
emulated hardware state.
"""

from __future__ import annotations

import argparse
import math
import struct
import sys
import time

from sega_emulator.frontend.qt.wasapi_audio import NativeAudioOutput


def _tone_block(
    sample_rate: int, frequency: float, start_sample: int, count: int
) -> bytes:
    amplitude = 0.15 * 32767
    samples = (
        round(
            amplitude
            * math.sin(
                2.0 * math.pi * frequency * (start_sample + offset) / sample_rate
            )
        )
        for offset in range(count)
    )
    return struct.pack(f"<{count}h", *samples)


def _run_backend(
    backend: str,
    sample_rate: int,
    buffer_size: int,
    queue_blocks: int,
    seconds: int,
    frequency: float,
) -> bool:
    print(
        f"\nBackend={backend}; taxa solicitada={sample_rate} Hz; "
        f"bloco={buffer_size} amostras; fila={queue_blocks}; duração={seconds}s"
    )
    try:
        output = NativeAudioOutput(
            sample_rate,
            buffer_size,
            backend=backend,
            queue_blocks=queue_blocks,
            balance=0,
        )
    except Exception as exc:
        print(f"FAIL: inicialização: {type(exc).__name__}: {exc}")
        return False

    started = time.monotonic()
    sample_index = 0
    try:
        while time.monotonic() - started < seconds:
            pcm = _tone_block(sample_rate, frequency, sample_index, buffer_size)
            output.write_pcm(pcm)
            sample_index += buffer_size
            time.sleep(buffer_size / sample_rate)
        underruns = output.underrun_callbacks
        dropped = output.dropped_blocks
    except Exception as exc:
        print(f"FAIL: escrita PCM: {type(exc).__name__}: {exc}")
        return False
    finally:
        try:
            output.close()
        except Exception as exc:
            print(f"FAIL: encerramento: {type(exc).__name__}: {exc}")
            return False

    elapsed = time.monotonic() - started
    print(
        f"Saída solicitada: {output.sample_rate} Hz / {output.buffer_size} amostras; "
        f"tempo decorrido={elapsed:.2f}s; underruns={underruns}; "
        f"blocos descartados={dropped}"
    )
    print("Confirme auditivamente que o tom foi reproduzido sem cortes ou distorção.")
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

    backends = ("wasapi", "xaudio2") if args.backend == "both" else (args.backend,)
    results = [
        _run_backend(
            backend,
            args.sample_rate,
            args.buffer_size,
            args.queue_blocks,
            args.seconds,
            args.frequency,
        )
        for backend in backends
    ]
    return 0 if all(results) else 1


if __name__ == "__main__":
    sys.exit(main())
