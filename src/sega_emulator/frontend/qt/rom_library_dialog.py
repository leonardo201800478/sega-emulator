"""Full-screen startup flow and horizontal ROM carousel for the Qt frontend.

The browser discovers candidate files only. Launching a game is delegated to the
existing frontend callback, which still validates the ROM through the current
Rust-backed SG-1000 runtime.
"""

from __future__ import annotations

import re
from difflib import SequenceMatcher
from hashlib import sha256
from pathlib import Path
from threading import Event

from PySide6.QtCore import (
    QObject,
    QRunnable,
    QSettings,
    QSize,
    QStandardPaths,
    Qt,
    QThreadPool,
    Signal,
)
from PySide6.QtGui import QIcon, QKeySequence, QPixmap, QShortcut
from PySide6.QtWidgets import (
    QDialog,
    QFileDialog,
    QGridLayout,
    QHBoxLayout,
    QLabel,
    QLineEdit,
    QListWidget,
    QListWidgetItem,
    QMenu,
    QMessageBox,
    QProgressBar,
    QPushButton,
    QSizePolicy,
    QStackedWidget,
    QToolButton,
    QVBoxLayout,
    QWidget,
)

from sega_emulator.frontend.qt.localization import translate
from sega_emulator.frontend.qt.scraper_settings_dialog import ScraperSettingsDialog
from sega_emulator.rom_library import (
    DEFAULT_DIRECTORY_NAMES,
    SUPPORTED_SYSTEMS,
    RomEntry,
    RomLibrary,
    ScanResult,
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
        self.language = str(settings.value("ui_language", "pt-BR"))
        self.setWindowTitle(translate("app_title", self.language))
        self.setWindowFlags(
            Qt.WindowType.Window
            | Qt.WindowType.WindowMinimizeButtonHint
            | Qt.WindowType.WindowMaximizeButtonHint
            | Qt.WindowType.WindowCloseButtonHint
        )
        self.resize(1180, 780)
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
                border-radius: 8px; padding: 6px; margin: 4px;
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
            translate("splash_subtitle", self.language),
            self.splash_page,
        )
        subtitle.setObjectName("subtitle")
        subtitle.setAlignment(Qt.AlignmentFlag.AlignCenter)
        assets_directory = Path(__file__).parent / "assets" / "systems"
        artwork_row = QHBoxLayout()
        artwork_row.setSpacing(10)
        for system in SUPPORTED_SYSTEMS:
            artwork = QLabel(self.splash_page)
            pixmap = QPixmap(str(assets_directory / f"{system}.svg"))
            artwork.setPixmap(
                pixmap.scaled(
                    QSize(132, 74),
                    Qt.AspectRatioMode.KeepAspectRatio,
                    Qt.TransformationMode.SmoothTransformation,
                )
            )
            artwork.setAlignment(Qt.AlignmentFlag.AlignCenter)
            artwork_row.addWidget(artwork)
        self.continue_button = QPushButton(translate("enter_library", self.language))
        self.continue_button.clicked.connect(self._show_systems)
        splash.addWidget(title)
        splash.addWidget(subtitle)
        splash.addSpacing(18)
        splash.addLayout(artwork_row)
        splash.addSpacing(18)
        splash.addWidget(self.continue_button, alignment=Qt.AlignmentFlag.AlignCenter)
        self.scraper_settings_button = QPushButton(
            translate("scraper_settings", self.language)
        )
        self.scraper_settings_button.clicked.connect(self._open_scraper_settings)
        splash.addWidget(
            self.scraper_settings_button, alignment=Qt.AlignmentFlag.AlignCenter
        )
        splash.addStretch(1)
        self.pages.addWidget(self.splash_page)

        self.system_page = QWidget()
        system_layout = QVBoxLayout(self.system_page)
        system_title = QLabel(translate("choose_system", self.language))
        system_title.setObjectName("heroTitle")
        system_layout.addWidget(system_title)
        system_subtitle = QLabel(translate("systems_unavailable", self.language))
        system_subtitle.setObjectName("subtitle")
        system_layout.addWidget(system_subtitle)
        system_layout.addStretch(1)
        grid = QGridLayout()
        grid.setHorizontalSpacing(14)
        grid.setVerticalSpacing(14)
        self.system_buttons: dict[str, QToolButton] = {}
        assets_directory = Path(__file__).parent / "assets" / "systems"
        for index, system in enumerate(SUPPORTED_SYSTEMS):
            label = SYSTEM_LABELS[system]
            button = QToolButton()
            button.setText(
                label
                if system == "sg1000"
                else f"{label}\n{translate('planned', self.language)}"
            )
            button.setToolButtonStyle(Qt.ToolButtonStyle.ToolButtonTextUnderIcon)
            button.setIcon(QIcon(str(assets_directory / f"{system}.svg")))
            button.setIconSize(QSize(220, 118))
            button.setMinimumSize(220, 166)
            button.setSizePolicy(
                QSizePolicy.Policy.Expanding, QSizePolicy.Policy.Fixed
            )
            button.setStyleSheet(
                "QToolButton { background: #14233b; border: 1px solid #31547c; "
                "border-radius: 10px; padding: 10px; font-size: 14px; "
                "font-weight: 600; } "
                "QToolButton:hover:enabled { background: #203c60; "
                "border-color: #42d8ff; } "
                "QToolButton:disabled { color: #71819a; background: #101a2a; }"
            )
            supported = system == "sg1000"
            button.setEnabled(supported)
            button.setToolTip(
                translate("integrated_core", self.language)
                if supported
                else translate("planned_core", self.language)
            )
            if supported:
                button.clicked.connect(
                    lambda checked=False, selected=system: self._open_library(selected)
                )
            self.system_buttons[system] = button
            grid.addWidget(button, index // 3, index % 3)
        system_layout.addLayout(grid)
        system_layout.addStretch(1)
        back_to_splash = QPushButton(translate("back", self.language))
        back_to_splash.clicked.connect(self._show_splash)
        system_layout.addWidget(back_to_splash, alignment=Qt.AlignmentFlag.AlignLeft)
        self.pages.addWidget(self.system_page)

        self.library_page = QWidget()
        library_layout = QVBoxLayout(self.library_page)
        header = QHBoxLayout()
        self.library_title = QLabel(translate("library", self.language))
        self.library_title.setObjectName("heroTitle")
        header.addWidget(self.library_title, 1)
        self.back_systems_button = QPushButton(translate("systems", self.language))
        self.back_systems_button.clicked.connect(self._show_systems)
        header.addWidget(self.back_systems_button)
        library_layout.addLayout(header)

        directory_row = QHBoxLayout()
        self.directory_edit = QLineEdit()
        self.directory_edit.setPlaceholderText(
            translate("directories_placeholder", self.language)
        )
        self.directory_edit.setToolTip(translate("directories_tooltip", self.language))
        self.browse_button = QPushButton(translate("add_folder", self.language))
        self.browse_button.clicked.connect(self._browse_directory)
        self.scan_button = QPushButton(translate("update_library", self.language))
        self.scan_button.clicked.connect(lambda: self._start_scan(full=False))
        self.full_scan_button = QPushButton(translate("full_scan", self.language))
        self.full_scan_button.clicked.connect(lambda: self._start_scan(full=True))
        directory_row.addWidget(self.directory_edit, 1)
        directory_row.addWidget(self.browse_button)
        directory_row.addWidget(self.scan_button)
        directory_row.addWidget(self.full_scan_button)
        library_layout.addLayout(directory_row)

        box_art_row = QHBoxLayout()
        self.box_art_directory_edit = QLineEdit()
        self.box_art_directory_edit.setPlaceholderText(
            "Pasta local de box arts (nome da imagem igual ao nome da ROM)"
        )
        self.box_art_directory_edit.setToolTip(
            "Cada sistema possui sua própria pasta de capas. As imagens são associadas "
            "pelo nome do arquivo, sem a extensão."
        )
        self.box_art_browse_button = QPushButton("Selecionar pasta de capas")
        self.box_art_browse_button.clicked.connect(self._browse_box_art_directory)
        self.refresh_box_art_button = QPushButton("Atualizar imagens")
        self.refresh_box_art_button.setToolTip(
            "Reexaminar as capas locais e atualizar as imagens da biblioteca (F5)."
        )
        self.refresh_box_art_button.clicked.connect(self._refresh_box_art)
        box_art_row.addWidget(self.box_art_directory_edit, 1)
        box_art_row.addWidget(self.box_art_browse_button)
        box_art_row.addWidget(self.refresh_box_art_button)
        library_layout.addLayout(box_art_row)

        self.rom_list = QListWidget()
        self.rom_list.setViewMode(QListWidget.ViewMode.IconMode)
        self.rom_list.setFlow(QListWidget.Flow.LeftToRight)
        self.rom_list.setWrapping(True)
        self.rom_list.setResizeMode(QListWidget.ResizeMode.Adjust)
        self.rom_list.setMovement(QListWidget.Movement.Static)
        self.rom_list.setGridSize(QSize(184, 244))
        self.rom_list.setIconSize(QSize(148, 184))
        self.rom_list.setWordWrap(True)
        self.rom_list.setUniformItemSizes(True)
        self.rom_list.setHorizontalScrollBarPolicy(
            Qt.ScrollBarPolicy.ScrollBarAlwaysOff
        )
        self.rom_list.setVerticalScrollBarPolicy(Qt.ScrollBarPolicy.ScrollBarAsNeeded)
        self.rom_list.itemSelectionChanged.connect(self._update_launch_state)
        self.rom_list.itemDoubleClicked.connect(lambda item: self._launch_selected())
        self.rom_list.setContextMenuPolicy(Qt.ContextMenuPolicy.CustomContextMenu)
        self.rom_list.customContextMenuRequested.connect(self._show_rom_context_menu)
        self._refresh_box_art_shortcut = QShortcut(
            QKeySequence.StandardKey.Refresh, self
        )
        self._refresh_box_art_shortcut.activated.connect(self._refresh_box_art)
        library_layout.addWidget(self.rom_list, 1)

        self.status_label = QLabel(translate("scan_not_run", self.language))
        self.status_label.setWordWrap(True)
        library_layout.addWidget(self.status_label)
        self.progress_bar = QProgressBar()
        self.progress_bar.setRange(0, 100)
        self.progress_bar.setValue(0)
        library_layout.addWidget(self.progress_bar)

        footer = QHBoxLayout()
        self.cancel_scan_button = QPushButton(translate("cancel_scan", self.language))
        self.cancel_scan_button.clicked.connect(self._cancel_scan)
        self.cancel_scan_button.setEnabled(False)
        self.launch_button = QPushButton(translate("launch_game", self.language))
        self.launch_button.setEnabled(False)
        self.launch_button.clicked.connect(self._launch_selected)
        close_button = QPushButton(translate("close", self.language))
        close_button.clicked.connect(self.reject)
        footer.addWidget(self.cancel_scan_button)
        footer.addStretch(1)
        footer.addWidget(close_button)
        footer.addWidget(self.launch_button)
        library_layout.addLayout(footer)
        self.pages.addWidget(self.library_page)

    def _open_scraper_settings(self) -> None:
        dialog = ScraperSettingsDialog(self.settings, self)
        if dialog.exec() == QDialog.DialogCode.Accepted:
            new_language = str(self.settings.value("ui_language", "pt-BR"))
            if new_language != self.language:
                QMessageBox.information(
                    self,
                    translate("scraper_title", new_language),
                    "O idioma será aplicado ao reabrir a biblioteca.",
                )

    def _show_splash(self) -> None:
        self.pages.setCurrentWidget(self.splash_page)

    def _show_systems(self) -> None:
        self.pages.setCurrentWidget(self.system_page)

    def _open_library(self, system: str) -> None:
        self.selected_system = system
        self.library_title.setText(f"{SYSTEM_LABELS[system]} — Biblioteca")
        self.directory_edit.setText(";".join(self._directories_for(system)))
        self.box_art_directory_edit.setText(self._box_art_directory_for(system))
        self.pages.setCurrentWidget(self.library_page)
        self._start_scan(full=False)

    def _directories_for(self, system: str) -> list[str]:
        saved = self.settings.value(f"rom_directories/{system}", [], type=list)
        if saved:
            return [
                str(Path(value).expanduser()) for value in saved if str(value).strip()
            ]
        documents = QStandardPaths.writableLocation(
            QStandardPaths.StandardLocation.DocumentsLocation
        )
        base = Path(documents) if documents else Path.home() / "Documents"
        default_directory = base / "ROMs" / DEFAULT_DIRECTORY_NAMES[system]
        return [str(default_directory)] if default_directory.is_dir() else []

    def _box_art_directory_for(self, system: str) -> str:
        return str(self.settings.value(f"box_art_directories/{system}", ""))

    def _browse_box_art_directory(self) -> None:
        if self.selected_system is None:
            return
        current = self.box_art_directory_edit.text().strip()
        chosen = QFileDialog.getExistingDirectory(
            self, "Selecionar pasta de box arts", current
        )
        if not chosen:
            return
        directory = Path(chosen).expanduser().resolve()
        self.box_art_directory_edit.setText(str(directory))
        self.settings.setValue(
            f"box_art_directories/{self.selected_system}", str(directory)
        )
        self._render_entries(self._entries)

    def _browse_directory(self) -> None:
        current = self.directory_edit.text().split(";")[0].strip()
        chosen = QFileDialog.getExistingDirectory(
            self, "Selecionar diretório de ROMs", current
        )
        if chosen:
            # Discard stale/nonexistent roots so they do not trigger the same
            # warning on every scan after the user selects a valid directory.
            values = [
                value.strip()
                for value in self.directory_edit.text().split(";")
                if value.strip() and Path(value.strip()).expanduser().is_dir()
            ]
            chosen_path = str(Path(chosen).expanduser().resolve())
            if chosen_path not in values:
                values.append(chosen_path)
            self.directory_edit.setText(";".join(values))

    def _start_scan(self, *, full: bool) -> None:
        if self._scan_task is not None or self.selected_system is None:
            return
        directories = tuple(
            Path(value.strip()).expanduser().resolve()
            for value in self.directory_edit.text().split(";")
            if value.strip() and Path(value.strip()).expanduser().is_dir()
        )
        if not directories:
            self.status_label.setText(
                "Selecione um diretório de ROMs existente antes de escanear."
            )
            QMessageBox.information(
                self,
                translate("scan_warnings", self.language),
                "O diretório padrão não existe. Use “Adicionar pasta” para selecionar "
                "a pasta real das ROMs; o caminho será salvo para as próximas sessões.",
            )
            return
        self.directory_edit.setText(";".join(str(path) for path in directories))
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
            translate("full_scanning", self.language)
            if full
            else translate("updating", self.language)
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
        self._render_entries(self._entries)
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
                translate("scan_warnings", self.language),
                "\n".join(
                    f"{error.path}: {error.message}" for error in result.errors[:8]
                ),
            )

    def _render_entries(self, entries: tuple[RomEntry, ...]) -> None:
        self.rom_list.clear()
        art_directory_text = self.box_art_directory_edit.text().strip()
        art_directory = Path(art_directory_text) if art_directory_text else None
        for entry in entries:
            rom_path = Path(entry.path)
            item = QListWidgetItem(f"{entry.name}\n{_format_size(entry.size)}")
            item.setSizeHint(QSize(176, 236))
            item.setData(Qt.ItemDataRole.UserRole, entry.path)
            item.setToolTip(f"{entry.path}\nSHA-256: {entry.sha256}")
            override = self._manual_box_art_for(rom_path)
            cover = (
                override
                if override is not None
                else (
                    _find_box_art(rom_path, art_directory)
                    if art_directory is not None
                    else None
                )
            )
            if cover is not None:
                item.setIcon(QIcon(str(cover)))
                item.setText(f"{rom_path.stem}\n{_format_size(entry.size)}")
                item.setToolTip(f"{entry.path}\nCapa: {cover}\nSHA-256: {entry.sha256}")
            self.rom_list.addItem(item)

    def _refresh_box_art(self) -> None:
        """Re-scan local artwork and redraw covers without rescanning ROM files."""
        configured = self.box_art_directory_edit.text().strip()
        if configured:
            directory = Path(configured).expanduser()
            if not directory.is_dir():
                QMessageBox.warning(
                    self,
                    "Pasta de capas indisponível",
                    "A pasta configurada não existe ou não está acessível:\n"
                    f"{directory}",
                )
                return
            if self.selected_system is not None:
                self.settings.setValue(
                    f"box_art_directories/{self.selected_system}",
                    str(directory.resolve()),
                )
        self._render_entries(self._entries)
        self.status_label.setText(
            f"Imagens atualizadas · {len(self._entries)} jogo(s) na biblioteca"
        )

    def _manual_box_art_key(self, rom_path: Path) -> str:
        system = self.selected_system or "unknown"
        normalized_path = str(rom_path.expanduser().resolve()).casefold()
        identity = sha256(normalized_path.encode("utf-8")).hexdigest()
        return f"box_art_overrides/{system}/{identity}"

    def _manual_box_art_for(self, rom_path: Path) -> Path | None:
        saved = str(self.settings.value(self._manual_box_art_key(rom_path), "")).strip()
        image = Path(saved) if saved else None
        return image if image is not None and image.is_file() else None

    def _show_rom_context_menu(self, position) -> None:
        item = self.rom_list.itemAt(position)
        if item is None:
            index = self.rom_list.indexAt(position)
            item = self.rom_list.item(index.row()) if index.isValid() else None
        if item is None:
            return
        self.rom_list.setCurrentItem(item)
        menu = QMenu(self.rom_list)
        choose_action = menu.addAction("Selecionar imagem da capa…")
        remove_action = menu.addAction("Remover capa escolhida manualmente")
        rom_path = Path(str(item.data(Qt.ItemDataRole.UserRole)))
        remove_action.setEnabled(self._manual_box_art_for(rom_path) is not None)
        selected = menu.exec(self.rom_list.mapToGlobal(position))
        if selected == choose_action:
            self._choose_box_art(rom_path)
        elif selected == remove_action:
            self.settings.remove(self._manual_box_art_key(rom_path))
            self._render_entries(self._entries)

    def _choose_box_art(self, rom_path: Path) -> None:
        configured = self.box_art_directory_edit.text().strip()
        start_directory = (
            Path(configured)
            if configured and Path(configured).is_dir()
            else rom_path.parent
        )
        image_filter = (
            "Imagens (*.png *.jpg *.jpeg *.webp *.bmp *.gif *.tif *.tiff "
            "*.avif *.heic *.svg *.ico);;Todos os arquivos (*)"
        )
        selected, _ = QFileDialog.getOpenFileName(
            self, "Selecionar imagem da capa", str(start_directory), image_filter
        )
        if not selected:
            return
        image = Path(selected).expanduser().resolve()
        self.settings.setValue(self._manual_box_art_key(rom_path), str(image))
        self._render_entries(self._entries)

    def _scan_failed(self, message: str) -> None:
        self._finish_scan()
        self.status_label.setText(
            f"{translate('scan_failure', self.language)}: {message}"
        )
        QMessageBox.warning(self, translate("scan_failure", self.language), message)

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
            self.status_label.setText(translate("cancel_requested", self.language))

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



# Prefer common formats, then formats supported by the installed Qt image plugins.
_COMMON_IMAGE_SUFFIXES = {
    ".png",
    ".jpg",
    ".jpeg",
    ".jpe",
    ".jfif",
    ".webp",
    ".bmp",
    ".dib",
    ".gif",
    ".tif",
    ".tiff",
    ".ico",
    ".icns",
    ".avif",
    ".heic",
    ".heif",
    ".jp2",
    ".j2k",
    ".jpf",
    ".jpx",
    ".jpm",
    ".mj2",
    ".pbm",
    ".pgm",
    ".ppm",
    ".pnm",
    ".xbm",
    ".xpm",
    ".svg",
    ".svgz",
    ".dds",
    ".tga",
}


def _normalize_art_name(value: str) -> str:
    """Normalize ROM/art names, ignoring regions and LaunchBox image numbering."""
    value = re.sub(
        r"\s+-\s+(?:box\s*[- ]\s*)?front.*$",
        "",
        value,
        flags=re.IGNORECASE,
    )
    # LaunchBox commonly appends -01, -02, etc. to alternative scans of one cover.
    value = re.sub(r"-\d{2,3}$", "", value.strip())
    value = re.sub(r"\[[^]]*\]|\([^)]*\)", " ", value)
    value = re.sub(r"[^a-z0-9]+", " ", value.casefold())
    return " ".join(value.split())


def _find_box_art(rom_path: Path, art_directory: Path) -> Path | None:
    """Find a recursively stored cover by exact or conservative fuzzy name matching."""
    if not art_directory.is_dir():
        return None
    try:
        from PySide6.QtGui import QImageReader

        supported = {
            f".{bytes(suffix).decode('ascii', errors='ignore').casefold()}"
            for suffix in QImageReader.supportedImageFormats()
        }
    except ImportError:
        supported = set()
    allowed = _COMMON_IMAGE_SUFFIXES | supported
    target = _normalize_art_name(rom_path.stem)
    if not target:
        return None
    try:
        candidates = [
            path
            for path in art_directory.rglob("*")
            if path.is_file() and path.suffix.casefold() in allowed
        ]
    except OSError:
        return None

    scored: list[tuple[float, str, Path]] = []
    for path in candidates:
        candidate = _normalize_art_name(path.stem)
        if not candidate:
            continue
        score = SequenceMatcher(None, target, candidate).ratio()
        if target == candidate:
            score = 1.0
        # Require a conservative threshold so unrelated games are not auto-matched.
        if score >= 0.68:
            scored.append((score, str(path).casefold(), path))
    if not scored:
        return None
    scored.sort(key=lambda item: (-item[0], item[1]))
    return scored[0][2]
