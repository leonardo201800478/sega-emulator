"""Offscreen tests for scraper selection and localized UI catalogs."""

from __future__ import annotations

import os

os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")

import pytest  # noqa: E402

pytest.importorskip("PySide6")

from PySide6.QtCore import QSettings  # noqa: E402
from PySide6.QtWidgets import QApplication  # noqa: E402

from sega_emulator.frontend.qt.localization import (  # noqa: E402
    SUPPORTED_LANGUAGES,
    translate,
)
from sega_emulator.frontend.qt.scraper_settings_dialog import (  # noqa: E402
    PROVIDERS,
    ScraperSettingsDialog,
)


@pytest.fixture
def qt_app():
    app = QApplication.instance() or QApplication([])
    yield app


@pytest.fixture
def settings(tmp_path, monkeypatch):
    monkeypatch.setenv("XDG_CONFIG_HOME", str(tmp_path))
    store = QSettings(str(tmp_path / "settings.ini"), QSettings.Format.IniFormat)
    store.clear()
    return store


def test_translation_catalog_has_all_five_languages() -> None:
    assert SUPPORTED_LANGUAGES == ("pt-BR", "en", "es", "it", "fr")
    for language in SUPPORTED_LANGUAGES:
        assert translate("choose_system", language)
        assert translate("provider_notice", language)


def test_translation_falls_back_to_portuguese_for_unknown_language() -> None:
    assert translate("choose_system", "unknown") == "Escolha o sistema"
    assert translate("missing_key", "unknown") == "missing_key"


def test_scraper_dialog_exposes_three_providers_and_local_only(qt_app, settings) -> None:
    dialog = ScraperSettingsDialog(settings)
    values = [
        dialog.provider_combo.itemData(index)
        for index in range(dialog.provider_combo.count())
    ]
    assert tuple(values) == PROVIDERS
    assert {"screenscraper", "thegamesdb", "igdb"}.issubset(values)
    assert dialog.credentials_widget.isHidden()
    dialog.close()


def test_screenscraper_shows_authentication_fields_without_saving_secrets(
    qt_app, settings
) -> None:
    dialog = ScraperSettingsDialog(settings)
    dialog.provider_combo.setCurrentIndex(
        dialog.provider_combo.findData("screenscraper")
    )
    dialog.developer_id_edit.setText("developer-id-placeholder")
    dialog.developer_password_edit.setText("developer-secret-placeholder")
    dialog.username_edit.setText("user-placeholder")
    dialog.password_edit.setText("user-secret-placeholder")
    dialog.auto_scrape_checkbox.setChecked(True)
    dialog._save()

    assert settings.value("metadata_provider") == "screenscraper"
    assert settings.value("auto_scrape_missing") is True
    assert settings.value("ui_language") == "pt-BR"
    assert settings.value("metadata_language") == "pt"
    all_keys = settings.allKeys()
    assert not any("password" in key.casefold() for key in all_keys)
    assert not any("developer_id" in key.casefold() for key in all_keys)


@pytest.mark.parametrize("language", ["pt-BR", "en", "es", "it", "fr"])
def test_language_selection_persists(qt_app, settings, language: str) -> None:
    dialog = ScraperSettingsDialog(settings)
    dialog.language_combo.setCurrentIndex(dialog.language_combo.findData(language))
    dialog._save()
    assert settings.value("ui_language") == language
