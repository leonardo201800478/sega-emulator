"""Integration checks that the selected Rust backend is the native SG-1000 machine."""

from __future__ import annotations

import pytest

pytest.importorskip("sega_z80_core")

from sega_emulator.frontend.qt_frontend import _runtime_diagnostic  # noqa: E402
from sega_emulator.machine.sg1000 import SG1000Machine  # noqa: E402


def test_rust_backend_instantiates_native_machine_and_devices() -> None:
    # JP 0000 is a deterministic, supported loop for advancing VDP frames.
    machine = SG1000Machine(bytes((0xC3, 0x00, 0x00)), cpu_core="rust")

    assert type(machine.native_machine).__name__ == "NativeSG1000Machine"
    assert type(machine.cpu).__name__ == "RustZ80Cpu"
    assert type(machine.memory_bus).__name__ == "NativeMemoryBus"
    assert type(machine.vdp).__name__ == "NativeTms9918A"
    assert type(machine.psg).__name__ == "NativeSn76489"
    assert type(machine.audio_sync).__name__ == "NativeAudioSynchronizer"


def test_rust_run_frame_executes_native_instruction_loop() -> None:
    machine = SG1000Machine(bytes((0xC3, 0x00, 0x00)), cpu_core="rust")

    steps = machine.run_frame()

    assert steps > 0
    assert machine.native_machine.total_instructions == steps
    assert machine.total_instructions == steps
    assert machine.total_tstates > 0
    assert machine.native_machine.total_tstates == machine.total_tstates
    assert machine.vdp.frame_number == 1

def test_runtime_diagnostic_identifies_loaded_native_binary() -> None:
    machine = SG1000Machine(bytes((0xC3, 0x00, 0x00)), cpu_core="rust")

    diagnostic = _runtime_diagnostic(machine, "test-rom-sha256")

    assert diagnostic["backend_requested"] == "rust"
    assert diagnostic["backend_effective"] == "rust_native_machine"
    assert diagnostic["native_machine_type"].endswith("NativeSG1000Machine")
    assert diagnostic["native_binary_path"]
    assert len(diagnostic["native_binary_sha256"]) == 64
    assert diagnostic["native_binary_size"] > 0
    assert diagnostic["rom_sha256"] == "test-rom-sha256"
