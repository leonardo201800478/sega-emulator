"""Tests for automatic pause/resume on application focus changes."""

from __future__ import annotations

from sega_emulator.frontend.qt.focus_pause import FocusPauseController


def test_focus_loss_pauses_once_and_focus_gain_resumes() -> None:
    events: list[str] = []
    controller = FocusPauseController(
        pause=lambda: events.append("pause") is None,
        resume=lambda: events.append("resume"),
    )

    controller.application_state_changed("inactive", "active")
    controller.application_state_changed("hidden", "active")
    assert events == ["pause"]

    controller.application_state_changed("active", "active")
    controller.application_state_changed("active", "active")
    assert events == ["pause", "resume"]


def test_focus_gain_does_not_resume_a_user_paused_session() -> None:
    events: list[str] = []
    controller = FocusPauseController(
        pause=lambda: False,
        resume=lambda: events.append("resume"),
    )

    controller.application_state_changed("inactive", "active")
    controller.application_state_changed("active", "active")

    assert events == []


def test_repeated_focus_loss_after_resume_can_pause_again() -> None:
    events: list[str] = []
    controller = FocusPauseController(
        pause=lambda: events.append("pause") is None,
        resume=lambda: events.append("resume"),
    )

    for state in ("inactive", "active", "hidden", "active"):
        controller.application_state_changed(state, "active")

    assert events == ["pause", "resume", "pause", "resume"]
