from __future__ import annotations

from PySide6.QtCore import QSize, Qt
from PySide6.QtGui import QImage

from sega_emulator.frontend.qt.video_widget import FramebufferImageProvider


def test_framebuffer_provider_returns_detached_frame_snapshot() -> None:
    source = QImage(2, 2, QImage.Format.Format_RGB32)
    source.fill(Qt.GlobalColor.red)

    provider = FramebufferImageProvider()
    provider.set_image(source)
    source.fill(Qt.GlobalColor.blue)

    result = provider.requestImage("frame", QSize(), QSize())

    assert not result.isNull()
    assert result.size() == QSize(2, 2)
    assert result.pixelColor(0, 0).red() == 255
    assert result.pixelColor(0, 0).blue() == 0


def test_framebuffer_provider_returns_empty_image_after_clear() -> None:
    provider = FramebufferImageProvider()
    image = QImage(1, 1, QImage.Format.Format_RGB32)
    image.fill(Qt.GlobalColor.green)
    provider.set_image(image)
    provider.set_image(None)

    assert provider.requestImage("frame", QSize(), QSize()).isNull()


def test_framebuffer_provider_returns_independent_images() -> None:
    source = QImage(1, 1, QImage.Format.Format_RGB32)
    source.fill(Qt.GlobalColor.yellow)
    provider = FramebufferImageProvider()
    provider.set_image(source)

    first = provider.requestImage("frame", QSize(), QSize())
    first.fill(Qt.GlobalColor.black)
    second = provider.requestImage("frame", QSize(), QSize())

    assert second.pixelColor(0, 0).red() == 255
    assert second.pixelColor(0, 0).green() == 255
