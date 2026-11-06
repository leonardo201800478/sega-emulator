"""Python-side pixel filtering is prohibited by the native video architecture."""

from __future__ import annotations

import pytest

from sega_emulator.frontend.qt.video_filters import apply_video_filter_rgb


def test_legacy_python_pixel_filter_is_rejected() -> None:
    with pytest.raises(RuntimeError, match="processamento de pixels pertence"):
        apply_video_filter_rgb(object(), "crt_ntsc")
