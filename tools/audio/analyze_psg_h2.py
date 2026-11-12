#!/usr/bin/env python3
"""Analyze the test-only SN76489 event-based reconstruction against fixed PCM references."""
from __future__ import annotations
import argparse
import json
import math
from pathlib import Path
from typing import Any

from compare_psg_wav import (
    TRACE_EVENTS, compare, fft_power, read_float_wav, spectral_delta,
    spectral_summary, summarize, transient_summary,
)

def tone_summary(samples: list[float], sample_rate: int, period: int) -> dict[str, Any]:
    """Measure a tone at its folded frequency below Nyquist."""
    expected_hz = 3_579_545.0 / (32.0 * period)
    wrapped = expected_hz % sample_rate
    alias_hz = min(wrapped, sample_rate - wrapped)
    steady = samples[max(256, len(samples) // 8):]
    power, fft_size = fft_power(steady)
    expected_bin = round(alias_hz * fft_size / sample_rate)
    first_bin = max(1, expected_bin - 4)
    end_bin = min(fft_size // 2 + 1, expected_bin + 5)
    if first_bin >= end_bin:
        raise ValueError(f"period {period}: no FFT bins around aliased fundamental")
    peak_bin = max(range(first_bin, end_bin), key=power.__getitem__)
    total_power = math.fsum(power[1:])
    fundamental_power = math.fsum(power[first_bin:end_bin])
    rms = math.sqrt(math.fsum(value * value for value in steady) / len(steady))
    return {
        "period": period,
        "expected_frequency_hz": expected_hz,
        "aliased_frequency_hz": alias_hz,
        "peak_frequency_hz": peak_bin * sample_rate / fft_size,
        "frequency_error_hz": peak_bin * sample_rate / fft_size - alias_hz,
        "rms": rms,
        "fundamental_relative_db": (
            10.0 * math.log10(fundamental_power / total_power)
            if fundamental_power > 0.0 and total_power > 0.0 else None
        ),
    }

def autocorrelation_summary(samples: list[float], maximum_lag: int = 64) -> dict[str, Any]:
    """Report normalized short-lag autocorrelation for deterministic noise traces."""
    mean = math.fsum(samples) / len(samples)
    centered = [sample - mean for sample in samples]
    energy = math.fsum(value * value for value in centered)
    correlations = {}
    for lag in range(1, maximum_lag + 1):
        numerator = math.fsum(
            centered[index] * centered[index + lag]
            for index in range(len(centered) - lag)
        )
        correlations[str(lag)] = numerator / energy if energy > 0.0 else 0.0
    maximum = max((abs(value) for value in correlations.values()), default=0.0)
    return {"lags": correlations, "max_abs_nonzero_lag": maximum}

def load_pair(reference_path: Path, candidate_path: Path, sample_rate: int):
    reference_rate, reference = read_float_wav(reference_path)
    candidate_rate, candidate = read_float_wav(candidate_path)
    if reference_rate != sample_rate or candidate_rate != sample_rate:
        raise ValueError("H2 trace sample-rate mismatch")
    if len(reference) != len(candidate):
        raise ValueError("H2 trace sample-count mismatch")
    return reference, candidate

def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--baseline-sms", type=Path, required=True)
    parser.add_argument("--current-sms", type=Path, required=True)
    parser.add_argument("--event-sms", type=Path, required=True)
    parser.add_argument("--baseline-sg1000", type=Path, required=True)
    parser.add_argument("--current-sg1000", type=Path, required=True)
    parser.add_argument("--event-sg1000", type=Path, required=True)
    parser.add_argument("--event-model-metrics", type=Path, required=True)
    parser.add_argument("--event-tone", action="append", nargs=4,
                        metavar=("PERIOD", "BASELINE_WAV", "CURRENT_WAV", "EVENT_WAV"),
                        default=[])
    parser.add_argument("--noise", action="append", nargs=4,
                        metavar=("PROFILE", "MODE", "CURRENT_WAV", "EVENT_WAV"),
                        default=[])
    parser.add_argument("--json", type=Path, default=Path("artifacts/psg-h2-report.json"))
    parser.add_argument("--markdown", type=Path, default=Path("artifacts/psg-h2-report.md"))
    args = parser.parse_args()

    sample_rate = 48_000
    model_metrics = json.loads(args.event_model_metrics.read_text())
    report: dict[str, Any] = {"model": model_metrics, "profiles": {}, "tone_sweep": {}, "noise": {}}

    for profile in ("sms", "sg1000"):
        baseline, current = load_pair(
            getattr(args, f"baseline_{profile}"),
            getattr(args, f"current_{profile}"),
            sample_rate,
        )
        event_rate, event = read_float_wav(getattr(args, f"event_{profile}"))
        if event_rate != sample_rate or len(event) != len(baseline):
            raise ValueError(f"{profile}: event/baseline sample-rate or count mismatch")
        baseline_spectrum = spectral_summary(baseline, sample_rate)
        current_spectrum = spectral_summary(current, sample_rate)
        event_spectrum = spectral_summary(event, sample_rate)
        report["profiles"][profile] = {
            "baseline": summarize(baseline, sample_rate),
            "current": summarize(current, sample_rate),
            "event": summarize(event, sample_rate),
            "baseline_vs_current": compare(baseline, current),
            "baseline_vs_event": compare(baseline, event),
            "current_vs_event": compare(current, event),
            "baseline_spectrum": baseline_spectrum,
            "current_spectrum": current_spectrum,
            "event_spectrum": event_spectrum,
            "event_spectral_delta_db": spectral_delta(baseline_spectrum, event_spectrum),
            "transients": [
                {
                    "name": name,
                    "baseline": transient_summary(baseline, sample_rate, clock),
                    "current": transient_summary(current, sample_rate, clock),
                    "event": transient_summary(event, sample_rate, clock),
                }
                for name, clock in TRACE_EVENTS
            ],
        }

    for period_text, baseline_path, current_path, event_path in args.event_tone:
        period = int(period_text)
        if not 1 <= period <= 1023:
            raise ValueError(f"tone period must be in 1..=1023, got {period}")
        baseline, current = load_pair(Path(baseline_path), Path(current_path), sample_rate)
        event_rate, event = read_float_wav(Path(event_path))
        if event_rate != sample_rate or len(event) != len(baseline):
            raise ValueError(f"period {period}: event sample-rate/count mismatch")
        report["tone_sweep"][str(period)] = {
            "baseline": tone_summary(baseline, sample_rate, period),
            "current": tone_summary(current, sample_rate, period),
            "event": tone_summary(event, sample_rate, period),
            "baseline_vs_event": compare(baseline, event),
            "current_vs_event": compare(current, event),
        }

    for profile, mode, current_path, event_path in args.noise:
        if profile not in ("sms", "sg1000") or mode not in ("periodic", "white"):
            raise ValueError(f"invalid noise profile/mode: {profile}/{mode}")
        current, event = load_pair(Path(current_path), Path(event_path), sample_rate)
        current_spectrum = spectral_summary(current, sample_rate)
        event_spectrum = spectral_summary(event, sample_rate)
        report["noise"][f"{profile}-{mode}"] = {
            "current": summarize(current, sample_rate),
            "event": summarize(event, sample_rate),
            "current_vs_event": compare(current, event),
            "current_spectrum": current_spectrum,
            "event_spectrum": event_spectrum,
            "event_spectral_delta_db": spectral_delta(current_spectrum, event_spectrum),
            "current_autocorrelation": autocorrelation_summary(current),
            "event_autocorrelation": autocorrelation_summary(event),
        }

    args.json.parent.mkdir(parents=True, exist_ok=True)
    args.markdown.parent.mkdir(parents=True, exist_ok=True)
    args.json.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")

    lines = [
        "# SN76489 H2 event-based reconstruction report", "",
        "The event renderer is test-only. No production PSG or Scheduler path is changed.", "",
        f"- Sample rate: {sample_rate} Hz mono float32 WAV",
        f"- Kernel: {model_metrics['kernel_taps']} taps; Kaiser beta {model_metrics['kaiser_beta']}",
        f"- Group delay: {model_metrics['group_delay_samples']} output samples",
        f"- Passband gain at 0.42: {model_metrics['passband_gain_at_0_42']:.9g}",
        f"- Stopband gain at Nyquist: {model_metrics['stopband_gain_at_nyquist']:.9g}",
        f"- Step undershoot: {model_metrics['step_undershoot']:.6%}",
        f"- Step overshoot: {model_metrics['step_overshoot']:.6%}",
        f"- Settling to 0.1%: {model_metrics['settling_to_0_1_percent_samples']:.3f} samples", "",
    ]
    for profile in ("sms", "sg1000"):
        data = report["profiles"][profile]
        lines.extend([
            f"## {profile.upper()}", "",
            "| Comparison | NRMSE | Correlation |", "|---|---:|---:|",
        ])
        for name, key in (
            ("Baseline vs current", "baseline_vs_current"),
            ("Baseline vs event", "baseline_vs_event"),
            ("Current vs event", "current_vs_event"),
        ):
            result = data[key]
            lines.append(
                f"| {name} | {result['normalized_rms_error']:.6%} | {result['correlation']:.9f} |"
            )
        lines.extend(["", "| Band | Event energy delta vs baseline (dB) |", "|---|---:|"])
        for band, value in data["event_spectral_delta_db"].items():
            lines.append(f"| {band} | {value:.4f} |")
        lines.extend([
            "", "| Transient | Baseline early peak | Current early peak | Event early peak | "
            "Baseline settled RMS | Current settled RMS | Event settled RMS |",
            "|---|---:|---:|---:|---:|---:|---:|",
        ])
        for event in data["transients"]:
            lines.append(
                f"| {event['name']} | {event['baseline']['early_peak']:.9f} | "
                f"{event['current']['early_peak']:.9f} | {event['event']['early_peak']:.9f} | "
                f"{event['baseline']['settled_rms']:.9f} | "
                f"{event['current']['settled_rms']:.9f} | "
                f"{event['event']['settled_rms']:.9f} |"
            )
        lines.append("")

    lines.extend([
        "## Tone sweep checkpoints", "",
        "| Period | Expected Hz | Aliased Hz | Baseline RMS | Current RMS | Event RMS | Event NRMSE vs baseline |",
        "|---:|---:|---:|---:|---:|---:|---:|",
    ])
    for period, data in sorted(report["tone_sweep"].items(), key=lambda item: int(item[0])):
        tone = data["event"]
        lines.append(
            f"| {period} | {tone['expected_frequency_hz']:.3f} | "
            f"{tone['aliased_frequency_hz']:.3f} | {data['baseline']['rms']:.9f} | "
            f"{data['current']['rms']:.9f} | {tone['rms']:.9f} | "
            f"{data['baseline_vs_event']['normalized_rms_error']:.6%} |"
        )
    lines.extend(["", "## Noise profiles", ""])
    for name, data in sorted(report["noise"].items()):
        lines.append(
            f"- **{name}:** NRMSE {data['current_vs_event']['normalized_rms_error']:.6%}; "
            f"event spectral centroid {data['event_spectrum']['spectral_centroid_hz']:.3f} Hz; "
            f"maximum short-lag autocorrelation "
            f"{data['event_autocorrelation']['max_abs_nonzero_lag']:.6f}"
        )
    lines.append("")
    args.markdown.write_text("\n".join(lines) + "\n")

if __name__ == "__main__":
    main()
