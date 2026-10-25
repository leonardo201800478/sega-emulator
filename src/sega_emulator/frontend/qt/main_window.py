"""Main Qt window composition; emulator operations are injected as callbacks."""

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


class EmulatorMainWindow(QMainWindow):
    """Presentation-only window with stable actions and no machine ownership."""

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
        parent=None,
    ) -> None:
        super().__init__(parent)
        self._on_video_scale = on_video_scale
        self._video_scale = 3
        self.setWindowTitle("Sega Emulator — SG-1000")
        self.resize(1024, 768)
        self.setMinimumSize(640, 480)
        self.setStyleSheet(
            "QMainWindow { background: #17191f; color: #e8eaf0; }"
            "QMenuBar, QMenu { background: #222630; color: #e8eaf0; }"
            "QMenuBar::item:selected, QMenu::item:selected { background: #39445a; }"
            "QToolBar { background: #222630; border: 0; spacing: 8px; padding: 6px; }"
            "QStatusBar { background: #222630; color: #b9c2d0; }"
            "QLabel { color: #e8eaf0; }"
            "QPushButton { padding: 6px 12px; }"
        )

        self.video_widget = VideoWidget(self)
        screen = QWidget(self)
        layout = QVBoxLayout(screen)
        layout.setContentsMargins(16, 16, 16, 16)
        layout.setSpacing(12)
        layout.addWidget(self.video_widget)
        self.setCentralWidget(screen)

        self.status = QStatusBar(self)
        self.setStatusBar(self.status)
        self.rom_label = QLabel("Nenhuma ROM carregada")
        self.status.addPermanentWidget(self.rom_label)

        self.open_action = QAction("Abrir ROM…", self)
        self.open_action.setShortcut(QKeySequence.StandardKey.Open)
        self.open_action.triggered.connect(on_open_rom)
        self.pause_action = QAction("Pausar/Continuar", self)
        self.pause_action.setShortcut("F5")
        self.pause_action.triggered.connect(on_pause_resume)
        self.reset_action = QAction("Reiniciar máquina", self)
        self.reset_action.setShortcut("F10")
        self.reset_action.triggered.connect(on_reset)

        file_menu = self.menuBar().addMenu("&Arquivo")
        file_menu.addAction(self.open_action)
        self.recent_rom_menu = file_menu.addMenu("ROMs recentes")
        file_menu.addSeparator()
        file_menu.addAction("Sair", self.close)

        emulation_menu = self.menuBar().addMenu("&Emulação")
        emulation_menu.addAction(self.pause_action)
        emulation_menu.addAction(self.reset_action)

        toolbar = QToolBar("Principal", self)
        toolbar.setMovable(False)
        toolbar.addAction(self.open_action)
        toolbar.addAction(self.pause_action)
        toolbar.addAction(self.reset_action)
        self.addToolBar(toolbar)

        video_menu = self.menuBar().addMenu("&Vídeo")
        for scale in (2, 3, 4):
            action = QAction(f"Escala {scale}×", self)
            action.triggered.connect(
                lambda checked=False, selected=scale: self._on_video_scale(selected)
            )
            video_menu.addAction(action)

        controls_menu = self.menuBar().addMenu("&Controles")
        controls_menu.addAction("Configurar controles…", on_configure_controls)
        controls_menu.addAction("Dispositivos detectados", on_show_controllers)

        diagnostics_menu = self.menuBar().addMenu("&Diagnóstico")
        diagnostics_menu.addAction(
            "Capturar snapshot VDP (JSON + PNG)…", on_vdp_snapshot
        )
        diagnostics_menu.addAction("Abrir pasta de logs", on_open_logs)
        diagnostics_menu.addAction("Estado da máquina", on_show_machine_state)

        help_menu = self.menuBar().addMenu("&Ajuda")
        help_menu.addAction(
            "Sobre",
            lambda: self.status.showMessage(
                "Sega Emulator — SG-1000; precisão e compatibilidade em validação.",
                5000,
            ),
        )

    def set_status_message(self, message: str, timeout_ms: int = 5000) -> None:
        self.status.showMessage(message, timeout_ms)

    def set_rom_label(self, label: str) -> None:
        self.rom_label.setText(label)

    def set_video_scale(self, scale: int) -> None:
        if scale not in (2, 3, 4):
            raise ValueError("video scale must be 2, 3, or 4")
        self._video_scale = scale
        self.resize(
            max(640, 256 * scale + 64),
            max(480, 192 * scale + 128),
        )

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
