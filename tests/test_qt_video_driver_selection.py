"""The former Python Qt video renderer must remain disabled."""

from __future__ import annotations

import pytest

from sega_emulator.frontend.qt.video_widget import VideoWidget


def test_legacy_video_widget_is_rejected() -> None:
    with pytest.raises(RuntimeError, match="não pode receber framebuffers"):
        VideoWidget()


def test_active_qt_modules_do_not_import_or_present_video_pixels() -> None:
    root = project_root()
    active_modules = (
        root / "src/sega_emulator/frontend/qt/minimal_sms_window.py",
        root / "src/sega_emulator/frontend/qt/main_window.py",
        root / "src/sega_emulator/frontend/qt/video_widget.py",
        root / "src/sega_emulator/frontend/qt_frontend.py",
    )
    forbidden = (
        "QImage",
        "QPixmap",
        "QQuickView",
        "FramebufferImageProvider",
        "set_framebuffer(",
        "machine.framebuffer",
    )

    for module in active_modules:
        source = module.read_text(encoding="utf-8")
        for symbol in forbidden:
            assert symbol not in source, f"{symbol} found in {module.name}"


def project_root():
    from pathlib import Path

    return Path(__file__).resolve().parents[1]
