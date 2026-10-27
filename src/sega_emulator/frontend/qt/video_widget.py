"""Qt Quick video surface using Vulkan, Direct3D 11/12, or raster presentation.

The emulator owns the framebuffer. This module only presents copies of completed
frames and never reads or modifies VDP state. Graphics API selection must happen
before the first QQuickWindow is created.
"""

from __future__ import annotations

import logging
from threading import Lock

import numpy as np

from PySide6.QtCore import Qt, QUrl
from PySide6.QtGui import QColor, QImage, QPixmap
from PySide6.QtQml import QQmlComponent, QQmlImageProviderBase
from PySide6.QtQuick import (
    QQuickImageProvider,
    QQuickView,
    QQuickWindow,
    QSGRendererInterface,
)
from PySide6.QtWidgets import QLabel, QSizePolicy, QStackedWidget, QVBoxLayout, QWidget

from sega_emulator.frontend.qt.video_filters import apply_video_filter_rgb

logger = logging.getLogger(__name__)


def _graphics_api_for_mode(mode: str):
    """Map a persisted backend name to Qt's native graphics API enum."""
    api_map = {
        "auto": QSGRendererInterface.GraphicsApi.Vulkan,
        "vulkan": QSGRendererInterface.GraphicsApi.Vulkan,
        "d3d11": QSGRendererInterface.GraphicsApi.Direct3D11,
        "d3d12": QSGRendererInterface.GraphicsApi.Direct3D12,
    }
    try:
        return api_map[mode]
    except KeyError as exc:
        raise ValueError(
            "graphics API must be 'auto', 'vulkan', 'd3d11', 'd3d12', or 'raster'"
        ) from exc


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
        snapshot = (
            QImage(image) if image is not None and not image.isNull() else QImage()
        )
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

    def __init__(self, parent=None, *, graphics_api: str = "vulkan") -> None:
        super().__init__(parent)
        if graphics_api not in {"auto", "vulkan", "d3d11", "d3d12", "raster"}:
            raise ValueError(
                "graphics_api must be auto, vulkan, d3d11, d3d12, or raster"
            )
        self._source_image: QImage | None = None
        self._present_image: QImage | None = None
        self._filter_mode = "nearest"
        self._crt_parameters = {
            "scanline_intensity": 40,
            "scanline_thickness": 50,
            "phosphor_mask": 25,
            "curvature": 0,
            "signal_mode": "rgb",
        }
        self._integer_scale = True
        self._graphics_failed = False
        self._graphics_api = graphics_api
        self._expected_graphics_api = (
            None if graphics_api == "raster" else _graphics_api_for_mode(graphics_api)
        )
        self._render_backend = "raster" if graphics_api == "raster" else "auto"
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

        self._initialize_graphics_view()

    def _initialize_graphics_view(self) -> None:
        """Create Qt Quick using the selected native API, with raster fallback."""
        if self._graphics_api == "raster":
            self._stack.setCurrentWidget(self._fallback)
            logger.info(
                "Renderização Qt raster selecionada",
                extra={"event": "video_api_initialized", "graphics_api": "raster"},
            )
            return
        try:
            # Qt requires the API to be selected before the first QQuickWindow.
            QQuickWindow.setGraphicsApi(self._expected_graphics_api)
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
                "Superfície Qt Quick criada; aguardando inicialização do driver",
                extra={
                    "event": "video_surface_created",
                    "graphics_api_requested": self._graphics_api,
                },
            )
        except Exception:
            logger.exception(
                "Falha ao criar superfície %s; usando renderização Qt raster",
                self._graphics_api,
            )
            self._graphics_failed = True
            self._stack.setCurrentWidget(self._fallback)

    def _on_scene_graph_initialized(self) -> None:
        """Verify the actual Qt Quick API against the requested native API."""
        if self._quick_view is None:
            return
        try:
            actual_api = self._quick_view.rendererInterface().graphicsApi()
            if actual_api != self._expected_graphics_api:
                self._on_scene_graph_error(
                    QQuickWindow.SceneGraphError.ContextNotAvailable,
                    f"Qt Quick inicializou com API gráfica inesperada: {actual_api}",
                )
                return
            logger.info(
                "Renderização Qt Quick inicializada",
                extra={
                    "event": "video_api_initialized",
                    "graphics_api_requested": self._graphics_api,
                    "graphics_api_effective": str(actual_api),
                },
            )
        except Exception:
            logger.exception("Falha ao confirmar a API gráfica efetiva")
            self._graphics_failed = True
            self._stack.setCurrentWidget(self._fallback)
            self._quick_view.hide()

    def _on_scene_graph_error(self, error, message: str) -> None:
        """Switch to the known raster path if the selected graphics API fails."""
        self._graphics_failed = True
        logger.error(
            "API gráfica indisponível; usando saída Qt raster: %s",
            message,
            extra={
                "event": "graphics_video_fallback",
                "graphics_error": str(error),
                "graphics_message": message,
            },
        )
        self._refresh_raster_fallback()
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

    def set_render_backend(self, mode: str) -> bool:
        """Select automatic Vulkan/raster, forced Vulkan, or forced Qt raster.

        Returns False when Vulkan was explicitly requested but is unavailable.
        """
        if mode not in {"auto", "vulkan", "raster"}:
            raise ValueError("mode deve ser 'auto', 'vulkan' ou 'raster'")
        if mode == "vulkan" and (
            self._graphics_api not in {"auto", "vulkan"}
            or self._graphics_failed
            or self._quick_container is None
        ):
            return False
        self._render_backend = mode
        use_vulkan = mode == "vulkan" or (mode == "auto" and not self._graphics_failed)
        if use_vulkan and self._quick_container is not None:
            self._stack.setCurrentWidget(self._quick_container)
        else:
            self._refresh_raster_fallback()
            self._stack.setCurrentWidget(self._fallback)
        return True

    def set_filter_mode(self, mode: str) -> None:
        if mode not in ("nearest", "bilinear", "dither_blending", "crt_ntsc"):
            raise ValueError("modo de filtro de vídeo inválido")
        self._filter_mode = mode
        self._apply_render_settings()
        self._refresh_filtered_frame()

    def set_crt_parameters(
        self,
        *,
        scanline_intensity: int,
        scanline_thickness: int,
        phosphor_mask: int,
        curvature: int,
        signal_mode: str,
    ) -> None:
        """Configure presentation-only CRT effects and refresh the current frame."""
        candidate = {
            "scanline_intensity": scanline_intensity,
            "scanline_thickness": scanline_thickness,
            "phosphor_mask": phosphor_mask,
            "curvature": curvature,
            "signal_mode": signal_mode,
        }
        # Validate the complete setting set before mutating widget state.
        apply_video_filter_rgb(
            np.zeros((1, 1, 3), dtype=np.uint8),
            "crt_ntsc",
            **candidate,
        )
        self._crt_parameters = candidate
        self._refresh_filtered_frame()

    def set_integer_scale(self, enabled: bool) -> None:
        if type(enabled) is not bool:
            raise TypeError("enabled deve ser bool")
        self._integer_scale = enabled
        self._apply_render_settings()
        self._refresh_raster_fallback()

    def set_framebuffer(self, image: QImage) -> None:
        """Publish a completed frame without modifying the emulator-owned buffer."""
        self._source_image = QImage(image)
        self._refresh_filtered_frame()
        if self._qml_root is not None and not self._graphics_failed:
            self._qml_root.setProperty("hasFrame", True)
        if self._render_backend == "raster" or self._graphics_failed:
            self._refresh_raster_fallback()
            self._stack.setCurrentWidget(self._fallback)
        elif self._quick_container is not None:
            self._stack.setCurrentWidget(self._quick_container)
        else:
            self._stack.setCurrentWidget(self._fallback)

    def clear_framebuffer(
        self, message: str = "Abra uma ROM SG-1000 pelo menu Arquivo"
    ) -> None:
        self._source_image = None
        self._present_image = None
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
        scale_filter = "bilinear" if self._filter_mode == "bilinear" else "nearest"
        self._qml_root.setProperty("filterMode", scale_filter)
        self._qml_root.setProperty("integerScale", self._integer_scale)

    def _refresh_filtered_frame(self) -> None:
        if self._source_image is None or self._source_image.isNull():
            self._present_image = None
            return
        if self._filter_mode in {"nearest", "bilinear"}:
            # Keep the established fast path unchanged for the reference filters.
            self._present_image = QImage(self._source_image)
        else:
            source = self._source_image.convertToFormat(QImage.Format.Format_RGB888)
            width, height = source.width(), source.height()
            stride = source.bytesPerLine()
            raw = np.frombuffer(
                source.constBits(), dtype=np.uint8, count=source.sizeInBytes()
            ).reshape(height, stride)
            rgb = raw[:, : width * 3].reshape(height, width, 3)
            filtered = apply_video_filter_rgb(
                rgb,
                self._filter_mode,
                **self._crt_parameters,
            )
            self._present_image = QImage(
                filtered.data,
                width,
                height,
                int(filtered.strides[0]),
                QImage.Format.Format_RGB888,
            ).copy()
        if self._image_provider is not None:
            self._image_provider.set_image(self._present_image)
        if self._qml_root is not None and not self._graphics_failed:
            self._qml_root.setProperty(
                "frameRevision", int(self._qml_root.property("frameRevision")) + 1
            )
        self._refresh_raster_fallback()

    def _refresh_raster_fallback(self) -> None:
        if self._present_image is None or self._present_image.isNull():
            return
        available = self._fallback.contentsRect().size()
        width_factor = available.width() // self.BASE_WIDTH
        height_factor = available.height() // self.BASE_HEIGHT
        factor = min(width_factor, height_factor)
        transformation = (
            Qt.TransformationMode.SmoothTransformation
            if self._filter_mode == "bilinear"
            else Qt.TransformationMode.FastTransformation
        )
        if self._integer_scale and factor >= 1:
            target = self._present_image.size() * factor
            scaled = QPixmap.fromImage(self._present_image).scaled(
                target,
                Qt.AspectRatioMode.KeepAspectRatio,
                transformation,
            )
        else:
            scaled = QPixmap.fromImage(self._present_image).scaled(
                available,
                Qt.AspectRatioMode.KeepAspectRatio,
                transformation,
            )
        self._fallback.setText("")
        self._fallback.setPixmap(scaled)
