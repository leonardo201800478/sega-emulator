"""Integration checks for the PyO3 adapter to the standalone Rust core."""

from __future__ import annotations

import pytest

native_core = pytest.importorskip("sega_z80_core")


def test_standalone_core_adapter_executes_rom_and_exposes_state() -> None:
    rom = bytes((0x3E, 0x42, 0x21, 0x00, 0xC0, 0x36, 0x99, 0x76))
    machine = native_core.StandaloneSG1000Core(rom)

    assert machine.pc == 0
    assert machine.run_instructions(4) == 31
    assert machine.a == 0x42
    assert machine.hl == 0xC000
    assert machine.read_memory(0xC000) == 0x99
    assert machine.halted is True
    assert machine.total_tstates == 31


def test_standalone_core_adapter_rejects_invalid_rom() -> None:
    with pytest.raises(ValueError):
        native_core.StandaloneSG1000Core(b"")


def test_standalone_core_adapter_rejects_negative_instruction_count() -> None:
    machine = native_core.StandaloneSG1000Core(b"\x00")
    with pytest.raises(ValueError, match="non-negative"):
        machine.run_instructions(-1)
