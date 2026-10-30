"""Application-focus and user pause policies for the Qt emulation loop."""

from __future__ import annotations

from collections.abc import Callable


class EmulationPauseController:
    """Own the paused state and stop/restart the host frame timer exactly once."""

    def __init__(
        self,
        *,
        can_pause: Callable[[], bool],
        pause: Callable[[], None],
        resume: Callable[[], None],
        on_change: Callable[[bool], None],
    ) -> None:
        self._can_pause = can_pause
        self._pause = pause
        self._resume = resume
        self._on_change = on_change
        self._paused = False

    @property
    def paused(self) -> bool:
        """Return whether the current emulation session is paused."""
        return self._paused

    def set_paused(self, paused: bool) -> bool:
        """Apply a pause transition; return false for unavailable/no-op requests."""
        if not self._can_pause() or self._paused == paused:
            return False
        if paused:
            self._pause()
        else:
            self._resume()
        self._paused = paused
        self._on_change(paused)
        return True

    def toggle(self) -> bool:
        """Toggle pause only when an emulation machine is loaded."""
        return self.set_paused(not self._paused)


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
