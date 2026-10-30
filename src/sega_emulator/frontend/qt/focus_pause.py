"""Cross-platform application-focus pause policy, independent of Qt bindings."""

from __future__ import annotations

from collections.abc import Callable


class FocusPauseController:
    """Resume only when focus loss itself caused the current pause."""

    def __init__(
        self,
        *,
        pause: Callable[[], bool],
        resume: Callable[[], None],
    ) -> None:
        self._pause = pause
        self._resume = resume
        self._paused_by_focus = False

    def application_state_changed(self, state: object, active_state: object) -> None:
        """Handle Qt application-state transitions without platform-specific APIs."""
        if state == active_state:
            if self._paused_by_focus:
                self._paused_by_focus = False
                self._resume()
            return

        if not self._paused_by_focus and self._pause():
            self._paused_by_focus = True
