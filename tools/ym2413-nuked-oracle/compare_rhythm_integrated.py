#!/usr/bin/env python3
"""Characterize seeded Rust Rhythm Mode state against the pinned Nuked oracle."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

from compare_fm3_integrated import ARRAY_FIELDS, SCALAR_FIELDS, load_jsonl


RHYTHM_OUTPUT_FIELDS = ("ch_out_hh", "ch_out_tm", "ch_out_bd", "ch_out_sd", "ch_out_tc")
OPERATOR_INPUT_ARRAY_FIELDS = ("op_calc_op_fb1", "op_calc_op_fb2")
HARD_SCALAR_FIELDS = (
    "cycle_before",
    "cycle_after",
    "lfo_vib_counter",
    "lfo_counter",
    "lfo_am_counter",
    "lfo_am_dir",
    "lfo_am_car",
    "lfo_am_step",
    "lfo_am_out",
    "eg_counter_state",
    "eg_counter_state_prev",
    "eg_timer",
    "eg_timer_carry",
    "eg_timer_low_lock",
    "eg_timer_shift",
    "eg_timer_shift_lock",
    "eg_timer_shift_stop",
    "eg_kon",
    "eg_dokon",
    "c_instr",
    "c_op",
    "c_fnum",
    "c_block",
    "c_ksr_freq",
    "c_ksl_freq",
    "c_ksl_block",
)
RHYTHM_HARD_FIELDS = (
    "rm_enable",
    "op_calc_rm_enable",
    "rm_select",
    "rhythm",
    "rm_noise",
)


def compare_hard_state(
    call_index: int, nuked: dict[str, object], rust: dict[str, object]
) -> None:
    """Fail on serial control, slot, selector, LFSR or global timing divergence."""
    for field in HARD_SCALAR_FIELDS:
        expected = int(nuked[field])
        actual = int(rust[field])
        if field == "lfo_counter":
            expected &= 0x1FFF
            actual &= 0x1FFF
        if expected != actual:
            raise ValueError(
                f"call {call_index}: hard field {field} mismatch "
                f"Nuked={expected} Rust={actual}"
            )

    pairs = (
        ("rm_enable", "rm_enable"),
        ("op_calc_rm_enable", "op_calc_rm_enable"),
        ("rm_select", "rm_select"),
        ("rhythm", "rhythm_control"),
        ("rm_noise", "rhythm_noise_register"),
    )
    for expected_field, actual_field in pairs:
        expected = int(nuked[expected_field])
        actual = int(rust[actual_field])
        if expected != actual:
            raise ValueError(
                f"call {call_index}: hard field {expected_field}/{actual_field} "
                f"mismatch Nuked={expected} Rust={actual}"
            )

    expected_enabled = int(bool(int(nuked["rhythm"]) & 0x20))
    actual_enabled = int(rust["rhythm_mode_enabled"])
    if expected_enabled != actual_enabled:
        raise ValueError(
            f"call {call_index}: Rhythm Mode enable bit mismatch "
            f"Nuked={expected_enabled} Rust={actual_enabled}"
        )


def main() -> int:
    """Check hard state and report, rather than hide, candidate-patch divergences."""
    parser = argparse.ArgumentParser()
    parser.add_argument("--nuked-trace", required=True, type=Path)
    parser.add_argument("--rust-trace", required=True, type=Path)
    args = parser.parse_args()

    nuked_rows = load_jsonl(args.nuked_trace, "Nuked")
    rust_rows = load_jsonl(args.rust_trace, "Rust")
    if nuked_rows[0].get("schema") != "ym2413-nuked-opll-trace-v1":
        raise ValueError("Nuked input must be a full ym2413-nuked-opll-trace-v1 trace")
    nuked_samples = nuked_rows[1:]
    if len(nuked_samples) != len(rust_rows):
        raise ValueError(
            f"trace length mismatch: Nuked={len(nuked_samples)} Rust={len(rust_rows)}"
        )
    if len(nuked_samples) != 147456:
        raise ValueError(f"expected 147456 calls, got {len(nuked_samples)}")

    rust_fields = (
        *HARD_SCALAR_FIELDS,
        "rm_enable",
        "op_calc_rm_enable",
        "rm_select",
        "rhythm_control",
        "rhythm_mode_enabled",
        "rhythm_noise_register",
        "pg_increment",
        "pg_output",
        "c_ksl_attenuation",
        *OPERATOR_INPUT_ARRAY_FIELDS,
        *RHYTHM_OUTPUT_FIELDS,
    )
    nuked_fields = (
        *HARD_SCALAR_FIELDS,
        "rm_enable",
        "op_calc_rm_enable",
        "rm_select",
        "rhythm",
        "rm_noise",
        *OPERATOR_INPUT_ARRAY_FIELDS,
        *RHYTHM_OUTPUT_FIELDS,
    )

    candidate_fields = tuple(
        field
        for field in (*SCALAR_FIELDS, *ARRAY_FIELDS, *OPERATOR_INPUT_ARRAY_FIELDS, *RHYTHM_OUTPUT_FIELDS)
        if field not in HARD_SCALAR_FIELDS and field not in RHYTHM_HARD_FIELDS
    )
    candidate_differences: dict[str, dict[str, object]] = {}
    cycles: set[int] = set()
    modes: set[int] = set()
    for index, (nuked, rust) in enumerate(zip(nuked_samples, rust_rows, strict=True)):
        if int(nuked["call_index"]) != index or int(rust["call_index"]) != index:
            raise ValueError(f"call sequence discontinuity at row {index}")
        missing = [field for field in (*nuked_fields, *candidate_fields) if field not in nuked]
        missing.extend(field for field in (*rust_fields, *candidate_fields) if field not in rust)
        if missing:
            raise ValueError(f"call {index}: missing required fields {missing}")

        compare_hard_state(index, nuked, rust)
        cycles.add(int(nuked["cycle_before"]))
        modes.add(int(nuked["op_calc_rm_enable"]))

        row_candidate_values = {
            field: (nuked[field], rust[field]) for field in candidate_fields
        }
        row_candidate_values["pg_increment_alias"] = (
            nuked["pg_inc"], rust["pg_increment"]
        )
        row_candidate_values["pg_output_alias"] = (nuked["pg_out"], rust["pg_output"])
        row_candidate_values["c_ksl_attenuation"] = (
            int(nuked["eg_ksltl"]) - (int(nuked["c_tl"]) << 1),
            int(rust["c_ksl_attenuation"]),
        )
        for field, (expected, actual) in row_candidate_values.items():
            if expected != actual:
                record = candidate_differences.setdefault(
                    field,
                    {
                        "rows_different": 0,
                        "first_call": index,
                        "first_nuked": expected,
                        "first_rust": actual,
                    },
                )
                record["rows_different"] = int(record["rows_different"]) + 1

    if cycles != set(range(18)):
        raise ValueError("Nuked trace does not cover all eighteen serial cycles")
    for bit in (0x40, 0x80):
        if not any(mode & bit for mode in modes):
            raise ValueError(f"trace never exercises effective rm_enable bit {bit:#04x}")

    for field in RHYTHM_OUTPUT_FIELDS:
        if not any(int(row[field]) != 0 for row in nuked_samples):
            raise ValueError(f"Nuked trace never produces nonzero {field}")
        if not any(int(row[field]) != 0 for row in rust_rows):
            raise ValueError(f"Rust trace never produces nonzero {field}")

    if not all(int(row["rhythm_noise_register"]) >= 0 for row in rust_rows):
        raise ValueError("Rust trace did not retain the explicit zero-seed LFSR candidate")

    print(
        json.dumps(
            {
                "status": (
                    "PASS"
                    if not candidate_differences
                    else "CHARACTERIZED_WITH_PATCH_CANDIDATE_DIVERGENCE"
                ),
                "compared_calls": len(nuked_samples),
                "serial_cycles": sorted(cycles),
                "noise_seed": 0,
                "noise_seed_interpretation": (
                    "Nuked software reset candidate only; not physical silicon seed"
                ),
                "hard_state_fields": [
                    *HARD_SCALAR_FIELDS,
                    *RHYTHM_HARD_FIELDS,
                    "rhythm_mode_enabled",
                ],
                "candidate_fields_compared": [*candidate_fields, "pg_increment_alias", "pg_output_alias", "c_ksl_attenuation"],
                "candidate_patch_differences": candidate_differences,
                "scope": (
                    "strict serial/control/LFSR/timing comparison; synthesis differences "
                    "are reported because rhythm patch data is not hardware-certified"
                ),
                "limitations": [
                    "candidate-dependent synthesis differences are characterized, not certified",
                    "does not validate operator-to-raw-DAC conversion or analog MO/RO voltage",
                ],
            },
            indent=2,
        )
    )
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, ValueError, json.JSONDecodeError) as error:
        print(f"YM2413 integrated Rhythm Mode characterization failed: {error}", file=sys.stderr)
        raise SystemExit(2)
