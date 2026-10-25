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



def test_main_window_exposes_cpu_core_selector(qt_app) -> None:
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
        on_select_cpu_core=lambda: called.append("select-core"),
    )
    assert window.select_cpu_core_action.isEnabled()
    window.select_cpu_core_action.trigger()
    assert called == ["select-core"]
    window.close()
