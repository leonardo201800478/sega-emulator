"""Full-screen startup flow and horizontal ROM carousel for the Qt frontend.

The browser discovers candidate files only. Launching a game is delegated to the
existing frontend callback, which still validates the ROM through the current
Rust-backed SG-1000 runtime.
"""

from __future__ import annotations

from pathlib import Path
from threading import Event

from PySide6.QtCore import (
    QObject,
    QRunnable,
    QSettings,
    Signal,
    QStandardPaths,
    Qt,
    QThreadPool,
)
from PySide6.QtWidgets import (
    QDialog,
    QFileDialog,
    QGridLayout,
    QHBoxLayout,
    QLabel,
    QLineEdit,
    QListWidget,
    QListWidgetItem,
    QMessageBox,
    QProgressBar,
    QPushButton,
    QStackedWidget,
    QVBoxLayout,
    QWidget,
)

from sega_emulator.rom_library import (
    DEFAULT_DIRECTORY_NAMES,
    RomEntry,
    RomLibrary,
    ScanResult,
    SUPPORTED_SYSTEMS,
)

SYSTEM_LABELS = {
    "sg1000": "SG-1000",
    "master_system": "Master System",
    "game_gear": "Game Gear",
    "mega_drive": "Mega Drive",
    "sega_cd": "Sega CD",
    "32x": "32X",
}


class _ScanSignals(QObject):
    progress = Signal(int, int, str)
    completed = Signal(object)
    failed = Signal(str)


class _ScanTask(QRunnable):
    def __init__(
        self,
        index_path: Path,
        system: str,
        directories: tuple[Path, ...],
        *,
        full: bool,
        cancel: Event,
    ) -> None:
        super().__init__()
        self.index_path = index_path
        self.system = system
        self.directories = directories
        self.full = full
        self.cancel = cancel
        self.signals = _ScanSignals()

    def run(self) -> None:
        try:
            library = RomLibrary(self.index_path)
            result = library.scan(
                self.system,
                self.directories,
                full=self.full,
                recursive=True,
                progress=self.signals.progress.emit,
                cancel=self.cancel,
            )
        except Exception as exc:
            self.signals.failed.emit(f"{type(exc).__name__}: {exc}")
        else:
            self.signals.completed.emit(result)


class RomLibraryDialog(QDialog):
    """Startup wizard and ROM browser. It never starts emulation itself."""

    def __init__(
        self,
        settings: QSettings,
        *,
        parent: QWidget | None = None,
    ) -> None:
        super().__init__(parent)
        self.settings = settings
        self.selected_rom: Path | None = None
        self.selected_system: str | None = None
        self._index_path = self._cache_path()
        self._pool = QThreadPool.globalInstance()
        self._scan_cancel: Event | None = None
        self._scan_task: _ScanTask | None = None
        self._entries: tuple[RomEntry, ...] = ()
        self.setWindowTitle("Sega Emulator — Biblioteca")
        self.setMinimumSize(900, 600)
        self.setStyleSheet(
            """
            QDialog, QWidget { background: #0c1424; color: #e9f2ff; }
            QLabel#heroTitle { color: #66d9ff; font-size: 30px; font-weight: 700; }
            QLabel#subtitle { color: #9bb3d2; font-size: 14px; }
            QPushButton {
                background: #172844; border: 1px solid #31547c;
                border-radius: 8px; padding: 10px 16px;
            }
            QPushButton:hover { background: #24436b; }
            QPushButton:disabled { color: #71819a; background: #111a2a; }
            QListWidget {
                background: #0a1020; border: 1px solid #294363;
                border-radius: 10px; padding: 12px;
            }
            QListWidget::item {
                background: #14233b; border: 1px solid #31547c;
                border-radius: 8px; padding: 14px; margin: 6px;
                min-width: 145px; min-height: 105px;
            }
            QListWidget::item:selected {
                border: 2px solid #42d8ff; background: #203c60;
            }
            QLineEdit {
                background: #111e33; border: 1px solid #31547c;
                border-radius: 6px; padding: 8px;
            }
            QProgressBar {
                background: #111e33; border: 1px solid #31547c;
                border-radius: 5px; text-align: center; min-height: 16px;
            }
            QProgressBar::chunk { background: #28b7e8; border-radius: 4px; }
            """
        )
        self._build_pages()
        self._show_splash()

    @staticmethod
    def _cache_path() -> Path:
        location = QStandardPaths.writableLocation(
            QStandardPaths.StandardLocation.AppLocalDataLocation
        )
        base = Path(location) if location else Path.home() / ".sega-emulator"
        return base / "library-index.json"

    def _build_pages(self) -> None:
        root = QVBoxLayout(self)
        root.setContentsMargins(24, 24, 24, 24)
        root.setSpacing(16)
        self.pages = QStackedWidget(self)
        root.addWidget(self.pages, 1)

        self.splash_page = QWidget()
        splash = QVBoxLayout(self.splash_page)
        splash.addStretch(1)
        title = QLabel("SEGA EMULATOR", self.splash_page)
        title.setObjectName("heroTitle")
        title.setAlignment(Qt.AlignmentFlag.AlignCenter)
        subtitle = QLabel(
            "Uma biblioteca para a história dos consoles Sega",
            self.splash_page,
        )
        subtitle.setObjectName("subtitle")
        subtitle.setAlignment(Qt.AlignmentFlag.AlignCenter)
        self.continue_button = QPushButton("Entrar na biblioteca  →")
        self.continue_button.clicked.connect(self._show_systems)
        splash.addWidget(title)
        splash.addWidget(subtitle)
        splash.addSpacing(28)
        splash.addWidget(self.continue_button, alignment=Qt.AlignmentFlag.AlignCenter)
        splash.addStretch(1)
        self.pages.addWidget(self.splash_page)

        self.system_page = QWidget()
        system_layout = QVBoxLayout(self.system_page)
        system_title = QLabel("Escolha o sistema")
        system_title.setObjectName("heroTitle")
        system_layout.addWidget(system_title)
        system_subtitle = QLabel(
            "Os sistemas sem integração funcional permanecem indisponíveis."
        )
        system_subtitle.setObjectName("subtitle")
        system_layout.addWidget(system_subtitle)
        grid = QGridLayout()
        self.system_buttons: dict[str, QPushButton] = {}
        for index, system in enumerate(SUPPORTED_SYSTEMS):
            label = SYSTEM_LABELS[system]
            button = QPushButton(f"▣\n{label}")
            button.setMinimumHeight(115)
            supported = system == "sg1000"
            button.setEnabled(supported)
            button.setToolTip(
                "Núcleo integrado" if supported else "Planejado — núcleo ainda não integrado"
            )
            if supported:
                button.clicked.connect(
                    lambda checked=False, selected=system: self._open_library(selected)
                )
            else:
                button.setText(f"▣\n{label}\n(planejado)")
            self.system_buttons[system] = button
            grid.addWidget(button, index // 3, index % 3)
        system_layout.addLayout(grid)
        back_to_splash = QPushButton("← Voltar")
        back_to_splash.clicked.connect(self._show_splash)
        system_layout.addWidget(back_to_splash, alignment=Qt.AlignmentFlag.AlignLeft)
        self.pages.addWidget(self.system_page)

        self.library_page = QWidget()
        library_layout = QVBoxLayout(self.library_page)
        header = QHBoxLayout()
        self.library_title = QLabel("Biblioteca")
        self.library_title.setObjectName("heroTitle")
        header.addWidget(self.library_title, 1)
        self.back_systems_button = QPushButton("← Sistemas")
        self.back_systems_button.clicked.connect(self._show_systems)
        header.addWidget(self.back_systems_button)
        library_layout.addLayout(header)

        directory_row = QHBoxLayout()
        self.directory_edit = QLineEdit()
        self.directory_edit.setPlaceholderText("Diretórios separados por ponto e vírgula")
        self.directory_edit.setToolTip(
            "Informe um ou mais diretórios; subpastas são incluídas automaticamente."
        )
        self.browse_button = QPushButton("Adicionar pasta…")
        self.browse_button.clicked.connect(self._browse_directory)
        self.scan_button = QPushButton("Atualizar biblioteca")
        self.scan_button.clicked.connect(lambda: self._start_scan(full=False))
        self.full_scan_button = QPushButton("Varredura completa")
        self.full_scan_button.clicked.connect(lambda: self._start_scan(full=True))
        directory_row.addWidget(self.directory_edit, 1)
        directory_row.addWidget(self.browse_button)
        directory_row.addWidget(self.scan_button)
        directory_row.addWidget(self.full_scan_button)
        library_layout.addLayout(directory_row)

        self.rom_list = QListWidget()
        self.rom_list.setViewMode(QListWidget.ViewMode.IconMode)
        self.rom_list.setFlow(QListWidget.Flow.LeftToRight)
        self.rom_list.setWrapping(False)
        self.rom_list.setResizeMode(QListWidget.ResizeMode.Adjust)
        self.rom_list.setMovement(QListWidget.Movement.Static)
        self.rom_list.setHorizontalScrollBarPolicy(
            Qt.ScrollBarPolicy.ScrollBarAsNeeded
        )
        self.rom_list.setVerticalScrollBarPolicy(Qt.ScrollBarPolicy.ScrollBarAlwaysOff)
        self.rom_list.itemSelectionChanged.connect(self._update_launch_state)
        self.rom_list.itemDoubleClicked.connect(lambda item: self._launch_selected())
        library_layout.addWidget(self.rom_list, 1)

        self.status_label = QLabel("A biblioteca ainda não foi examinada.")
        self.status_label.setWordWrap(True)
        library_layout.addWidget(self.status_label)
        self.progress_bar = QProgressBar()
        self.progress_bar.setRange(0, 100)
        self.progress_bar.setValue(0)
        library_layout.addWidget(self.progress_bar)

        footer = QHBoxLayout()
        self.cancel_scan_button = QPushButton("Cancelar varredura")
        self.cancel_scan_button.clicked.connect(self._cancel_scan)
        self.cancel_scan_button.setEnabled(False)
        self.launch_button = QPushButton("Iniciar jogo selecionado")
        self.launch_button.setEnabled(False)
        self.launch_button.clicked.connect(self._launch_selected)
        close_button = QPushButton("Fechar")
        close_button.clicked.connect(self.reject)
        footer.addWidget(self.cancel_scan_button)
        footer.addStretch(1)
        footer.addWidget(close_button)
        footer.addWidget(self.launch_button)
        library_layout.addLayout(footer)
        self.pages.addWidget(self.library_page)

    def _show_splash(self) -> None:
        self.pages.setCurrentWidget(self.splash_page)

    def _show_systems(self) -> None:
        self.pages.setCurrentWidget(self.system_page)

    def _open_library(self, system: str) -> None:
        self.selected_system = system
        self.library_title.setText(f"{SYSTEM_LABELS[system]} — Biblioteca")
        self.directory_edit.setText(";".join(self._directories_for(system)))
        self.pages.setCurrentWidget(self.library_page)
        self._start_scan(full=False)

    def _directories_for(self, system: str) -> list[str]:
        saved = self.settings.value(f"rom_directories/{system}", [], type=list)
        if saved:
            return [str(Path(value).expanduser()) for value in saved if str(value).strip()]
        documents = QStandardPaths.writableLocation(
            QStandardPaths.StandardLocation.DocumentsLocation
        )
        base = Path(documents) if documents else Path.home() / "Documents"
        return [str(base / "ROMs" / DEFAULT_DIRECTORY_NAMES[system])]

    def _browse_directory(self) -> None:
        current = self.directory_edit.text().split(";")[0].strip()
        chosen = QFileDialog.getExistingDirectory(self, "Selecionar diretório de ROMs", current)
        if chosen:
            values = [value.strip() for value in self.directory_edit.text().split(";") if value.strip()]
            if chosen not in values:
                values.append(chosen)
            self.directory_edit.setText(";".join(values))

    def _start_scan(self, *, full: bool) -> None:
        if self._scan_task is not None or self.selected_system is None:
            return
        directories = tuple(
            Path(value.strip()).expanduser()
            for value in self.directory_edit.text().split(";")
            if value.strip()
        )
        self.settings.setValue(
            f"rom_directories/{self.selected_system}",
            [str(path) for path in directories],
        )
        self._scan_cancel = Event()
        task = _ScanTask(
            self._index_path,
            self.selected_system,
            directories,
            full=full,
            cancel=self._scan_cancel,
        )
        task.signals.progress.connect(self._scan_progress)
        task.signals.completed.connect(self._scan_completed)
        task.signals.failed.connect(self._scan_failed)
        self._scan_task = task
        for button in (self.scan_button, self.full_scan_button, self.browse_button):
            button.setEnabled(False)
        self.cancel_scan_button.setEnabled(True)
        self.launch_button.setEnabled(False)
        self.progress_bar.setRange(0, 0)
        self.status_label.setText(
            "Executando varredura completa…" if full else "Atualizando a biblioteca…"
        )
        self._pool.start(task)

    def _scan_progress(self, current: int, total: int, path: str) -> None:
        if total > 0:
            self.progress_bar.setRange(0, total)
            self.progress_bar.setValue(current)
        self.status_label.setText(f"Examinando {current}/{total}: {Path(path).name}")

    def _scan_completed(self, result: object) -> None:
        if not isinstance(result, ScanResult):
            self._scan_failed("O scanner retornou um resultado inválido.")
            return
        self._entries = result.entries
        self.rom_list.clear()
        for entry in self._entries:
            item = QListWidgetItem(f"▣\n{entry.name}\n{_format_size(entry.size)}")
            item.setData(Qt.ItemDataRole.UserRole, entry.path)
            item.setToolTip(f"{entry.path}\nSHA-256: {entry.sha256}")
            self.rom_list.addItem(item)
        summary = (
            f"{len(result.entries)} arquivo(s) candidato(s) · "
            f"+{result.added} novo(s) · {result.updated} alterado(s) · "
            f"{result.removed} removido(s) · {len(result.errors)} erro(s)"
        )
        if result.cancelled:
            summary += " · cancelado"
        if result.duplicate_groups:
            summary += f" · {len(result.duplicate_groups)} grupo(s) duplicado(s)"
        self.status_label.setText(summary)
        self._finish_scan()
        self._update_launch_state()
        if result.errors:
            QMessageBox.warning(
                self,
                "Avisos do scanner",
                "\n".join(f"{error.path}: {error.message}" for error in result.errors[:8]),
            )

    def _scan_failed(self, message: str) -> None:
        self._finish_scan()
        self.status_label.setText(f"Falha na varredura: {message}")
        QMessageBox.warning(self, "Falha no scanner", message)

    def _finish_scan(self) -> None:
        self._scan_task = None
        self._scan_cancel = None
        self.progress_bar.setRange(0, 100)
        self.progress_bar.setValue(100)
        for button in (self.scan_button, self.full_scan_button, self.browse_button):
            button.setEnabled(True)
        self.cancel_scan_button.setEnabled(False)

    def _cancel_scan(self) -> None:
        if self._scan_cancel is not None:
            self._scan_cancel.set()
            self.cancel_scan_button.setEnabled(False)
            self.status_label.setText("Cancelamento solicitado…")

    def _update_launch_state(self) -> None:
        selected = bool(self.rom_list.selectedItems())
        self.launch_button.setEnabled(
            selected and self.selected_system == "sg1000" and self._scan_task is None
        )

    def _launch_selected(self) -> None:
        items = self.rom_list.selectedItems()
        if not items or self.selected_system != "sg1000":
            return
        path = items[0].data(Qt.ItemDataRole.UserRole)
        if path:
            self.selected_rom = Path(path)
            self.accept()


def _format_size(size: int) -> str:
    if size < 1024:
        return f"{size} B"
    if size < 1024 * 1024:
        return f"{size / 1024:.1f} KiB"
    return f"{size / (1024 * 1024):.1f} MiB"
