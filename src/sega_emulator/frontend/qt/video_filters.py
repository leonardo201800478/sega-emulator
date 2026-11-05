"""Retired Python pixel filters.

All video processing belongs to the native Rust graphics host. This module
remains only as an explicit compatibility guard for older imports.
"""

from __future__ import annotations


def apply_video_filter_rgb(*args: object, **kwargs: object) -> None:
    """Reject legacy Python-side pixel processing."""
    del args, kwargs
    raise RuntimeError(
        "Filtros de vídeo em Python foram desativados. "
        "O processamento de pixels pertence ao host nativo em Rust."
    )
