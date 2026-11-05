"""Minimal Qt host for the Scheduler-driven Master System runtime.

This is intentionally an emulator window, not a product frontend: one viewport,
a few session controls, and direct keyboard input. Hardware timing stays in Rust.
"""

from __future__ import annotations

import sys
from pathlib import Path

from PySide6.QtCore import Qt, QTimer
from PySide6.QtGui import QAction, QImage, QKeyEvent, QPixmap
from PySide6.QtWidgets import (
    QApplication,
    QFileDialog,
    QLabel,
    QMainWindow,
    QMessageBox,
    QToolBar,
)

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
    """Single-window SMS emulator shell with no library or advanced frontend."""

    def __init__(self, *, region: str = "usa", parent=None) -> None:
        super().__init__(parent)
        self._region = region
        self._machine: TemporalSMSMachine | None = None
        self._rom_path: Path | None = None
        self._pressed_actions: set[str] = set()

        self.setWindowTitle("Sega Emulator — Master System")
        self.resize(800, 640)
        self.setMinimumSize(512, 432)

        self.viewport = QLabel("Abra uma ROM do Master System")
        self.viewport.setObjectName("smsViewport")
        self.viewport.setAlignment(Qt.AlignmentFlag.AlignCenter)
        self.viewport.setMinimumSize(256, 192)
        self.viewport.setStyleSheet(
            "QLabel { background: #000000; color: #d0d0d0; }"
        )
        self.setCentralWidget(self.viewport)

        toolbar = QToolBar("Emulação", self)
        toolbar.setObjectName("minimalSmsToolbar")
        toolbar.setMovable(False)
        self.addToolBar(toolbar)

        self.open_action = QAction("Abrir ROM", self)
        self.open_action.setShortcut(QKeySequence.StandardKey.Open)
        self.open_action.triggered.connect(self._open_rom_dialog)
        toolbar.addAction(self.open_action)

        self.run_action = QAction("Executar", self)
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

        self.statusBar().showMessage("Pronto — abra uma ROM .sms")
        self._update_actions()

        self._frame_timer = QTimer(self)
        self._frame_timer.setTimerType(Qt.TimerType.PreciseTimer)
        self._frame_timer.setInterval(20 if region == "europe" else 16)
        self._frame_timer.timeout.connect(self._run_one_frame)

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
            self._machine.close_audio_output()

        self._machine = machine
        self._rom_path = path
        self._pressed_actions.clear()
        try:
            machine.configure_audio_output("wasapi")
            audio_status = "áudio nativo ativo"
        except RuntimeError as exc:
            audio_status = f"áudio indisponível: {exc}"

        self.setWindowTitle(f"Sega Emulator — {path.name}")
        self.viewport.setText("Pressione Executar ou F5")
        self.statusBar().showMessage(f"{path.name} | {self._region} | {audio_status}")
        self._update_actions()

    def _toggle_running(self) -> None:
        machine = self._machine
        if machine is None:
            return
        if machine.is_running:
            machine.pause()
            self._frame_timer.stop()
            self.run_action.setText("Executar")
        else:
            machine.start()
            self._frame_timer.start()
            self.run_action.setText("Pausar")
        self._update_actions()

    def _run_one_frame(self) -> None:
        machine = self._machine
        if machine is None or not machine.is_running:
            return
        try:
            machine.step_frame()
            self._present_frame()
            self.statusBar().showMessage(
                f"{self._rom_path.name if self._rom_path else 'ROM'}"
                f" | quadro {machine.frame_number}"
                f" | {machine.total_instructions} instruções"
            )
        except (RuntimeError, ValueError) as exc:
            machine.pause()
            self._frame_timer.stop()
            self.run_action.setText("Executar")
            QMessageBox.critical(self, "Erro durante a emulação", str(exc))
            self._update_actions()

    def _present_frame(self) -> None:
        machine = self._machine
        if machine is None:
            return
        frame = machine.framebuffer
        width = machine.width
        height = machine.height
        expected_size = width * height * 3
        if len(frame) != expected_size:
            raise RuntimeError(
                f"Framebuffer RGB24 inválido: esperado {expected_size} bytes, "
                f"recebido {len(frame)}."
            )
        image = QImage(
            frame,
            width,
            height,
            width * 3,
            QImage.Format.Format_RGB888,
        ).copy()
        pixmap = QPixmap.fromImage(image)
        self.viewport.setPixmap(
            pixmap.scaled(
                self.viewport.size(),
                Qt.AspectRatioMode.KeepAspectRatio,
                Qt.TransformationMode.FastTransformation,
            )
        )

    def _reset(self) -> None:
        machine = self._machine
        if machine is None:
            return
        was_running = machine.is_running
        self._frame_timer.stop()
        machine.reset()
        self.viewport.setText("Reiniciado")
        self.run_action.setText("Executar")
        if was_running:
            machine.start()
            self._frame_timer.start()
            self.run_action.setText("Pausar")
        self._update_actions()

    def _close_rom(self) -> None:
        self._frame_timer.stop()
        if self._machine is not None:
            self._machine.stop()
            self._machine.close_audio_output()
        self._machine = None
        self._rom_path = None
        self._pressed_actions.clear()
        self.viewport.setPixmap(QPixmap())
        self.viewport.setText("Abra uma ROM do Master System")
        self.setWindowTitle("Sega Emulator — Master System")
        self.statusBar().showMessage("ROM fechada")
        self._update_actions()

    def _update_actions(self) -> None:
        loaded = self._machine is not None
        self.run_action.setEnabled(loaded)
        self.reset_action.setEnabled(loaded)
        self.close_rom_action.setEnabled(loaded)
        if not loaded:
            self.run_action.setText("Executar")
        elif not self._machine.is_running:
            self.run_action.setText("Executar")

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
        self._frame_timer.stop()
        if self._machine is not None:
            self._machine.stop()
            self._machine.close_audio_output()
        super().closeEvent(event)


def run_minimal_sms(rom_path: Path | None = None, *, region: str = "usa") -> int:
    """Run the minimal Master System Qt application."""
    app = QApplication.instance() or QApplication(sys.argv)
    window = MinimalSmsWindow(region=region)
    if rom_path is not None:
        window.load_rom(rom_path)
    window.show()
    return app.exec()
