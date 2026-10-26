"""Wall-clock pacing for the SG-1000 Qt frontend."""

from __future__ import annotations


class FramePacer:
    """Limit frame starts to a target period without catch-up bursts.

    The first frame after reset is allowed immediately. When a frame starts,
    the next deadline is based on the current monotonic timestamp, so a slow
    frame cannot trigger a burst of back-to-back catch-up frames.
    """

    def __init__(self, period_seconds: float) -> None:
        if period_seconds <= 0:
            raise ValueError("period_seconds must be positive")
        self.period_seconds = period_seconds
        self._next_deadline: float | None = None

    def reset(self) -> None:
        """Allow the next frame immediately, e.g. after loading a new ROM."""
        self._next_deadline = None

    def should_run(self, now: float) -> bool:
        """Return whether a frame may start at the supplied monotonic time."""
        if self._next_deadline is not None and now < self._next_deadline:
            return False
        self._next_deadline = now + self.period_seconds
        return True
