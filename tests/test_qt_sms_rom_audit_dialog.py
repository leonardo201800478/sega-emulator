"""Offscreen tests for the SMS ROM audit dialog."""

from __future__ import annotations

import os

os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")

import pytest  # noqa: E402

pytest.importorskip("PySide6")

from PySide6.QtWidgets import QApplication  # noqa: E402

from sega_emulator.frontend.qt.sms_rom_audit_dialog import SmsRomAuditDialog  # noqa: E402


@pytest.fixture
def qt_app():
    app = QApplication.instance() or QApplication([])
    yield app


def test_sms_audit_dialog_exposes_folder_and_runtime_controls(qt_app):
    dialog = SmsRomAuditDialog(lambda _data: object())

    assert dialog.windowTitle() == "Auditoria em lote — ROMs Master System"
    assert dialog.directory_edit.objectName() == "smsAuditDirectory"
    assert dialog.start_button.objectName() == "startSmsAudit"
    assert dialog.results_table.objectName() == "smsAuditResults"
    assert dialog.recursive_check.isChecked()
    assert dialog.frames_spin.value() == 120
    assert dialog.timeout_spin.value() == 5.0
    assert dialog.results_table.columnCount() == 7
    assert dialog.start_button.isEnabled()
    assert not dialog.cancel_button.isEnabled()
    dialog.close()
