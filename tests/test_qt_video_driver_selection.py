"""The former Python Qt video renderer must remain disabled."""

from __future__ import annotations

import pytest

from sega_emulator.frontend.qt.video_widget import VideoWidget


def test_legacy_video_widget_is_rejected() -> None:
    with pytest.raises(RuntimeError, match="não pode receber framebuffers"):
        VideoWidget()
