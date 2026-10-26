"""Qt diagnostics for the standalone Rust SG-1000 CPU/memory slice.

This is intentionally a developer tool, not a playable video/audio backend:
the standalone crate does not yet implement VDP, PSG, or machine scheduling.
"""

from __future__ import annotations

from PySide6.QtCore import QTimer
from PySide6.QtWidgets import (
    QDialog,
    QHBoxLayout,
    QLabel,
    QMessageBox,
    QPlainTextEdit,
    QPushButton,
    QVBoxLayout,
)

from sega_emulator.machine.cartridge_profiles import cartridge_profile


class StandaloneCoreDialog(QDialog):
    """Inspect and execute the Python-independent Rust core against a ROM."""

    def __init__(self, rom: bytes, rom_name: str, parent=None) -> None:
        super().__init__(parent)
        try:
            from sega_z80_core import StandaloneSG1000Core
        except ImportError as exc:
            raise RuntimeError(
                "O adaptador do core SG-1000 standalone não está instalado. "
                "Compile a extensão Rust com o workflow Qt/Rust."
            ) from exc

        self._rom = rom
        self._core_type = StandaloneSG1000Core
        self._machine = self._new_machine()
        self.setWindowTitle(f"Core Rust standalone — {rom_name}")
        self.resize(640, 520)

        layout = QVBoxLayout(self)
        warning = QLabel(
            "Modo de diagnóstico: CPU Z80 e memória Rust. "
            "VDP, PSG, entrada e execução de quadros ainda não fazem parte "
            "deste core; esta janela não representa a execução visual do jogo."
        )
        warning.setWordWrap(True)
        warning.setStyleSheet("color: #e5b567; padding: 8px;")
        layout.addWidget(warning)

        self.state_view = QPlainTextEdit(self)
        self.state_view.setReadOnly(True)
        self.state_view.setLineWrapMode(QPlainTextEdit.LineWrapMode.NoWrap)
        layout.addWidget(self.state_view, 1)

        controls = QHBoxLayout()
        self.step_button = QPushButton("Passo (F7)")
        self.run_100_button = QPushButton("Executar 100")
        self.run_1000_button = QPushButton("Executar 1.000")
        self.run_button = QPushButton("Executar continuamente")
        self.reset_button = QPushButton("Reiniciar")
        for button in (
            self.step_button,
            self.run_100_button,
            self.run_1000_button,
            self.run_button,
            self.reset_button,
        ):
            controls.addWidget(button)
        layout.addLayout(controls)

        self._timer = QTimer(self)
        self._timer.setInterval(20)
        self._timer.timeout.connect(self._run_batch)
        self.step_button.clicked.connect(self.step_once)
        self.run_100_button.clicked.connect(lambda: self.run_batch(100))
        self.run_1000_button.clicked.connect(lambda: self.run_batch(1000))
        self.run_button.clicked.connect(self.toggle_running)
        self.reset_button.clicked.connect(self.reset_machine)
        self._refresh_state()

    def _new_machine(self):
        profile = cartridge_profile(self._rom)
        return self._core_type(
            self._rom,
            profile.internal_ram_size,
            profile.cartridge_ram_size,
            profile.cartridge_ram_start,
            profile.cartridge_ram_end if profile.cartridge_ram_size else 0,
        )

    def step_once(self) -> None:
        try:
            self._machine.step()
            self._refresh_state()
        except Exception as exc:
            self._report_execution_error(exc)

    def run_batch(self, count: int) -> None:
        try:
            self._machine.run_instructions(count)
            self._refresh_state()
        except Exception as exc:
            self._report_execution_error(exc)

    def toggle_running(self) -> None:
        if self._timer.isActive():
            self._timer.stop()
            self.run_button.setText("Executar continuamente")
        else:
            self._timer.start()
            self.run_button.setText("Pausar execução")

    def _run_batch(self) -> None:
        self.run_batch(500)

    def reset_machine(self) -> None:
        self._timer.stop()
        self.run_button.setText("Executar continuamente")
        try:
            self._machine = self._new_machine()
            self._refresh_state()
        except Exception as exc:
            self._report_execution_error(exc)

    def _report_execution_error(self, error: Exception) -> None:
        self._timer.stop()
        self.run_button.setText("Executar continuamente")
        QMessageBox.warning(self, "Core Rust interrompido", str(error))
        self._refresh_state()

    def _refresh_state(self) -> None:
        machine = self._machine
        lines = [
            "Backend: sega-sg1000-core (Rust standalone)",
            f"PC={machine.pc:04X}  SP={machine.sp:04X}  "
            f"AF={machine.a:02X}{machine.f:02X}",
            f"BC={machine.bc:04X}  DE={machine.de:04X}  HL={machine.hl:04X}",
            f"T-states nominais: {machine.total_tstates}",
            f"HALT: {'sim' if machine.halted else 'não'}",
            "",
            "ROM / memória (32 bytes por região):",
            "0000: " + self._hex_bytes(0x0000),
            "0020: " + self._hex_bytes(0x0020),
            "C000: " + self._hex_bytes(0xC000),
            "C020: " + self._hex_bytes(0xC020),
            "",
            "Observação: os T-states são os ciclos nominais atualmente "
            "implementados; não constituem certificação de precisão ciclo a ciclo.",
        ]
        self.state_view.setPlainText("\n".join(lines))

    def _hex_bytes(self, start: int) -> str:
        return " ".join(
            f"{self._machine.read_memory(start + offset):02X}" for offset in range(32)
        )
