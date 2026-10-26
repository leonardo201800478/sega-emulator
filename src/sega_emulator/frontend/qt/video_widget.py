"""Vulkan-backed Qt Quick video surface with a raster fallback.

The emulator owns the framebuffer. This module only presents copies of completed
frames and never reads or modifies VDP state.
"""

from __future__ import annotations

import logging
from threading import Lock

from PySide6.QtCore import QUrl, Qt
from PySide6.QtGui import QColor, QImage, QPixmap
from PySide6.QtQml import QQmlComponent, QQmlImageProviderBase
from PySide6.QtQuick import (
    QQuickImageProvider,
    QQuickView,
    QQuickWindow,
    QSGRendererInterface,
)
from PySide6.QtWidgets import QLabel, QSizePolicy, QStackedWidget, QVBoxLayout, QWidget

logger = logging.getLogger(__name__)

_QML_SOURCE = """
import QtQuick

Item {
    id: root
    width: 768
    height: 576

    property int frameRevision: 0
    property bool hasFrame: false
    property bool integerScale: true
    property string filterMode: "nearest"

    Rectangle {
        anchors.fill: parent
        color: "#080b10"
    }

    Text {
        anchors.centerIn: parent
        visible: !root.hasFrame
        text: "Abra uma ROM SG-1000 pelo menu Arquivo"
        color: "#9aa9bf"
        font.pixelSize: 16
    }

    Image {
        id: frame
        cache: false
        visible: root.hasFrame
        source: root.hasFrame
                ? "image://segaFrame/current?revision=" + root.frameRevision
                : ""
        sourceSize.width: 256
        sourceSize.height: 192
        fillMode: Image.PreserveAspectFit
        smooth: root.filterMode === "bilinear"
        mipmap: false
        width: {
            var factor = Math.min(root.width / 256, root.height / 192)
            if (root.integerScale && factor >= 1)
                factor = Math.floor(factor)
            return 256 * factor
        }
        height: width * 192 / 256
        anchors.centerIn: parent
    }
}
"""


class FramebufferImageProvider(QQuickImageProvider):
    """Thread-safe provider for immutable snapshots of the current framebuffer."""

    def __init__(self) -> None:
        super().__init__(QQmlImageProviderBase.ImageType.Image)
        self._lock = Lock()
        self._image = QImage()

    def set_image(self, image: QImage | None) -> None:
        """Publish a detached image snapshot for the next scene-graph update."""
        snapshot = QImage(image) if image is not None and not image.isNull() else QImage()
        with self._lock:
            self._image = snapshot

    def requestImage(self, image_id, size, requested_size) -> QImage:
        """Return a stable copy; Qt may call this method away from the UI thread."""
        del image_id, size, requested_size
        with self._lock:
            return QImage(self._image)


class VideoWidget(QWidget):
    """Present the SG-1000 framebuffer through Vulkan, falling back to Qt raster."""

    BASE_WIDTH = 256
    BASE_HEIGHT = 192

    def __init__(self, parent=None) -> None:
        super().__init__(parent)
        self._source_image: QImage | None = None
        self._filter_mode = "nearest"
        self._integer_scale = True
        self._vulkan_failed = False
        self._quick_view = None
        self._quick_container = None
        self._qml_root = None
        self._qml_component = None
        self._image_provider = None

        self.setMinimumSize(self.BASE_WIDTH, self.BASE_HEIGHT)
        self.setSizePolicy(QSizePolicy.Policy.Expanding, QSizePolicy.Policy.Expanding)
        self.setFocusPolicy(Qt.FocusPolicy.StrongFocus)
        self.setStyleSheet("background: #080b10;")

        self._stack = QStackedWidget(self)
        self._fallback = QLabel(self)
        self._fallback.setAlignment(Qt.AlignmentFlag.AlignCenter)
        self._fallback.setMinimumSize(self.BASE_WIDTH, self.BASE_HEIGHT)
        self._fallback.setSizePolicy(
            QSizePolicy.Policy.Expanding, QSizePolicy.Policy.Expanding
        )
        self._fallback.setFocusPolicy(Qt.FocusPolicy.StrongFocus)
        self._fallback.setText("Abra uma ROM SG-1000 pelo menu Arquivo")
        self._fallback.setStyleSheet(
            "background: #080b10; color: #9aa9bf; border: 1px solid #252f40;"
        )
        self._stack.addWidget(self._fallback)

        layout = QVBoxLayout(self)
        layout.setContentsMargins(0, 0, 0, 0)
        layout.setSpacing(0)
        layout.addWidget(self._stack)

        self._initialize_vulkan_view()

    def _initialize_vulkan_view(self) -> None:
        """Request Vulkan before creating QQuickView and retain raster fallback."""
        try:
            # Qt documents that this must happen before the first QQuickWindow.
            QQuickWindow.setGraphicsApi(QSGRendererInterface.GraphicsApi.Vulkan)
            quick_view = QQuickView()
            quick_view.setColor(QColor("#080b10"))
            quick_view.setResizeMode(QQuickView.ResizeMode.SizeRootObjectToView)
            quick_view.sceneGraphError.connect(self._on_scene_graph_error)
            quick_view.sceneGraphInitialized.connect(
                self._on_scene_graph_initialized,
                Qt.ConnectionType.QueuedConnection,
            )

            provider = FramebufferImageProvider()
            quick_view.engine().addImageProvider("segaFrame", provider)
            component = QQmlComponent(quick_view.engine())
            component.setData(_QML_SOURCE.encode("utf-8"), QUrl())
            if component.isError():
                details = "; ".join(error.toString() for error in component.errors())
                raise RuntimeError(f"Falha ao compilar a cena QML: {details}")
            root = component.create()
            if root is None:
                details = "; ".join(error.toString() for error in component.errors())
                raise RuntimeError(f"Falha ao criar a cena QML: {details}")
            quick_view.setContent(QUrl(), component, root)

            container = QWidget.createWindowContainer(quick_view, self)
            container.setFocusPolicy(Qt.FocusPolicy.StrongFocus)
            container.setMinimumSize(self.BASE_WIDTH, self.BASE_HEIGHT)
            self._stack.addWidget(container)
            self._stack.setCurrentWidget(container)
            self._quick_view = quick_view
            self._quick_container = container
            self._qml_root = root
            self._qml_component = component
            self._image_provider = provider
            self.setFocusProxy(container)
            self._apply_render_settings()
            logger.info(
                "Superfície de vídeo Vulkan criada; aguardando inicialização do driver",
                extra={"event": "vulkan_video_surface_created"},
            )
        except Exception:
            logger.exception(
                "Falha ao criar superfície Vulkan; usando renderização Qt raster"
            )
            self._vulkan_failed = True
            self._stack.setCurrentWidget(self._fallback)

    def _on_scene_graph_initialized(self) -> None:
        """Verify the API actually selected by Qt Quick after initialization."""
        if self._quick_view is None:
            return
        try:
            actual_api = self._quick_view.rendererInterface().graphicsApi()
            if actual_api != QSGRendererInterface.GraphicsApi.Vulkan:
                self._on_scene_graph_error(
                    QQuickWindow.SceneGraphError.ContextNotAvailable,
                    f"Qt Quick inicializou com API gráfica inesperada: {actual_api}",
                )
                return
            logger.info(
                "Renderização Vulkan inicializada",
                extra={"event": "vulkan_video_initialized", "graphics_api": "Vulkan"},
            )
        except Exception:
            logger.exception("Falha ao confirmar a API gráfica efetiva")
            self._vulkan_failed = True
            self._stack.setCurrentWidget(self._fallback)
            self._quick_view.hide()

    def _on_scene_graph_error(self, error, message: str) -> None:
        """Switch to the known raster path if Vulkan cannot initialize."""
        self._vulkan_failed = True
        logger.error(
            "Vulkan indisponível; usando saída Qt raster: %s",
            message,
            extra={
                "event": "vulkan_video_fallback",
                "graphics_error": str(error),
                "graphics_message": message,
            },
        )
        self._stack.setCurrentWidget(self._fallback)
        if self._quick_view is not None:
            self._quick_view.hide()

    def installEventFilter(self, event_filter) -> None:
        """Keep keyboard/controller event filters attached to the focused surface."""
        super().installEventFilter(event_filter)
        if self._fallback is not None:
            self._fallback.installEventFilter(event_filter)
        if self._quick_container is not None:
            self._quick_container.installEventFilter(event_filter)
        if self._quick_view is not None:
            self._quick_view.installEventFilter(event_filter)

    def set_filter_mode(self, mode: str) -> None:
        if mode not in ("nearest", "bilinear"):
            raise ValueError("mode deve ser 'nearest' ou 'bilinear'")
        self._filter_mode = mode
        self._apply_render_settings()
        self._refresh_raster_fallback()

    def set_integer_scale(self, enabled: bool) -> None:
        if type(enabled) is not bool:
            raise TypeError("enabled deve ser bool")
        self._integer_scale = enabled
        self._apply_render_settings()
        self._refresh_raster_fallback()

    def set_framebuffer(self, image: QImage) -> None:
        """Publish a copy of the completed frame; the emulator may reuse its buffer."""
        self._source_image = image.copy()
        if self._image_provider is not None:
            self._image_provider.set_image(self._source_image)
        if self._qml_root is not None and not self._vulkan_failed:
            self._qml_root.setProperty("hasFrame", True)
            self._qml_root.setProperty(
                "frameRevision", int(self._qml_root.property("frameRevision")) + 1
            )
        self._refresh_raster_fallback()
        if self._quick_container is not None and not self._vulkan_failed:
            self._stack.setCurrentWidget(self._quick_container)
        else:
            self._stack.setCurrentWidget(self._fallback)

    def clear_framebuffer(
        self, message: str = "Abra uma ROM SG-1000 pelo menu Arquivo"
    ) -> None:
        self._source_image = None
        if self._image_provider is not None:
            self._image_provider.set_image(None)
        if self._qml_root is not None:
            self._qml_root.setProperty("hasFrame", False)
            self._qml_root.setProperty(
                "frameRevision", int(self._qml_root.property("frameRevision")) + 1
            )
        self._fallback.setPixmap(QPixmap())
        self._fallback.setText(message)

    def resizeEvent(self, event) -> None:
        super().resizeEvent(event)
        self._refresh_raster_fallback()

    def _apply_render_settings(self) -> None:
        if self._qml_root is None:
            return
        self._qml_root.setProperty("filterMode", self._filter_mode)
        self._qml_root.setProperty("integerScale", self._integer_scale)

    def _refresh_raster_fallback(self) -> None:
        if self._source_image is None or self._source_image.isNull():
            return
        available = self._fallback.contentsRect().size()
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
        self._fallback.setText("")
        self._fallback.setPixmap(scaled)
