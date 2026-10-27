"""Synthetic reference tests for presentation-only Dither and CRT filters."""

from __future__ import annotations

import pytest

np = pytest.importorskip("numpy")

from sega_emulator.frontend.qt.video_filters import apply_video_filter_rgb


def test_nearest_and_bilinear_leave_source_pixels_unchanged() -> None:
    source = np.arange(4 * 4 * 3, dtype=np.uint8).reshape(4, 4, 3)
    original = source.copy()

    nearest = apply_video_filter_rgb(source, "nearest")
    bilinear = apply_video_filter_rgb(source, "bilinear")

    assert np.array_equal(nearest, original)
    assert np.array_equal(bilinear, original)
    assert not np.shares_memory(nearest, source)
    assert not np.shares_memory(bilinear, source)
    assert np.array_equal(source, original)


def test_dither_blending_only_merges_detected_checkerboard_tiles() -> None:
    source = np.full((2, 4, 3), 20, dtype=np.uint8)
    source[:, :2] = np.array(
        [[[0, 0, 0], [255, 255, 255]], [[255, 255, 255], [0, 0, 0]]],
        dtype=np.uint8,
    )
    original = source.copy()

    result = apply_video_filter_rgb(source, "dither_blending")

    assert np.all(result[:, :2] == 127)
    assert np.all(result[:, 2:] == 20)
    assert np.array_equal(source, original)


def test_dither_does_not_blur_solid_or_non_checkerboard_edges() -> None:
    source = np.zeros((4, 4, 3), dtype=np.uint8)
    source[:, 2:] = 255
    original = source.copy()

    result = apply_video_filter_rgb(source, "dither_blending")

    assert np.array_equal(result, original)


def test_crt_rgb_mode_is_identity_when_effects_are_disabled() -> None:
    source = np.arange(8 * 8 * 3, dtype=np.uint8).reshape(8, 8, 3)
    original = source.copy()

    result = apply_video_filter_rgb(
        source,
        "crt_ntsc",
        scanline_intensity=0,
        scanline_thickness=0,
        phosphor_mask=0,
        curvature=0,
        signal_mode="rgb",
    )

    assert np.array_equal(result, original)
    assert np.array_equal(source, original)


def test_crt_scanlines_and_phosphor_mask_change_only_output_copy() -> None:
    source = np.full((8, 12, 3), 200, dtype=np.uint8)
    original = source.copy()

    result = apply_video_filter_rgb(
        source,
        "crt_ntsc",
        scanline_intensity=70,
        scanline_thickness=50,
        phosphor_mask=50,
        curvature=0,
        signal_mode="rgb",
    )

    assert result.shape == source.shape
    assert result.dtype == np.uint8
    assert not np.array_equal(result, original)
    assert np.array_equal(source, original)


def test_composite_and_rf_are_deterministic_spatial_approximations() -> None:
    source = np.zeros((8, 12, 3), dtype=np.uint8)
    source[:, ::2, 0] = 255
    source[:, 1::2, 2] = 255

    composite = apply_video_filter_rgb(
        source,
        "crt_ntsc",
        scanline_intensity=0,
        scanline_thickness=0,
        phosphor_mask=0,
        curvature=0,
        signal_mode="composite",
    )
    composite_again = apply_video_filter_rgb(
        source,
        "crt_ntsc",
        scanline_intensity=0,
        scanline_thickness=0,
        phosphor_mask=0,
        curvature=0,
        signal_mode="composite",
    )
    rf = apply_video_filter_rgb(
        source,
        "crt_ntsc",
        scanline_intensity=0,
        scanline_thickness=0,
        phosphor_mask=0,
        curvature=0,
        signal_mode="rf",
    )

    assert not np.array_equal(composite, source)
    assert np.array_equal(composite, composite_again)
    assert not np.array_equal(rf, composite)


def test_curvature_remaps_corners_and_preserves_dimensions() -> None:
    source = np.full((16, 16, 3), 180, dtype=np.uint8)

    result = apply_video_filter_rgb(
        source,
        "crt_ntsc",
        scanline_intensity=0,
        scanline_thickness=0,
        phosphor_mask=0,
        curvature=100,
        signal_mode="rgb",
    )

    assert result.shape == source.shape
    assert np.all(result[0, 0] == 0)
    assert np.all(result[-1, -1] == 0)


@pytest.mark.parametrize(
    ("mode", "kwargs"),
    [
        ("unknown", {}),
        ("crt_ntsc", {"signal_mode": "pal"}),
        ("crt_ntsc", {"scanline_intensity": 101}),
        ("crt_ntsc", {"curvature": -1}),
    ],
)
def test_rejects_invalid_filter_configuration(mode: str, kwargs: dict) -> None:
    source = np.zeros((2, 2, 3), dtype=np.uint8)
    with pytest.raises(ValueError):
        apply_video_filter_rgb(source, mode, **kwargs)
