"""Integration checks that the SG-1000 runtime is Rust-only."""

from __future__ import annotations

import pytest

pytest.importorskip("sega_z80_core")

from sega_emulator.frontend.qt_frontend import _runtime_diagnostic  # noqa: E402
from sega_emulator.machine.sg1000 import SG1000Machine  # noqa: E402


def test_obsolete_executor_and_memory_adapter_are_not_exported() -> None:
    import sega_z80_core

    assert hasattr(sega_z80_core, "NativeSG1000Machine")
    assert not hasattr(sega_z80_core, "RustZ80Cpu")
    assert not hasattr(sega_z80_core, "NativeMemoryBus")


def test_rust_backend_instantiates_native_machine_and_devices() -> None:
    # JP 0000 is a deterministic, supported loop for advancing VDP frames.
    machine = SG1000Machine(bytes((0xC3, 0x00, 0x00)))

    assert type(machine.native_machine).__name__ == "NativeSG1000Machine"
    assert machine.cpu is not None
    assert machine.vdp is not None
    assert machine.psg is not None


def test_rust_run_frame_executes_native_instruction_loop() -> None:
    machine = SG1000Machine(bytes((0xC3, 0x00, 0x00)))

    steps = machine.run_frame()

    assert steps > 0
    assert machine.native_machine.total_instructions == steps
    assert machine.total_instructions == steps
    assert machine.total_tstates > 0
    assert machine.native_machine.total_tstates == machine.total_tstates
    assert machine.vdp.frame_number == 1


def test_runtime_diagnostic_identifies_loaded_native_binary() -> None:
    machine = SG1000Machine(bytes((0xC3, 0x00, 0x00)))

    diagnostic = _runtime_diagnostic(machine, "test-rom-sha256")

    assert diagnostic["backend_requested"] == "rust"
    assert diagnostic["backend_effective"] == "rust_native_machine"
    assert diagnostic["native_machine_type"].endswith("NativeSG1000Machine")
    assert diagnostic["native_binary_path"]
    assert len(diagnostic["native_binary_sha256"]) == 64
    assert diagnostic["native_binary_size"] > 0
    assert diagnostic["rom_sha256"] == "test-rom-sha256"
