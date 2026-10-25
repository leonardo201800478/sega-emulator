"""Exhaustive differential checks of the Rust ALU against the Python reference."""

from __future__ import annotations

import subprocess
from pathlib import Path

from sega_emulator.cpu.z80 import Z80Cpu

ROOT = Path(__file__).resolve().parents[2]
MANIFEST = ROOT / "native" / "z80_alu" / "Cargo.toml"


class _MemoryBus:
    def __init__(self) -> None:
        self.memory = bytearray(0x10000)

    def read(self, address: int) -> int:
        return self.memory[address & 0xFFFF]

    def write(self, address: int, value: int) -> None:
        self.memory[address & 0xFFFF] = value & 0xFF


def test_rust_add_sub_adc_sbc_match_python_for_all_8_bit_operands() -> None:
    operations = ("A", "S", "D0", "D1", "B0", "B1")
    vectors = [
        (operation, lhs, rhs)
        for operation in operations
        for lhs in range(256)
        for rhs in range(256)
    ]
    input_text = "".join(
        f"{operation} {lhs:02X} {rhs:02X}\n"
        for operation, lhs, rhs in vectors
    )
    completed = subprocess.run(
        [
            "cargo",
            "run",
            "--quiet",
            "--release",
            "--manifest-path",
            str(MANIFEST),
            "--bin",
            "z80-alu-vectors",
        ],
        cwd=ROOT,
        input=input_text,
        text=True,
        capture_output=True,
        check=True,
        timeout=180,
    )
    actual_lines = completed.stdout.splitlines()
    assert len(actual_lines) == len(vectors)

    cpu = Z80Cpu(_MemoryBus())
    for index, (operation, lhs, rhs) in enumerate(vectors):
        cpu.a = lhs
        cpu.f = 1 if operation.endswith("1") else 0
        if operation == "A":
            cpu._add_a(rhs)
        elif operation == "S":
            cpu._sub_a(rhs)
        elif operation.startswith("D"):
            cpu._adc_a(rhs)
        else:
            cpu._sbc_a(rhs)

        rust_value, rust_flags = (
            int(part, 16) for part in actual_lines[index].split()
        )
        assert (cpu.a, cpu.f) == (rust_value, rust_flags), (
            f"operation={operation} lhs=0x{lhs:02X} rhs=0x{rhs:02X}: "
            f"Python={(cpu.a, cpu.f)!r}, Rust={(rust_value, rust_flags)!r}"
        )
