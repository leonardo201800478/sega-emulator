"""Smoke tests for the video-free SMS Qt control panel."""

from __future__ import annotations

import os
import zipfile
from pathlib import Path

import pytest

os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")
pytest.importorskip("PySide6")

from PySide6.QtCore import QSettings
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
        self.video_output_is_running = False
        self.video_last_published_sequence = 0
        self.video_last_presented_sequence = 0
        self.video_replaced_frames = 0
        self.video_device_recovery_count = 0
        self.video_output_error: str | None = None
        self.runtime_error: str | None = None
        self.video_close_error: str | None = None
        self.audio_output_error: str | None = None
        self.audio_output_metrics = (0, 0)
        self.audio_configure_error: str | None = None
        self.calls: list[tuple[object, ...]] = []

    def configure_audio_output(self, backend: str, **kwargs: object) -> None:
        self.calls.append(("configure_audio_output", backend, kwargs))
        if self.audio_configure_error is not None:
            raise RuntimeError(self.audio_configure_error)

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
        self.video_output_is_running = True

    def close_video_output(self) -> None:
        self.calls.append(("close_video_output",))
        if self.video_close_error is not None:
            raise RuntimeError(self.video_close_error)
        self.video_output_is_running = False

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

    settings = QSettings(str(tmp_path / "settings.ini"), QSettings.Format.IniFormat)
    window = minimal_window.MinimalSmsWindow(region="brazil", settings=settings)
    window.load_rom(rom_path)

    machine = window._machine
    assert isinstance(machine, FakeTemporalSMSMachine)
    assert machine.rom == b"test-rom"
    assert machine.region == "brazil"
    assert window.reset_action.isEnabled()
    assert window.close_rom_action.isEnabled()
    assert not hasattr(window, "_frame_timer")
    assert "F5" in window.status_label.text()
    assert "Áudio WASAPI inicializado" in window.status_label.text()
    assert window.run_action.isEnabled()
    assert [action.text() for action in window.menuBar().actions()] == [
        "Arquivo",
        "Emulação",
        "Configurações",
        "Ajuda",
    ]
    assert window.settings_action.shortcut().toString() == "Ctrl+,"
    audio_calls = [
        call for call in machine.calls if call[0] == "configure_audio_output"
    ]
    assert len(audio_calls) == 1
    assert audio_calls[0][1] == "wasapi"
    assert audio_calls[0][2] == {
        "sample_rate": 48_000,
        "buffer_size": 512,
        "queue_blocks": 4,
        "volume": 80,
        "balance": 0,
    }

    window.show()
    qapplication.processEvents()
    video_calls = [
        call for call in machine.calls if call[0] == "open_video_output_in_viewport"
    ]
    assert len(video_calls) == 1
    assert video_calls[0][1] > 0
    assert "aguardando o primeiro frame" in window.status_label.text()
    assert "aguardando o primeiro frame" in window.statusBar().currentMessage()

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


def test_video_telemetry_reports_published_and_presented_sequences(
    qapplication: QApplication,
) -> None:
    window = minimal_window.MinimalSmsWindow()
    machine = FakeTemporalSMSMachine(b"rom", region="usa")
    machine.video_output_is_running = True
    machine.frame_number = 25
    machine.video_last_published_sequence = 7
    machine.video_last_presented_sequence = 5
    machine.video_replaced_frames = 2
    machine.video_device_recovery_count = 1
    window._machine = machine
    window._video_output_started = True

    window._refresh_video_telemetry()

    message = window.statusBar().currentMessage()
    assert "Frames 25" in message
    assert "apresentado 5/7" in message
    assert "substituídos 2" in message
    assert "recuperações 1" in message
    window.close()


def test_video_telemetry_reports_presentation_failure(
    qapplication: QApplication,
) -> None:
    window = minimal_window.MinimalSmsWindow()
    machine = FakeTemporalSMSMachine(b"rom", region="usa")
    machine.video_output_is_running = False
    machine.video_output_error = "DXGI device removed"
    window._machine = machine
    window._video_output_started = True

    window._refresh_video_telemetry()

    assert "DXGI device removed" in window.status_label.text()
    assert "falha" in window.statusBar().currentMessage()
    window.close()


def test_close_rom_completes_when_d3d12_shutdown_reports_error(
    qapplication: QApplication,
) -> None:
    window = minimal_window.MinimalSmsWindow()
    machine = FakeTemporalSMSMachine(b"rom", region="usa")
    machine.video_close_error = "presentation worker failed"
    window._machine = machine
    window._video_output_started = True
    window._video_telemetry_timer.start()

    window._close_rom()

    assert window._machine is None
    assert not window._video_telemetry_timer.isActive()
    assert ("close_audio_output",) in machine.calls
    assert "presentation worker failed" in window.statusBar().currentMessage()
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


def test_audio_output_failure_does_not_prevent_loading_rom(
    qapplication: QApplication,
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
) -> None:
    class FailingAudioMachine(FakeTemporalSMSMachine):
        def configure_audio_output(self, backend: str, **kwargs: object) -> None:
            self.calls.append(("configure_audio_output", backend, kwargs))
            raise RuntimeError("WASAPI endpoint unavailable")

    monkeypatch.setattr(minimal_window, "TemporalSMSMachine", FailingAudioMachine)
    rom_path = tmp_path / "audio-fallback.sms"
    rom_path.write_bytes(b"test-rom")

    window = minimal_window.MinimalSmsWindow(region="usa")
    window.load_rom(rom_path)

    assert isinstance(window._machine, FailingAudioMachine)
    assert window._audio_output_error == "WASAPI endpoint unavailable"
    assert "Áudio WASAPI indisponível" in window.status_label.text()
    assert window.run_action.isEnabled()
    window.close()


def test_minimal_sms_window_loads_single_rom_from_zip(
    qapplication: QApplication,
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
) -> None:
    monkeypatch.setattr(minimal_window, "TemporalSMSMachine", FakeTemporalSMSMachine)
    archive_path = tmp_path / "game.zip"
    rom_bytes = b"valid-sms-rom"
    with zipfile.ZipFile(archive_path, "w") as archive:
        archive.writestr("roms/game.sms", rom_bytes)

    window = minimal_window.MinimalSmsWindow(region="usa")
    window.load_rom(archive_path)

    assert isinstance(window._machine, FakeTemporalSMSMachine)
    assert window._machine.rom == rom_bytes
    assert window._rom_path == archive_path
    assert "game.sms" in window.windowTitle()
    window.close()


def test_video_telemetry_reports_native_runtime_failure(
    qapplication: QApplication,
) -> None:
    window = minimal_window.MinimalSmsWindow()
    machine = FakeTemporalSMSMachine(b"rom", region="usa")
    machine.runtime_error = "FrameStepLimitReached"
    machine.video_output_is_running = True
    window._machine = machine
    window._video_output_started = True
    window._video_telemetry_timer.start()

    window._refresh_video_telemetry()

    assert "FrameStepLimitReached" in window.status_label.text()
    assert "Execução SMS interrompida" in window.statusBar().currentMessage()
    assert not window._video_telemetry_timer.isActive()
    window.close()



def test_sms_window_applies_persisted_audio_preferences(
    qapplication: QApplication,
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
) -> None:
    monkeypatch.setattr(minimal_window, "TemporalSMSMachine", FakeTemporalSMSMachine)
    settings = QSettings(str(tmp_path / "audio.ini"), QSettings.Format.IniFormat)
    settings.setValue("sms_audio_backend", "xaudio2")
    settings.setValue("audio_sample_rate", 44_100)
    settings.setValue("audio_buffer_size", 1024)
    settings.setValue("audio_queue_blocks", 8)
    settings.setValue("audio_volume", 55)
    settings.setValue("audio_balance", -30)
    rom_path = tmp_path / "configured.sms"
    rom_path.write_bytes(b"test-rom")

    window = minimal_window.MinimalSmsWindow(region="usa", settings=settings)
    window.load_rom(rom_path)

    machine = window._machine
    assert isinstance(machine, FakeTemporalSMSMachine)
    audio_call = next(call for call in machine.calls if call[0] == "configure_audio_output")
    assert audio_call[1] == "xaudio2"
    assert audio_call[2] == {
        "sample_rate": 44_100,
        "buffer_size": 1024,
        "queue_blocks": 8,
        "volume": 55,
        "balance": -30,
    }
    assert "XAudio2 inicializado" in window.status_label.text()
    window.close()


def test_sms_window_can_disable_native_audio_from_preferences(
    qapplication: QApplication,
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
) -> None:
    monkeypatch.setattr(minimal_window, "TemporalSMSMachine", FakeTemporalSMSMachine)
    settings = QSettings(str(tmp_path / "disabled.ini"), QSettings.Format.IniFormat)
    settings.setValue("sms_audio_backend", "disabled")
    rom_path = tmp_path / "silent.sms"
    rom_path.write_bytes(b"test-rom")

    window = minimal_window.MinimalSmsWindow(region="usa", settings=settings)
    window.load_rom(rom_path)

    machine = window._machine
    assert isinstance(machine, FakeTemporalSMSMachine)
    assert ("close_audio_output",) in machine.calls
    assert not any(call[0] == "configure_audio_output" for call in machine.calls)
    assert "Áudio desativado" in window.status_label.text()
    window.close()


def test_sms_video_viewport_supports_integer_scaling(
    qapplication: QApplication,
) -> None:
    viewport = SmsVideoViewport()
    viewport.resize(1000, 700)
    viewport.set_integer_scale(True)
    qapplication.processEvents()

    surface = viewport.surface_widget.geometry()
    assert surface.width() == 768
    assert surface.height() == 576
    assert surface.width() % 256 == 0
    assert surface.height() % 192 == 0

    viewport.set_integer_scale(False)
    qapplication.processEvents()
    fractional_surface = viewport.surface_widget.geometry()
    assert fractional_surface.width() / fractional_surface.height() == pytest.approx(
        4 / 3, abs=0.01
    )
    viewport.close()


def test_sms_window_uses_saved_region_when_cli_does_not_override(
    qapplication: QApplication,
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
) -> None:
    monkeypatch.setattr(minimal_window, "TemporalSMSMachine", FakeTemporalSMSMachine)
    settings = QSettings(str(tmp_path / "region.ini"), QSettings.Format.IniFormat)
    settings.setValue("sms_region_preference", "europe")
    rom_path = tmp_path / "region.sms"
    rom_path.write_bytes(b"test-rom")

    window = minimal_window.MinimalSmsWindow(settings=settings)
    window.load_rom(rom_path)

    assert isinstance(window._machine, FakeTemporalSMSMachine)
    assert window._machine.region == "europe"
    window.close()


def test_sms_menu_opens_library_on_master_system_and_loads_selection(
    qapplication: QApplication,
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
) -> None:
    selected_rom = tmp_path / "library-game.sms"
    loaded: list[Path] = []

    class FakeLibraryDialog:
        def __init__(self, settings, *, initial_system: str, parent=None) -> None:
            assert initial_system == "master_system"
            self.selected_rom = selected_rom

        def exec(self) -> int:
            return 1

    monkeypatch.setattr(minimal_window, "RomLibraryDialog", FakeLibraryDialog)
    window = minimal_window.MinimalSmsWindow()
    monkeypatch.setattr(window, "load_rom", loaded.append)

    window._open_library_dialog()

    assert loaded == [selected_rom]
    window.close()
