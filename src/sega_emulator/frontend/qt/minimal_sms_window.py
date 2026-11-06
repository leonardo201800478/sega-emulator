"""Qt control panel for the Rust Master System runtime and native D3D12 viewport.

Qt owns menus, controls, status, input capture and viewport geometry only.
Framebuffer storage, pixel conversion, filters and presentation remain in Rust.
"""

from __future__ import annotations

import sys
from pathlib import Path

from PySide6.QtCore import Qt
from PySide6.QtGui import QAction, QKeyEvent, QKeySequence
from PySide6.QtWidgets import (
    QApplication,
    QFileDialog,
    QLabel,
    QMainWindow,
    QMessageBox,
    QToolBar,
    QVBoxLayout,
    QWidget,
)

from sega_emulator.frontend.qt.native_video_viewport import SmsVideoViewport

from sega_emulator.machine.sms_temporal import TemporalSMSMachine

_KEY_ACTIONS = {
    Qt.Key.Key_Up: "up",
    Qt.Key.Key_Down: "down",
    Qt.Key.Key_Left: "left",
    Qt.Key.Key_Right: "right",
    Qt.Key.Key_Z: "button_1",
    Qt.Key.Key_X: "button_2",
}


class MinimalSmsWindow(QMainWindow):
    """SMS control panel with a native Rust-owned D3D12 presentation surface."""

    def __init__(self, *, region: str = "usa", parent=None) -> None:
        super().__init__(parent)
        self._region = region
        self._machine: TemporalSMSMachine | None = None
        self._rom_path: Path | None = None
        self._pressed_actions: set[str] = set()
        self._video_output_started = False

        self.setWindowTitle("Sega Emulator — Master System")
        self.resize(960, 720)
        self.setMinimumSize(640, 480)

        central_widget = QWidget(self)
        central_layout = QVBoxLayout(central_widget)
        central_layout.setContentsMargins(12, 12, 12, 12)
        central_layout.setSpacing(8)
        self.video_viewport = SmsVideoViewport(central_widget)
        central_layout.addWidget(self.video_viewport, 1)

        self.status_label = QLabel("Abra uma ROM do Master System")
        self.status_label.setObjectName("smsSessionStatus")
        self.status_label.setAlignment(Qt.AlignmentFlag.AlignCenter)
        self.status_label.setWordWrap(True)
        self.status_label.setStyleSheet(
            "QLabel { padding: 8px; color: palette(text); }"
        )
        central_layout.addWidget(self.status_label)
        self.setCentralWidget(central_widget)

        toolbar = QToolBar("Emulação", self)
        toolbar.setObjectName("minimalSmsToolbar")
        toolbar.setMovable(False)
        self.addToolBar(toolbar)

        self.open_action = QAction("Abrir ROM", self)
        self.open_action.setShortcut(QKeySequence.StandardKey.Open)
        self.open_action.triggered.connect(self._open_rom_dialog)
        toolbar.addAction(self.open_action)

        self.run_action = QAction("Iniciar", self)
        self.run_action.setShortcut("F5")
        self.run_action.triggered.connect(self._toggle_running)
        toolbar.addAction(self.run_action)

        self.reset_action = QAction("Reset", self)
        self.reset_action.setShortcut("F10")
        self.reset_action.triggered.connect(self._reset)
        toolbar.addAction(self.reset_action)

        self.close_rom_action = QAction("Fechar ROM", self)
        self.close_rom_action.triggered.connect(self._close_rom)
        toolbar.addAction(self.close_rom_action)

        self.statusBar().showMessage(
            "Viewport 4:3 nativo — apresentação D3D12 ativada ao carregar uma ROM"
        )
        self._update_actions()

    def _open_rom_dialog(self) -> None:
        filename, _ = QFileDialog.getOpenFileName(
            self,
            "Abrir ROM do Master System",
            "",
            "ROMs Master System (*.sms *.bin *.rom);;Todos os arquivos (*)",
        )
        if filename:
            self.load_rom(Path(filename))

    def load_rom(self, path: Path) -> None:
        """Load a cartridge and create the Rust temporal machine."""
        try:
            rom = path.read_bytes()
            machine = TemporalSMSMachine(rom, region=self._region)
        except (OSError, RuntimeError, ValueError, TypeError) as exc:
            QMessageBox.critical(self, "Falha ao abrir ROM", str(exc))
            return

        if self._machine is not None:
            self._machine.stop()
            self._machine.close_video_output()
            self._machine.close_audio_output()

        self._video_output_started = False
        self._machine = machine
        self._rom_path = path
        self._pressed_actions.clear()
        self.setWindowTitle(f"Sega Emulator — {path.name}")
        self.status_label.setText(
            f"ROM carregada: {path.name}\n"
            "Preparando a apresentação nativa D3D12 no viewport 4:3."
        )
        self.statusBar().showMessage(
            f"{path.name} | {self._region} | vídeo: inicializando host D3D12"
        )
        self._start_video_output_if_visible()
        self._update_actions()

    def _toggle_running(self) -> None:
        machine = self._machine
        if machine is None:
            return
        if machine.is_running:
            machine.pause()
            state = "pausada"
        else:
            machine.start()
            state = "iniciada"
        self.status_label.setText(
            f"ROM: {self._rom_path.name if self._rom_path else 'carregada'}\n"
            f"Sessão {state}. Execução, vídeo e áudio permanecem no backend Rust."
        )
        self._update_actions()

    def _reset(self) -> None:
        machine = self._machine
        if machine is None:
            return
        machine.reset()
        self.status_label.setText(
            f"ROM: {self._rom_path.name if self._rom_path else 'carregada'}\n"
            "Máquina reiniciada; o host D3D12 continua apresentando frames nativos."
        )
        self._update_actions()

    def _close_rom(self) -> None:
        if self._machine is not None:
            self._machine.stop()
            self._machine.close_video_output()
            self._machine.close_audio_output()
        self._video_output_started = False
        self._machine = None
        self._rom_path = None
        self._pressed_actions.clear()
        self.status_label.setText("Abra uma ROM do Master System")
        self.setWindowTitle("Sega Emulator — Master System")
        self.statusBar().showMessage("ROM fechada")
        self._update_actions()

    def _start_video_output_if_visible(self) -> None:
        """Attach the Rust D3D12 swap chain to the Qt-owned native viewport."""
        machine = self._machine
        if machine is None or self._video_output_started or not self.isVisible():
            return
        try:
            machine.open_video_output_in_viewport(self.video_viewport.native_handle())
        except (AttributeError, OSError, RuntimeError, ValueError) as exc:
            self.status_label.setText(
                f"ROM carregada: {self._rom_path.name if self._rom_path else 'SMS'}\n"
                f"Vídeo D3D12 indisponível: {exc}"
            )
            self.statusBar().showMessage(
                "Emulação disponível; falha ao inicializar o host de vídeo nativo"
            )
            return

        self._video_output_started = True
        self.status_label.setText(
            f"ROM: {self._rom_path.name if self._rom_path else 'carregada'}\n"
            "Apresentação D3D12 ativa — pixels e filtros processados em Rust."
        )
        self.statusBar().showMessage(
            f"{self._rom_path.name if self._rom_path else 'SMS'} | "
            "vídeo: D3D12 nativo | viewport 4:3"
        )

    def showEvent(self, event) -> None:  # noqa: N802 - Qt override name
        """Start native presentation after Qt has created the viewport HWND."""
        super().showEvent(event)
        self._start_video_output_if_visible()

    def _update_actions(self) -> None:
        loaded = self._machine is not None
        self.run_action.setEnabled(loaded)
        self.reset_action.setEnabled(loaded)
        self.close_rom_action.setEnabled(loaded)
        if not loaded or not self._machine.is_running:
            self.run_action.setText("Iniciar")
        else:
            self.run_action.setText("Pausar")

    def _update_controller(self) -> None:
        if self._machine is None:
            return
        actions = self._pressed_actions
        self._machine.set_controller(
            1,
            up="up" in actions,
            down="down" in actions,
            left="left" in actions,
            right="right" in actions,
            button_1="button_1" in actions,
            button_2="button_2" in actions,
        )

    def keyPressEvent(self, event: QKeyEvent) -> None:
        action = _KEY_ACTIONS.get(event.key())
        if action is None:
            super().keyPressEvent(event)
            return
        if not event.isAutoRepeat():
            self._pressed_actions.add(action)
            self._update_controller()
        event.accept()

    def keyReleaseEvent(self, event: QKeyEvent) -> None:
        action = _KEY_ACTIONS.get(event.key())
        if action is None:
            super().keyReleaseEvent(event)
            return
        if not event.isAutoRepeat():
            self._pressed_actions.discard(action)
            self._update_controller()
        event.accept()

    def closeEvent(self, event) -> None:  # noqa: N802 - Qt override name
        if self._machine is not None:
            self._machine.stop()
            self._machine.close_video_output()
            self._machine.close_audio_output()
        self._video_output_started = False
        super().closeEvent(event)


def run_minimal_sms(rom_path: Path | None = None, *, region: str = "usa") -> int:
    """Run the minimal Master System control application."""
    app = QApplication.instance() or QApplication(sys.argv)
    window = MinimalSmsWindow(region=region)
    if rom_path is not None:
        window.load_rom(rom_path)
    window.show()
    return app.exec()
