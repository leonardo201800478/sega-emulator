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


def fft_power(samples: list[float]) -> tuple[list[float], int]:
    """Return Hann-windowed one-sided FFT power and radix-2 transform size."""
    sample_count = len(samples)
    if sample_count < 2:
        raise ValueError("FFT requires at least two samples")
    fft_size = 1 << (sample_count - 1).bit_length()
    values = [
        complex(
            sample
            * (0.5 - 0.5 * math.cos(math.tau * index / (sample_count - 1))),
            0.0,
        )
        for index, sample in enumerate(samples)
    ]
    values.extend([0j] * (fft_size - sample_count))

    reversed_index = 0
    for index in range(1, fft_size):
        bit = fft_size >> 1
        while reversed_index & bit:
            reversed_index ^= bit
            bit >>= 1
        reversed_index ^= bit
        if index < reversed_index:
            values[index], values[reversed_index] = (
                values[reversed_index],
                values[index],
            )

    size = 2
    while size <= fft_size:
        angle = -math.tau / size
        twiddle_step = complex(math.cos(angle), math.sin(angle))
        half_size = size // 2
        for start in range(0, fft_size, size):
            twiddle = 1.0 + 0.0j
            for offset in range(half_size):
                even = values[start + offset]
                odd = values[start + offset + half_size] * twiddle
                values[start + offset] = even + odd
                values[start + offset + half_size] = even - odd
                twiddle *= twiddle_step
        size *= 2

    power = [
        value.real * value.real + value.imag * value.imag
        for value in values[: fft_size // 2 + 1]
    ]
    return power, fft_size


def spectral_summary(samples: list[float], sample_rate: int) -> dict[str, Any]:
    """Calculate Hann-windowed spectral energy using a dependency-free radix-2 FFT."""
    power, fft_size = fft_power(samples)
    total_power = math.fsum(power[1:])
    centroid = (
        math.fsum(
            index * sample_rate / fft_size * value
            for index, value in enumerate(power[1:], start=1)
        )
        / total_power
        if total_power > 0.0
        else 0.0
    )
    bands: dict[str, dict[str, float | None]] = {}
    for low_hz, high_hz in (
        (20, 200),
        (200, 1_000),
        (1_000, 4_000),
        (4_000, 8_000),
        (8_000, 12_000),
        (12_000, 16_000),
        (16_000, 20_000),
        (20_000, 24_000),
    ):
        first_bin = max(1, math.ceil(low_hz * fft_size / sample_rate))
        end_bin = min(
            fft_size // 2 + 1,
            math.ceil(high_hz * fft_size / sample_rate),
        )
        band_power = math.fsum(power[first_bin:end_bin])
        relative_db = (
            10.0 * math.log10(band_power / total_power)
            if band_power > 0.0 and total_power > 0.0
            else None
        )
        bands[f"{low_hz}-{high_hz} Hz"] = {
            "power": band_power,
            "relative_db": relative_db,
        }

    return {
        "fft_size": fft_size,
        "window": "Hann",
        "spectral_centroid_hz": centroid,
        "bands": bands,
    }


def tone_response_summary(
    samples: list[float], sample_rate: int, period: int
) -> dict[str, Any]:
    """Measure an isolated tone's RMS and energy around its expected fundamental."""
    steady_state = samples[min(256, len(samples) // 4) :]
    power, fft_size = fft_power(steady_state)
    expected_hz = 3_579_545.0 / (32.0 * period)
    expected_bin = round(expected_hz * fft_size / sample_rate)
    first_bin = max(1, expected_bin - 4)
    end_bin = min(fft_size // 2 + 1, expected_bin + 5)
    peak_bin = max(range(first_bin, end_bin), key=power.__getitem__)
    fundamental_power = math.fsum(power[first_bin:end_bin])
    total_power = math.fsum(power[1:])
    harmonic_powers: dict[str, float] = {}
    max_harmonic = int((sample_rate / 2) // expected_hz)
    for harmonic in range(2, max_harmonic + 1):
        harmonic_bin = round(harmonic * expected_hz * fft_size / sample_rate)
        harmonic_start = max(1, harmonic_bin - 4)
        harmonic_end = min(fft_size // 2 + 1, harmonic_bin + 5)
        harmonic_powers[str(harmonic)] = math.fsum(
            power[harmonic_start:harmonic_end]
        )
    total_harmonic_power = math.fsum(harmonic_powers.values())
    thd_ratio = (
        math.sqrt(total_harmonic_power / fundamental_power)
        if fundamental_power > 0.0
        else 0.0
    )
    rms = math.sqrt(
        math.fsum(value * value for value in steady_state) / len(steady_state)
    )
    mean = math.fsum(samples) / len(samples)
    peak_index = max(range(len(samples)), key=lambda index: abs(samples[index]))
    peak = abs(samples[peak_index])
    startup_window = samples[: min(256, len(samples))]
    startup_peak = max((abs(value) for value in startup_window), default=0.0)
    steady_peak = max((abs(value) for value in steady_state), default=0.0)
    max_delta = max(
        (abs(samples[index] - samples[index - 1]) for index in range(1, len(samples))),
        default=0.0,
    )
    return {
        "expected_frequency_hz": expected_hz,
        "peak_frequency_hz": peak_bin * sample_rate / fft_size,
        "frequency_error_hz": peak_bin * sample_rate / fft_size - expected_hz,
        "rms": rms,
        "mean": mean,
        "peak": peak,
        "peak_index": peak_index,
        "startup_peak": startup_peak,
        "steady_peak": steady_peak,
        "max_delta": max_delta,
        "fundamental_power": fundamental_power,
        "fundamental_relative_db": (
            10.0 * math.log10(fundamental_power / total_power)
            if fundamental_power > 0.0 and total_power > 0.0
            else None
        ),
        "harmonic_powers": harmonic_powers,
        "thd_ratio": thd_ratio,
        "thd_percent": 100.0 * thd_ratio,
        "thd_db": 20.0 * math.log10(thd_ratio) if thd_ratio > 0.0 else None,
    }


def spectral_delta(
    baseline: dict[str, Any], current: dict[str, Any]
) -> dict[str, float | None]:
    """Return the current-to-baseline band-energy change in decibels."""
    changes: dict[str, float | None] = {}
    for band, baseline_values in baseline["bands"].items():
        baseline_power = baseline_values["power"]
        current_power = current["bands"][band]["power"]
        changes[band] = (
            10.0 * math.log10(current_power / baseline_power)
            if baseline_power > 0.0 and current_power > 0.0
            else None
        )
    return changes


TRACE_EVENTS = (
    ("tone0-volume", 2_000_000),
    ("noise-divider", 4_000_000),
    ("noise-volume", 6_000_000),
    ("tone0-period", 8_000_000),
    ("tone1-volume", 10_000_000),
    ("noise-reset", 12_000_000),
    ("noise-volume-final", 14_000_000),
)


def transient_summary(
    samples: list[float], sample_rate: int, event_clock: int
) -> dict[str, float | int]:
    """Measure short pre-event, early-post-event, and settled windows."""
    center = int(event_clock * sample_rate / 3_579_545)
    pre_length = round(sample_rate * 0.002)
    early_length = round(sample_rate * 0.002)
    settle_start = round(sample_rate * 0.005)
    settle_end = round(sample_rate * 0.010)
    pre = samples[max(0, center - pre_length) : center]
    early = samples[center : center + early_length]
    settled = samples[
        center + settle_start : min(len(samples), center + settle_end)
    ]
    if not pre or not early or not settled or center == 0:
        raise ValueError(f"event at clock {event_clock} lacks complete analysis windows")

    def rms(window: list[float]) -> float:
        return math.sqrt(math.fsum(value * value for value in window) / len(window))

    return {
        "event_clock": event_clock,
        "sample_index": center,
        "pre_rms": rms(pre),
        "early_rms": rms(early),
        "settled_rms": rms(settled),
        "early_peak": max(abs(value) for value in early),
        "settled_peak": max(abs(value) for value in settled),
        "edge_delta": abs(samples[center] - samples[center - 1]),
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
    parser.add_argument(
        "--tone",
        action="append",
        nargs=3,
        metavar=("PERIOD", "BASELINE_WAV", "CURRENT_WAV"),
        default=[],
        help="Add an isolated SMS tone comparison for the supplied SN76489 period",
    )
    parser.add_argument(
        "--lagrange4-tone",
        action="append",
        nargs=4,
        metavar=("PERIOD", "BASELINE_WAV", "LINEAR_WAV", "LAGRANGE4_WAV"),
        default=[],
        help="Compare experimental Lagrange4 interpolation against baseline and linear output",
    )
    parser.add_argument(
        "--lagrange3-tone",
        action="append",
        nargs=4,
        metavar=("PERIOD", "BASELINE_WAV", "LINEAR_WAV", "LAGRANGE3_WAV"),
        default=[],
        help="Compare experimental Lagrange3 interpolation against baseline and linear output",
    )
    parser.add_argument(
        "--sinc8-tone",
        action="append",
        nargs=4,
        metavar=("PERIOD", "BASELINE_WAV", "LINEAR_WAV", "SINC8_WAV"),
        default=[],
        help="Compare the experimental windowed-sinc fractional interpolator",
    )
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
        "isolated_tone_response": {},
        "lagrange4_tone_response": {},
        "lagrange3_tone_response": {},
        "sinc8_tone_response": {},
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
        baseline_spectrum = spectral_summary(baseline, sample_rate)
        current_spectrum = spectral_summary(current, sample_rate)

        transient_events = [
            {
                "name": event_name,
                "baseline": transient_summary(baseline, sample_rate, event_clock),
                "current": transient_summary(current, sample_rate, event_clock),
            }
            for event_name, event_clock in TRACE_EVENTS
        ]
        report["profiles"][profile] = {
            "baseline": summarize(baseline, sample_rate),
            "current": summarize(current, sample_rate),
            "current_chunked": summarize(chunked, sample_rate),
            "baseline_vs_current": baseline_comparison,
            "current_vs_chunked": chunk_comparison,
            "baseline_spectrum": baseline_spectrum,
            "current_spectrum": current_spectrum,
            "spectral_band_delta_db": spectral_delta(
                baseline_spectrum, current_spectrum
            ),
            "transient_events": transient_events,
        }

    for period_text, baseline_path, current_path in args.tone:
        period = int(period_text)
        if not 1 <= period <= 1023:
            raise ValueError(f"tone period must be in 1..=1023, got {period}")
        baseline_rate, baseline_samples = read_float_wav(Path(baseline_path))
        current_rate, current_samples = read_float_wav(Path(current_path))
        if baseline_rate != sample_rate or current_rate != sample_rate:
            raise ValueError(f"tone period {period}: sample-rate mismatch")
        if len(baseline_samples) != len(current_samples):
            raise ValueError(f"tone period {period}: baseline/current sample counts differ")
        baseline_tone = tone_response_summary(
            baseline_samples, sample_rate, period
        )
        current_tone = tone_response_summary(current_samples, sample_rate, period)
        baseline_power = baseline_tone["fundamental_power"]
        current_power = current_tone["fundamental_power"]
        report["isolated_tone_response"][str(period)] = {
            "baseline": baseline_tone,
            "current": current_tone,
            "fundamental_energy_delta_db": (
                10.0 * math.log10(current_power / baseline_power)
                if baseline_power > 0.0 and current_power > 0.0
                else None
            ),
        }

    for period_text, baseline_path, linear_path, lagrange_path in args.lagrange4_tone:
        period = int(period_text)
        if not 1 <= period <= 1023:
            raise ValueError(f"tone period must be in 1..=1023, got {period}")
        baseline_rate, baseline_samples = read_float_wav(Path(baseline_path))
        linear_rate, linear_samples = read_float_wav(Path(linear_path))
        lagrange_rate, lagrange_samples = read_float_wav(Path(lagrange_path))
        if {baseline_rate, linear_rate, lagrange_rate} != {sample_rate}:
            raise ValueError(f"period {period}: Lagrange4 sample-rate mismatch")
        if len({len(baseline_samples), len(linear_samples), len(lagrange_samples)}) != 1:
            raise ValueError(f"period {period}: Lagrange4 sample counts differ")

        baseline_tone = tone_response_summary(
            baseline_samples, sample_rate, period
        )
        linear_tone = tone_response_summary(linear_samples, sample_rate, period)
        lagrange_tone = tone_response_summary(
            lagrange_samples, sample_rate, period
        )
        baseline_power = baseline_tone["fundamental_power"]
        linear_power = linear_tone["fundamental_power"]
        lagrange_power = lagrange_tone["fundamental_power"]
        report["lagrange4_tone_response"][str(period)] = {
            "baseline": baseline_tone,
            "linear": linear_tone,
            "lagrange4": lagrange_tone,
            "lagrange4_vs_baseline_db": (
                10.0 * math.log10(lagrange_power / baseline_power)
                if baseline_power > 0.0 and lagrange_power > 0.0
                else None
            ),
            "lagrange4_vs_linear_db": (
                10.0 * math.log10(lagrange_power / linear_power)
                if linear_power > 0.0 and lagrange_power > 0.0
                else None
            ),
        }


    for period_text, baseline_path, linear_path, lagrange_path in args.lagrange3_tone:
        period = int(period_text)
        if not 1 <= period <= 1023:
            raise ValueError(f"tone period must be in 1..=1023, got {period}")
        baseline_rate, baseline_samples = read_float_wav(Path(baseline_path))
        linear_rate, linear_samples = read_float_wav(Path(linear_path))
        lagrange_rate, lagrange_samples = read_float_wav(Path(lagrange_path))
        if {baseline_rate, linear_rate, lagrange_rate} != {sample_rate}:
            raise ValueError(f"period {period}: Lagrange3 sample-rate mismatch")
        if len({len(baseline_samples), len(linear_samples), len(lagrange_samples)}) != 1:
            raise ValueError(f"period {period}: Lagrange3 sample counts differ")

        baseline_tone = tone_response_summary(
            baseline_samples, sample_rate, period
        )
        linear_tone = tone_response_summary(linear_samples, sample_rate, period)
        lagrange_tone = tone_response_summary(
            lagrange_samples, sample_rate, period
        )
        baseline_power = baseline_tone["fundamental_power"]
        linear_power = linear_tone["fundamental_power"]
        lagrange_power = lagrange_tone["fundamental_power"]
        report["lagrange3_tone_response"][str(period)] = {
            "baseline": baseline_tone,
            "linear": linear_tone,
            "lagrange3": lagrange_tone,
            "lagrange3_vs_baseline_db": (
                10.0 * math.log10(lagrange_power / baseline_power)
                if baseline_power > 0.0 and lagrange_power > 0.0
                else None
            ),
            "lagrange3_vs_linear_db": (
                10.0 * math.log10(lagrange_power / linear_power)
                if linear_power > 0.0 and lagrange_power > 0.0
                else None
            ),
        }

    for period_text, baseline_path, linear_path, sinc_path in args.sinc8_tone:
        period = int(period_text)
        if not 1 <= period <= 1023:
            raise ValueError(f"tone period must be in 1..=1023, got {period}")
        baseline_rate, baseline_samples = read_float_wav(Path(baseline_path))
        linear_rate, linear_samples = read_float_wav(Path(linear_path))
        sinc_rate, sinc_samples = read_float_wav(Path(sinc_path))
        if {baseline_rate, linear_rate, sinc_rate} != {sample_rate}:
            raise ValueError(f"period {period}: sinc8 sample-rate mismatch")
        common_count = min(len(baseline_samples), len(linear_samples), len(sinc_samples))
        if common_count < sample_rate // 10:
            raise ValueError(f"period {period}: insufficient common sinc8 analysis window")
        baseline_samples = baseline_samples[:common_count]
        linear_samples = linear_samples[:common_count]
        sinc_samples = sinc_samples[:common_count]

        baseline_tone = tone_response_summary(baseline_samples, sample_rate, period)
        linear_tone = tone_response_summary(linear_samples, sample_rate, period)
        sinc_tone = tone_response_summary(sinc_samples, sample_rate, period)
        baseline_power = baseline_tone["fundamental_power"]
        linear_power = linear_tone["fundamental_power"]
        sinc_power = sinc_tone["fundamental_power"]
        report["sinc8_tone_response"][str(period)] = {
            "baseline": baseline_tone,
            "linear": linear_tone,
            "sinc8": sinc_tone,
            "sinc8_vs_baseline_db": (
                10.0 * math.log10(sinc_power / baseline_power)
                if baseline_power > 0.0 and sinc_power > 0.0
                else None
            ),
            "sinc8_vs_linear_db": (
                10.0 * math.log10(sinc_power / linear_power)
                if linear_power > 0.0 and sinc_power > 0.0
                else None
            ),
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
                "### Spectral band energy",
                "",
                "| Band | Baseline (dB rel.) | Current (dB rel.) | Energy delta (dB) |",
                "|---|---:|---:|---:|",
            ]
        )
        for band, baseline_values in data["baseline_spectrum"]["bands"].items():
            current_values = data["current_spectrum"]["bands"][band]
            baseline_db = baseline_values["relative_db"]
            current_db = current_values["relative_db"]
            delta_db = data["spectral_band_delta_db"][band]
            values = (baseline_db, current_db, delta_db)
            formatted = [
                f"{value:.3f}" if value is not None else "n/a"
                for value in values
            ]
            lines.append(f"| {band} | " + " | ".join(formatted) + " |")
        lines.extend(
            [
                "",
                (
                    "Spectral centroid (Hz): "
                    f"baseline={data['baseline_spectrum']['spectral_centroid_hz']:.3f}, "
                    f"current={data['current_spectrum']['spectral_centroid_hz']:.3f}"
                ),
                "",
                "### Register-write transient windows",
                "",
                "| Event | Baseline early peak | Current early peak | Baseline settled RMS | Current settled RMS | Edge delta baseline/current |",
                "|---|---:|---:|---:|---:|---:|",
            ]
        )
        for event in data["transient_events"]:
            baseline_event = event["baseline"]
            current_event = event["current"]
            lines.append(
                f"| {event['name']} | "
                f"{baseline_event['early_peak']:.9f} | "
                f"{current_event['early_peak']:.9f} | "
                f"{baseline_event['settled_rms']:.9f} | "
                f"{current_event['settled_rms']:.9f} | "
                f"{baseline_event['edge_delta']:.9f} / "
                f"{current_event['edge_delta']:.9f} |"
            )
        lines.append("")
    if report["isolated_tone_response"]:
        lines.extend(
            [
                "## Isolated SMS tone response",
                "",
                "| Period | Expected Hz | Baseline RMS | Current RMS | Fundamental energy delta | Baseline THD | Current THD |",
                "|---:|---:|---:|---:|---:|---:|---:|",
            ]
        )
        for period, data in sorted(
            report["isolated_tone_response"].items(),
            key=lambda item: int(item[0]),
        ):
            lines.append(
                f"| {period} | "
                f"{data['baseline']['expected_frequency_hz']:.3f} | "
                f"{data['baseline']['rms']:.9f} | "
                f"{data['current']['rms']:.9f} | "
                f"{data['fundamental_energy_delta_db']:.4f} dB | "
                f"{data['baseline']['thd_percent']:.4f}% | "
                f"{data['current']['thd_percent']:.4f}% |"
            )
        lines.append("")

    if report["lagrange4_tone_response"]:
        lines.extend(
            [
                "## Experimental Lagrange4 interpolation",
                "",
                "| Period | Baseline THD | Linear THD | Lagrange4 THD | Lagrange4 vs. linear |",
                "|---:|---:|---:|---:|---:|",
            ]
        )
        for period, data in sorted(
            report["lagrange4_tone_response"].items(),
            key=lambda item: int(item[0]),
        ):
            lines.append(
                f"| {period} | "
                f"{data['baseline']['thd_percent']:.4f}% | "
                f"{data['linear']['thd_percent']:.4f}% | "
                f"{data['lagrange4']['thd_percent']:.4f}% | "
                f"{data['lagrange4_vs_linear_db']:.4f} dB |"
            )
        lines.append("")

    if report["lagrange3_tone_response"]:
        lines.extend(
            [
                "## Experimental Lagrange3 interpolation",
                "",
                "| Period | Baseline THD | Linear THD | Lagrange3 THD | Lagrange3 vs. linear |",
                "|---:|---:|---:|---:|---:|",
            ]
        )
        for period, data in sorted(
            report["lagrange3_tone_response"].items(),
            key=lambda item: int(item[0]),
        ):
            lines.append(
                f"| {period} | "
                f"{data['baseline']['thd_percent']:.4f}% | "
                f"{data['linear']['thd_percent']:.4f}% | "
                f"{data['lagrange3']['thd_percent']:.4f}% | "
                f"{data['lagrange3_vs_linear_db']:.4f} dB |"
            )
        lines.append("")

    if report["sinc8_tone_response"]:
        lines.extend(
            [
                "## Experimental windowed-sinc fractional interpolation",
                "",
                "| Period | Baseline THD | Linear THD | Sinc8 THD | Baseline peak | Linear peak | Sinc8 peak | Sinc8 vs. baseline | Sinc8 vs. linear |",
                "|---:|---:|---:|---:|---:|---:|---:|---:|---:|",
            ]
        )
        for period, data in sorted(
            report["sinc8_tone_response"].items(),
            key=lambda item: int(item[0]),
        ):
            lines.append(
                f"| {period} | "
                f"{data['baseline']['thd_percent']:.4f}% | "
                f"{data['linear']['thd_percent']:.4f}% | "
                f"{data['sinc8']['thd_percent']:.4f}% | "
                f"{data['baseline']['peak']:.6f} | "
                f"{data['linear']['peak']:.6f} | "
                f"{data['sinc8']['peak']:.6f} | "
                f"{data['sinc8_vs_baseline_db']:.4f} dB | "
                f"{data['sinc8_vs_linear_db']:.4f} dB |"
            )
        lines.append("")

    args.markdown.write_text("\n".join(lines) + "\n")


if __name__ == "__main__":
    main()
