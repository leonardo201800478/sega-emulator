"""Qt dialog for the supported SG-1000 PSG configuration surface.

Hardware-model constants are read-only: editable unsupported parameters would
imply behavior the core does not yet implement.
"""
from __future__ import annotations


def configure_psg(parent, settings, *, audio_enabled: bool, output_gain: int):
    """Show PSG/output tabs and return a pair or None if cancelled."""
    from PySide6.QtCore import Qt
    from PySide6.QtWidgets import (
        QCheckBox,
        QDialog,
        QDialogButtonBox,
        QFormLayout,
        QHBoxLayout,
        QLabel,
        QSlider,
        QSpinBox,
        QTabWidget,
        QVBoxLayout,
        QWidget,
    )

    dialog = QDialog(parent)
    dialog.setWindowTitle("Configurações de som — PSG")
    dialog.resize(600, 440)
    root = QVBoxLayout(dialog)
    tabs = QTabWidget(dialog)
    root.addWidget(tabs)

    output_page = QWidget(dialog)
    output_form = QFormLayout(output_page)
    enabled = QCheckBox("Ativar saída de áudio", output_page)
    enabled.setChecked(audio_enabled)
    output_form.addRow("Áudio:", enabled)

    gain_row = QWidget(output_page)
    gain_layout = QHBoxLayout(gain_row)
    gain_layout.setContentsMargins(0, 0, 0, 0)
    gain = QSlider(Qt.Orientation.Horizontal, gain_row)
    gain.setRange(0, 200)
    gain.setSingleStep(5)
    gain.setPageStep(10)
    gain.setValue(max(0, min(200, int(output_gain))))
    gain.setToolTip("Ganho digital da saída; não altera os registradores do PSG.")
    gain_value = QSpinBox(gain_row)
    gain_value.setRange(0, 200)
    gain_value.setSuffix(" %")
    gain_value.setValue(gain.value())
    gain.valueChanged.connect(gain_value.setValue)
    gain_value.valueChanged.connect(gain.setValue)
    gain_layout.addWidget(gain, 1)
    gain_layout.addWidget(gain_value)
    output_form.addRow("Ganho de saída:", gain_row)
    gain_note = QLabel(
        "O ganho é aplicado após a emulação. 100% mantém o nível digital nominal; "
        "0% silencia a saída sem alterar o estado do chip.",
        output_page,
    )
    gain_note.setWordWrap(True)
    output_form.addRow(gain_note)
    tabs.addTab(output_page, "Saída")

    psg_page = QWidget(dialog)
    psg_form = QFormLayout(psg_page)
    fixed_values = (
        ("Perfil de hardware", "Sega SG-1000 — SN76489AN"),
        ("Clock nominal do PSG", "3.579.545 Hz"),
        ("Divisor interno modelado", "16"),
        ("Geradores", "3 canais de tom + 1 canal de ruído"),
        ("Saída do chip modelada", "Mono digital"),
        ("Taxa de saída atual", "44.100 amostras/s"),
        ("Antialiasing de saída", "FIR passa-baixas Blackman, 63 taps"),
    )
    for label, value in fixed_values:
        value_label = QLabel(value, psg_page)
        value_label.setTextInteractionFlags(
            Qt.TextInteractionFlag.TextSelectableByMouse
        )
        psg_form.addRow(f"{label}:", value_label)
    note = QLabel(
        "Esses parâmetros são fixos no núcleo atual e, por isso, são informativos "
        "nesta versão. O FIR reduz aliasing da conversão digital, mas não representa "
        "o circuito analógico do console nem comprova fidelidade ao hardware real. "
        "Variantes de chip, clock, taxa de amostragem e resposta analógica só devem "
        "ser habilitadas quando existirem implementação e testes diferenciais.",
        psg_page,
    )
    note.setWordWrap(True)
    psg_form.addRow(note)
    tabs.addTab(psg_page, "PSG / Hardware")

    buttons = QDialogButtonBox(
        QDialogButtonBox.StandardButton.Ok | QDialogButtonBox.StandardButton.Cancel,
        parent=dialog,
    )
    buttons.accepted.connect(dialog.accept)
    buttons.rejected.connect(dialog.reject)
    root.addWidget(buttons)

    if dialog.exec() != QDialog.DialogCode.Accepted:
        return None

    new_enabled = enabled.isChecked()
    new_gain = gain.value()
    settings.setValue("audio_enabled", new_enabled)
    settings.setValue("psg_output_gain_percent", new_gain)
    return new_enabled, new_gain
