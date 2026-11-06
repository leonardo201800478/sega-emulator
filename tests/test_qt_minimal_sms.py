"""Smoke tests for the video-free SMS Qt control panel."""

from __future__ import annotations

import os
from pathlib import Path

import pytest

os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")
pytest.importorskip("PySide6")

from PySide6.QtWidgets import QApplication

import sega_emulator.frontend.qt.minimal_sms_window as minimal_window
from sega_emulator.frontend.qt.native_video_viewport import SmsVideoViewport


class FakeTemporalSMSMachine:
    def __init__(self, rom: bytes, *, region: str) -> None:
        self.rom = rom
        self.region = region
        self.frame_number = 0
        self.total_instructions = 0
        self.is_running = False
        self.calls: list[tuple[object, ...]] = []

    def configure_audio_output(self, backend: str) -> None:
        self.calls.append(("configure_audio_output", backend))

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

    def step_frame(self) -> int:
        self.calls.append(("step_frame",))
        self.frame_number += 1
        return 100

    def set_controller(self, player: int, **buttons: bool) -> None:
        self.calls.append(("set_controller", player, buttons))

    def open_video_output_in_viewport(self, window_handle: int) -> None:
        self.calls.append(("open_video_output_in_viewport", window_handle))

    def close_video_output(self) -> None:
        self.calls.append(("close_video_output",))

    def close_audio_output(self) -> None:
        self.calls.append(("close_audio_output",))

    @property
    def framebuffer(self) -> bytes:
        raise AssertionError("Qt must never read or render the video framebuffer")


@pytest.fixture(scope="module")
def qapplication() -> QApplication:
    return QApplication.instance() or QApplication([])


def test_minimal_sms_window_loads_controls_and_closes_rom(
    qapplication: QApplication,
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
) -> None:
    monkeypatch.setattr(minimal_window, "TemporalSMSMachine", FakeTemporalSMSMachine)
    rom_path = tmp_path / "test.sms"
    rom_path.write_bytes(b"test-rom")

    window = minimal_window.MinimalSmsWindow(region="brazil")
    window.load_rom(rom_path)

    machine = window._machine
    assert isinstance(machine, FakeTemporalSMSMachine)
    assert machine.rom == b"test-rom"
    assert machine.region == "brazil"
    assert window.reset_action.isEnabled()
    assert window.close_rom_action.isEnabled()
    assert not hasattr(window, "_frame_timer")
    assert "Preparando" in window.status_label.text()
    assert window.run_action.isEnabled()
    assert not any(call[0] == "configure_audio_output" for call in machine.calls)

    window.show()
    qapplication.processEvents()
    video_calls = [
        call for call in machine.calls if call[0] == "open_video_output_in_viewport"
    ]
    assert len(video_calls) == 1
    assert video_calls[0][1] > 0
    assert "D3D12 ativa" in window.status_label.text()

    window._toggle_running()
    assert machine.is_running
    window._toggle_running()
    assert not machine.is_running

    window._reset()
    assert machine.frame_number == 0
    window._close_rom()
    assert window._machine is None
    assert not window.reset_action.isEnabled()
    assert ("stop",) in machine.calls
    assert ("close_video_output",) in machine.calls
    assert ("close_audio_output",) in machine.calls
    assert not any(call[0] == "step_frame" for call in machine.calls)
    window.close()


def test_minimal_sms_window_maps_keyboard_levels_without_rendering_video(
    qapplication: QApplication,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setattr(minimal_window, "TemporalSMSMachine", FakeTemporalSMSMachine)
    window = minimal_window.MinimalSmsWindow()
    machine = FakeTemporalSMSMachine(b"rom", region="usa")
    window._machine = machine

    window._pressed_actions.update({"up", "button_1"})
    window._update_controller()

    assert machine.calls[-1] == (
        "set_controller",
        1,
        {
            "up": True,
            "down": False,
            "left": False,
            "right": False,
            "button_1": True,
            "button_2": False,
        },
    )
    assert not any(call[0] == "step_frame" for call in machine.calls)
    window.close()



def test_sms_video_viewport_keeps_a_centered_four_by_three_surface(
    qapplication: QApplication,
) -> None:
    viewport = SmsVideoViewport()
    viewport.resize(1000, 700)
    qapplication.processEvents()

    surface = viewport.surface_widget.geometry()
    assert surface.width() / surface.height() == pytest.approx(4 / 3, abs=0.01)
    assert surface.x() >= 0
    assert surface.y() >= 0
    assert surface.right() < viewport.width()
    assert surface.bottom() < viewport.height()
    viewport.close()
