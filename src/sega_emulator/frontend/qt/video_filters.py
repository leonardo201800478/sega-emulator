"""Deterministic presentation-only filters for completed RGB framebuffers.

These operations never access the emulated machine or modify its source framebuffer.
CRT signal modes are explicitly spatial approximations, not cycle-accurate NTSC models.
"""

from __future__ import annotations

import numpy as np
from numpy.typing import NDArray

RgbFrame = NDArray[np.uint8]

FILTER_MODES = ("nearest", "bilinear", "dither_blending", "crt_ntsc")
CRT_SIGNAL_MODES = ("rgb", "composite", "rf")


def apply_video_filter_rgb(
    frame: RgbFrame,
    mode: str,
    *,
    scanline_intensity: int = 40,
    scanline_thickness: int = 50,
    phosphor_mask: int = 25,
    curvature: int = 0,
    signal_mode: str = "rgb",
) -> RgbFrame:
    """Return a filtered RGB copy; never mutate the caller's framebuffer."""
    source = np.asarray(frame)
    if source.dtype != np.uint8 or source.ndim != 3 or source.shape[2] != 3:
        raise ValueError("frame must be an HxWx3 uint8 RGB array")
    if source.shape[0] == 0 or source.shape[1] == 0:
        raise ValueError("frame dimensions must be non-zero")
    if mode not in FILTER_MODES:
        raise ValueError(f"unsupported video filter: {mode}")
    for name, value in (
        ("scanline_intensity", scanline_intensity),
        ("scanline_thickness", scanline_thickness),
        ("phosphor_mask", phosphor_mask),
        ("curvature", curvature),
    ):
        if type(value) is not int or not 0 <= value <= 100:
            raise ValueError(f"{name} must be an integer from 0 to 100")
    if signal_mode not in CRT_SIGNAL_MODES:
        raise ValueError(f"unsupported CRT signal mode: {signal_mode}")

    result = np.array(source, dtype=np.uint8, copy=True)
    if mode == "dither_blending":
        return _blend_detected_checkerboards(result)
    if mode != "crt_ntsc":
        # Nearest and bilinear are performed by the existing Qt scaling stage.
        return result
    result = _apply_signal_approximation(result, signal_mode)
    result = _apply_crt_effects(
        result,
        scanline_intensity=scanline_intensity,
        scanline_thickness=scanline_thickness,
        phosphor_mask=phosphor_mask,
        curvature=curvature,
    )
    return result


def _blend_detected_checkerboards(frame: RgbFrame) -> RgbFrame:
    """Blend only aligned 2x2 checkerboards with two stable color clusters."""
    height, width, _ = frame.shape
    out = frame.copy()
    if height < 2 or width < 2:
        return out

    # Non-overlapping 2x2 tiles avoid a global blur and make the result stable.
    even_h = height - height % 2
    even_w = width - width % 2
    p00 = frame[0:even_h:2, 0:even_w:2].astype(np.int16)
    p01 = frame[0:even_h:2, 1:even_w:2].astype(np.int16)
    p10 = frame[1:even_h:2, 0:even_w:2].astype(np.int16)
    p11 = frame[1:even_h:2, 1:even_w:2].astype(np.int16)

    pair_a_close = np.max(np.abs(p00 - p11), axis=2) <= 18
    pair_b_close = np.max(np.abs(p01 - p10), axis=2) <= 18
    color_a = (p00 + p11) // 2
    color_b = (p01 + p10) // 2
    contrast = np.max(np.abs(color_a - color_b), axis=2) >= 24
    checker = pair_a_close & pair_b_close & contrast
    if not np.any(checker):
        return out

    blended = ((p00 + p01 + p10 + p11) // 4).astype(np.uint8)
    out[0:even_h:2, 0:even_w:2][checker] = blended[checker]
    out[0:even_h:2, 1:even_w:2][checker] = blended[checker]
    out[1:even_h:2, 0:even_w:2][checker] = blended[checker]
    out[1:even_h:2, 1:even_w:2][checker] = blended[checker]
    return out


def _apply_signal_approximation(frame: RgbFrame, mode: str) -> RgbFrame:
    if mode == "rgb":
        return frame
    rgb = frame.astype(np.float32) / 255.0
    red, green, blue = rgb[..., 0], rgb[..., 1], rgb[..., 2]
    y = 0.299 * red + 0.587 * green + 0.114 * blue
    i = 0.596 * red - 0.274 * green - 0.322 * blue
    q = 0.211 * red - 0.523 * green + 0.312 * blue

    # A small horizontal low-pass approximates composite chroma bandwidth.
    i = _horizontal_lowpass(i)
    q = _horizontal_lowpass(q)
    if mode == "rf":
        y = _horizontal_lowpass(y)
        i *= 0.55
        q *= 0.55

    out = np.empty_like(rgb)
    out[..., 0] = y + 0.956 * i + 0.621 * q
    out[..., 1] = y - 0.272 * i - 0.647 * q
    out[..., 2] = y - 1.106 * i + 1.703 * q
    return np.clip(out * 255.0 + 0.5, 0, 255).astype(np.uint8)


def _horizontal_lowpass(channel: NDArray[np.float32]) -> NDArray[np.float32]:
    padded = np.pad(channel, ((0, 0), (1, 1)), mode="edge")
    return padded[:, :-2] * 0.25 + padded[:, 1:-1] * 0.5 + padded[:, 2:] * 0.25


def _apply_crt_effects(
    frame: RgbFrame,
    *,
    scanline_intensity: int,
    scanline_thickness: int,
    phosphor_mask: int,
    curvature: int,
) -> RgbFrame:
    out = frame.astype(np.float32) / 255.0
    height, width, _ = out.shape

    if scanline_intensity and scanline_thickness:
        # Use a four-row period and cap coverage below 100% so the maximum
        # thickness never degenerates into a uniformly dark image.
        rows_per_period = min(3, round(scanline_thickness * 3 / 100))
        if rows_per_period:
            line_rows = (np.arange(height) % 4) < rows_per_period
            attenuation = (scanline_intensity / 100.0) * 0.65
            out[line_rows, :, :] *= 1.0 - attenuation

    if phosphor_mask:
        strength = (phosphor_mask / 100.0) * 0.35
        x = np.arange(width) % 3
        mask = np.full((width, 3), 1.0 - strength, dtype=np.float32)
        mask[x == 0, 0] = 1.0
        mask[x == 1, 1] = 1.0
        mask[x == 2, 2] = 1.0
        out *= mask[None, :, :]

    if curvature:
        yy, xx = np.meshgrid(
            np.linspace(-1.0, 1.0, height, dtype=np.float32),
            np.linspace(-1.0, 1.0, width, dtype=np.float32),
            indexing="ij",
        )
        strength = curvature / 100.0 * 0.18
        radial = 1.0 + strength * (xx * xx + yy * yy)
        source_x = np.rint(((xx * radial + 1.0) * 0.5) * (width - 1)).astype(np.int32)
        source_y = np.rint(((yy * radial + 1.0) * 0.5) * (height - 1)).astype(np.int32)
        valid = (
            (source_x >= 0)
            & (source_x < width)
            & (source_y >= 0)
            & (source_y < height)
        )
        source_x = np.clip(source_x, 0, width - 1)
        source_y = np.clip(source_y, 0, height - 1)
        curved = np.zeros_like(out)
        curved[valid] = out[source_y[valid], source_x[valid]]
        out = curved

    return np.clip(out * 255.0 + 0.5, 0, 255).astype(np.uint8)
