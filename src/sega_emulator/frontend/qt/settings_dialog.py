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
    QSpinBox,
    QTabWidget,
    QVBoxLayout,
    QWidget,
)

from sega_emulator.frontend.qt.preferences import FrontendPreferences


class SettingsDialog(QDialog):
    """Editor de configurações de saída, sem alterar clocks da máquina emulada."""

    def __init__(
        self,
        preferences: FrontendPreferences,
        parent=None,
        *,
        audio_backend: str = "wasapi",
    ) -> None:
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
        backend_label = {
            "wasapi": "WASAPI nativo — modo compartilhado (Rust)",
            "xaudio2": "XAudio2 nativo (Rust)",
            "disabled": "Áudio desativado",
        }.get(audio_backend, "Backend desconhecido")
        audio_form.addRow("Backend de saída:", QLabel(backend_label))

        self.sample_rate = QComboBox()
        for rate in (22050, 44100, 48000):
            self.sample_rate.addItem(f"{rate:,} Hz".replace(",", "."), rate)
        index = self.sample_rate.findData(preferences.audio_sample_rate)
        self.sample_rate.setCurrentIndex(max(0, index))
        audio_form.addRow("Taxa de saída", self.sample_rate)

        self.buffer_size = QComboBox()
        for samples, label in (
            (256, "256 amostras — menor buffer solicitado"),
            (512, "512 amostras — equilibrado"),
            (1024, "1.024 amostras — maior tolerância"),
            (2048, "2.048 amostras — buffer maior"),
        ):
            self.buffer_size.addItem(label, samples)
        index = self.buffer_size.findData(preferences.audio_buffer_size)
        self.buffer_size.setCurrentIndex(max(0, index))
        audio_form.addRow("Buffer de saída", self.buffer_size)

        audio_note = QLabel(
            "A taxa e o buffer configuram somente a saída do Windows. No modo "
            "compartilhado, o Windows determina o período efetivo; o buffer é uma "
            "solicitação, não uma medição de latência. Nada disso altera o clock "
            "do PSG nem aumenta a fidelidade do modelo digital."
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
        psg_form.addRow("Antialiasing:", QLabel("FIR Blackman de 63 taps, ativo"))

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
            "filtro ainda são fixos no núcleo. O FIR é antialiasing digital, não "
            "um modelo do circuito analógico real. Esses controles só serão "
            "liberados quando houver implementação e testes de regressão "
            "independentes no núcleo Rust.",
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
        video_form.addRow(
            "Backend de apresentação:",
            QLabel("Vulkan (Qt Quick); fallback Qt raster se indisponível"),
        )
        self.video_filter = QComboBox()
        self.video_filter.addItem("Nativo / Nearest — pixels nítidos", "nearest")
        self.video_filter.addItem("Bilinear — suavização", "bilinear")
        self.video_filter.addItem(
            "Dither Blending — mistura seletiva de padrões", "dither_blending"
        )
        self.video_filter.addItem(
            "CRT / NTSC — scanlines e sinal aproximado", "crt_ntsc"
        )
        index = self.video_filter.findData(preferences.video_filter)
        self.video_filter.setCurrentIndex(max(0, index))
        video_form.addRow("Filtro de escala", self.video_filter)

        self.vsync_mode = QComboBox()
        for label, value in (
            ("Ligado — sincronização vertical", "on"),
            ("Desligado — menor latência, pode rasgar", "off"),
            ("Adaptativo — se suportado pelo backend", "adaptive"),
        ):
            self.vsync_mode.addItem(label, value)
        self.vsync_mode.setCurrentIndex(
            max(0, self.vsync_mode.findData(preferences.vsync_mode))
        )
        video_form.addRow("VSync", self.vsync_mode)

        self.vrr_mode = QComboBox()
        for label, value in (
            ("Controlado pelo driver (recomendado)", "driver"),
            ("Preferir VRR — requer monitor/driver compatíveis", "prefer"),
            ("Não solicitar VRR", "off"),
        ):
            self.vrr_mode.addItem(label, value)
        self.vrr_mode.setCurrentIndex(
            max(0, self.vrr_mode.findData(preferences.vrr_mode))
        )
        video_form.addRow("VRR / G-Sync", self.vrr_mode)

        self.av_sync_mode = QComboBox()
        self.av_sync_mode.addItem("Baixa latência (atual)", "low_latency")
        self.av_sync_mode.addItem("Relógio de áudio (experimental)", "audio_clock")
        self.av_sync_mode.setCurrentIndex(
            max(0, self.av_sync_mode.findData(preferences.av_sync_mode))
        )
        video_form.addRow("Sincronismo A/V", self.av_sync_mode)

        self.integer_scale = QCheckBox("Preferir escala inteira quando couber")
        self.integer_scale.setChecked(preferences.integer_scale)
        video_form.addRow(self.integer_scale)

        self.sprite_limit_unlocked = QCheckBox(
            "Destravar limite de sprites por linha (modo compatibilidade)"
        )
        self.sprite_limit_unlocked.setChecked(not preferences.sprite_limit_enabled)
        self.sprite_limit_unlocked.setToolTip(
            "Ative para desenhar mais de quatro sprites na mesma linha. "
            "O VDP continua registrando o status de overflow do hardware."
        )
        video_form.addRow(self.sprite_limit_unlocked)

        video_note = QLabel(
            "Nearest e bilinear preservam os modos de referência. Dither Blending "
            "mistura somente blocos 2x2 reconhecidos como xadrez. CRT/NTSC aplica "
            "scanlines, máscara, curvatura e uma aproximação espacial de sinal; não "
            "é uma simulação temporal ou eletricamente precisa de NTSC/RF. VSync e "
            "VRR/G-Sync continuam dependendo do backend, driver e monitor."
        )
        video_note.setWordWrap(True)
        video_form.addRow(video_note)
        video_layout.addWidget(video_group)
        video_layout.addStretch(1)
        tabs.addTab(video_page, "Vídeo")

        crt_page = QWidget(self)
        crt_layout = QVBoxLayout(crt_page)
        crt_group = QGroupBox("CRT / NTSC — efeitos de apresentação")
        crt_form = QFormLayout(crt_group)
        self.crt_signal_mode = QComboBox()
        self.crt_signal_mode.addItem("RGB — sinal limpo", "rgb")
        self.crt_signal_mode.addItem(
            "Composite NTSC — crominância suavizada", "composite"
        )
        self.crt_signal_mode.addItem("RF — sinal mais degradado (aproximação)", "rf")
        self.crt_signal_mode.setCurrentIndex(
            max(0, self.crt_signal_mode.findData(preferences.crt_signal_mode))
        )
        crt_form.addRow("Modelo de sinal", self.crt_signal_mode)
        self.crt_scanline_intensity = QSpinBox()
        self.crt_scanline_intensity.setRange(0, 100)
        self.crt_scanline_intensity.setSuffix("%")
        self.crt_scanline_intensity.setValue(preferences.crt_scanline_intensity)
        crt_form.addRow("Intensidade das scanlines", self.crt_scanline_intensity)
        self.crt_scanline_thickness = QSpinBox()
        self.crt_scanline_thickness.setRange(0, 100)
        self.crt_scanline_thickness.setSuffix("%")
        self.crt_scanline_thickness.setValue(preferences.crt_scanline_thickness)
        crt_form.addRow("Espessura das scanlines", self.crt_scanline_thickness)
        self.crt_phosphor_mask = QSpinBox()
        self.crt_phosphor_mask.setRange(0, 100)
        self.crt_phosphor_mask.setSuffix("%")
        self.crt_phosphor_mask.setValue(preferences.crt_phosphor_mask)
        crt_form.addRow("Máscara de fósforo", self.crt_phosphor_mask)
        self.crt_curvature = QSpinBox()
        self.crt_curvature.setRange(0, 100)
        self.crt_curvature.setSuffix("%")
        self.crt_curvature.setValue(preferences.crt_curvature)
        crt_form.addRow("Curvatura da tela", self.crt_curvature)
        crt_note = QLabel(
            "Composite e RF são aproximações espaciais determinísticas. Esta versão "
            "não modela fase/subportadora por linha, artefatos temporais, sincronismo "
            "de cor ou cadeia elétrica real. A curvatura remapeia pixels por CPU; "
            "compare desempenho antes de usar em hardware lento."
        )
        crt_note.setWordWrap(True)
        crt_form.addRow(crt_note)
        crt_layout.addWidget(crt_group)
        crt_layout.addStretch(1)
        tabs.addTab(crt_page, "CRT / NTSC")

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
        self.sprite_limit_unlocked.setChecked(not defaults.sprite_limit_enabled)
        self.vsync_mode.setCurrentIndex(self.vsync_mode.findData(defaults.vsync_mode))
        self.vrr_mode.setCurrentIndex(self.vrr_mode.findData(defaults.vrr_mode))
        self.av_sync_mode.setCurrentIndex(
            self.av_sync_mode.findData(defaults.av_sync_mode)
        )
        self.crt_scanline_intensity.setValue(defaults.crt_scanline_intensity)
        self.crt_scanline_thickness.setValue(defaults.crt_scanline_thickness)
        self.crt_phosphor_mask.setValue(defaults.crt_phosphor_mask)
        self.crt_curvature.setValue(defaults.crt_curvature)
        self.crt_signal_mode.setCurrentIndex(
            self.crt_signal_mode.findData(defaults.crt_signal_mode)
        )

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
            sprite_limit_enabled=not self.sprite_limit_unlocked.isChecked(),
            vsync_mode=str(self.vsync_mode.currentData()),
            vrr_mode=str(self.vrr_mode.currentData()),
            av_sync_mode=str(self.av_sync_mode.currentData()),
            crt_scanline_intensity=self.crt_scanline_intensity.value(),
            crt_scanline_thickness=self.crt_scanline_thickness.value(),
            crt_phosphor_mask=self.crt_phosphor_mask.value(),
            crt_curvature=self.crt_curvature.value(),
            crt_signal_mode=str(self.crt_signal_mode.currentData()),
        )
