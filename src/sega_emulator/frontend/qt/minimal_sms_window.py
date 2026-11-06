"""Qt control panel for the Rust Master System runtime and native D3D12 viewport.

Qt owns menus, controls, status, input capture and viewport geometry only.
Framebuffer storage, pixel conversion, filters and presentation remain in Rust.
"""

from __future__ import annotations

import logging
import sys
import time
from pathlib import Path

from PySide6.QtCore import QSettings, Qt, QTimer
from PySide6.QtGui import QAction, QKeyEvent, QKeySequence
from PySide6.QtWidgets import (
    QApplication,
    QDialog,
    QFileDialog,
    QLabel,
    QMainWindow,
    QMessageBox,
    QToolBar,
    QVBoxLayout,
    QWidget,
)

from sega_emulator.frontend.qt.native_video_viewport import SmsVideoViewport
from sega_emulator.frontend.qt.preferences import FrontendPreferences
from sega_emulator.frontend.qt.rom_library_dialog import RomLibraryDialog
from sega_emulator.frontend.qt.sms_settings_dialog import SmsSettingsDialog
from sega_emulator.machine.sms import load_sms_rom
from sega_emulator.machine.sms_temporal import TemporalSMSMachine

logger = logging.getLogger(__name__)

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

    def __init__(
        self,
        *,
        region: str | None = None,
        settings: QSettings | None = None,
        parent=None,
    ) -> None:
        super().__init__(parent)
        self._settings = settings or QSettings("SegaEmulator", "SegaEmulator")
        self._preferences = FrontendPreferences.from_store(self._settings)
        saved_region = str(self._settings.value("sms_region_preference", "usa"))
        self._region = region or (
            saved_region
            if saved_region in {"usa", "brazil", "europe", "japan"}
            else "usa"
        )
        self._audio_backend = str(self._settings.value("sms_audio_backend", "wasapi"))
        if self._audio_backend not in {"wasapi", "xaudio2", "disabled"}:
            self._audio_backend = "wasapi"
        self._active_region: str | None = None
        self._machine: TemporalSMSMachine | None = None
        self._rom_path: Path | None = None
        self._pressed_actions: set[str] = set()
        self._video_output_started = False
        self._last_frame_sample: tuple[float, int] | None = None
        self._last_telemetry_log_time = 0.0
        self._audio_output_error: str | None = None
        self._video_telemetry_timer = QTimer(self)
        self._video_telemetry_timer.setInterval(250)
        self._video_telemetry_timer.timeout.connect(self._refresh_video_telemetry)

        self.setWindowTitle("Sega Emulator — Master System")
        self.resize(960, 720)
        self.setMinimumSize(640, 480)

        central_widget = QWidget(self)
        central_layout = QVBoxLayout(central_widget)
        central_layout.setContentsMargins(12, 12, 12, 12)
        central_layout.setSpacing(8)
        self.video_viewport = SmsVideoViewport(central_widget)
        self.video_viewport.set_integer_scale(self._preferences.integer_scale)
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

        file_menu = self.menuBar().addMenu("Arquivo")
        file_menu.addAction(self.open_action)
        self.library_action = QAction("Biblioteca de ROMs…", self)
        self.library_action.triggered.connect(self._open_library_dialog)
        file_menu.addAction(self.library_action)
        file_menu.addAction(self.close_rom_action)
        file_menu.addSeparator()
        exit_action = QAction("Sair", self)
        exit_action.triggered.connect(self.close)
        file_menu.addAction(exit_action)

        emulation_menu = self.menuBar().addMenu("Emulação")
        emulation_menu.addAction(self.run_action)
        emulation_menu.addAction(self.reset_action)

        settings_menu = self.menuBar().addMenu("Configurações")
        self.settings_action = QAction("Áudio, vídeo e sistema…", self)
        self.settings_action.setShortcut("Ctrl+,")
        self.settings_action.triggered.connect(self._show_settings_dialog)
        settings_menu.addAction(self.settings_action)

        help_menu = self.menuBar().addMenu("Ajuda")
        about_action = QAction("Sobre o Sega Emulator…", self)
        about_action.triggered.connect(self._show_about)
        help_menu.addAction(about_action)

        self.statusBar().showMessage(
            "Viewport 4:3 nativo — apresentação D3D12 ativada ao carregar uma ROM"
        )
        self._update_actions()

    def _open_rom_dialog(self) -> None:
        filename, _ = QFileDialog.getOpenFileName(
            self,
            "Abrir ROM do Master System",
            "",
            "ROMs Master System (*.sms *.bin *.rom *.zip);;Todos os arquivos (*)",
        )
        if filename:
            self.load_rom(Path(filename))

    def _open_library_dialog(self) -> None:
        """Open the existing ROM browser directly on the Master System catalog."""
        dialog = RomLibraryDialog(
            self._settings,
            initial_system="master_system",
            parent=self,
        )
        if (
            dialog.exec() == QDialog.DialogCode.Accepted
            and dialog.selected_rom is not None
        ):
            if dialog.selected_system != "master_system":
                QMessageBox.information(
                    self,
                    "Sistema ainda não disponível",
                    "Esta janela executa somente o Master System temporal.",
                )
                return
            self.load_rom(dialog.selected_rom)

    def load_rom(self, path: Path) -> None:
        """Load a cartridge and create the Rust temporal machine."""
        try:
            loaded_rom = load_sms_rom(path)
            machine = TemporalSMSMachine(loaded_rom.data, region=self._region)
        except (OSError, RuntimeError, ValueError, TypeError) as exc:
            logger.exception("Falha ao carregar ROM SMS: %s", path.name)
            QMessageBox.critical(self, "Falha ao abrir ROM", str(exc))
            return

        display_name = loaded_rom.display_name
        logger.info(
            "SMS ROM carregada: name=%s size=%d sha256=%s region=%s",
            display_name,
            len(loaded_rom.data),
            loaded_rom.sha256,
            self._region,
        )
        audio_error = self._configure_audio_output(machine)

        if self._machine is not None:
            self._video_telemetry_timer.stop()
            self._video_output_started = False
            try:
                self._machine.stop()
            except RuntimeError:
                pass
            self._close_video_output(self._machine)
            try:
                self._machine.close_audio_output()
            except RuntimeError:
                pass

        self._video_output_started = False
        self._last_frame_sample = None
        self._last_telemetry_log_time = 0.0
        self._audio_output_error = audio_error
        self._machine = machine
        self._rom_path = path
        self._active_region = self._region
        self._pressed_actions.clear()
        self.setWindowTitle(f"Sega Emulator — {display_name}")
        self.status_label.setText(
            f"ROM carregada: {display_name}\n"
            "Pressione F5 ou Iniciar para executar a emulação.\n"
            f"{self._audio_status_text()}"
        )
        self.statusBar().showMessage(
            f"{display_name} | {self._region} | {self._audio_status_text()}"
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
        machine = self._machine
        self._video_telemetry_timer.stop()
        stop_error = None
        video_error = None
        audio_error = None
        if machine is not None:
            try:
                machine.stop()
            except RuntimeError as exc:
                stop_error = str(exc)
            video_error = self._close_video_output(machine)
            try:
                machine.close_audio_output()
            except RuntimeError as exc:
                audio_error = str(exc)
        self._video_output_started = False
        self._last_frame_sample = None
        self._machine = None
        self._rom_path = None
        self._active_region = None
        self._pressed_actions.clear()
        self._audio_output_error = None
        self.status_label.setText("Abra uma ROM do Master System")
        self.setWindowTitle("Sega Emulator — Master System")
        if stop_error:
            self.statusBar().showMessage(
                f"ROM fechada; falha ao parar worker: {stop_error}"
            )
        elif video_error:
            self.statusBar().showMessage(
                f"ROM fechada; falha ao encerrar vídeo: {video_error}"
            )
        elif audio_error:
            self.statusBar().showMessage(
                f"ROM fechada; falha ao encerrar áudio: {audio_error}"
            )
        else:
            self.statusBar().showMessage("ROM fechada")
        self._update_actions()

    def _configure_audio_output(self, machine: TemporalSMSMachine) -> str | None:
        """Apply persisted host audio preferences through the coarse Rust API."""
        if self._audio_backend == "disabled":
            try:
                machine.close_audio_output()
            except RuntimeError as exc:
                logger.exception("Falha ao desativar áudio SMS")
                return str(exc)
            logger.info("SMS audio output disabled by user preference")
            return None
        try:
            machine.configure_audio_output(
                self._audio_backend,
                sample_rate=self._preferences.audio_sample_rate,
                buffer_size=self._preferences.audio_buffer_size,
                queue_blocks=self._preferences.audio_queue_blocks,
                volume=self._preferences.audio_volume,
                balance=self._preferences.audio_balance,
            )
        except (OSError, RuntimeError, ValueError) as exc:
            # Audio is a host service: report its failure without blocking video
            # presentation or preventing the cartridge from being loaded.
            logger.exception("Falha ao configurar áudio SMS (%s)", self._audio_backend)
            try:
                machine.close_audio_output()
            except RuntimeError:
                logger.exception("Falha ao encerrar áudio SMS após erro de configuração")
            return str(exc)
        logger.info(
            "SMS audio configured: backend=%s sample_rate=%d buffer=%d queue=%d "
            "volume=%d balance=%d",
            self._audio_backend,
            self._preferences.audio_sample_rate,
            self._preferences.audio_buffer_size,
            self._preferences.audio_queue_blocks,
            self._preferences.audio_volume,
            self._preferences.audio_balance,
        )
        return None

    def _audio_status_text(self) -> str:
        """Describe the selected native audio endpoint without exposing PCM."""
        backend_label = {
            "wasapi": "WASAPI",
            "xaudio2": "XAudio2",
        }.get(self._audio_backend, self._audio_backend)
        if self._audio_output_error:
            return f"Áudio {backend_label} indisponível: {self._audio_output_error}"
        if self._audio_backend == "disabled":
            return "Áudio desativado"
        rate = self._preferences.audio_sample_rate
        return (
            f"Áudio {backend_label} inicializado ({rate:,} Hz)".replace(",", ".")
        )

    def _show_settings_dialog(self) -> None:
        """Edit and persist SMS host preferences; apply only supported settings."""
        dialog = SmsSettingsDialog(
            self._preferences,
            region=self._region,
            audio_backend=self._audio_backend,
            parent=self,
        )
        if dialog.exec() != dialog.DialogCode.Accepted:
            return

        previous_region = self._region
        self._preferences = dialog.preferences()
        self._audio_backend = dialog.selected_audio_backend
        self._region = dialog.selected_region
        self._preferences.save_to_store(self._settings)
        self._settings.setValue("sms_audio_backend", self._audio_backend)
        self._settings.setValue("sms_region_preference", self._region)
        self.video_viewport.set_integer_scale(self._preferences.integer_scale)

        if self._machine is not None:
            self._audio_output_error = self._configure_audio_output(self._machine)
            self.statusBar().showMessage(self._audio_status_text())
        if self._machine is not None and self._region != previous_region:
            QMessageBox.information(
                self,
                "Região atualizada",
                "A nova região será aplicada quando a ROM atual for carregada novamente.",
            )
        else:
            self.statusBar().showMessage("Configurações SMS salvas")

    def _show_about(self) -> None:
        """Show project identity and the current SMS runtime boundary."""
        QMessageBox.about(
            self,
            "Sobre o Sega Emulator",
            "Sega Emulator — Master System\n"
            "Núcleo, Scheduler, VDP, PSG e apresentação D3D12 em Rust.\n"
            "Qt/Python controla a interface e as preferências do host.",
        )

    def _close_video_output(self, machine: TemporalSMSMachine) -> str | None:
        """Release D3D12 and return an error without interrupting Qt teardown."""
        try:
            machine.close_video_output()
        except RuntimeError as exc:
            logger.exception("Falha ao encerrar saída de vídeo D3D12 SMS")
            return str(exc)
        return None

    def _sample_frame_rate(self, frame_number: int) -> str:
        """Measure host-observed frame throughput without controlling emulation."""
        now = time.monotonic()
        previous = self._last_frame_sample
        self._last_frame_sample = (now, frame_number)
        if previous is None:
            return "FPS: calculando"
        elapsed = now - previous[0]
        if elapsed <= 0:
            return "FPS: indisponível"
        frames = max(0, frame_number - previous[1])
        return f"FPS: {frames / elapsed:.1f}"

    def _refresh_video_telemetry(self) -> None:
        """Poll native execution and presentation counters without touching pixels."""
        machine = self._machine
        if machine is None or not self._video_output_started:
            self._video_telemetry_timer.stop()
            return

        runtime_error = machine.runtime_error
        if runtime_error:
            logger.error("SMS native runtime stopped: %s", runtime_error)
            self._video_telemetry_timer.stop()
            self.status_label.setText(
                f"Falha de execução do núcleo Rust: {runtime_error}"
            )
            self.statusBar().showMessage(f"Execução SMS interrompida: {runtime_error}")
            return

        error = machine.video_output_error
        if error:
            logger.error("SMS D3D12 presentation failed: %s", error)
            self._video_telemetry_timer.stop()
            self.status_label.setText(f"Falha na apresentação D3D12: {error}")
            self.statusBar().showMessage(f"Vídeo D3D12: falha — {error}")
            return

        if not machine.video_output_is_running:
            logger.warning("SMS D3D12 presentation worker stopped unexpectedly")
            self._video_telemetry_timer.stop()
            self.statusBar().showMessage(
                "Vídeo D3D12: worker de apresentação encerrado"
            )
            return

        frame_number = machine.frame_number
        frame_rate = self._sample_frame_rate(frame_number)
        prefix = f"Frames {frame_number} | {frame_rate} | "
        published = machine.video_last_published_sequence
        presented = machine.video_last_presented_sequence
        replaced = machine.video_replaced_frames
        recoveries = machine.video_device_recovery_count
        now = time.monotonic()
        if now - self._last_telemetry_log_time >= 5.0:
            logger.info(
                "SMS output telemetry: region=%s frame=%d fps=%s "
                "published=%d presented=%d replaced=%d device_recoveries=%d",
                self._active_region or self._region,
                frame_number,
                frame_rate,
                published,
                presented,
                replaced,
                recoveries,
            )
            self._last_telemetry_log_time = now
        if presented:
            self.statusBar().showMessage(
                f"{prefix}D3D12: apresentado {presented}/{published} | "
                f"substituídos {replaced} | recuperações {recoveries}"
            )
        elif published:
            self.statusBar().showMessage(
                f"{prefix}D3D12: {published} frame(s) publicados; "
                "aguardando primeiro Present"
            )
        else:
            self.statusBar().showMessage(
                f"{prefix}D3D12 inicializado; aguardando o primeiro frame da emulação"
            )

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
            "Host D3D12 inicializado; aguardando o primeiro frame. "
            "Pressione F5/Iniciar para executar.\n"
            f"{self._audio_status_text()}"
        )
        self._video_telemetry_timer.start()
        self._refresh_video_telemetry()

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
        self._video_telemetry_timer.stop()
        if self._machine is not None:
            try:
                self._machine.stop()
            except RuntimeError as exc:
                self.statusBar().showMessage(f"Falha ao parar worker: {exc}")
            video_error = self._close_video_output(self._machine)
            try:
                self._machine.close_audio_output()
            except RuntimeError as exc:
                self.statusBar().showMessage(f"Falha ao encerrar áudio: {exc}")
            if video_error:
                self.statusBar().showMessage(f"Falha ao encerrar vídeo: {video_error}")
        self._video_output_started = False
        super().closeEvent(event)


def run_minimal_sms(rom_path: Path | None = None, *, region: str | None = None) -> int:
    """Run the Master System application with persisted host preferences."""
    app = QApplication.instance() or QApplication(sys.argv)
    window = MinimalSmsWindow(region=region)
    if rom_path is not None:
        window.load_rom(rom_path)
    window.show()
    return app.exec()
