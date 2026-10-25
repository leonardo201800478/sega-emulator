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
