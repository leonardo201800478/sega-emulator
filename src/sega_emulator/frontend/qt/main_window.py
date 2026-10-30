"""Janela Qt moderna para o frontend SG-1000; operações vêm por callbacks."""

from __future__ import annotations

from collections.abc import Callable

from PySide6.QtCore import Qt, Signal
from PySide6.QtGui import QAction, QActionGroup, QKeySequence
from PySide6.QtWidgets import (
    QCheckBox,
    QLabel,
    QMainWindow,
    QMessageBox,
    QPlainTextEdit,
    QPushButton,
    QStatusBar,
    QTabWidget,
    QToolBar,
    QVBoxLayout,
    QWidget,
)

from sega_emulator import __version__
from sega_emulator.frontend.qt.localization import translate
from sega_emulator.frontend.qt.video_widget import VideoWidget


def persisted_window_dimensions(
    window: QMainWindow, fallback_width: int, fallback_height: int
) -> tuple[int, int]:
    """Return windowed dimensions, even if the application exits fullscreen."""
    geometry = window.normalGeometry() if window.isFullScreen() else window.geometry()
    if not geometry.isValid():
        return fallback_width, fallback_height
    return geometry.width(), geometry.height()


class EmulatorMainWindow(QMainWindow):
    """Interface de apresentação; o núcleo de emulação continua em SG1000Machine."""

    debug_logging_toggled = Signal(bool)
    debug_lines_ready = Signal(str)

    def __init__(
        self,
        *,
        on_open_rom: Callable[[], None],
        on_pause_resume: Callable[[], None],
        on_reset: Callable[[], None],
        on_configure_controls: Callable[[], None],
        on_show_controllers: Callable[[], None],
        on_vdp_snapshot: Callable[[], None],
        on_open_logs: Callable[[], None],
        on_show_machine_state: Callable[[], None],
        on_video_scale: Callable[[int], None],
        on_change_system: Callable[[], None] | None = None,
        on_configure_drivers: Callable[[], None] | None = None,
        on_configure_region: Callable[[str], None] | None = None,
        region_preference: str = "auto",
        on_configure_audio_video: Callable[[], None] | None = None,
        on_start_vgm_recording: Callable[[], None] | None = None,
        on_stop_vgm_recording: Callable[[], None] | None = None,
        video_driver: str = "auto",
        language: str = "pt-BR",
        parent=None,
    ) -> None:
        super().__init__(parent)
        self._on_video_scale = on_video_scale
        self._language = language
        self._video_scale = 3
        self.setWindowTitle(translate("main_title", language))
        self.resize(1180, 820)
        self.setMinimumSize(760, 560)
        self.setStyleSheet(
            """
            QMainWindow, QWidget { background: #10141c; color: #e7edf7; }
            QMenuBar, QMenu { background: #171e2a; color: #e7edf7; }
            QMenuBar::item { padding: 7px 10px; }
            QMenuBar::item:selected, QMenu::item:selected { background: #2d405b; }
            QToolBar { background: #171e2a; border: 0; spacing: 8px; padding: 9px; }
            QToolButton { color: #e7edf7; padding: 7px 12px; border-radius: 5px; }
            QToolButton:hover { background: #2d405b; }
            QToolButton:pressed { background: #385579; }
            QMenu { border: 1px solid #303b4d; padding: 5px; }
            QMenu::item { padding: 6px 24px 6px 10px; border-radius: 4px; }
            QStatusBar { background: #171e2a; color: #aebbd0; }
            QStatusBar::item { border: 0; }
            QLabel { color: #e7edf7; }
            QPushButton {
                padding: 8px 14px;
                border: 1px solid #39475d;
                border-radius: 6px;
                background: #202a3a;
            }
            QPushButton:hover { background: #2d405b; }
            QPushButton:pressed { background: #385579; }
            QPushButton:disabled { color: #748197; background: #171e2a; }
            """
        )

        self.video_widget = VideoWidget(self, graphics_api=video_driver)
        self.video_widget.setObjectName("emulatorVideoSurface")
        screen = QWidget(self)
        layout = QVBoxLayout(screen)
        self._central_layout = layout
        layout.setContentsMargins(18, 18, 18, 18)
        layout.setSpacing(12)
        layout.addWidget(self.video_widget, 1)

        self.central_tabs = QTabWidget(self)
        self.central_tabs.setObjectName("mainContentTabs")
        self.central_tabs.addTab(screen, translate("tab_emulator", language))

        debug_page = QWidget(self)
        debug_layout = QVBoxLayout(debug_page)
        self.debug_logging_checkbox = QCheckBox(translate("debug_enable", language))
        self.debug_logging_checkbox.setObjectName("debugLoggingEnabled")
        self.debug_logging_checkbox.setToolTip(translate("debug_tooltip", language))
        debug_layout.addWidget(self.debug_logging_checkbox)
        self.debug_status_label = QLabel(translate("debug_disabled", language))
        self.debug_status_label.setObjectName("debugLoggingStatus")
        self.debug_status_label.setWordWrap(True)
        debug_layout.addWidget(self.debug_status_label)
        self.debug_output = QPlainTextEdit(debug_page)
        self.debug_output.setObjectName("debugLogOutput")
        self.debug_output.setReadOnly(True)
        self.debug_output.setMaximumBlockCount(2000)
        self.debug_output.setPlaceholderText(translate("debug_placeholder", language))
        debug_layout.addWidget(self.debug_output, 1)
        clear_debug_button = QPushButton(translate("debug_clear", language), debug_page)
        clear_debug_button.clicked.connect(self.debug_output.clear)
        debug_layout.addWidget(clear_debug_button)
        self.central_tabs.addTab(debug_page, translate("tab_debug", language))
        self.setCentralWidget(self.central_tabs)
        self.debug_logging_checkbox.toggled.connect(self.debug_logging_toggled.emit)
        self.debug_lines_ready.connect(self.append_debug_lines)

        self.status = QStatusBar(self)
        self.setStatusBar(self.status)
        self._status_bar = self.status
        self._fullscreen_ui_active = False
        self.rom_label = QLabel(translate("rom_none", language))
        self.rom_label.setMinimumWidth(220)
        self.status.addPermanentWidget(self.rom_label)

        self.open_action = QAction(translate("action_open_rom", language), self)
        self.open_action.setShortcut(QKeySequence.StandardKey.Open)
        self.open_action.triggered.connect(on_open_rom)
        self.pause_action = QAction(translate("action_pause", language), self)
        self.pause_action.setObjectName("pauseAction")
        self.pause_action.setShortcut("F5")
        self.pause_action.setEnabled(False)
        self.pause_action.triggered.connect(on_pause_resume)
        self.reset_action = QAction(translate("action_reset", language), self)
        self.reset_action.setShortcut("F10")
        self.reset_action.triggered.connect(on_reset)
        self.change_system_action = QAction("Mudar sistema/ROM…", self)
        self.change_system_action.setObjectName("changeSystemAction")
        if on_change_system is not None:
            self.change_system_action.triggered.connect(on_change_system)
        else:
            self.change_system_action.setEnabled(False)
        self.fullscreen_action = QAction(translate("action_fullscreen", language), self)
        self.fullscreen_action.setShortcuts(["F11", "Alt+Return"])
        self.fullscreen_action.setCheckable(True)
        self.fullscreen_action.toggled.connect(self._toggle_fullscreen)
        self.exit_fullscreen_action = QAction(
            translate("action_exit_fullscreen", language), self
        )
        self.exit_fullscreen_action.setShortcut("Escape")
        self.exit_fullscreen_action.setShortcutContext(
            Qt.ShortcutContext.WindowShortcut
        )
        self.exit_fullscreen_action.setEnabled(False)
        self.exit_fullscreen_action.triggered.connect(
            lambda: self._toggle_fullscreen(False)
        )
        self.addAction(self.exit_fullscreen_action)

        file_menu = self.menuBar().addMenu(translate("menu_file", language))
        file_menu.addAction(self.open_action)
        self.recent_rom_menu = file_menu.addMenu(translate("menu_recent", language))
        file_menu.addSeparator()
        file_menu.addAction(translate("action_exit", language), self.close)

        emulation_menu = self.menuBar().addMenu(translate("menu_emulation", language))
        emulation_menu.addAction(self.pause_action)
        emulation_menu.addAction(self.reset_action)
        emulation_menu.addSeparator()
        region_menu = emulation_menu.addMenu("Região do Master System")
        self.region_action_group = QActionGroup(self)
        self.region_action_group.setExclusive(True)
        self.region_actions: dict[str, QAction] = {}
        for region, label in (
            ("auto", "Automático (header/DAT; padrão EUA)"),
            ("usa", "EUA (NTSC)"),
            ("europe", "Europa (PAL)"),
            ("japan", "Japão (NTSC)"),
            ("brazil", "Brasil (PAL-M)"),
        ):
            action = QAction(label, self)
            action.setCheckable(True)
            action.setData(region)
            action.setChecked(region == region_preference)
            self.region_action_group.addAction(action)
            region_menu.addAction(action)
            self.region_actions[region] = action
            if on_configure_region is not None:
                action.triggered.connect(
                    lambda checked=False, selected=region: on_configure_region(selected)
                )
            else:
                action.setEnabled(False)
        emulation_menu.addSeparator()
        toolbar = QToolBar(translate("toolbar_main", language), self)
        self._main_toolbar = toolbar
        toolbar.setMovable(False)
        toolbar.setFloatable(False)
        toolbar.addAction(self.open_action)
        toolbar.addAction(self.change_system_action)
        toolbar.addAction(self.pause_action)
        toolbar.addAction(self.reset_action)
        toolbar.addAction(region_menu.menuAction())
        toolbar.addSeparator()
        toolbar.addAction(self.fullscreen_action)
        self.addToolBar(toolbar)

        drivers_menu = self.menuBar().addMenu(translate("menu_drivers", language))
        self.configure_drivers_action = QAction("APIs de entrada, áudio e vídeo…", self)
        if on_configure_drivers is not None:
            self.configure_drivers_action.triggered.connect(on_configure_drivers)
        else:
            self.configure_drivers_action.setEnabled(False)
        drivers_menu.addAction(self.configure_drivers_action)

        video_menu = self.menuBar().addMenu(translate("menu_video", language))
        for scale in (2, 3, 4):
            action = QAction(f"Escala de janela {scale}×", self)
            action.triggered.connect(
                lambda checked=False, selected=scale: self._on_video_scale(selected)
            )
            video_menu.addAction(action)
        video_menu.addAction(self.fullscreen_action)
        settings_menu = self.menuBar().addMenu(translate("menu_settings", language))
        self.audio_video_action = QAction("Áudio e vídeo…", self)
        if on_configure_audio_video is not None:
            self.audio_video_action.triggered.connect(on_configure_audio_video)
        else:
            self.audio_video_action.setEnabled(False)
        settings_menu.addAction(self.audio_video_action)
        controls_menu = settings_menu.addMenu("Controles")
        controls_menu.addAction("Configurar controles…", on_configure_controls)
        controls_menu.addAction("Dispositivos detectados", on_show_controllers)

        diagnostics_menu = self.menuBar().addMenu(
            translate("menu_diagnostics", language)
        )
        self.vdp_snapshot_action = QAction("Capturar snapshot VDP (JSON + PNG)…", self)
        self.vdp_snapshot_action.setShortcut("F8")
        self.vdp_snapshot_action.triggered.connect(on_vdp_snapshot)
        diagnostics_menu.addAction(self.vdp_snapshot_action)
        diagnostics_menu.addSeparator()
        self.start_vgm_recording_action = QAction("Iniciar gravação PSG em VGM…", self)
        self.start_vgm_recording_action.setShortcut("Ctrl+Shift+V")
        if on_start_vgm_recording is not None:
            self.start_vgm_recording_action.triggered.connect(on_start_vgm_recording)
        else:
            self.start_vgm_recording_action.setEnabled(False)
        diagnostics_menu.addAction(self.start_vgm_recording_action)
        self.stop_vgm_recording_action = QAction("Parar gravação VGM", self)
        if on_stop_vgm_recording is not None:
            self.stop_vgm_recording_action.triggered.connect(on_stop_vgm_recording)
        else:
            self.stop_vgm_recording_action.setEnabled(False)
        self.stop_vgm_recording_action.setEnabled(False)
        diagnostics_menu.addAction(self.stop_vgm_recording_action)
        diagnostics_menu.addSeparator()
        diagnostics_menu.addAction("Abrir pasta de logs", on_open_logs)
        diagnostics_menu.addAction("Estado da máquina", on_show_machine_state)

        help_menu = self.menuBar().addMenu(translate("menu_help", language))
        self.about_action = QAction("Sobre o Sega Emulator…", self)
        self.about_action.triggered.connect(self._show_about)
        help_menu.addAction(self.about_action)

    def append_debug_lines(self, text: str) -> None:
        """Append a bounded batch of log lines delivered by the writer thread."""
        if text:
            self.debug_output.appendPlainText(text)

    def set_debug_status(self, text: str) -> None:
        """Update the user-visible state of the optional diagnostic collector."""
        self.debug_status_label.setText(text)

    def _show_about(self) -> None:
        """Show project scope, version, credits, and license in a proper dialog."""
        QMessageBox.about(
            self,
            "Sobre o Sega Emulator",
            f"""
            <h2>Sega Emulator</h2>
            <p><b>Versão:</b> {__version__}</p>
            <p><b>Marco atual:</b> SG-1000</p>
            <p>Emulador independente com núcleo de hardware em Rust e
            interface Qt/PySide6, carregadores de ROM/perfis e integração
            PyO3 em Python.</p>
            <p><b>Escopo:</b> CPU Z80, memória e I/O, VDP TMS9918A,
            PSG SN76489AN, entrada, temporização e apresentação audiovisual,
            com limitações documentadas no repositório.</p>
            <p><b>Créditos tecnológicos:</b> Rust, Python, Qt for Python
            (PySide6), PyO3, SDL3 e PortAudio/sounddevice.</p>
            <p><b>Licença:</b> MIT. Consulte o arquivo LICENSE incluído
            na distribuição.</p>
            <p><small>SEGA e SG-1000 são marcas de seus respectivos titulares.
            Este é um projeto independente, sem afiliação oficial.</small></p>
            """,
        )

    def _toggle_fullscreen(self, enabled: bool) -> None:
        """Use a game-only presentation in fullscreen and restore the full UI."""
        if enabled == self._fullscreen_ui_active:
            return
        self._fullscreen_ui_active = enabled
        if enabled:
            self.central_tabs.setCurrentWidget(self.central_tabs.widget(0))
        self.central_tabs.tabBar().setVisible(not enabled)
        self.menuBar().setVisible(not enabled)
        self._main_toolbar.setVisible(not enabled)
        self._status_bar.setVisible(not enabled)
        if enabled:
            self._central_layout.setContentsMargins(0, 0, 0, 0)
            self._central_layout.setSpacing(0)
        else:
            self._central_layout.setContentsMargins(18, 18, 18, 18)
            self._central_layout.setSpacing(12)
        self.exit_fullscreen_action.setEnabled(enabled)
        if enabled:
            self.showFullScreen()
            self.video_widget.setFocus()
        else:
            self.showNormal()

    def changeEvent(self, event) -> None:
        super().changeEvent(event)
        # Keep the checkable action in sync when fullscreen is left with Escape.
        if hasattr(self, "fullscreen_action"):
            from PySide6.QtCore import QEvent

            if event.type() == QEvent.Type.WindowStateChange:
                self.fullscreen_action.blockSignals(True)
                self.fullscreen_action.setChecked(self.isFullScreen())
                self.fullscreen_action.blockSignals(False)

    def set_pause_state(self, paused: bool, *, enabled: bool) -> None:
        """Update the toolbar/menu pause label and availability for the loaded session."""
        label = "action_resume" if paused else "action_pause"
        self.pause_action.setText(translate(label, self._language))
        self.pause_action.setEnabled(enabled)

    def set_vgm_recording(self, active: bool) -> None:
        """Keep the VGM recording controls synchronized with recorder state."""
        self.start_vgm_recording_action.setEnabled(not active)
        self.stop_vgm_recording_action.setEnabled(active)

    def set_status_message(self, message: str, timeout_ms: int = 5000) -> None:
        self.status.showMessage(message, timeout_ms)

    def set_rom_label(self, label: str) -> None:
        self.rom_label.setText(label)

    def set_active_system(self, system: str) -> None:
        """Reflect the currently loaded hardware system in the window title."""
        labels = {"sg1000": "SG-1000", "master_system": "Master System"}
        if system not in labels:
            raise ValueError("unsupported active system")
        self.setWindowTitle(f"Sega Emulator | {labels[system]}")

    def set_video_scale(self, scale: int) -> None:
        if scale not in (2, 3, 4):
            raise ValueError("video scale must be 2, 3, or 4")
        self._video_scale = scale
        self.resize(max(760, 256 * scale + 64), max(560, 192 * scale + 150))

    def set_recent_roms(
        self,
        paths: list[str],
        on_select: Callable[[str], None],
        on_clear: Callable[[], None],
    ) -> None:
        self.recent_rom_menu.clear()
        if not paths:
            empty_action = self.recent_rom_menu.addAction("(Nenhuma ROM recente)")
            empty_action.setEnabled(False)
        else:
            for path in paths:
                action = self.recent_rom_menu.addAction(path)
                action.triggered.connect(
                    lambda checked=False, selected=path: on_select(selected)
                )
        self.recent_rom_menu.addSeparator()
        clear_action = self.recent_rom_menu.addAction("Limpar histórico")
        clear_action.triggered.connect(on_clear)
