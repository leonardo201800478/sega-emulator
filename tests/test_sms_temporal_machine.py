"""Contract tests for the thin Python adapter to the temporal SMS runtime."""

from __future__ import annotations

import sys
from types import SimpleNamespace

import pytest

from sega_emulator.machine.sms_temporal import TemporalSMSMachine


class FakeNativeSMSTemporalMachine:
    def __init__(self, rom: bytes, *, region: str) -> None:
        self.rom = rom
        self.region = region
        self.frame_number = 0
        self.total_instructions = 0
        self.is_running = False
        self.audio_output_error = None
        self.audio_output_metrics = (0, 0)
        self.video_output_is_running = True
        self.video_last_published_sequence = 0
        self.video_last_presented_sequence = 0
        self.video_replaced_frames = 0
        self.video_device_recovery_count = 0
        self.video_output_error = None
        self.calls: list[tuple[object, ...]] = []

    def load_rom(self, rom: bytes) -> None:
        self.calls.append(("load_rom", rom))
        self.rom = rom
        self.frame_number = 0

    def start(self) -> None:
        self.calls.append(("start",))
        self.is_running = True

    def pause(self) -> None:
        self.calls.append(("pause",))
        self.is_running = False

    def stop(self) -> None:
        self.calls.append(("stop",))
        self.is_running = False

    def reset(self) -> None:
        self.calls.append(("reset",))
        self.frame_number = 0

    def step_frame(self, max_instructions: int) -> int:
        self.calls.append(("step_frame", max_instructions))
        self.frame_number += 1
        self.total_instructions += 123
        return 123

    def set_controller(self, player: int, *buttons: bool) -> None:
        self.calls.append(("set_controller", player, *buttons))

    def request_nmi(self) -> None:
        self.calls.append(("request_nmi",))

    def configure_audio_output(self, *args: object, **kwargs: object) -> None:
        self.calls.append(("configure_audio_output", *args, *kwargs.values()))

    def close_audio_output(self) -> None:
        self.calls.append(("close_audio_output",))


@pytest.fixture
def temporal_machine_module(monkeypatch: pytest.MonkeyPatch) -> SimpleNamespace:
    module = SimpleNamespace(NativeSMSTemporalMachine=FakeNativeSMSTemporalMachine)
    monkeypatch.setitem(sys.modules, "sega_z80_core", module)
    return module


def test_temporal_adapter_exposes_session_controls_without_video_buffers(
    temporal_machine_module: SimpleNamespace,
) -> None:
    machine = TemporalSMSMachine(b"rom", region="brazil")

    assert machine.region == "brazil"
    assert machine.frame_number == 0
    machine.start()
    assert machine.is_running
    assert machine.step_frame() == 123
    assert machine.frame_number == 1
    assert machine.total_instructions == 123
    machine.pause()
    assert not machine.is_running
    machine.stop()
    machine.reset()

    assert ("step_frame", 100_000) in machine.native_machine.calls
    assert ("pause",) in machine.native_machine.calls
    assert ("stop",) in machine.native_machine.calls
    assert ("reset",) in machine.native_machine.calls


def test_load_rom_and_controller_input_are_coarse_grained(
    temporal_machine_module: SimpleNamespace,
) -> None:
    machine = TemporalSMSMachine(b"first")
    machine.load_rom(b"second")
    machine.set_controller(
        1,
        up=True,
        button_1=True,
    )
    machine.request_nmi()

    assert machine.cartridge_rom == b"second"
    assert ("load_rom", b"second") in machine.native_machine.calls
    assert (
        "set_controller",
        1,
        True,
        False,
        False,
        False,
        True,
        False,
    ) in machine.native_machine.calls
    assert ("request_nmi",) in machine.native_machine.calls


def test_temporal_adapter_validates_rom_region_and_frame_limit(
    temporal_machine_module: SimpleNamespace,
) -> None:
    with pytest.raises(ValueError):
        TemporalSMSMachine(b"")
    with pytest.raises(TypeError):
        TemporalSMSMachine("not bytes")  # type: ignore[arg-type]
    with pytest.raises(ValueError):
        TemporalSMSMachine(b"rom", region="invalid")

    machine = TemporalSMSMachine(b"rom")
    with pytest.raises(ValueError):
        machine.step_frame(0)
    with pytest.raises(ValueError):
        machine.step_frame(-1)
    with pytest.raises(ValueError):
        machine.set_controller(3)
    with pytest.raises(TypeError):
        machine.set_controller(1, up=1)  # type: ignore[arg-type]


def test_temporal_adapter_forwards_native_audio_configuration(
    temporal_machine_module: SimpleNamespace,
) -> None:
    machine = TemporalSMSMachine(b"rom")
    machine.configure_audio_output("wasapi", sample_rate=48_000)
    machine.close_audio_output()

    assert (
        "configure_audio_output",
        "wasapi",
        48_000,
        512,
        4,
        100,
        0,
    ) in machine.native_machine.calls
    assert ("close_audio_output",) in machine.native_machine.calls
    assert machine.audio_output_error is None
    assert machine.audio_output_metrics == (0, 0)


def test_reset_reopens_the_previous_native_audio_configuration(
    temporal_machine_module: SimpleNamespace,
) -> None:
    machine = TemporalSMSMachine(b"rom")
    machine.configure_audio_output("wasapi", sample_rate=48_000)
    machine.reset()

    audio_calls = [
        call
        for call in machine.native_machine.calls
        if call[0] == "configure_audio_output"
    ]
    assert len(audio_calls) == 2
    assert audio_calls[1] == (
        "configure_audio_output",
        "wasapi",
        48_000,
        512,
        4,
        100,
        0,
    )


def test_temporal_adapter_exposes_read_only_video_presentation_telemetry(
    temporal_machine_module: SimpleNamespace,
) -> None:
    machine = TemporalSMSMachine(b"rom")
    native = machine.native_machine
    native.video_output_is_running = True
    native.video_last_published_sequence = 18
    native.video_last_presented_sequence = 16
    native.video_replaced_frames = 2
    native.video_device_recovery_count = 1
    native.video_output_error = "DXGI device removed"

    assert machine.video_output_is_running is True
    assert machine.video_last_published_sequence == 18
    assert machine.video_last_presented_sequence == 16
    assert machine.video_replaced_frames == 2
    assert machine.video_device_recovery_count == 1
    assert machine.video_output_error == "DXGI device removed"
