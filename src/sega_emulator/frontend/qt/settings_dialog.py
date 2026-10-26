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
    QTabWidget,
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
        tabs = QTabWidget(self)
        root.addWidget(tabs)

        audio_page = QWidget(self)
        audio_layout = QVBoxLayout(audio_page)
        audio_group = QGroupBox("Saída de áudio do emulador")
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

        audio_note = QLabel(
            "A taxa e o buffer configuram somente a saída do Windows. "
            "Não alteram o clock do PSG nem aumentam a fidelidade do modelo digital."
        )
        audio_note.setWordWrap(True)
        audio_form.addRow(audio_note)
        audio_layout.addWidget(audio_group)
        audio_layout.addStretch(1)
        tabs.addTab(audio_page, "Saída de áudio")

        psg_page = QWidget(self)
        psg_layout = QVBoxLayout(psg_page)
        psg_group = QGroupBox("PSG — Sega SG-1000 / SN76489AN")
        psg_form = QFormLayout(psg_group)
        psg_form.addRow("Perfil de hardware:", QLabel("SN76489AN do SG-1000"))
        psg_form.addRow("Clock nominal:", QLabel("3.579.545 Hz (fixo)"))
        psg_form.addRow("Divisor interno:", QLabel("16 (fixo no modelo atual)"))
        psg_form.addRow("Canais:", QLabel("3 tons + 1 ruído; saída mono"))
        psg_form.addRow(
            "Tratamento da saída:",
            QLabel("Reamostragem linear no frontend; sem FIR no núcleo desta branch"),
        )

        volume_row = QWidget(psg_group)
        volume_layout = QHBoxLayout(volume_row)
        volume_layout.setContentsMargins(0, 0, 0, 0)
        self.volume = QSlider(Qt.Orientation.Horizontal, volume_row)
        self.volume.setRange(0, 100)
        self.volume.setValue(preferences.audio_volume)
        self.volume_label = QLabel(f"{preferences.audio_volume}%", volume_row)
        self.volume.valueChanged.connect(
            lambda value: self.volume_label.setText(f"{value}%")
        )
        volume_layout.addWidget(self.volume, 1)
        volume_layout.addWidget(self.volume_label)
        psg_form.addRow("Ganho da saída do PSG:", volume_row)

        psg_note = QLabel(
            "O ganho afeta somente as amostras reproduzidas; não altera os "
            "registradores nem o clock lógico. Clock, divisor, variante do chip e "
            "tratamento de saída do núcleo ainda não são configuráveis. A reamostragem "
            "linear ocorre no frontend e não é um modelo do circuito analógico real. "
            "Novos parâmetros só serão liberados quando houver implementação e "
            "testes diferenciais Python/Rust.",
            psg_page,
        )
        psg_note.setWordWrap(True)
        psg_form.addRow(psg_note)
        psg_layout.addWidget(psg_group)
        psg_layout.addStretch(1)
        tabs.addTab(psg_page, "PSG / Hardware")

        video_page = QWidget(self)
        video_layout = QVBoxLayout(video_page)
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
        video_layout.addWidget(video_group)
        video_layout.addStretch(1)
        tabs.addTab(video_page, "Vídeo")

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
        self.sample_rate.setCurrentIndex(
            self.sample_rate.findData(defaults.audio_sample_rate)
        )
        self.buffer_size.setCurrentIndex(
            self.buffer_size.findData(defaults.audio_buffer_size)
        )
        self.volume.setValue(defaults.audio_volume)
        self.video_filter.setCurrentIndex(
            self.video_filter.findData(defaults.video_filter)
        )
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
