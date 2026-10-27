"""Offscreen checks for the Qt raster/Vulkan presentation selector."""
from __future__ import annotations

import os

os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")

import pytest

pytest.importorskip("PySide6")

from PySide6.QtGui import QImage
from PySide6.QtWidgets import QApplication

from sega_emulator.frontend.qt.video_widget import VideoWidget


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
