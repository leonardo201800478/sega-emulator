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
        audio_form.addRow("Backend de saída:", QLabel("WASAPI — modo compartilhado"))

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
            "liberados quando houver implementação e testes diferenciais Python/Rust.",
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
        self.video_filter.addItem("Nearest-neighbor — pixels nítidos", "nearest")
        self.video_filter.addItem("Bilinear — suavização", "bilinear")
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

        self.sprite_limit_enabled = QCheckBox(
            "Limitar a quatro sprites por linha (modo hardware)"
        )
        self.sprite_limit_enabled.setChecked(preferences.sprite_limit_enabled)
        self.sprite_limit_enabled.setToolTip(
            "Desative para diagnóstico visual; o comportamento deixa de seguir "
            "o limite de sprites do SG-1000."
        )
        video_form.addRow(self.sprite_limit_enabled)

        video_note = QLabel(
            "A proporção 4:3 é preservada. Nearest e bilinear são os únicos filtros "
            "visuais implementados. VSync ainda requer integração com o modo de "
            "apresentação do backend; VRR/G-Sync depende do Windows, driver e monitor "
            "e não é ativado por esta preferência. O relógio de áudio é experimental "
            "e não altera o scheduler nesta etapa."
        )
        video_note.setWordWrap(True)
        video_form.addRow(video_note)
        video_layout.addWidget(video_group)
        video_layout.addStretch(1)
        tabs.addTab(video_page, "Vídeo")

        crt_page = QWidget(self)
        crt_layout = QVBoxLayout(crt_page)
        crt_group = QGroupBox("CRT / NTSC — controles preparatórios")
        crt_form = QFormLayout(crt_group)
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
            "Os parâmetros são persistidos, mas nenhum efeito CRT/NTSC, RF ou "
            "dithering é renderizado nesta PR. A implementação exige pipeline de "
            "filtro e testes de referência."
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
        self.sprite_limit_enabled.setChecked(defaults.sprite_limit_enabled)
        self.vsync_mode.setCurrentIndex(self.vsync_mode.findData(defaults.vsync_mode))
        self.vrr_mode.setCurrentIndex(self.vrr_mode.findData(defaults.vrr_mode))
        self.av_sync_mode.setCurrentIndex(self.av_sync_mode.findData(defaults.av_sync_mode))
        self.crt_scanline_intensity.setValue(defaults.crt_scanline_intensity)
        self.crt_scanline_thickness.setValue(defaults.crt_scanline_thickness)
        self.crt_phosphor_mask.setValue(defaults.crt_phosphor_mask)
        self.crt_curvature.setValue(defaults.crt_curvature)

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
            sprite_limit_enabled=self.sprite_limit_enabled.isChecked(),
            vsync_mode=str(self.vsync_mode.currentData()),
            vrr_mode=str(self.vrr_mode.currentData()),
            av_sync_mode=str(self.av_sync_mode.currentData()),
            crt_scanline_intensity=self.crt_scanline_intensity.value(),
            crt_scanline_thickness=self.crt_scanline_thickness.value(),
            crt_phosphor_mask=self.crt_phosphor_mask.value(),
            crt_curvature=self.crt_curvature.value(),
        )
