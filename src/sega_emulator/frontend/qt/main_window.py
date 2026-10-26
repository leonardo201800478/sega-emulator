"""Janela Qt moderna para o frontend SG-1000; operações vêm por callbacks."""

from __future__ import annotations

from collections.abc import Callable

from PySide6.QtGui import QAction, QKeySequence
from PySide6.QtWidgets import (
    QLabel,
    QMainWindow,
    QStatusBar,
    QToolBar,
    QVBoxLayout,
    QWidget,
)

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
        on_select_emulation_backend: Callable[[], None] | None = None,
        on_configure_audio_video: Callable[[], None] | None = None,
        on_start_vgm_recording: Callable[[], None] | None = None,
        on_stop_vgm_recording: Callable[[], None] | None = None,
        parent=None,
    ) -> None:
        super().__init__(parent)
        self._on_video_scale = on_video_scale
        self._video_scale = 3
        self.setWindowTitle("Sega Emulator | SG-1000")
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
            QStatusBar { background: #171e2a; color: #aebbd0; }
            QLabel { color: #e7edf7; }
            QPushButton { padding: 7px 12px; }
            """
        )

        self.video_widget = VideoWidget(self)
        self.video_widget.setObjectName("emulatorVideoSurface")
        screen = QWidget(self)
        layout = QVBoxLayout(screen)
        layout.setContentsMargins(18, 18, 18, 18)
        layout.setSpacing(12)
        layout.addWidget(self.video_widget, 1)
        self.setCentralWidget(screen)

        self.status = QStatusBar(self)
        self.setStatusBar(self.status)
        self.rom_label = QLabel("Nenhuma ROM carregada")
        self.rom_label.setMinimumWidth(220)
        self.status.addPermanentWidget(self.rom_label)

        self.open_action = QAction("Abrir ROM…", self)
        self.open_action.setShortcut(QKeySequence.StandardKey.Open)
        self.open_action.triggered.connect(on_open_rom)
        self.pause_action = QAction("Pausar / continuar", self)
        self.pause_action.setShortcut("F5")
        self.pause_action.triggered.connect(on_pause_resume)
        self.reset_action = QAction("Reiniciar máquina", self)
        self.reset_action.setShortcut("F10")
        self.reset_action.triggered.connect(on_reset)
        self.fullscreen_action = QAction("Tela cheia", self)
        self.fullscreen_action.setShortcut("Alt+Return")
        self.fullscreen_action.setCheckable(True)
        self.fullscreen_action.toggled.connect(self._toggle_fullscreen)

        file_menu = self.menuBar().addMenu("&Arquivo")
        file_menu.addAction(self.open_action)
        self.recent_rom_menu = file_menu.addMenu("ROMs recentes")
        file_menu.addSeparator()
        file_menu.addAction("Sair", self.close)

        emulation_menu = self.menuBar().addMenu("&Emulação")
        emulation_menu.addAction(self.pause_action)
        emulation_menu.addAction(self.reset_action)
        emulation_menu.addSeparator()
        self.select_emulation_backend_action = QAction("Backend de emulação…", self)
        if on_select_emulation_backend is not None:
            self.select_emulation_backend_action.triggered.connect(
                on_select_emulation_backend
            )
        else:
            self.select_emulation_backend_action.setEnabled(False)
        emulation_menu.addAction(self.select_emulation_backend_action)

        toolbar = QToolBar("Principal", self)
        toolbar.setMovable(False)
        toolbar.setFloatable(False)
        toolbar.addAction(self.open_action)
        toolbar.addAction(self.pause_action)
        toolbar.addAction(self.reset_action)
        toolbar.addSeparator()
        toolbar.addAction(self.fullscreen_action)
        self.addToolBar(toolbar)

        video_menu = self.menuBar().addMenu("&Vídeo")
        for scale in (2, 3, 4):
            action = QAction(f"Escala de janela {scale}×", self)
            action.triggered.connect(
                lambda checked=False, selected=scale: self._on_video_scale(selected)
            )
            video_menu.addAction(action)
        video_menu.addAction(self.fullscreen_action)

        settings_menu = self.menuBar().addMenu("&Configurações")
        self.audio_video_action = QAction("Áudio e vídeo…", self)
        if on_configure_audio_video is not None:
            self.audio_video_action.triggered.connect(on_configure_audio_video)
        else:
            self.audio_video_action.setEnabled(False)
        settings_menu.addAction(self.audio_video_action)
        controls_menu = settings_menu.addMenu("Controles")
        controls_menu.addAction("Configurar controles…", on_configure_controls)
        controls_menu.addAction("Dispositivos detectados", on_show_controllers)

        diagnostics_menu = self.menuBar().addMenu("&Diagnóstico")
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

        help_menu = self.menuBar().addMenu("&Ajuda")
        help_menu.addAction(
            "Sobre",
            lambda: self.status.showMessage(
                "Sega Emulator — SG-1000 | frontend Qt em evolução.", 5000
            ),
        )

    def _toggle_fullscreen(self, enabled: bool) -> None:
        if enabled:
            self.showFullScreen()
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

    def set_vgm_recording(self, active: bool) -> None:
        """Keep the VGM recording controls synchronized with recorder state."""
        self.start_vgm_recording_action.setEnabled(not active)
        self.stop_vgm_recording_action.setEnabled(active)

    def set_status_message(self, message: str, timeout_ms: int = 5000) -> None:
        self.status.showMessage(message, timeout_ms)

    def set_rom_label(self, label: str) -> None:
        self.rom_label.setText(label)

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
