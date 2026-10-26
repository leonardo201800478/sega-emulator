"""Tests for wall-clock pacing used by the Qt frontend."""

import pytest

from sega_emulator.frontend.qt.frame_pacer import FramePacer


def test_first_frame_runs_immediately_after_creation_or_reset() -> None:
    pacer = FramePacer(1 / 60)

    assert pacer.should_run(10.0)
    assert not pacer.should_run(10.001)

    pacer.reset()

    assert pacer.should_run(10.002)


def test_frame_pacer_waits_until_target_period() -> None:
    pacer = FramePacer(1 / 60)

    assert pacer.should_run(1.0)
    assert not pacer.should_run(1.01)
    assert pacer.should_run(1.0 + 1 / 60)


def test_slow_frame_does_not_cause_catch_up_burst() -> None:
    pacer = FramePacer(1 / 60)

    assert pacer.should_run(1.0)
    # The previous frame took longer than one period; one new frame may run,
    # but the next one must still respect a full period from this start.
    assert pacer.should_run(1.1)
    assert not pacer.should_run(1.101)


@pytest.mark.parametrize("period", [0.0, -0.01])
def test_frame_pacer_rejects_non_positive_period(period: float) -> None:
    with pytest.raises(ValueError, match="period_seconds"):
        FramePacer(period)
