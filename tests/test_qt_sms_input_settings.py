"""Offscreen tests for the SMS input and Light Phaser Qt settings page."""

from __future__ import annotations

import os

os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")

import pytest

pytest.importorskip("PySide6")

from PySide6.QtWidgets import QApplication, QWidget  # noqa: E402

from sega_emulator.frontend.qt.sms_input_settings import (  # noqa: E402
    SmsInputSettingsPage,
)


class FakeSettings:
    def __init__(self, values=None):
        self.values = dict(values or {})

    def value(self, key, default=None):
        return self.values.get(key, default)

    def setValue(self, key, value):
        self.values[key] = value


@pytest.fixture
def qt_app():
    app = QApplication.instance() or QApplication([])
    yield app


def test_sms_input_page_shows_controller_and_light_phaser_layouts(qt_app):
    page = SmsInputSettingsPage(FakeSettings())

    assert page.objectName() == "smsInputSettingsPage"
    assert page.findChild(QWidget, "smsControllerLayoutPlayer1") is not None
    assert page.findChild(QWidget, "smsControllerLayoutPlayer2") is not None
    assert page.findChild(QWidget, "smsLightPhaserLayout") is not None
    assert page.phaser_enabled.isChecked() is False
    assert page.mouse_aim.isChecked() is True
    assert page.mouse_trigger.isChecked() is True
    assert page.sensitivity.value() == 100
    assert page.findChild(QWidget, "smsInputIntegrationNotice") is not None
    page.close()


def test_sms_light_phaser_preferences_are_loaded_and_persisted(qt_app):
    settings = FakeSettings(
        {
            "sms_light_phaser_enabled": True,
            "sms_light_phaser_mouse_aim": False,
            "sms_light_phaser_mouse_trigger": False,
            "sms_light_phaser_right_stick_aim": True,
            "sms_light_phaser_invert_y": True,
            "sms_light_phaser_crosshair": False,
            "sms_light_phaser_sensitivity": 175,
        }
    )
    page = SmsInputSettingsPage(settings)

    assert page.phaser_enabled.isChecked() is True
    assert page.mouse_aim.isChecked() is False
    assert page.mouse_trigger.isChecked() is False
    assert page.right_stick_aim.isChecked() is True
    assert page.invert_y.isChecked() is True
    assert page.show_crosshair.isChecked() is False
    assert page.sensitivity.value() == 175

    page.phaser_enabled.setChecked(False)
    page.mouse_aim.setChecked(True)
    page.mouse_trigger.setChecked(True)
    page.sensitivity.setValue(225)
    page.save_settings(settings)

    assert settings.values["sms_light_phaser_enabled"] is False
    assert settings.values["sms_light_phaser_mouse_aim"] is True
    assert settings.values["sms_light_phaser_mouse_trigger"] is True
    assert settings.values["sms_light_phaser_right_stick_aim"] is True
    assert settings.values["sms_light_phaser_invert_y"] is True
    assert settings.values["sms_light_phaser_crosshair"] is False
    assert settings.values["sms_light_phaser_sensitivity"] == 225
    page.close()


def test_light_phaser_controls_follow_enable_and_mouse_aim_state(qt_app):
    page = SmsInputSettingsPage(FakeSettings())

    assert not page.mouse_aim.isEnabled()
    page.phaser_enabled.setChecked(True)
    assert page.mouse_aim.isEnabled()
    assert page.mouse_trigger.isEnabled()
    page.mouse_aim.setChecked(False)
    assert not page.mouse_trigger.isEnabled()
    page.close()
