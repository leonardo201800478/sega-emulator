#!/usr/bin/env python3
"""Convert the pinned Nuked trace into inputs and expected outputs for Rust replay."""

from __future__ import annotations

import argparse
import csv
import json
from pathlib import Path


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--trace", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()

    calls = 0
    qualified_writes: list[int] = []
    nonzero_channel_outputs = 0

    with args.trace.open(encoding="utf-8") as source, args.output.open(
        "w", encoding="utf-8", newline=""
    ) as destination:
        writer = csv.writer(destination)
        writer.writerow(
            [
                "call_index",
                "cycle_before",
                "ch_out_before",
                "ch_out_after",
                "mode_write_data",
                "buffer_m",
                "buffer_r",
                "pin_m_after",
                "pin_r_after",
                "rm_enable_signed_before",
                "rm_enable_signed_after",
            ]
        )

        schema = json.loads(next(source))
        if schema.get("schema") != "ym2413-nuked-opll-trace-v1":
            raise SystemExit(f"unexpected Nuked trace schema: {schema.get('schema')}")

        for line_number, line in enumerate(source, start=2):
            record = json.loads(line)
            call_index = int(record["call_index"])
            cycle = int(record["cycle_before"])
            if call_index != calls or cycle != calls % 18:
                raise SystemExit(
                    f"non-contiguous Nuked trace at line {line_number}: "
                    f"call={call_index}, cycle={cycle}, expected={calls}/{calls % 18}"
                )

            mode_address = int(record["output_pipe_mode_write_address_before"])
            write_d_en = int(record["output_pipe_write_d_en_before"])
            write_data = int(record["output_pipe_write_data_before"])
            is_qualified_rhythm_write = (
                (mode_address & 0x10) != 0
                and (mode_address & 0x0F) == 0x0E
                and write_d_en != 0
            )
            mode_write = str(write_data) if is_qualified_rhythm_write else ""
            if is_qualified_rhythm_write:
                qualified_writes.append(write_data)

            channel_output = int(record["ch_out"])
            if channel_output != 0:
                nonzero_channel_outputs += 1

            rm_enable_before = int(record["output_pipe_rm_enable_before"])
            rm_enable_raw_after = int(record["rm_enable"]) & 0xFF
            rm_enable_after = (
                rm_enable_raw_after
                if rm_enable_raw_after < 0x80
                else rm_enable_raw_after - 0x100
            )

            writer.writerow(
                [
                    call_index,
                    cycle,
                    int(record["output_pipe_ch_out_before"]),
                    channel_output,
                    mode_write,
                    int(record["output_m"]),
                    int(record["output_r"]),
                    int(record["output_pins_m"]),
                    int(record["output_pins_r"]),
                    rm_enable_before,
                    rm_enable_after,
                ]
            )
            calls += 1

    if calls == 0:
        raise SystemExit("Nuked trace contains no clock samples")
    if 0x3F not in qualified_writes or 0x00 not in qualified_writes:
        raise SystemExit(
            "trace did not exercise qualified register-0x0e enable and disable writes: "
            f"{qualified_writes}"
        )

    print(
        json.dumps(
            {
                "schema": "ym2413-output-pipeline-input-v1",
                "calls": calls,
                "qualified_rhythm_writes": qualified_writes,
                "nonzero_channel_outputs": nonzero_channel_outputs,
                "csv": str(args.output),
            },
            sort_keys=True,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
