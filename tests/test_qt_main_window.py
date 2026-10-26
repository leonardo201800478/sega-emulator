"""Offscreen smoke tests for the modular Qt window."""

from __future__ import annotations

import os

os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")

import pytest  # noqa: E402

pytest.importorskip("PySide6")

from PySide6.QtWidgets import QApplication  # noqa: E402

from sega_emulator.frontend.qt.main_window import EmulatorMainWindow  # noqa: E402


@pytest.fixture
def qt_app():
    app = QApplication.instance() or QApplication([])
    yield app


def test_main_window_exposes_required_menus_and_shortcuts(qt_app) -> None:
    called: list[str] = []

    window = EmulatorMainWindow(
        on_open_rom=lambda: called.append("open"),
        on_pause_resume=lambda: called.append("pause"),
        on_reset=lambda: called.append("reset"),
        on_configure_controls=lambda: called.append("configure"),
        on_show_controllers=lambda: called.append("controllers"),
        on_vdp_snapshot=lambda: called.append("snapshot"),
        on_open_logs=lambda: called.append("logs"),
        on_show_machine_state=lambda: called.append("state"),
        on_video_scale=lambda scale: called.append(f"scale:{scale}"),
    )

    menu_titles = [action.text() for action in window.menuBar().actions()]
    assert menu_titles == [
        "&Arquivo",
        "&Emulação",
        "&Vídeo",
        "&Controles",
        "&Diagnóstico",
        "&Ajuda",
    ]
    assert window.open_action.shortcut().toString() == "Ctrl+O"
    assert window.pause_action.shortcut().toString() == "F5"
    assert window.reset_action.shortcut().toString() == "F10"
    assert window.vdp_snapshot_action.shortcut().toString() == "F8"

    window.pause_action.trigger()
    window.reset_action.trigger()
    assert called == ["pause", "reset"]
    window.close()


@pytest.mark.parametrize("scale", [2, 3, 4])
def test_video_scale_updates_window_size(qt_app, scale: int) -> None:
    window = EmulatorMainWindow(
        on_open_rom=lambda: None,
        on_pause_resume=lambda: None,
        on_reset=lambda: None,
        on_configure_controls=lambda: None,
        on_show_controllers=lambda: None,
        on_vdp_snapshot=lambda: None,
        on_open_logs=lambda: None,
        on_show_machine_state=lambda: None,
        on_video_scale=lambda selected: None,
    )

    window.set_video_scale(scale)

    assert window.width() == max(640, 256 * scale + 64)
    assert window.height() == max(480, 192 * scale + 128)
    window.close()


def test_main_window_exposes_emulation_backend_selector(qt_app) -> None:
    called: list[str] = []
    window = EmulatorMainWindow(
        on_open_rom=lambda: None,
        on_pause_resume=lambda: None,
        on_reset=lambda: None,
        on_configure_controls=lambda: None,
        on_show_controllers=lambda: None,
        on_vdp_snapshot=lambda: None,
        on_open_logs=lambda: None,
        on_show_machine_state=lambda: None,
        on_video_scale=lambda scale: None,
        on_select_emulation_backend=lambda: called.append("select-backend"),
    )
    assert window.select_emulation_backend_action.isEnabled()
    window.select_emulation_backend_action.trigger()
    assert called == ["select-backend"]
    window.close()


def test_main_window_exposes_standalone_rust_core_diagnostics(qt_app) -> None:
    called: list[str] = []
    window = EmulatorMainWindow(
        on_open_rom=lambda: None,
        on_pause_resume=lambda: None,
        on_reset=lambda: None,
        on_configure_controls=lambda: None,
        on_show_controllers=lambda: None,
        on_vdp_snapshot=lambda: None,
        on_open_logs=lambda: None,
        on_show_machine_state=lambda: None,
        on_video_scale=lambda scale: None,
        on_run_standalone_core=lambda: called.append("standalone-core"),
    )

    assert window.standalone_core_action.isEnabled()
    window.standalone_core_action.trigger()
    assert called == ["standalone-core"]
    window.close()
