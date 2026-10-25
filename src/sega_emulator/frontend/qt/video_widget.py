"""Pixel-accurate, aspect-preserving video surface for the Qt frontend."""

from __future__ import annotations

from PySide6.QtCore import Qt
from PySide6.QtGui import QImage, QPixmap
from PySide6.QtWidgets import QLabel, QSizePolicy


class VideoWidget(QLabel):
    """Display an SG-1000 framebuffer without stretching its aspect ratio."""

    BASE_WIDTH = 256
    BASE_HEIGHT = 192

    def __init__(self, parent=None) -> None:
        super().__init__(parent)
        self._source_image: QImage | None = None
        self.setAlignment(Qt.AlignmentFlag.AlignCenter)
        self.setMinimumSize(self.BASE_WIDTH, self.BASE_HEIGHT)
        self.setSizePolicy(
            QSizePolicy.Policy.Expanding,
            QSizePolicy.Policy.Expanding,
        )
        self.setFocusPolicy(Qt.FocusPolicy.StrongFocus)
        self.setText("Abra uma ROM SG-1000 pelo menu Arquivo")
        self.setStyleSheet("background: #090a0d; color: #9aa4b2; border: 0;")

    def set_framebuffer(self, image: QImage) -> None:
        """Copy the frame so the caller may safely reuse its backing buffer."""
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
        pixmap = QPixmap.fromImage(self._source_image)
        scaled = pixmap.scaled(
            self.contentsRect().size(),
            Qt.AspectRatioMode.KeepAspectRatio,
            Qt.TransformationMode.FastTransformation,
        )
        self.setPixmap(scaled)
