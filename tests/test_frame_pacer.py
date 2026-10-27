"""Unit tests for deterministic host frame pacing."""

from __future__ import annotations

import pytest

from sega_emulator.frontend.qt.frame_pacer import FramePacer


def test_first_frame_runs_immediately_then_waits_for_period() -> None:
    pacer = FramePacer(1.0)

    assert pacer.should_run(0.0)
    assert not pacer.should_run(0.999)
    assert pacer.should_run(1.0)
    assert not pacer.should_run(1.999)
    assert pacer.should_run(2.0)


def test_late_frame_skips_missed_slots_without_drifting_cadence() -> None:
    pacer = FramePacer(1.0)

    assert pacer.should_run(0.0)
    assert pacer.should_run(1.2)
    assert not pacer.should_run(1.99)
    assert pacer.should_run(2.0)


def test_frame_pacer_rejects_non_positive_period() -> None:
    for period in (0.0, -1.0):
        with pytest.raises(ValueError, match="positive"):
            FramePacer(period)


def test_reset_allows_immediate_frame_again() -> None:
    pacer = FramePacer(1.0)
    assert pacer.should_run(0.0)
    assert not pacer.should_run(0.5)

    pacer.reset()

    assert pacer.should_run(0.5)
