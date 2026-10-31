"""Dialog for selecting host input, audio and video driver APIs.

Only implemented drivers are offered. Preferences are stored by the Qt frontend;
the emulated hardware core receives normalized input and completed audio/video data.
"""

from __future__ import annotations

from PySide6.QtWidgets import (
    QComboBox,
    QDialog,
    QDialogButtonBox,
    QFormLayout,
    QLabel,
    QVBoxLayout,
)

INPUT_DRIVERS = (
    ("Automático (SDL3 nativo em Rust)", "auto"),
)
AUDIO_DRIVERS = (
    ("WASAPI nativo (Rust)", "wasapi"),
    ("XAudio2 nativo (Rust)", "xaudio2"),
    ("Áudio desativado", "disabled"),
)
VIDEO_DRIVERS = (
    ("Automático (Vulkan → Qt Raster)", "auto"),
    ("Vulkan (Qt Quick)", "vulkan"),
    ("Direct3D 11 (Qt Quick)", "d3d11"),
    ("Direct3D 12 (Qt Quick)", "d3d12"),
    ("Qt Raster", "raster"),
)


class DriversDialog(QDialog):
    """Select host APIs; unavailable APIs are never advertised as implemented."""

    def __init__(
        self,
        *,
        input_driver: str = "auto",
        audio_driver: str = "wasapi",
        video_driver: str = "auto",
        parent=None,
    ) -> None:
        super().__init__(parent)
        self.setWindowTitle("Drivers — APIs de entrada, áudio e vídeo")
        self.setMinimumWidth(560)
        layout = QVBoxLayout(self)
        form = QFormLayout()
        self.input_combo = self._combo(INPUT_DRIVERS, input_driver)
        self.audio_combo = self._combo(AUDIO_DRIVERS, audio_driver)
        self.video_combo = self._combo(VIDEO_DRIVERS, video_driver)
        form.addRow("API de input:", self.input_combo)
        form.addRow("API de áudio:", self.audio_combo)
        form.addRow("API de vídeo:", self.video_combo)
        layout.addLayout(form)
        note = QLabel(
            "As escolhas são persistidas globalmente. O núcleo recebe snapshots "
            "normalizados de input; nenhuma API do sistema é chamada por instrução. "
            "A API gráfica é definida na inicialização do Qt Quick: mudanças entre "
            "Vulkan, Direct3D 11, Direct3D 12 e raster exigem reiniciar o emulador."
        )
        note.setWordWrap(True)
        layout.addWidget(note)
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
        layout.addWidget(buttons)

    @staticmethod
    def _combo(options: tuple[tuple[str, str], ...], selected: str) -> QComboBox:
        combo = QComboBox()
        for label, value in options:
            combo.addItem(label, value)
        index = combo.findData(selected)
        combo.setCurrentIndex(index if index >= 0 else 0)
        return combo

    def _restore_defaults(self) -> None:
        self.input_combo.setCurrentIndex(self.input_combo.findData("auto"))
        self.audio_combo.setCurrentIndex(self.audio_combo.findData("wasapi"))
        self.video_combo.setCurrentIndex(self.video_combo.findData("auto"))

    def drivers(self) -> tuple[str, str, str]:
        return (
            str(self.input_combo.currentData()),
            str(self.audio_combo.currentData()),
            str(self.video_combo.currentData()),
        )
