"""Superfície de vídeo Qt com proporção 4:3 e filtros configuráveis."""

from __future__ import annotations

from PySide6.QtCore import Qt
from PySide6.QtGui import QImage, QPixmap
from PySide6.QtWidgets import QLabel, QSizePolicy


class VideoWidget(QLabel):
    """Apresenta o framebuffer SG-1000 sem alterar o estado do hardware."""

    BASE_WIDTH = 256
    BASE_HEIGHT = 192

    def __init__(self, parent=None) -> None:
        super().__init__(parent)
        self._source_image: QImage | None = None
        self._filter_mode = "nearest"
        self._integer_scale = True
        self.setAlignment(Qt.AlignmentFlag.AlignCenter)
        self.setMinimumSize(self.BASE_WIDTH, self.BASE_HEIGHT)
        self.setSizePolicy(QSizePolicy.Policy.Expanding, QSizePolicy.Policy.Expanding)
        self.setFocusPolicy(Qt.FocusPolicy.StrongFocus)
        self.setText("Abra uma ROM SG-1000 pelo menu Arquivo")
        self.setStyleSheet(
            "background: #080b10; color: #9aa9bf; border: 1px solid #252f40;"
        )

    def set_filter_mode(self, mode: str) -> None:
        if mode not in ("nearest", "bilinear"):
            raise ValueError("mode deve ser 'nearest' ou 'bilinear'")
        self._filter_mode = mode
        self._refresh_pixmap()

    def set_integer_scale(self, enabled: bool) -> None:
        if type(enabled) is not bool:
            raise TypeError("enabled deve ser bool")
        self._integer_scale = enabled
        self._refresh_pixmap()

    def set_framebuffer(self, image: QImage) -> None:
        """Copia o frame para que o buffer do emulador possa ser reutilizado."""
        self._source_image = image.copy()
        self._refresh_pixmap()

    def clear_framebuffer(
        self, message: str = "Abra uma ROM SG-1000 pelo menu Arquivo"
    ) -> None:
        self._source_image = None
        self.setPixmap(QPixmap())
        self.setText(message)

    def resizeEvent(self, event) -> None:
        super().resizeEvent(event)
        self._refresh_pixmap()

    def _refresh_pixmap(self) -> None:
        if self._source_image is None or self._source_image.isNull():
            return
        available = self.contentsRect().size()
        width_factor = available.width() // self.BASE_WIDTH
        height_factor = available.height() // self.BASE_HEIGHT
        factor = min(width_factor, height_factor)
        transformation = (
            Qt.TransformationMode.FastTransformation
            if self._filter_mode == "nearest"
            else Qt.TransformationMode.SmoothTransformation
        )
        if self._integer_scale and factor >= 1:
            target = self._source_image.size() * factor
            scaled = QPixmap.fromImage(self._source_image).scaled(
                target,
                Qt.AspectRatioMode.KeepAspectRatio,
                transformation,
            )
        else:
            scaled = QPixmap.fromImage(self._source_image).scaled(
                available,
                Qt.AspectRatioMode.KeepAspectRatio,
                transformation,
            )
        self.setPixmap(scaled)
