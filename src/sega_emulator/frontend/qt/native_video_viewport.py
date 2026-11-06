"""Native child-window viewport for Rust-owned D3D12 presentation.

This widget owns geometry and an HWND only. It never receives frame pixels,
creates QImages, applies filters, or performs rendering.
"""

from __future__ import annotations

from PySide6.QtCore import QSize, Qt
from PySide6.QtGui import QResizeEvent
from PySide6.QtWidgets import QSizePolicy, QWidget


class SmsVideoViewport(QWidget):
    """Letterboxed 4:3 native surface for the Rust D3D12 swap chain."""

    def __init__(self, parent: QWidget | None = None) -> None:
        super().__init__(parent)
        self.setObjectName("smsVideoViewport")
        self.setMinimumSize(384, 288)
        self.setSizePolicy(
            QSizePolicy.Policy.Expanding,
            QSizePolicy.Policy.Expanding,
        )
        self.setStyleSheet("QWidget#smsVideoViewport { background: #080a0f; }")

        self.surface_widget = QWidget(self)
        self.surface_widget.setObjectName("smsD3d12Surface")
        self.surface_widget.setAttribute(
            Qt.WidgetAttribute.WA_NativeWindow,
            True,
        )
        self.surface_widget.setAttribute(
            Qt.WidgetAttribute.WA_OpaquePaintEvent,
            True,
        )
        self.surface_widget.setStyleSheet(
            "QWidget#smsD3d12Surface { background: #000000; }"
        )
        self._layout_surface()

    def sizeHint(self) -> QSize:  # noqa: N802 - Qt override name
        """Prefer a 768x576 viewport while allowing normal window resizing."""
        return QSize(768, 576)

    def native_handle(self) -> int:
        """Return the native HWND used by the Rust DXGI swap chain."""
        handle = int(self.surface_widget.winId())
        if handle <= 0:
            raise RuntimeError("Qt não conseguiu criar o HWND do viewport de vídeo.")
        return handle

    def resizeEvent(self, event: QResizeEvent) -> None:  # noqa: N802 - Qt override name
        """Keep the native surface at 4:3 and center it in the available area."""
        super().resizeEvent(event)
        self._layout_surface()

    def _layout_surface(self) -> None:
        """Compute the largest centered 4:3 rectangle inside this widget."""
        available_width = max(0, self.width())
        available_height = max(0, self.height())
        if available_width == 0 or available_height == 0:
            self.surface_widget.setGeometry(0, 0, 0, 0)
            return

        scale = min(available_width / 4, available_height / 3)
        surface_width = max(1, int(4 * scale))
        surface_height = max(1, int(3 * scale))
        left = (available_width - surface_width) // 2
        top = (available_height - surface_height) // 2
        self.surface_widget.setGeometry(
            left,
            top,
            surface_width,
            surface_height,
        )
