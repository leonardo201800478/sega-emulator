"""Retired compatibility shim for the former Python video presentation widget.

ADR-0012 assigns all framebuffer ownership, graphics resources and presentation
to the native Rust video host. This module intentionally contains no Qt image,
scene-graph, pixel-copy or rendering implementation.
"""

from __future__ import annotations


class VideoWidget:
    """Reject legacy Python-side video presentation."""

    def __init__(self, *args: object, **kwargs: object) -> None:
        del args, kwargs
        raise RuntimeError(
            "VideoWidget foi desativado: a camada Python/Qt não pode receber "
            "framebuffers nem renderizar vídeo. Use o host gráfico nativo em Rust."
        )
