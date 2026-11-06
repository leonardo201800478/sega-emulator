"""Configurações efetivas do runtime temporal do Sega Master System."""

from __future__ import annotations

from dataclasses import replace

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


class SmsSettingsDialog(QDialog):
    """Edita somente opções que o runtime SMS/D3D12 atual realmente consome."""

    def __init__(
        self,
        preferences: FrontendPreferences,
        *,
        region: str,
        audio_backend: str,
        parent: QWidget | None = None,
    ) -> None:
        super().__init__(parent)
        self.setObjectName("smsSettingsDialog")
        self.setWindowTitle("Configurações — Master System")
        self.setMinimumWidth(560)
        self._initial = preferences

        root = QVBoxLayout(self)
        tabs = QTabWidget(self)
        root.addWidget(tabs)

        audio_page = QWidget(self)
        audio_form = QFormLayout(audio_page)
        self.audio_backend = QComboBox(audio_page)
        self.audio_backend.setObjectName("smsAudioBackend")
        self.audio_backend.addItem("WASAPI — compartilhado (padrão)", "wasapi")
        self.audio_backend.addItem("XAudio2 — saída nativa", "xaudio2")
        self.audio_backend.addItem("Desativado", "disabled")
        backend_index = self.audio_backend.findData(audio_backend)
        self.audio_backend.setCurrentIndex(max(0, backend_index))
        audio_form.addRow("Backend:", self.audio_backend)

        self.sample_rate = QComboBox(audio_page)
        self.sample_rate.setObjectName("smsAudioSampleRate")
        for rate in (22_050, 44_100, 48_000):
            self.sample_rate.addItem(f"{rate:,} Hz".replace(",", "."), rate)
        self.sample_rate.setCurrentIndex(
            max(0, self.sample_rate.findData(preferences.audio_sample_rate))
        )
        audio_form.addRow("Taxa de saída:", self.sample_rate)

        self.buffer_size = QComboBox(audio_page)
        self.buffer_size.setObjectName("smsAudioBufferSize")
        for samples, label in (
            (256, "256 amostras — menor buffer solicitado"),
            (512, "512 amostras — equilibrado"),
            (1024, "1.024 amostras — maior tolerância"),
            (2048, "2.048 amostras — buffer maior"),
        ):
            self.buffer_size.addItem(label, samples)
        self.buffer_size.setCurrentIndex(
            max(0, self.buffer_size.findData(preferences.audio_buffer_size))
        )
        audio_form.addRow("Tamanho do bloco:", self.buffer_size)

        self.queue_blocks = QComboBox(audio_page)
        self.queue_blocks.setObjectName("smsAudioQueueBlocks")
        for blocks in (2, 4, 8):
            self.queue_blocks.addItem(f"{blocks} blocos", blocks)
        self.queue_blocks.setCurrentIndex(
            max(0, self.queue_blocks.findData(preferences.audio_queue_blocks))
        )
        audio_form.addRow("Profundidade da fila:", self.queue_blocks)

        self.volume = QSlider(Qt.Orientation.Horizontal, audio_page)
        self.volume.setObjectName("smsAudioVolume")
        self.volume.setRange(0, 100)
        self.volume.setValue(preferences.audio_volume)
        self.volume_label = QLabel(f"{preferences.audio_volume}%")
        self.volume.valueChanged.connect(
            lambda value: self.volume_label.setText(f"{value}%")
        )
        volume_row = QWidget(audio_page)
        volume_layout = QHBoxLayout(volume_row)
        volume_layout.setContentsMargins(0, 0, 0, 0)
        volume_layout.addWidget(self.volume, 1)
        volume_layout.addWidget(self.volume_label)
        audio_form.addRow("Volume:", volume_row)

        self.balance = QSlider(Qt.Orientation.Horizontal, audio_page)
        self.balance.setObjectName("smsAudioBalance")
        self.balance.setRange(-100, 100)
        self.balance.setValue(preferences.audio_balance)
        self.balance_label = QLabel()
        self.balance.valueChanged.connect(self._update_balance_label)
        balance_row = QWidget(audio_page)
        balance_layout = QHBoxLayout(balance_row)
        balance_layout.setContentsMargins(0, 0, 0, 0)
        balance_layout.addWidget(QLabel("Esquerda"))
        balance_layout.addWidget(self.balance, 1)
        balance_layout.addWidget(QLabel("Direita"))
        balance_layout.addWidget(self.balance_label)
        audio_form.addRow("Balanço L/R:", balance_row)
        self._update_balance_label(preferences.audio_balance)

        audio_note = QLabel(
            "As opções configuram somente a saída de áudio no Windows. "
            "Não alteram o clock do PSG, os T-states ou o Scheduler. "
            "A saída usa o dispositivo padrão do sistema; seleção de dispositivo "
            "e modo exclusivo ainda não estão disponíveis."
        )
        audio_note.setWordWrap(True)
        audio_form.addRow(audio_note)
        tabs.addTab(audio_page, "Áudio")

        video_page = QWidget(self)
        video_form = QFormLayout(video_page)
        video_backend = QLabel(
            "D3D12 nativo em Rust · framebuffer SMS 256×192 · proporção 4:3"
        )
        video_backend.setWordWrap(True)
        video_form.addRow("Apresentação:", video_backend)

        self.integer_scale = QCheckBox(
            "Preferir escala inteira (pixels sem frações)"
        )
        self.integer_scale.setObjectName("smsIntegerScale")
        self.integer_scale.setChecked(preferences.integer_scale)
        video_form.addRow(self.integer_scale)

        video_note = QLabel(
            "A escala inteira é aplicada ao viewport Qt e não modifica os pixels "
            "do VDP. Filtros CRT, shaders, seleção de VSync/VRR e ajuste de "
            "resolução ainda não são expostos neste caminho D3D12; os controles "
            "permanecem ocultos até existir implementação nativa verificável."
        )
        video_note.setWordWrap(True)
        video_form.addRow(video_note)
        tabs.addTab(video_page, "Vídeo")

        system_page = QWidget(self)
        system_form = QFormLayout(system_page)
        self.region = QComboBox(system_page)
        self.region.setObjectName("smsRegion")
        for label, value in (
            ("Estados Unidos / NTSC", "usa"),
            ("Brasil / PAL-M", "brazil"),
            ("Europa / PAL", "europe"),
            ("Japão / NTSC", "japan"),
        ):
            self.region.addItem(label, value)
        region_index = self.region.findData(region)
        self.region.setCurrentIndex(max(0, region_index))
        system_form.addRow("Região do console:", self.region)
        region_note = QLabel(
            "A região é aplicada no próximo carregamento de ROM. Alterar esta "
            "opção não reinicia nem muda o clock da máquina atual."
        )
        region_note.setWordWrap(True)
        system_form.addRow(region_note)
        tabs.addTab(system_page, "Sistema")

        buttons = QDialogButtonBox(
            QDialogButtonBox.StandardButton.Ok
            | QDialogButtonBox.StandardButton.Cancel
            | QDialogButtonBox.StandardButton.RestoreDefaults,
            parent=self,
        )
        buttons.accepted.connect(self.accept)
        buttons.rejected.connect(self.reject)
        buttons.button(QDialogButtonBox.StandardButton.RestoreDefaults).clicked.connect(
            self._restore_defaults
        )
        root.addWidget(buttons)

    @property
    def selected_audio_backend(self) -> str:
        """Return the selected native audio backend identifier."""
        return str(self.audio_backend.currentData())

    @property
    def selected_region(self) -> str:
        """Return the region that will be used by the next SMS machine."""
        return str(self.region.currentData())

    def preferences(self) -> FrontendPreferences:
        """Return validated preferences without mutating the persistent store."""
        return replace(
            self._initial,
            audio_enabled=self.selected_audio_backend != "disabled",
            audio_sample_rate=int(self.sample_rate.currentData()),
            audio_buffer_size=int(self.buffer_size.currentData()),
            audio_queue_blocks=int(self.queue_blocks.currentData()),
            audio_volume=self.volume.value(),
            audio_balance=self.balance.value(),
            integer_scale=self.integer_scale.isChecked(),
        )

    def _update_balance_label(self, value: int) -> None:
        if value == 0:
            label = "Centro"
        elif value < 0:
            label = f"L {100 + value}% / R 100%"
        else:
            label = f"L 100% / R {100 - value}%"
        self.balance_label.setText(label)

    def _restore_defaults(self) -> None:
        defaults = FrontendPreferences()
        self.audio_backend.setCurrentIndex(self.audio_backend.findData("wasapi"))
        self.sample_rate.setCurrentIndex(
            self.sample_rate.findData(defaults.audio_sample_rate)
        )
        self.buffer_size.setCurrentIndex(
            self.buffer_size.findData(defaults.audio_buffer_size)
        )
        self.queue_blocks.setCurrentIndex(
            self.queue_blocks.findData(defaults.audio_queue_blocks)
        )
        self.volume.setValue(defaults.audio_volume)
        self.balance.setValue(defaults.audio_balance)
        self.integer_scale.setChecked(defaults.integer_scale)
        self.region.setCurrentIndex(self.region.findData("usa"))
