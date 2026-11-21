#!/usr/bin/env python3
"""Compare pinned external PSG traces with the Sega Emulator core trace."""

from __future__ import annotations

import hashlib
import json
import sys
from pathlib import Path
from typing import Any


def read_jsonl(path: Path, expected_schema: str) -> tuple[dict[str, Any], list[dict[str, Any]], bytes]:
    raw = path.read_bytes()
    lines = raw.decode("utf-8").splitlines()
    if not lines:
        raise SystemExit(f"{path}: empty trace")
    header = json.loads(lines[0])
    if header.get("schema") != expected_schema:
        raise SystemExit(
            f"{path}: expected schema {expected_schema!r}, got {header.get('schema')!r}"
        )
    rows = [json.loads(line) for line in lines[1:] if line.strip()]
    ticks = [row.get("tick") for row in rows]
    if ticks != list(range(len(rows))):
        raise SystemExit(f"{path}: tick sequence is not contiguous from zero")
    return header, rows, raw


def first_index(values: list[bool]) -> int | None:
    return next((index for index, value in enumerate(values) if value), None)



def compare_ares(
    path: Path, core_rows: list[dict[str, Any]], core_header: dict[str, Any]
) -> dict[str, Any]:
    """Compare Ares component clocks with one Rust SMS PSG /16 tick per row."""
    header, rows, raw = read_jsonl(path, "ares-sn76489-state-trace-v1")
    expected_commit = "8bc30c8c3d3c8081d2a66587add602df771441dd"
    if header.get("source_commit") != expected_commit:
        raise SystemExit("Ares reference commit pin mismatch")
    if header.get("time_unit") != "sn76489_component_clock":
        raise SystemExit("Ares trace must use one component clock per row")
    if len(rows) != len(core_rows):
        raise SystemExit(
            f"Ares/core trace lengths differ: reference={len(rows)} core={len(core_rows)}"
        )
    if core_header.get("input_clocks_per_row") != 16:
        raise SystemExit("Rust core trace must advance exactly 16 input clocks per row")

    fields = {
        "tone_pitch": ("tone_pitch", "tone_periods_before"),
        "tone_counter": ("tone_counter", "tone_counters_before"),
        "tone_output": ("tone_output", "tone_outputs_before"),
        "tone_volume": ("tone_volume", "attenuations_before"),
        "tone_pitch_post": ("tone_pitch", "tone_periods_after", "post"),
        "tone_counter_post": ("tone_counter", "tone_counters_after", "post"),
        "tone_output_post": ("tone_output", "tone_outputs_after", "post"),
        "tone_volume_post": ("tone_volume", "attenuations_after", "post"),
    }
    mismatch_counts: dict[str, int] = {}
    first_mismatch: dict[str, int | None] = {}
    for label, mapping in fields.items():
        ares_key, core_key = mapping[:2]
        post = len(mapping) == 3
        ticks = []
        for tick, (ares, core) in enumerate(zip(rows, core_rows, strict=True)):
            ares_state = ares["post_clock"] if post else ares
            if ares_state[ares_key] != core[core_key]:
                ticks.append(tick)
        mismatch_counts[label] = len(ticks)
        first_mismatch[label] = ticks[0] if ticks else None

    scalar_mappings = {
        "noise_counter": ("noise_counter", "noise_counter_before"),
        "noise_counter_post": ("noise_counter", "noise_counter_after", "post"),
        "noise_flip": ("noise_flip", "noise_phase_before"),
        "noise_flip_post": ("noise_flip", "noise_phase_after", "post"),
        "lfsr": ("lfsr", "lfsr_before"),
        "lfsr_post": ("lfsr", "lfsr_after", "post"),
        "noise_output": ("noise_output", "noise_output_before"),
        "noise_output_post": ("noise_output", "noise_output_after", "post"),
        "latch_type": ("latch_type", "latched_register_before", "type"),
        "latch_channel": ("latch_channel", "latched_register_before", "channel"),
        "noise_rate": ("noise_rate", "noise_control_before", "rate"),
        "noise_enable": ("noise_enable", "noise_control_before", "enable"),
        "latch_type_post": ("latch_type", "latched_register_after", "type_post"),
        "latch_channel_post": ("latch_channel", "latched_register_after", "channel_post"),
        "noise_rate_post": ("noise_rate", "noise_control_after", "rate_post"),
        "noise_enable_post": ("noise_enable", "noise_control_after", "enable_post"),
    }
    for label, mapping in scalar_mappings.items():
        ares_key, core_key, *mode = mapping
        ticks = []
        for tick, (ares, core) in enumerate(zip(rows, core_rows, strict=True)):
            ares_state = ares["post_clock"] if mode and mode[0].endswith("_post") else ares
            core_value = core[core_key]
            if mode and mode[0].startswith("type"):
                core_value = int(core_value) & 1
            elif mode and mode[0].startswith("channel"):
                core_value = (int(core_value) >> 1) & 3
            elif mode and mode[0].startswith("rate"):
                core_value = int(core_value) & 3
            elif mode and mode[0].startswith("enable"):
                core_value = (int(core_value) >> 2) & 1
            if int(ares_state[ares_key]) != int(core_value):
                ticks.append(tick)
        mismatch_counts[label] = len(ticks)
        first_mismatch[label] = ticks[0] if ticks else None

    output_ticks = []
    for tick, (ares, core) in enumerate(zip(rows, core_rows, strict=True)):
        core_expected = [
            core["tone_outputs_after"][channel]
            and core["attenuations_after"][channel]
            or (15 if not core["tone_outputs_after"][channel] else 0)
            for channel in range(3)
        ]
        core_expected.append(
            core["attenuations_after"][3]
            if core["noise_output_after"]
            else 15
        )
        if [int(value) for value in ares["output"]] != [int(value) for value in core_expected]:
            output_ticks.append(tick)

    tone2_rate_ticks = []
    for tick, (ares, core) in enumerate(zip(rows, core_rows, strict=True)):
        if (int(ares["noise_rate"]) == 3 and
            int(ares["noise_pitch"]) != int(core["tone_periods_before"][2])):
            tone2_rate_ticks.append(tick)

    return {
        "name": "Ares SN76489 component",
        "commit": header["source_commit"],
        "license": header["license"],
        "trace_sha256": hashlib.sha256(raw).hexdigest(),
        "clock_mapping": (
            "one Ares component clock per row compared to one Rust SMS PSG tick; "
            "the Rust tick is exactly 16 input clocks. This is a component-level "
            "mapping, not a claim about Ares host integration or hardware truth."
        ),
        "same_tick_comparison": True,
        "lfsr_raw_state_comparable": True,
        "field_mismatch_counts": mismatch_counts,
        "field_first_mismatch_ticks": first_mismatch,
        "channel_output_nibble_mismatch_count": len(output_ticks),
        "channel_output_nibble_first_mismatch_tick": output_ticks[0] if output_ticks else None,
        "noise_pitch_vs_tone2_mismatch_count": len(tone2_rate_ticks),
        "noise_pitch_vs_tone2_first_mismatch_tick": (
            tone2_rate_ticks[0] if tone2_rate_ticks else None
        ),
        "note": (
            "Ares is the Texas Instruments SN76489 component used by its SMS host. "
            "It is not a guaranteed silicon oracle for every Sega PSG revision. "
            "All mismatches are diagnostics; no automatic alignment is applied. "
            "Ares output nibbles are compared against the Rust channel polarity "
            "and attenuation state after the corresponding clock."
        ),
    }


def main() -> int:
    if len(sys.argv) == 4 and sys.argv[1] == "--ares-only":
        core_header, core_rows, _ = read_jsonl(
            Path(sys.argv[2]), "sega-sn76489-core-state-trace-v1"
        )
        ares_summary = compare_ares(Path(sys.argv[3]), core_rows, core_header)
        print(json.dumps({
            "schema": "sega-psg-ares-differential-summary-v1",
            "comparison_policy": "same tick index; no waveform alignment",
            "ticks_compared": len(core_rows),
            "ares": ares_summary,
        }, sort_keys=True, indent=2))
        return 0

    if len(sys.argv) not in (3, 4, 5):
        raise SystemExit(
            "usage: compare.py crabemu-reference.jsonl core.jsonl "
            "[gearsystem-reference.jsonl] [ares-reference.jsonl]"
        )

    ref_header, ref_rows, ref_raw = read_jsonl(
        Path(sys.argv[1]), "smsplus-crabemu-psg-trace-v1"
    )
    core_header, core_rows, core_raw = read_jsonl(
        Path(sys.argv[2]), "sega-sn76489-core-state-trace-v1"
    )

    if len(ref_rows) != len(core_rows):
        raise SystemExit(
            f"CrabEmu/core trace lengths differ: reference={len(ref_rows)} core={len(core_rows)}"
        )
    if ref_header.get("source_commit") != "3844b46caa926b6494987b97da63092818c4ddef":
        raise SystemExit("CrabEmu reference commit pin mismatch")
    if ref_header.get("noise_bits") != 16 or ref_header.get("noise_tapped") != 9:
        raise SystemExit("CrabEmu SMS noise configuration mismatch")

    state_mismatch = []
    sample_vs_core_lfsr = []
    sample_vs_core_latch = []
    ref_shift_ticks = []
    core_shift_ticks = []
    sample_bits = []

    for tick, (reference, core) in enumerate(zip(ref_rows, core_rows, strict=True)):
        ref_before = int(reference["noise_shift_before"])
        ref_after = int(reference["noise_shift_after"])
        core_before = int(core["lfsr_before"])
        core_after = int(core["lfsr_after"])
        sample_bit = int(reference["sample_l"] != 0)

        # In this trace sequence the three tone channels remain attenuated.
        # CrabEmu's raw noise sample must equal its pre-update LFSR bit 0.
        if sample_bit != int(reference["noise_bit_before"]):
            raise SystemExit(
                f"CrabEmu sample does not match its pre-update LFSR bit at tick {tick}"
            )

        if ref_before != ref_after:
            ref_shift_ticks.append(tick)
        if core_before != core_after:
            core_shift_ticks.append(tick)
        if ref_after != core_after:
            state_mismatch.append(tick)
        if sample_bit != int(core["lfsr_bit_before"]):
            sample_vs_core_lfsr.append(tick)
        if sample_bit != int(core["noise_output_after"]):
            sample_vs_core_latch.append(tick)
        sample_bits.append(sample_bit)

    summary: dict[str, Any] = {
        "schema": "sega-psg-differential-summary-v1",
        "comparison_policy": "same tick index; no waveform alignment or correlation-based shifting",
        "reference": {
            "name": "SMS Plus GX CrabEmu SN76489",
            "commit": ref_header["source_commit"],
            "license": ref_header["license"],
            "trace_sha256": hashlib.sha256(ref_raw).hexdigest(),
        },
        "core": {
            "profile": core_header["profile"],
            "trace_sha256": hashlib.sha256(core_raw).hexdigest(),
        },
        "ticks_compared": len(ref_rows),
        "reference_lfsr_shift_count": len(ref_shift_ticks),
        "core_lfsr_shift_count": len(core_shift_ticks),
        "reference_first_lfsr_shift_tick": first_index(
            [tick in ref_shift_ticks for tick in range(len(ref_rows))]
        ),
        "core_first_lfsr_shift_tick": first_index(
            [tick in core_shift_ticks for tick in range(len(core_rows))]
        ),
        "reference_lfsr_shift_ticks_first_20": ref_shift_ticks[:20],
        "core_lfsr_shift_ticks_first_20": core_shift_ticks[:20],
        "lfsr_state_mismatch_count": len(state_mismatch),
        "lfsr_state_first_mismatch_tick": first_index(
            [tick in state_mismatch for tick in range(len(ref_rows))]
        ),
        "reference_sample_vs_core_lfsr_bit_mismatch_count": len(sample_vs_core_lfsr),
        "reference_sample_vs_core_lfsr_bit_first_mismatch_tick": first_index(
            [tick in sample_vs_core_lfsr for tick in range(len(ref_rows))]
        ),
        "reference_sample_vs_core_noise_output_latch_mismatch_count": len(
            sample_vs_core_latch
        ),
        "reference_sample_vs_core_noise_output_latch_first_mismatch_tick": first_index(
            [tick in sample_vs_core_latch for tick in range(len(ref_rows))]
        ),
        "reference_sample_bit_ones": sum(sample_bits),
        "note": (
            "Differences are diagnostic, not an automatic hardware pass/fail. "
            "The CrabEmu sample is captured before its LFSR update; the Rust "
            "core exposes both current LFSR state and its latched noise_output."
        ),
    }

    if len(sys.argv) in (4, 5):
        gear_header, gear_rows, gear_raw = read_jsonl(
            Path(sys.argv[3]), "gearsystem-sms-psg-state-trace-v1"
        )
        if len(gear_rows) != len(core_rows):
            raise SystemExit(
                f"Gearsystem/core trace lengths differ: reference={len(gear_rows)} core={len(core_rows)}"
            )
        if gear_header.get("source_commit") != "1be9a12b82cae84ddd6f9b7006d546abe2d58bd4":
            raise SystemExit("Gearsystem reference commit pin mismatch")
        if gear_header.get("row_duration_tstates") != 16:
            raise SystemExit("Gearsystem trace must advance exactly 16 Z80 T-states per row")

        gear_shift_ticks = []
        gear_bit_vs_core_bit = []
        gear_bit_vs_core_latch = []
        for tick, (gear, core) in enumerate(zip(gear_rows, core_rows, strict=True)):
            if int(gear["noise_shifter_before"]) != int(gear["noise_shifter_after"]):
                gear_shift_ticks.append(tick)
            gear_bit = int(gear["noise_bit_after"])
            if gear_bit != int(core["lfsr_bit_after"]):
                gear_bit_vs_core_bit.append(tick)
            if gear_bit != int(core["noise_output_after"]):
                gear_bit_vs_core_latch.append(tick)

        summary["gearsystem"] = {
            "name": "Gearsystem Sms_Apu",
            "commit": gear_header["source_commit"],
            "license": gear_header["license"],
            "trace_sha256": hashlib.sha256(gear_raw).hexdigest(),
            "lfsr_shift_count": len(gear_shift_ticks),
            "first_lfsr_shift_tick": first_index(
                [tick in gear_shift_ticks for tick in range(len(gear_rows))]
            ),
            "lfsr_shift_ticks_first_20": gear_shift_ticks[:20],
            "galois_state_raw_values_compared": False,
            "observable_bit0_vs_core_lfsr_bit_mismatch_count": len(gear_bit_vs_core_bit),
            "observable_bit0_vs_core_lfsr_bit_first_mismatch_tick": first_index(
                [tick in gear_bit_vs_core_bit for tick in range(len(gear_rows))]
            ),
            "observable_bit0_vs_core_noise_output_latch_mismatch_count": len(
                gear_bit_vs_core_latch
            ),
            "observable_bit0_vs_core_noise_output_latch_first_mismatch_tick": first_index(
                [tick in gear_bit_vs_core_latch for tick in range(len(gear_rows))]
            ),
            "note": (
                "Gearsystem uses a Galois-form LFSR. Raw state integers are not "
                "compared to the core's Fibonacci-form register; only observable "
                "bit-0 state and timestamped shift events are compared."
            ),
        }

    if len(sys.argv) == 5:
        summary["ares"] = compare_ares(Path(sys.argv[4]), core_rows, core_header)


    print(json.dumps(summary, sort_keys=True, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
