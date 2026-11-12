#!/usr/bin/env python3
"""Compare deterministic SN76489 baseline/current WAV traces without third-party packages."""

from __future__ import annotations

import argparse
import json
import math
import struct
from pathlib import Path
from typing import Any


def read_float_wav(path: Path) -> tuple[int, list[float]]:
    """Read the mono IEEE-float WAV format emitted by the Rust PSG trace tool."""
    raw = path.read_bytes()
    if raw[:4] != b"RIFF" or raw[8:12] != b"WAVE":
        raise ValueError(f"{path}: not a RIFF/WAVE file")

    offset = 12
    fmt: tuple[int, int, int, int, int, int] | None = None
    payload: bytes | None = None
    while offset + 8 <= len(raw):
        chunk_id = raw[offset : offset + 4]
        chunk_size = struct.unpack_from("<I", raw, offset + 4)[0]
        start = offset + 8
        end = start + chunk_size
        if end > len(raw):
            raise ValueError(f"{path}: truncated WAV chunk {chunk_id!r}")
        if chunk_id == b"fmt ":
            if chunk_size < 16:
                raise ValueError(f"{path}: invalid fmt chunk")
            fmt = struct.unpack_from("<HHIIHH", raw, start)
        elif chunk_id == b"data":
            payload = raw[start:end]
        offset = end + (chunk_size & 1)

    if fmt is None or payload is None:
        raise ValueError(f"{path}: missing fmt or data chunk")
    format_tag, channels, sample_rate, _byte_rate, block_align, bits = fmt
    if (format_tag, channels, block_align, bits) != (3, 1, 4, 32):
        raise ValueError(
            f"{path}: expected mono IEEE-float 32-bit WAV; "
            f"got format={format_tag}, channels={channels}, "
            f"block_align={block_align}, bits={bits}"
        )
    if len(payload) % 4:
        raise ValueError(f"{path}: sample payload is not aligned to float32")
    samples = [item[0] for item in struct.iter_unpack("<f", payload)]
    if not samples or not all(math.isfinite(value) for value in samples):
        raise ValueError(f"{path}: empty or non-finite sample stream")
    return sample_rate, samples


def summarize(samples: list[float], sample_rate: int) -> dict[str, Any]:
    """Calculate basic level, DC, and transition metrics for a PCM trace."""
    count = len(samples)
    mean = math.fsum(samples) / count
    rms = math.sqrt(math.fsum(value * value for value in samples) / count)
    peak = max(abs(value) for value in samples)
    crossings = sum(
        (left < 0.0) != (right < 0.0)
        for left, right in zip(samples, samples[1:])
    )
    return {
        "sample_rate_hz": sample_rate,
        "sample_count": count,
        "duration_seconds": count / sample_rate,
        "peak": peak,
        "rms": rms,
        "dc_mean": mean,
        "zero_crossings": crossings,
    }


def compare(
    reference: list[float], candidate: list[float]
) -> dict[str, Any]:
    """Measure normalized sample-domain differences over the common interval."""
    count = min(len(reference), len(candidate))
    left = reference[:count]
    right = candidate[:count]
    errors = [actual - expected for expected, actual in zip(left, right)]
    reference_rms = math.sqrt(math.fsum(value * value for value in left) / count)
    error_rms = math.sqrt(math.fsum(value * value for value in errors) / count)
    reference_mean = math.fsum(left) / count
    candidate_mean = math.fsum(right) / count
    covariance = math.fsum(
        (a - reference_mean) * (b - candidate_mean) for a, b in zip(left, right)
    )
    reference_energy = math.fsum((value - reference_mean) ** 2 for value in left)
    candidate_energy = math.fsum((value - candidate_mean) ** 2 for value in right)
    denominator = math.sqrt(reference_energy * candidate_energy)
    correlation = covariance / denominator if denominator > 0.0 else None
    return {
        "compared_samples": count,
        "reference_samples": len(reference),
        "candidate_samples": len(candidate),
        "sample_count_equal": len(reference) == len(candidate),
        "exactly_equal": reference == candidate,
        "max_absolute_error": max(abs(value) for value in errors),
        "difference_rms": error_rms,
        "normalized_rms_error": (
            error_rms / reference_rms if reference_rms > 0.0 else None
        ),
        "correlation": correlation,
    }


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--baseline-sms", type=Path, required=True)
    parser.add_argument("--current-sms", type=Path, required=True)
    parser.add_argument("--chunked-sms", type=Path, required=True)
    parser.add_argument("--baseline-sg1000", type=Path, required=True)
    parser.add_argument("--current-sg1000", type=Path, required=True)
    parser.add_argument("--chunked-sg1000", type=Path, required=True)
    parser.add_argument("--json", type=Path, default=Path("artifacts/psg-audio-report.json"))
    parser.add_argument(
        "--markdown",
        type=Path,
        default=Path("artifacts/psg-audio-report.md"),
    )
    args = parser.parse_args()

    inputs = {
        "sms_baseline": args.baseline_sms,
        "sms_current": args.current_sms,
        "sms_chunked": args.chunked_sms,
        "sg1000_baseline": args.baseline_sg1000,
        "sg1000_current": args.current_sg1000,
        "sg1000_chunked": args.chunked_sg1000,
    }
    loaded = {name: read_float_wav(path) for name, path in inputs.items()}
    rates = {rate for rate, _samples in loaded.values()}
    if len(rates) != 1:
        raise ValueError(f"sample-rate mismatch across traces: {sorted(rates)}")
    sample_rate = rates.pop()
    if sample_rate != 48_000:
        raise ValueError(f"expected 48000 Hz reference output, got {sample_rate}")

    traces = {name: samples for name, (_rate, samples) in loaded.items()}
    report: dict[str, Any] = {
        "reference": {
            "baseline_commit": "f47f71830344b85f72329376505d400bfbfaae69",
            "sample_rate_hz": sample_rate,
            "format": "mono IEEE-float 32-bit WAV; raw PSG before host output conversion",
            "interpretation": (
                "Difference metrics quantify implementation changes; they do not "
                "by themselves determine which sound is perceptually better."
            ),
        },
        "profiles": {},
    }

    for profile in ("sms", "sg1000"):
        baseline = traces[f"{profile}_baseline"]
        current = traces[f"{profile}_current"]
        chunked = traces[f"{profile}_chunked"]
        baseline_comparison = compare(baseline, current)
        chunk_comparison = compare(current, chunked)
        if not baseline_comparison["sample_count_equal"]:
            raise ValueError(f"{profile}: baseline/current sample counts differ")
        if not chunk_comparison["exactly_equal"]:
            raise ValueError(f"{profile}: continuous and segmented PCM differ")

        report["profiles"][profile] = {
            "baseline": summarize(baseline, sample_rate),
            "current": summarize(current, sample_rate),
            "current_chunked": summarize(chunked, sample_rate),
            "baseline_vs_current": baseline_comparison,
            "current_vs_chunked": chunk_comparison,
        }

    args.json.parent.mkdir(parents=True, exist_ok=True)
    args.markdown.parent.mkdir(parents=True, exist_ok=True)
    args.json.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")

    lines = [
        "# SN76489 deterministic PCM A/B report",
        "",
        f"- Baseline: `{report['reference']['baseline_commit']}`",
        f"- Output: {sample_rate} Hz, mono, IEEE-float 32-bit WAV",
        "- Source: fixed PSG register-write trace; no ROM, host volume, or bass management",
        "",
        "> Difference metrics quantify changes; they do not alone establish perceptual quality.",
        "",
    ]
    for profile in ("sms", "sg1000"):
        data = report["profiles"][profile]
        lines.extend(
            [
                f"## {profile.upper()}",
                "",
                "| Metric | Baseline | Current | Current segmented |",
                "|---|---:|---:|---:|",
            ]
        )
        for key, label in (
            ("sample_count", "Samples"),
            ("duration_seconds", "Duration (s)"),
            ("peak", "Peak"),
            ("rms", "RMS"),
            ("dc_mean", "DC mean"),
            ("zero_crossings", "Zero crossings"),
        ):
            values = [
                data["baseline"][key],
                data["current"][key],
                data["current_chunked"][key],
            ]
            lines.append(
                f"| {label} | "
                + " | ".join(f"{value:.12g}" if isinstance(value, float) else str(value) for value in values)
                + " |"
            )
        lines.extend(
            [
                "",
                f"- Baseline vs current: `{json.dumps(data['baseline_vs_current'], sort_keys=True)}`",
                f"- Continuous vs segmented: `{json.dumps(data['current_vs_chunked'], sort_keys=True)}`",
                "",
            ]
        )
    args.markdown.write_text("\n".join(lines) + "\n")


if __name__ == "__main__":
    main()
