#!/usr/bin/env python3
"""Execute all four SMS pacing ROM profiles twice through the full-machine runner."""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
from pathlib import Path

PROFILES = ("cpu-light", "cpu-fixed", "vdp-sprites", "combined")
SUITE_REVISION = "pacing-rom-v1"


def parse_args() -> argparse.Namespace:
    """Parse build output, report destination, and bounded run length."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--rom-dir", required=True, type=Path)
    parser.add_argument("--report-dir", required=True, type=Path)
    parser.add_argument("--frames", type=int, default=600)
    parser.add_argument("--max-instructions-per-frame", type=int, default=100_000)
    return parser.parse_args()


def run_once(
    wrapper: Path,
    rom: Path,
    profile: str,
    frames: int,
    instruction_limit: int,
) -> tuple[list[dict[str, object]], str]:
    """Run one ROM snapshot through SmsTemporalMachine and decode its JSONL."""
    command = [
        sys.executable,
        str(wrapper),
        "--rom",
        str(rom),
        "--suite-id",
        f"pacing-{profile}",
        "--suite-revision",
        SUITE_REVISION,
        "--region",
        "usa",
        "--frames",
        str(frames),
        "--max-instructions-per-frame",
        str(instruction_limit),
    ]
    result = subprocess.run(command, check=False, capture_output=True, text=True)
    if result.returncode != 0:
        raise RuntimeError(
            f"{profile}: runner exited {result.returncode}\n"
            f"stdout:\n{result.stdout}\nstderr:\n{result.stderr}"
        )

    records: list[dict[str, object]] = []
    for line_number, line in enumerate(result.stdout.splitlines(), start=1):
        try:
            record = json.loads(line)
        except json.JSONDecodeError as error:
            raise RuntimeError(
                f"{profile}: invalid JSONL at stdout line {line_number}: {line!r}"
            ) from error
        if not isinstance(record, dict):
            raise RuntimeError(f"{profile}: JSONL line {line_number} is not an object")
        records.append(record)

    return records, result.stderr


def validate_run(
    records: list[dict[str, object]], profile: str, expected_frames: int
) -> None:
    """Require a complete bounded execution, without treating it as a test-suite PASS."""
    starts = [record for record in records if record.get("record") == "start"]
    frames = [record for record in records if record.get("record") == "frame"]
    summaries = [record for record in records if record.get("record") == "summary"]

    if len(starts) != 1 or len(summaries) != 1:
        raise RuntimeError(f"{profile}: expected exactly one start and summary record")
    if len(frames) != expected_frames:
        raise RuntimeError(
            f"{profile}: expected {expected_frames} frame records, got {len(frames)}"
        )

    summary = summaries[0]
    if summary.get("status") != "EXECUTED_NO_ORACLE":
        raise RuntimeError(f"{profile}: unexpected runner status: {summary!r}")
    if summary.get("frames_completed") != expected_frames:
        raise RuntimeError(f"{profile}: incomplete frame budget: {summary!r}")
    if not isinstance(summary.get("total_instructions"), int) or summary[
        "total_instructions"
    ] <= 0:
        raise RuntimeError(f"{profile}: no CPU instructions were reported")
    if not isinstance(summary.get("total_audio_samples_drained"), int):
        raise RuntimeError(f"{profile}: missing PSG sample accounting")


def main() -> int:
    """Run each workload twice, compare exact JSONL, and retain reports."""
    args = parse_args()
    if args.frames <= 0 or args.max_instructions_per_frame <= 0:
        print("frame and instruction limits must be greater than zero", file=sys.stderr)
        return 64

    repository_root = Path(__file__).resolve().parents[3]
    wrapper = repository_root / "tests" / "sms" / "scripts" / "run_rom.py"
    rom_dir = args.rom_dir.resolve()
    report_dir = args.report_dir.resolve()
    report_dir.mkdir(parents=True, exist_ok=True)

    try:
        for profile in PROFILES:
            rom = rom_dir / f"{profile}.sms"
            if not rom.is_file():
                raise RuntimeError(f"missing profile ROM: {rom}")

            outputs: list[list[dict[str, object]]] = []
            for repetition in (1, 2):
                records, stderr = run_once(
                    wrapper,
                    rom,
                    profile,
                    args.frames,
                    args.max_instructions_per_frame,
                )
                validate_run(records, profile, args.frames)
                output_path = report_dir / f"{profile}-run-{repetition}.jsonl"
                output_path.write_text(
                    "".join(json.dumps(record, separators=(",", ":")) + "\n" for record in records),
                    encoding="utf-8",
                )
                (report_dir / f"{profile}-run-{repetition}.stderr.log").write_text(
                    stderr, encoding="utf-8"
                )
                outputs.append(records)

            if outputs[0] != outputs[1]:
                raise RuntimeError(
                    f"{profile}: repeated full-machine executions produced different JSONL"
                )

            summary = next(
                record for record in outputs[0] if record.get("record") == "summary"
            )
            print(
                f"{profile}: {args.frames} frames x2; "
                f"instructions={summary['total_instructions']}; "
                f"audio_samples={summary['total_audio_samples_drained']}; "
                "deterministic JSONL=identical; status=EXECUTED_NO_ORACLE"
            )
    except (OSError, RuntimeError) as error:
        print(str(error), file=sys.stderr)
        return 1

    print(f"All four pacing ROM profiles completed; reports: {report_dir}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
