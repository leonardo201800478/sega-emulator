"""Qt settings for metadata providers and the five-language frontend."""

from __future__ import annotations

from PySide6.QtCore import QSettings, QUrl
from PySide6.QtGui import QDesktopServices
from PySide6.QtWidgets import (
    QCheckBox,
    QComboBox,
    QDialog,
    QDialogButtonBox,
    QFormLayout,
    QHBoxLayout,
    QLabel,
    QLineEdit,
    QPushButton,
    QVBoxLayout,
    QWidget,
)

from sega_emulator.frontend.qt.localization import (
    LANGUAGE_LABELS,
    SUPPORTED_LANGUAGES,
    translate,
)

PROVIDERS = ("none", "screenscraper", "thegamesdb", "igdb")
PROVIDER_URLS = {
    "screenscraper": "https://www.screenscraper.fr/",
    "thegamesdb": "https://api.thegamesdb.net/",
    "igdb": "https://api-docs.igdb.com/",
}
METADATA_LANGUAGES = {
    "pt": "Português",
    "en": "English",
    "es": "Español",
    "it": "Italiano",
    "fr": "Français",
}


class ScraperSettingsDialog(QDialog):
    """Persist provider/language preferences; never persist API secrets."""

    def __init__(self, settings: QSettings, parent: QWidget | None = None) -> None:
        super().__init__(parent)
        self.settings = settings
        self.language = str(settings.value("ui_language", "pt-BR"))
        if self.language not in SUPPORTED_LANGUAGES:
            self.language = "pt-BR"
        self.setWindowTitle(translate("scraper_title", self.language))
        self.setMinimumWidth(520)

        layout = QVBoxLayout(self)
        form = QFormLayout()
        layout.addLayout(form)

        self.provider_combo = QComboBox(self)
        for provider in PROVIDERS:
            self.provider_combo.addItem(
                translate(
                    {
                        "none": "provider_none",
                        "screenscraper": "provider_screenscraper",
                        "thegamesdb": "provider_thegamesdb",
                        "igdb": "provider_igdb",
                    }[provider],
                    self.language,
                ),
                provider,
            )
        provider_value = str(settings.value("metadata_provider", "none"))
        provider_index = self.provider_combo.findData(provider_value)
        self.provider_combo.setCurrentIndex(max(0, provider_index))
        form.addRow(translate("provider", self.language), self.provider_combo)

        self.language_combo = QComboBox(self)
        for code in SUPPORTED_LANGUAGES:
            self.language_combo.addItem(LANGUAGE_LABELS[code], code)
        language_index = self.language_combo.findData(self.language)
        self.language_combo.setCurrentIndex(max(0, language_index))
        form.addRow(translate("ui_language", self.language), self.language_combo)

        self.metadata_language_combo = QComboBox(self)
        for code, label in METADATA_LANGUAGES.items():
            self.metadata_language_combo.addItem(label, code)
        metadata_language = str(settings.value("metadata_language", "pt"))
        metadata_index = self.metadata_language_combo.findData(metadata_language)
        self.metadata_language_combo.setCurrentIndex(max(0, metadata_index))
        form.addRow(
            translate("metadata_language", self.language),
            self.metadata_language_combo,
        )

        self.auto_scrape_checkbox = QCheckBox(
            translate("auto_scrape", self.language), self
        )
        self.auto_scrape_checkbox.setChecked(
            _setting_bool(settings.value("auto_scrape_missing", False))
        )
        layout.addWidget(self.auto_scrape_checkbox)

        self.provider_link = QPushButton(translate("open_provider", self.language))
        self.provider_link.clicked.connect(self._open_provider)
        layout.addWidget(self.provider_link)

        self.credentials_label = QLabel(self)
        self.credentials_label.setWordWrap(True)
        layout.addWidget(self.credentials_label)

        self.credentials_widget = QWidget(self)
        credentials_layout = QFormLayout(self.credentials_widget)
        self.developer_id_edit = QLineEdit(self.credentials_widget)
        self.developer_password_edit = QLineEdit(self.credentials_widget)
        self.developer_password_edit.setEchoMode(QLineEdit.EchoMode.Password)
        self.username_edit = QLineEdit(self.credentials_widget)
        self.password_edit = QLineEdit(self.credentials_widget)
        self.password_edit.setEchoMode(QLineEdit.EchoMode.Password)
        credentials_layout.addRow(
            translate("developer_id", self.language), self.developer_id_edit
        )
        credentials_layout.addRow(
            translate("developer_password", self.language),
            self.developer_password_edit,
        )
        credentials_layout.addRow(
            translate("screenscraper_username", self.language), self.username_edit
        )
        credentials_layout.addRow(
            translate("screenscraper_password", self.language), self.password_edit
        )
        layout.addWidget(self.credentials_widget)

        self.show_passwords = QCheckBox(
            translate("show_passwords", self.language), self
        )
        self.show_passwords.toggled.connect(self._toggle_password_visibility)
        layout.addWidget(self.show_passwords)

        self.notice_label = QLabel(translate("provider_notice", self.language), self)
        self.notice_label.setWordWrap(True)
        layout.addWidget(self.notice_label)

        buttons = QDialogButtonBox(
            QDialogButtonBox.StandardButton.Save
            | QDialogButtonBox.StandardButton.Cancel,
            parent=self,
        )
        buttons.button(QDialogButtonBox.StandardButton.Save).setText(
            translate("save", self.language)
        )
        buttons.button(QDialogButtonBox.StandardButton.Cancel).setText(
            translate("cancel", self.language)
        )
        buttons.accepted.connect(self._save)
        buttons.rejected.connect(self.reject)
        layout.addWidget(buttons)

        self.provider_combo.currentIndexChanged.connect(self._update_provider_ui)
        self._update_provider_ui()

    def _update_provider_ui(self) -> None:
        provider = str(self.provider_combo.currentData())
        is_screenscraper = provider == "screenscraper"
        self.credentials_widget.setVisible(is_screenscraper)
        self.show_passwords.setVisible(is_screenscraper)
        self.credentials_label.setText(
            translate("credentials_notice", self.language) if is_screenscraper else ""
        )
        self.provider_link.setEnabled(provider in PROVIDER_URLS)

    def _open_provider(self) -> None:
        provider = str(self.provider_combo.currentData())
        url = PROVIDER_URLS.get(provider)
        if url:
            QDesktopServices.openUrl(QUrl(url))

    def _toggle_password_visibility(self, visible: bool) -> None:
        mode = QLineEdit.EchoMode.Normal if visible else QLineEdit.EchoMode.Password
        self.developer_password_edit.setEchoMode(mode)
        self.password_edit.setEchoMode(mode)

    def _save(self) -> None:
        self.settings.setValue("metadata_provider", self.provider_combo.currentData())
        self.settings.setValue("ui_language", self.language_combo.currentData())
        self.settings.setValue(
            "metadata_language", self.metadata_language_combo.currentData()
        )
        self.settings.setValue(
            "auto_scrape_missing", self.auto_scrape_checkbox.isChecked()
        )
        # Credentials are deliberately not persisted or sent yet. A subsequent
        # provider-adapter change must use the OS credential vault, not QSettings.
        self.accept()


def _setting_bool(value: object) -> bool:
    if isinstance(value, bool):
        return value
    if isinstance(value, int) and value in (0, 1):
        return bool(value)
    if isinstance(value, str):
        return value.strip().casefold() in {"true", "1", "yes", "on"}
    return False
