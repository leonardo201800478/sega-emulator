"""Dialog de configuração de áudio e vídeo do frontend Qt."""

from __future__ import annotations

from PySide6.QtCore import Qt
from PySide6.QtWidgets import (
    QCheckBox,
    QComboBox,
    QDialog,
    QDialogButtonBox,
    QFormLayout,
    QGroupBox,
    QHBoxLayout,
    QLabel,
    QSlider,
    QVBoxLayout,
    QWidget,
)

from sega_emulator.frontend.qt.preferences import FrontendPreferences


class SettingsDialog(QDialog):
    """Editor de configurações de saída, sem alterar clocks da máquina emulada."""

    def __init__(self, preferences: FrontendPreferences, parent=None) -> None:
        super().__init__(parent)
        self.setWindowTitle("Configurações de áudio e vídeo")
        self.setMinimumWidth(440)
        self._initial = preferences
        root = QVBoxLayout(self)

        audio_group = QGroupBox("Áudio — saída do PSG SN76489AN")
        audio_form = QFormLayout(audio_group)
        self.audio_enabled = QCheckBox("Ativar saída de áudio")
        self.audio_enabled.setChecked(preferences.audio_enabled)
        audio_form.addRow(self.audio_enabled)

        self.sample_rate = QComboBox()
        for rate in (22050, 44100, 48000):
            self.sample_rate.addItem(f"{rate:,} Hz".replace(",", "."), rate)
        index = self.sample_rate.findData(preferences.audio_sample_rate)
        self.sample_rate.setCurrentIndex(max(0, index))
        audio_form.addRow("Taxa de saída", self.sample_rate)

        self.buffer_size = QComboBox()
        for samples, label in (
            (256, "256 amostras — menor latência"),
            (512, "512 amostras — equilibrado"),
            (1024, "1.024 amostras — mais estável"),
            (2048, "2.048 amostras — maior tolerância"),
        ):
            self.buffer_size.addItem(label, samples)
        index = self.buffer_size.findData(preferences.audio_buffer_size)
        self.buffer_size.setCurrentIndex(max(0, index))
        audio_form.addRow("Buffer de saída", self.buffer_size)

        volume_row = QWidget()
        volume_layout = QHBoxLayout(volume_row)
        volume_layout.setContentsMargins(0, 0, 0, 0)
        self.volume = QSlider()
        self.volume.setOrientation(Qt.Orientation.Horizontal)
        self.volume.setRange(0, 100)
        self.volume.setValue(preferences.audio_volume)
        self.volume_label = QLabel(f"{preferences.audio_volume}%")
        self.volume.valueChanged.connect(
            lambda value: self.volume_label.setText(f"{value}%")
        )
        volume_layout.addWidget(self.volume, 1)
        volume_layout.addWidget(self.volume_label)
        audio_form.addRow("Volume de saída", volume_row)
        audio_note = QLabel(
            "A taxa e o buffer configuram somente a saída do Windows. "
            "Não alteram o clock do PSG nem aumentam a fidelidade do modelo digital."
        )
        audio_note.setWordWrap(True)
        audio_form.addRow(audio_note)
        root.addWidget(audio_group)

        video_group = QGroupBox("Vídeo")
        video_form = QFormLayout(video_group)
        self.video_filter = QComboBox()
        self.video_filter.addItem("Nearest-neighbor — pixels nítidos", "nearest")
        self.video_filter.addItem("Bilinear — suavização", "bilinear")
        index = self.video_filter.findData(preferences.video_filter)
        self.video_filter.setCurrentIndex(max(0, index))
        video_form.addRow("Escala", self.video_filter)

        self.integer_scale = QCheckBox("Preferir escala inteira quando couber")
        self.integer_scale.setChecked(preferences.integer_scale)
        video_form.addRow(self.integer_scale)

        video_note = QLabel(
            "A proporção 4:3 é preservada. A escala inteira reduz artefatos "
            "de interpolação; o modo bilinear suaviza a imagem, mas pode borrar pixels."
        )
        video_note.setWordWrap(True)
        video_form.addRow(video_note)
        root.addWidget(video_group)

        buttons = QDialogButtonBox(
            QDialogButtonBox.StandardButton.Ok
            | QDialogButtonBox.StandardButton.Cancel
            | QDialogButtonBox.StandardButton.RestoreDefaults
        )
        buttons.accepted.connect(self.accept)
        buttons.rejected.connect(self.reject)
        buttons.button(QDialogButtonBox.StandardButton.RestoreDefaults).clicked.connect(
            self._restore_defaults
        )
        root.addWidget(buttons)

    def _restore_defaults(self) -> None:
        defaults = FrontendPreferences()
        self.audio_enabled.setChecked(defaults.audio_enabled)
        self.sample_rate.setCurrentIndex(self.sample_rate.findData(defaults.audio_sample_rate))
        self.buffer_size.setCurrentIndex(self.buffer_size.findData(defaults.audio_buffer_size))
        self.volume.setValue(defaults.audio_volume)
        self.video_filter.setCurrentIndex(self.video_filter.findData(defaults.video_filter))
        self.integer_scale.setChecked(defaults.integer_scale)

    def preferences(self) -> FrontendPreferences:
        """Return only settings edited by this dialog."""
        old = self._initial
        return FrontendPreferences(
            window_width=old.window_width,
            window_height=old.window_height,
            video_scale=old.video_scale,
            recent_roms=old.recent_roms,
            controller_config=old.controller_config,
            audio_enabled=self.audio_enabled.isChecked(),
            diagnostics_enabled=old.diagnostics_enabled,
            cpu_core=old.cpu_core,
            audio_sample_rate=int(self.sample_rate.currentData()),
            audio_buffer_size=int(self.buffer_size.currentData()),
            audio_volume=self.volume.value(),
            video_filter=str(self.video_filter.currentData()),
            integer_scale=self.integer_scale.isChecked(),
        )
