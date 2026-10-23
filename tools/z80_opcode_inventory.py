"""Static/dynamic smoke inventory for the Z80 opcode spaces.

Run from the repository root:
    python tools/z80_opcode_inventory.py
    python tools/z80_opcode_inventory.py --json

Each encoding is executed once from a deterministic, isolated CPU state. This
is a dispatch smoke test, not a semantic, timing, or hardware-accuracy test.
"""

from __future__ import annotations

import argparse
import json
from collections import Counter
from dataclasses import asdict, dataclass
from typing import Any

from sega_emulator.cpu.z80 import UnsupportedOpcodeError, Z80Cpu


class InventoryMemory:
    """Simple 64 KiB memory used only to safely exercise instruction dispatch."""

    def __init__(self, program: bytes) -> None:
        self.data = bytearray(0x10000)
        self.data[: len(program)] = program

    def read(self, address: int) -> int:
        return self.data[address & 0xFFFF]

    def write(self, address: int, value: int) -> None:
        self.data[address & 0xFFFF] = value & 0xFF


class InventoryIO:
    """Deterministic I/O bus that returns zero and accepts all writes."""

    def read_port(self, address: int) -> int:
        del address
        return 0

    def write_port(self, address: int, value: int) -> None:
        del address, value


@dataclass(frozen=True)
class Result:
    space: str
    opcode: str
    status: str
    cycles: int | None
    detail: str | None


def _program_for(space: str, opcode: int) -> bytes:
    if space == "base":
        return bytes((opcode, 0, 0, 0))
    if space in ("CB", "ED"):
        return bytes((0xCB if space == "CB" else 0xED, opcode, 0, 0))
    if space in ("DD", "FD"):
        prefix = 0xDD if space == "DD" else 0xFD
        if opcode == 0xCB:
            # DD/FD-CB encodings are prefix, CB, signed displacement, operation.
            return bytes((prefix, 0xCB, 0, 0))
        return bytes((prefix, opcode, 0, 0, 0))
    if space in ("DD-CB", "FD-CB"):
        prefix = 0xDD if space == "DD-CB" else 0xFD
        return bytes((prefix, 0xCB, 0, opcode))
    raise ValueError(f"Unknown opcode space: {space}")


def _run_one(space: str, opcode: int) -> Result:
    memory = InventoryMemory(_program_for(space, opcode))
    cpu = Z80Cpu(memory, io_bus=InventoryIO())
    # Keep indirect accesses and stack activity deterministic and away from code.
    cpu.sp = 0x8000
    cpu.hl = 0x4000
    cpu.bc = 0x4100
    cpu.de = 0x4200
    cpu.ix = 0x4300
    cpu.iy = 0x4400

    try:
        cycles = cpu.step()
    except UnsupportedOpcodeError as exc:
        return Result(space, f"{opcode:02X}", "unsupported", None, str(exc))
    # Expose unexpected dispatch/runtime errors instead of hiding them.
    except Exception as exc:
        return Result(
            space,
            f"{opcode:02X}",
            "unexpected_error",
            None,
            f"{type(exc).__name__}: {exc}",
        )
    return Result(space, f"{opcode:02X}", "executed", cycles, None)


def build_inventory() -> list[Result]:
    """Execute every byte in each supported dispatch space exactly once."""
    results: list[Result] = []
    spaces = ("base", "CB", "ED", "DD", "FD", "DD-CB", "FD-CB")
    for space in spaces:
        results.extend(_run_one(space, opcode) for opcode in range(0x100))
    return results


def _summary(results: list[Result]) -> dict[str, Any]:
    by_space: dict[str, dict[str, int]] = {}
    for space in dict.fromkeys(result.space for result in results):
        counts = Counter(
            result.status for result in results if result.space == space
        )
        by_space[space] = {
            "total": sum(counts.values()),
            "executed": counts["executed"],
            "unsupported": counts["unsupported"],
            "unexpected_error": counts["unexpected_error"],
        }
    return {
        "scope": "single-step opcode dispatch smoke inventory",
        "warning": (
            "Executed means one controlled step returned normally; it does not "
            "prove semantic correctness, timing accuracy, or hardware equivalence."
        ),
        "by_space": by_space,
        "results": [asdict(result) for result in results],
        "unexpected_errors": [
            asdict(result)
            for result in results
            if result.status == "unexpected_error"
        ],
        "unsupported": [
            asdict(result) for result in results if result.status == "unsupported"
        ],
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--json",
        action="store_true",
        help="print the full machine-readable inventory as JSON",
    )
    args = parser.parse_args()
    results = build_inventory()
    summary = _summary(results)

    if args.json:
        print(json.dumps(summary, indent=2, ensure_ascii=False))
    else:
        print("Z80 opcode dispatch inventory")
        print("Status: executed=normal return, unsupported=UnsupportedOpcodeError")
        print("        unexpected_error=other exception (investigate)")
        print()
        for space, counts in summary["by_space"].items():
            print(
                f"{space:7} total={counts['total']:3} "
                f"executed={counts['executed']:3} "
                f"unsupported={counts['unsupported']:3} "
                f"unexpected_error={counts['unexpected_error']:3}"
            )
        if summary["unsupported"]:
            print("\nUnsupported encodings:")
            for item in summary["unsupported"]:
                print(f"  {item['space']} {item['opcode']}: {item['detail']}")
        if summary["unexpected_errors"]:
            print("\nUnexpected errors:")
            for item in summary["unexpected_errors"]:
                print(f"  {item['space']} {item['opcode']}: {item['detail']}")
        print("\nReminder: dispatch coverage is not correctness coverage.")

    return 1 if summary["unexpected_errors"] else 0


if __name__ == "__main__":
    raise SystemExit(main())
