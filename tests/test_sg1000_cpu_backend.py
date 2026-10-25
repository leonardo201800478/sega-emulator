"""Tests for selectable SG-1000 CPU backends."""

from __future__ import annotations

import sys

import pytest

from sega_emulator.cpu.z80 import Z80Cpu
from sega_emulator.machine.sg1000 import SG1000Machine


def test_python_cpu_remains_the_safe_default() -> None:
    machine = SG1000Machine(b"\x00")
    assert machine.cpu_core == "python"
    assert isinstance(machine.cpu, Z80Cpu)


def test_unknown_cpu_backend_is_rejected() -> None:
    with pytest.raises(ValueError, match="cpu_core"):
        SG1000Machine(b"\x00", cpu_core="native")


def test_rust_backend_reports_missing_extension_clearly(monkeypatch) -> None:
    monkeypatch.setitem(sys.modules, "sega_z80_core", None)
    with pytest.raises(RuntimeError, match="Rust experimental"):
        SG1000Machine(b"\x00", cpu_core="rust")


def test_rust_backend_executes_instructions_in_native_cpu_instance() -> None:
    rust_module = pytest.importorskip(
        "sega_z80_core",
        reason="Rust extension is optional outside the Rust-core CI workflow",
    )
    from sega_z80_core import RustZ80Cpu

    # LD A,12h; INC A; NOP. Assert architectural effects, not just selection metadata.
    machine = SG1000Machine(b"\x3e\x12\x3c\x00", cpu_core="rust")
    assert machine.cpu_core == "rust"
    assert isinstance(machine.cpu, RustZ80Cpu)
    assert machine.cpu.__class__.__module__ == rust_module.__name__

    assert machine.step() == 7
    assert machine.cpu.a == 0x12
    assert machine.cpu.pc == 2

    assert machine.step() == 4
    assert machine.cpu.a == 0x13
    assert machine.cpu.pc == 3

    assert machine.step() == 4
    assert machine.cpu.pc == 4
    assert machine.total_instructions == 3
    assert machine.total_tstates == 15
