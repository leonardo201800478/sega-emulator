#!/usr/bin/env python3
"""Compare the Rust serial operator stage with pinned Nuked OPLL_Operator state.

The trace adapter records the exact pre-call latches consumed by Nuked's
OPLL_Operator. This comparator feeds those latches to the production Rust
serial operator and compares every internal operator/feedback latch after the
call. It intentionally isolates the operator datapath from EG/PG generation.
"""

from __future__ import annotations

import argparse
import csv
import json
import subprocess
import sys
import tempfile
from pathlib import Path


INPUT_FIELDS = (
    "call_index",
    "cycle_before",
    "op_calc_pg_out",
    "op_calc_eg_out",
    "op_calc_eg_silent",
    "op_calc_c_fb",
    "op_calc_c_dc",
    "op_calc_c_dm",
)
OUTPUT_FIELDS = (
    "op_exp_m",
    "op_exp_s",
    "op_logsin",
    "op_neg",
    "op_mod",
    "op_fbsum",
    "op_fb1",
    "op_fb2",
    "ch_out",
    "op_phase_index",
    "op_output",
    "ch_out_hh",
    "ch_out_tm",
    "ch_out_bd",
    "ch_out_sd",
    "ch_out_tc",
)


def load_trace(path: Path) -> list[dict[str, object]]:
    """Load a full Nuked trace and require the pre-call operator latches."""
    with path.open(encoding="utf-8") as stream:
        rows = [json.loads(line) for line in stream if line.strip()]
    if len(rows) < 2 or rows[0].get("schema") != "ym2413-nuked-opll-trace-v1":
        raise ValueError("input must be a full ym2413-nuked-opll-trace-v1 trace")
    if rows[0].get("time_unit") != "opll_clock_call":
        raise ValueError("trace time unit must be opll_clock_call")

    samples = rows[1:]
    for row in samples:
        required = (*INPUT_FIELDS, *OUTPUT_FIELDS, "op_calc_rm_enable")
        missing = [field for field in required if field not in row]
        if missing:
            raise ValueError(f"call {row.get('call_index')}: missing {missing}")
    if {int(row["cycle_before"]) for row in samples} != set(range(18)):
        raise ValueError("trace must cover all eighteen serial operator cycles")
    return samples


def main() -> int:
    """Replay every Nuked operator call through the production Rust datapath."""
    parser = argparse.ArgumentParser()
    parser.add_argument("--nuked-trace", required=True, type=Path)
    parser.add_argument("--repo-root", type=Path, default=Path(__file__).resolve().parents[2])
    parser.add_argument("--require-rhythm", action="store_true")
    args = parser.parse_args()

    samples = load_trace(args.nuked_trace)
    rhythm_modes = {int(row["op_calc_rm_enable"]) for row in samples}
    if args.require_rhythm:
        if not any(mode & 0x40 for mode in rhythm_modes):
            raise ValueError("trace never exercises rm_enable bit 6 at OPLL_Operator")
        if not any(mode & 0x80 for mode in rhythm_modes):
            raise ValueError("trace never exercises rm_enable bit 7 at OPLL_Operator")
        for field in ("ch_out_hh", "ch_out_tm", "ch_out_bd", "ch_out_sd", "ch_out_tc"):
            if not any(int(row[field]) != 0 for row in samples):
                raise ValueError(f"rhythm trace never produces nonzero {field}")
    feedback_values = {int(row["op_calc_c_fb"]) for row in samples}
    waveform_values = {
        (int(row["op_calc_c_dc"]), int(row["op_calc_c_dm"])) for row in samples
    }
    active_eg_calls = sum(
        int(row["op_calc_eg_out"]) > 0 and int(row["op_calc_eg_silent"]) == 0
        for row in samples
    )
    if not args.require_rhythm:
        if not any(feedback_values):
            raise ValueError("trace never exercises nonzero feedback")
        if not any(dc & 0x04 or dm & 0x04 for dc, dm in waveform_values):
            raise ValueError("trace never exercises a delayed DC/DM waveform bit")
    if active_eg_calls == 0:
        raise ValueError("trace never exercises a non-silent nonzero EG output")

    with tempfile.TemporaryDirectory(prefix="ym2413-operator-diff-") as temp_dir:
        input_csv = Path(temp_dir) / "rust_operator_inputs.csv"
        with input_csv.open("w", newline="", encoding="utf-8") as stream:
            writer = csv.writer(stream)
            for row in samples:
                writer.writerow(
                    (
                        *(row[field] for field in INPUT_FIELDS),
                        row["op_exp_m"],
                        row["op_exp_s"],
                        row["op_logsin"],
                        row["op_neg"],
                        row["op_mod"],
                        row["op_fbsum"],
                        *row["op_fb1"],
                        *row["op_fb2"],
                        row["ch_out"],
                        row["op_phase_index"],
                        row["op_output"],
                        row["op_calc_rm_enable"],
                        row["ch_out_hh"],
                        row["ch_out_tm"],
                        row["ch_out_bd"],
                        row["ch_out_sd"],
                        row["ch_out_tc"],
                    )
                )

        command = [
            "cargo",
            "run",
            "--quiet",
            "--locked",
            "--manifest-path",
            str(args.repo_root / "native/ym2413/Cargo.toml"),
            "--example",
            "serial_operator_trace",
            "--",
            str(input_csv),
        ]
        completed = subprocess.run(
            command,
            cwd=args.repo_root,
            check=False,
            capture_output=True,
            text=True,
        )
        if completed.returncode != 0:
            raise ValueError("Rust serial operator replay failed:\n" + completed.stderr[-8000:])
        result = json.loads(completed.stdout.strip().splitlines()[-1])

    if result.get("status") != "PASS" or int(result.get("compared_calls", -1)) != len(samples):
        raise ValueError(f"unexpected Rust operator comparator result: {result}")

    print(
        json.dumps(
            {
                "status": "PASS",
                "compared_calls": len(samples),
                "serial_cycles": sorted({int(row["cycle_before"]) for row in samples}),
                "feedback_values": sorted(feedback_values),
                "waveform_latch_states": len(waveform_values),
                "non_silent_eg_input_calls": active_eg_calls,
                "compared_fields": [
                    "op_exp_m",
                    "op_exp_s",
                    "op_logsin",
                    "op_neg",
                    "op_mod",
                    "op_fbsum",
                    "op_fb1[9]",
                    "op_fb2[9]",
                    "ch_out",
                    "op_phase_index",
                    "op_output",
                    "rhythm_channel_outputs[HH,TOM,BD,SD,TC]",
                ],
                "scope": "Nuked OPLL_Operator stage with oracle-supplied pre-call latches and post-call rhythm output latches; not integrated EG/PG validation",
            },
            indent=2,
        )
    )
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, ValueError, subprocess.CalledProcessError, json.JSONDecodeError) as error:
        print(f"YM2413 operator differential failed: {error}", file=sys.stderr)
        raise SystemExit(2)
