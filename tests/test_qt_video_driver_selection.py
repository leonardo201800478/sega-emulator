"""Offscreen checks for the Qt raster/Vulkan presentation selector."""

from __future__ import annotations

import os

os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")

import pytest  # noqa: E402

pytest.importorskip("PySide6")

from PySide6.QtGui import QImage  # noqa: E402
from PySide6.QtWidgets import QApplication  # noqa: E402

from sega_emulator.frontend.qt.video_widget import (  # noqa: E402
    VideoWidget,
    _graphics_api_for_mode,
)
from PySide6.QtQuick import QSGRendererInterface  # noqa: E402


@pytest.fixture
def qt_app():
    app = QApplication.instance() or QApplication([])
    yield app


def test_forced_raster_driver_remains_selected_after_frame_updates(qt_app) -> None:
    widget = VideoWidget()
    assert widget.set_render_backend("raster")
    frame = QImage(256, 192, QImage.Format.Format_RGB888)
    frame.fill(0)
    widget.set_framebuffer(frame)
    assert widget._render_backend == "raster"
    assert widget._stack.currentWidget() is widget._fallback
    widget.close()


def test_unknown_video_driver_is_rejected(qt_app) -> None:
    widget = VideoWidget()
    with pytest.raises(ValueError, match="mode"):
        widget.set_render_backend("directx")
    widget.close()


@pytest.mark.parametrize(
    ("mode", "expected"),
    [
        ("auto", QSGRendererInterface.GraphicsApi.Vulkan),
        ("vulkan", QSGRendererInterface.GraphicsApi.Vulkan),
        ("d3d11", QSGRendererInterface.GraphicsApi.Direct3D11),
        ("d3d12", QSGRendererInterface.GraphicsApi.Direct3D12),
    ],
)
def test_native_graphics_api_mapping(mode, expected) -> None:
    assert _graphics_api_for_mode(mode) == expected


def test_raster_is_not_a_hardware_graphics_api() -> None:
    with pytest.raises(ValueError, match="graphics API"):
        _graphics_api_for_mode("raster")
