"""Guard the architectural boundary: Python must not own video pixels."""

from __future__ import annotations

import pytest

from sega_emulator.frontend.qt.video_widget import VideoWidget


def test_legacy_qt_video_widget_cannot_be_constructed() -> None:
    with pytest.raises(RuntimeError, match="host gráfico nativo em Rust"):
        VideoWidget()
