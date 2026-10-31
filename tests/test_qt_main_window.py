"""Offscreen smoke tests for the modular Qt window."""

from __future__ import annotations

import os

os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")

import pytest  # noqa: E402

pytest.importorskip("PySide6")

from PySide6.QtWidgets import QApplication, QMessageBox  # noqa: E402

from sega_emulator.frontend.qt.main_window import (
    EmulatorMainWindow,
    persisted_window_dimensions,
)  # noqa: E402


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
        on_change_system=lambda: called.append("change-system"),
        on_configure_drivers=lambda: called.append("drivers"),
        on_configure_audio_video=lambda: called.append("audio-settings"),
        on_show_audio_diagnostics=lambda: called.append("audio-diagnostics"),
        on_start_vgm_recording=lambda: called.append("vgm-start"),
        on_stop_vgm_recording=lambda: called.append("vgm-stop"),
    )

    assert window.audio_video_action.objectName() == "configureAudioOutputAction"
    assert window.audio_drivers_action.objectName() == "configureAudioBackendAction"
    assert window.audio_diagnostics_action.objectName() == "audioOutputDiagnosticsAction"
    window.audio_video_action.trigger()
    window.audio_drivers_action.trigger()
    window.audio_diagnostics_action.trigger()
    window.start_vgm_recording_action.trigger()
    assert not window.stop_vgm_recording_action.isEnabled()
    window.set_vgm_recording(True)
    assert window.stop_vgm_recording_action.isEnabled()
    assert not window.start_vgm_recording_action.isEnabled()
    window.set_vgm_recording(False)
    assert called == ["audio-settings", "drivers", "audio-diagnostics", "vgm-start"]
    called.clear()

    menu_titles = [action.text() for action in window.menuBar().actions()]
    assert menu_titles == [
        "&Arquivo",
        "&Emulação",
        "&Drivers",
        "&Vídeo",
        "Áudio",
        "&Configurações",
        "&Diagnóstico",
        "&Ajuda",
    ]
    assert window.open_action.shortcut().toString() == "Ctrl+O"
    assert window.pause_action.shortcut().toString() == "F5"
    assert window.pause_action.objectName() == "pauseAction"
    assert not window.pause_action.isEnabled()
    window.set_pause_state(False, enabled=True)
    assert window.pause_action.isEnabled()
    assert window.pause_action.text() == "Pausar"
    window.set_pause_state(True, enabled=True)
    assert window.pause_action.text() == "Retomar"
    window.set_pause_state(False, enabled=False)
    assert not window.pause_action.isEnabled()
    window.set_pause_state(False, enabled=True)
    assert window.reset_action.shortcut().toString() == "F10"
    assert window.change_system_action.isEnabled()
    assert window.change_system_action.objectName() == "changeSystemAction"
    assert window.vdp_snapshot_action.shortcut().toString() == "F8"
    assert window.central_tabs.count() == 2
    assert window.central_tabs.tabText(0) == "Emulador"
    assert window.central_tabs.tabText(1) == "Debug"
    assert not window.debug_logging_checkbox.isChecked()

    window.pause_action.trigger()
    window.reset_action.trigger()
    window.change_system_action.trigger()
    assert called == ["pause", "reset", "change-system"]
    window.close()


def test_main_window_exposes_brazil_pal_m_region_action(qt_app) -> None:
    selected: list[str] = []
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
        on_configure_region=selected.append,
        region_preference="brazil",
    )

    assert set(window.region_actions) == {"auto", "usa", "europe", "japan", "brazil"}
    assert window.region_actions["brazil"].isChecked()
    window.region_actions["brazil"].trigger()
    assert selected == ["brazil"]
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

    assert window.width() == max(760, 256 * scale + 64)
    assert window.height() == max(560, 192 * scale + 150)
    window.close()


def test_main_window_exposes_drivers_menu_and_callback(qt_app) -> None:
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
        on_configure_drivers=lambda: called.append("drivers"),
    )
    assert window.configure_drivers_action.isEnabled()
    window.configure_drivers_action.trigger()
    assert called == ["drivers"]
    assert not hasattr(window, "select_emulation_backend_action")
    window.close()


def test_main_window_exposes_audio_video_settings(qt_app) -> None:
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
        on_configure_audio_video=lambda: called.append("audio-video"),
    )

    assert window.audio_video_action.isEnabled()
    window.audio_video_action.trigger()
    assert called == ["audio-video"]
    window.close()


def test_main_window_exposes_vgm_recording_actions(qt_app) -> None:
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
        on_start_vgm_recording=lambda: called.append("start"),
        on_stop_vgm_recording=lambda: called.append("stop"),
    )

    assert window.start_vgm_recording_action.isEnabled()
    assert not window.stop_vgm_recording_action.isEnabled()
    window.start_vgm_recording_action.trigger()
    assert called == ["start"]

    window.set_vgm_recording(True)
    assert not window.start_vgm_recording_action.isEnabled()
    assert window.stop_vgm_recording_action.isEnabled()
    window.stop_vgm_recording_action.trigger()
    assert called == ["start", "stop"]

    window.set_vgm_recording(False)
    assert window.start_vgm_recording_action.isEnabled()
    assert not window.stop_vgm_recording_action.isEnabled()
    window.close()


def test_persisted_window_dimensions_uses_normal_geometry_in_fullscreen(qt_app) -> None:
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
    )
    window.resize(900, 700)
    window.show()
    qt_app.processEvents()
    expected = (window.width(), window.height())

    window.showFullScreen()
    qt_app.processEvents()
    assert persisted_window_dimensions(window, 1180, 820) == expected

    window.showNormal()
    window.close()


def test_persisted_window_dimensions_falls_back_for_invalid_geometry(qt_app) -> None:
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
    )
    window.hide()
    assert persisted_window_dimensions(window, 1180, 820) == (1180, 820)
    window.close()


def test_fullscreen_hides_application_chrome_and_escape_restores_it(qt_app) -> None:
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
    )
    assert not window.exit_fullscreen_action.isEnabled()
    window.show()
    qt_app.processEvents()

    window.fullscreen_action.trigger()
    qt_app.processEvents()
    assert window.isFullScreen()
    assert window.menuBar().isHidden()
    assert window._main_toolbar.isHidden()
    assert window.statusBar().isHidden()
    assert window._central_layout.contentsMargins().left() == 0
    assert window.exit_fullscreen_action.isEnabled()

    window.exit_fullscreen_action.trigger()
    qt_app.processEvents()
    assert not window.isFullScreen()
    assert not window.menuBar().isHidden()
    assert not window._main_toolbar.isHidden()
    assert not window.statusBar().isHidden()
    assert window._central_layout.contentsMargins().left() == 18
    assert not window.exit_fullscreen_action.isEnabled()
    window.close()


def test_about_dialog_shows_version_scope_credits_and_mit_license(
    qt_app, monkeypatch
) -> None:
    shown: list[tuple[str, str, str]] = []

    def capture_about(parent, title: str, text: str) -> None:
        shown.append((title, text, parent.windowTitle()))

    monkeypatch.setattr(QMessageBox, "about", capture_about)
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
    )

    window.about_action.trigger()

    assert len(shown) == 1
    title, content, parent_title = shown[0]
    assert title == "Sobre o Sega Emulator"
    assert parent_title == "Sega Emulator | SG-1000"
    assert "Versão:</b> 0.1.0" in content
    assert "SG-1000" in content
    assert "PySide6" in content
    assert "Licença:</b> MIT" in content
    assert "sem afiliação oficial" in content
    window.close()


@pytest.mark.parametrize(
    ("language", "expected_file_menu"),
    [
        ("pt-BR", "&Arquivo"),
        ("en", "&File"),
        ("es", "&Archivo"),
        ("it", "&File"),
        ("fr", "&Fichier"),
    ],
)
def test_main_window_localizes_navigation(qt_app, language, expected_file_menu) -> None:
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
        language=language,
    )
    assert window.menuBar().actions()[0].text() == expected_file_menu
    assert window.windowTitle() == "Sega Emulator | SG-1000"
    window.close()
