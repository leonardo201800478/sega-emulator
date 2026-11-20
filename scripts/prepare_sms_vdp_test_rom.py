#!/usr/bin/env python3
"""Prepare the pinned upstream SmsVdpTest ROM for unattended conformance runs.

The upstream source is not modified in place by the repository. This script is
run against a temporary CI clone and changes only its boot/menu flow:
- enter the normal WarmReset initialization instead of the register-dump screen;
- skip the interactive menu and the pause between test groups 1 and 2;
- isolate RunTest25 scratch samples at an unused RAM address without adding cycles;
- stop immediately after RunTest34 and publish a RAM completion marker.
"""

from __future__ import annotations

import argparse
import re
from pathlib import Path


def replace_once(source: str, old: str, new: str, label: str) -> str:
    """Replace one required source fragment and fail on source drift."""
    count = source.count(old)
    if count != 1:
        raise SystemExit(
            f"Expected exactly one occurrence for {label!r}, found {count}"
        )
    return source.replace(old, new, 1)


def prepare(source_path: Path) -> None:
    """Patch the upstream assembly entry flow for headless RunTest01–34."""
    source = source_path.read_text(encoding="utf-8").replace("\r\n", "\n")

    source = replace_once(
        source,
        "jp DumpAllRegisters",
        "jp WarmReset",
        "boot entry",
    )
    scratch_define, count = re.subn(
        r"(?m)^\.define TestScratch\s+\$c880$",
        ".define TestScratch $c880\n.define TestScratch25 $c9f0",
        source,
        count=1,
    )
    if count != 1:
        raise SystemExit(
            f"Expected exactly one TestScratch definition, found {count}"
        )
    source = scratch_define

    test25_start = source.index("RunTest25:")
    test26_start = source.index("RunTest25_MD:", test25_start)
    test25_block = source[test25_start:test26_start]
    if test25_block.count("TestScratch") != 5:
        raise SystemExit("RunTest25 scratch references changed; refusing to instrument timing")
    source = (
        source[:test25_start]
        + test25_block.replace("TestScratch", "TestScratch25")
        + source[test26_start:]
    )

    source = replace_once(
        source,
        "call WaitForNoButton",
        "nop\n\tnop\n\tnop",
        "startup input wait",
    )
    source = replace_once(
        source,
        "MenuStart:\n\tld a,$00",
        "MenuStart:\n\tjp StartSMSTest\n\tld a,$00",
        "interactive menu bypass",
    )
    source = replace_once(
        source,
        "call WaitForButton\n;==============================================================\nSMSTest2:",
        "jp SMSTest2\n;==============================================================\nSMSTest2:",
        "pause between SMS test groups",
    )

    sms_test2 = source.index("SMSTest2:")
    run_test35 = source.index("call RunTest35", sms_test2)
    marker_and_halt = "ld a,$AA\n\tld ($C9FF),a\n\thalt"
    source = (
        source[:run_test35]
        + marker_and_halt
        + source[run_test35 + len("call RunTest35") :]
    )

    source_path.write_text(source, encoding="utf-8", newline="\n")


def main() -> None:
    """Parse the upstream assembly path and apply deterministic test patches."""
    parser = argparse.ArgumentParser()
    parser.add_argument("assembly", type=Path)
    args = parser.parse_args()
    prepare(args.assembly)


if __name__ == "__main__":
    main()
