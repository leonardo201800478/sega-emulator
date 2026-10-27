"""VGM capture integration against the Rust-owned SG-1000 runtime."""

from __future__ import annotations

import pytest

from sega_emulator.machine.sg1000 import SG1000Machine


def test_rust_machine_exposes_timestamped_psg_writes() -> None:
    pytest.importorskip("sega_z80_core")
    machine = SG1000Machine(b"\x00")
    machine.write_port_at(0x7F, 0x90, 123)
    machine.write_port_at(0x40, 0x9F, 456)

    assert machine.drain_psg_write_events() == [(123, 0x90), (456, 0x9F)]
    assert machine.drain_psg_write_events() == []
