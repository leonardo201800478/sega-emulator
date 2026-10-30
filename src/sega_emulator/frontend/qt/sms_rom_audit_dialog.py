"""Qt interface for the sequential native SMS ROM audit."""

from __future__ import annotations

from collections.abc import Callable
from datetime import datetime
from pathlib import Path
from threading import Event

from PySide6.QtCore import QThread, Signal
from PySide6.QtWidgets import (
    QCheckBox,
    QDialog,
    QDoubleSpinBox,
    QFileDialog,
    QHBoxLayout,
    QLabel,
    QLineEdit,
    QMessageBox,
    QProgressBar,
    QPushButton,
    QSpinBox,
    QTableWidget,
    QTableWidgetItem,
    QVBoxLayout,
    QWidget,
)

from sega_emulator.sms_rom_audit import (
    SmsRomAuditReport,
    SmsRomAuditResult,
    discover_sms_rom_targets,
    run_sms_rom_audit,
)


class _SmsRomAuditThread(QThread):
    """Executa descoberta e emulação fora da thread gráfica."""

    discovered = Signal(int)
    progress = Signal(int, int, object)
    report_ready = Signal(object)
    failed = Signal(str)

    def __init__(
        self,
        directory: Path,
        machine_factory: Callable[[bytes], object],
        *,
        recursive: bool,
        frames_per_rom: int,
        timeout_seconds: float,
        cancel_event: Event,
        parent: QWidget | None = None,
    ) -> None:
        super().__init__(parent)
        self.directory = directory
        self.machine_factory = machine_factory
        self.recursive = recursive
        self.frames_per_rom = frames_per_rom
        self.timeout_seconds = timeout_seconds
        self.cancel_event = cancel_event

    def run(self) -> None:
        """Descobre os arquivos e executa cada ROM em uma instância Rust isolada."""
        try:
            targets = discover_sms_rom_targets(self.directory, recursive=self.recursive)
            self.discovered.emit(len(targets))
            report = run_sms_rom_audit(
                targets,
                self.machine_factory,
                root_directory=self.directory,
                recursive=self.recursive,
                frames_per_rom=self.frames_per_rom,
                timeout_seconds=self.timeout_seconds,
                cancel_event=self.cancel_event,
                on_result=lambda index, total, result: self.progress.emit(
                    index, total, result
                ),
            )
        except Exception as exc:
            self.failed.emit(f"{type(exc).__name__}: {exc}")
        else:
            self.report_ready.emit(report)


class SmsRomAuditDialog(QDialog):
    """Seleciona uma pasta e apresenta a auditoria de boot das ROMs SMS."""

    def __init__(
        self,
        machine_factory: Callable[[bytes], object],
        *,
        parent: QWidget | None = None,
    ) -> None:
        super().__init__(parent)
        self.machine_factory = machine_factory
        self._thread: _SmsRomAuditThread | None = None
        self._cancel_event: Event | None = None
        self._report: SmsRomAuditReport | None = None
        self._report_path: Path | None = None
        self.setWindowTitle("Auditoria em lote — ROMs Master System")
        self.resize(1050, 650)
        self.setMinimumSize(820, 500)

        root = QVBoxLayout(self)
        intro = QLabel(
            "Cada ROM será carregada em uma máquina SMS independente e executada "
            "pelo núcleo Rust. ZIPs são lidos sem extração permanente. "
            "A auditoria não altera sua biblioteca nem a sessão de jogo atual."
        )
        intro.setWordWrap(True)
        root.addWidget(intro)

        folder_row = QHBoxLayout()
        self.directory_edit = QLineEdit()
        self.directory_edit.setObjectName("smsAuditDirectory")
        self.directory_edit.setPlaceholderText("Selecione a pasta com suas ROMs SMS…")
        self.browse_button = QPushButton("Selecionar pasta…")
        self.browse_button.clicked.connect(self._browse_directory)
        folder_row.addWidget(self.directory_edit, 1)
        folder_row.addWidget(self.browse_button)
        root.addLayout(folder_row)

        options = QHBoxLayout()
        self.recursive_check = QCheckBox("Incluir subpastas")
        self.recursive_check.setChecked(True)
        self.frames_spin = QSpinBox()
        self.frames_spin.setRange(1, 1800)
        self.frames_spin.setValue(120)
        self.timeout_spin = QDoubleSpinBox()
        self.timeout_spin.setRange(1.0, 120.0)
        self.timeout_spin.setDecimals(1)
        self.timeout_spin.setSingleStep(1.0)
        self.timeout_spin.setValue(5.0)
        options.addWidget(self.recursive_check)
        options.addWidget(QLabel("Frames por ROM:"))
        options.addWidget(self.frames_spin)
        options.addWidget(QLabel("Timeout por ROM (s):"))
        options.addWidget(self.timeout_spin)
        options.addStretch(1)
        root.addLayout(options)

        self.progress_bar = QProgressBar()
        self.progress_bar.setRange(0, 1)
        self.progress_bar.setValue(0)
        root.addWidget(self.progress_bar)
        self.summary_label = QLabel("Aguardando seleção da pasta.")
        self.summary_label.setWordWrap(True)
        root.addWidget(self.summary_label)

        self.results_table = QTableWidget(0, 6)
        self.results_table.setObjectName("smsAuditResults")
        self.results_table.setHorizontalHeaderLabels(
            ["ROM", "Resultado", "Frames", "PC", "Tempo (ms)", "Detalhes"]
        )
        self.results_table.setSortingEnabled(False)
        self.results_table.setAlternatingRowColors(True)
        self.results_table.horizontalHeader().setStretchLastSection(True)
        self.results_table.horizontalHeader().setMinimumSectionSize(90)
        root.addWidget(self.results_table, 1)

        footer = QHBoxLayout()
        self.start_button = QPushButton("Iniciar auditoria")
        self.start_button.setObjectName("startSmsAudit")
        self.start_button.clicked.connect(self._start_audit)
        self.cancel_button = QPushButton("Cancelar")
        self.cancel_button.setEnabled(False)
        self.cancel_button.clicked.connect(self._cancel_audit)
        self.close_button = QPushButton("Fechar")
        self.close_button.clicked.connect(self.close)
        footer.addWidget(self.start_button)
        footer.addWidget(self.cancel_button)
        footer.addStretch(1)
        footer.addWidget(self.close_button)
        root.addLayout(footer)

    def _browse_directory(self) -> None:
        """Abre o seletor nativo de diretórios."""
        selected = QFileDialog.getExistingDirectory(
            self, "Selecionar pasta de ROMs SMS", self.directory_edit.text()
        )
        if selected:
            self.directory_edit.setText(selected)

    def _start_audit(self) -> None:
        """Valida opções e inicia a auditoria em uma QThread dedicada."""
        directory_text = self.directory_edit.text().strip()
        if not directory_text:
            QMessageBox.information(
                self, "Auditoria SMS", "Selecione a pasta que contém suas ROMs."
            )
            return
        directory = Path(directory_text).expanduser()
        if not directory.is_dir():
            QMessageBox.warning(
                self, "Auditoria SMS", "A pasta selecionada não existe."
            )
            return

        self.results_table.setRowCount(0)
        self.progress_bar.setRange(0, 0)
        self.summary_label.setText("Descobrindo ROMs e preparando o runtime Rust…")
        self._report = None
        self._report_path = None
        self._cancel_event = Event()
        self._thread = _SmsRomAuditThread(
            directory,
            self.machine_factory,
            recursive=self.recursive_check.isChecked(),
            frames_per_rom=self.frames_spin.value(),
            timeout_seconds=self.timeout_spin.value(),
            cancel_event=self._cancel_event,
            parent=self,
        )
        self._thread.discovered.connect(self._on_discovered)
        self._thread.progress.connect(self._on_progress)
        self._thread.report_ready.connect(self._on_report_ready)
        self._thread.failed.connect(self._on_failed)
        self._thread.finished.connect(self._set_idle)
        self._thread.start()
        self.start_button.setEnabled(False)
        self.browse_button.setEnabled(False)
        self.cancel_button.setEnabled(True)

    def _on_discovered(self, total: int) -> None:
        """Atualiza a barra após a descoberta de todos os alvos."""
        self.progress_bar.setRange(0, max(1, total))
        self.progress_bar.setValue(0)
        self.summary_label.setText(f"{total} item(ns) encontrado(s). Executando…")

    def _on_progress(self, index: int, total: int, result: SmsRomAuditResult) -> None:
        """Acrescenta o resultado individual sem bloquear a execução do worker."""
        row = self.results_table.rowCount()
        self.results_table.insertRow(row)
        details = result.details or result.error_type or ""
        values = (
            result.display_name,
            result.status,
            f"{result.frames_advanced}/{result.frames_requested}",
            f"{result.pc:04X}" if result.pc is not None else "—",
            f"{result.elapsed_ms:.1f}",
            details,
        )
        for column, value in enumerate(values):
            self.results_table.setItem(row, column, QTableWidgetItem(value))
        self.progress_bar.setRange(0, max(1, total))
        self.progress_bar.setValue(index)
        self.summary_label.setText(
            f"Auditando {index} de {total}: {result.display_name}"
        )

    def _on_report_ready(self, report: SmsRomAuditReport) -> None:
        """Persiste somente telemetria e hashes em um JSON local."""
        self._report = report
        output_dir = Path("logs") / "sms_rom_audit"
        try:
            output_dir.mkdir(parents=True, exist_ok=True)
            stamp = datetime.now().strftime("%Y%m%d-%H%M%S")
            output_path = output_dir / f"sms-rom-audit-{stamp}.json"
            output_path.write_text(report.to_json(), encoding="utf-8")
            self._report_path = output_path.resolve()
        except OSError as exc:
            self.summary_label.setText(
                f"Auditoria concluída, mas o JSON não pôde ser salvo: {exc}"
            )
        else:
            counts = (
                ", ".join(
                    f"{status}: {count}"
                    for status, count in sorted(report.counts.items())
                )
                or "nenhuma ROM encontrada"
            )
            self.summary_label.setText(
                f"Concluída — {counts}. Relatório local: {self._report_path}"
            )

    def _on_failed(self, message: str) -> None:
        """Apresenta falhas operacionais da auditoria sem encerrar a aplicação."""
        self.summary_label.setText(f"Falha na auditoria: {message}")

    def _cancel_audit(self) -> None:
        """Solicita cancelamento entre frames; o frame nativo atual termina limitado."""
        if self._cancel_event is not None:
            self._cancel_event.set()
        self.cancel_button.setEnabled(False)
        self.summary_label.setText(
            "Cancelamento solicitado; aguardando o frame nativo atual terminar…"
        )

    def _set_idle(self) -> None:
        """Restaura os controles após a thread finalizar."""
        self.start_button.setEnabled(True)
        self.browse_button.setEnabled(True)
        self.cancel_button.setEnabled(False)
        self.progress_bar.setRange(0, max(1, self.progress_bar.maximum()))
        if self._report is not None:
            self.progress_bar.setValue(self.progress_bar.maximum())

    def closeEvent(self, event) -> None:
        """Impede destruir a janela enquanto a thread ainda usa o runtime."""
        if self._thread is not None and self._thread.isRunning():
            if self._cancel_event is not None:
                self._cancel_event.set()
            self.summary_label.setText(
                "Cancelamento solicitado; feche a janela após o término do frame."
            )
            event.ignore()
            return
        super().closeEvent(event)
