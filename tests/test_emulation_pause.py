"""Unit tests for deterministic pause/resume transitions in the Qt host loop."""

from __future__ import annotations

from sega_emulator.frontend.qt.focus_pause import EmulationPauseController, FocusPauseController


class FakeTimer:
    def __init__(self, events: list[str]) -> None:
        self.events = events

    def stop(self) -> None:
        self.events.append("timer.stop")

    def start(self) -> None:
        self.events.append("timer.start")


def test_manual_pause_stops_timer_and_resume_restarts_it_once() -> None:
    events: list[str] = []
    timer = FakeTimer(events)
    controller = EmulationPauseController(
        can_pause=lambda: True,
        pause=timer.stop,
        resume=timer.start,
        on_change=lambda paused: events.append(f"state:{paused}"),
    )

    assert controller.set_paused(True)
    assert controller.paused
    assert not controller.set_paused(True)
    assert controller.set_paused(False)
    assert not controller.paused
    assert events == ["timer.stop", "state:True", "timer.start", "state:False"]


def test_pause_is_rejected_without_a_loaded_machine() -> None:
    events: list[str] = []
    controller = EmulationPauseController(
        can_pause=lambda: False,
        pause=lambda: events.append("pause"),
        resume=lambda: events.append("resume"),
        on_change=lambda paused: events.append(f"state:{paused}"),
    )

    assert not controller.toggle()
    assert not controller.paused
    assert events == []


def test_toggle_alternates_pause_and_resume_transitions() -> None:
    events: list[str] = []
    controller = EmulationPauseController(
        can_pause=lambda: True,
        pause=lambda: events.append("pause"),
        resume=lambda: events.append("resume"),
        on_change=lambda paused: events.append(f"state:{paused}"),
    )

    assert controller.toggle()
    assert controller.toggle()
    assert events == ["pause", "state:True", "resume", "state:False"]


def test_focus_loss_does_not_resume_a_manually_paused_session() -> None:
    controller = EmulationPauseController(
        can_pause=lambda: True,
        pause=lambda: None,
        resume=lambda: None,
        on_change=lambda paused: None,
    )
    focus = FocusPauseController(
        pause=lambda: controller.set_paused(True),
        resume=lambda: controller.set_paused(False),
    )

    assert controller.set_paused(True)
    focus.application_state_changed("inactive", "active")
    focus.application_state_changed("active", "active")

    assert controller.paused


def test_focus_loss_pause_is_resumed_when_focus_returns() -> None:
    controller = EmulationPauseController(
        can_pause=lambda: True,
        pause=lambda: None,
        resume=lambda: None,
        on_change=lambda paused: None,
    )
    focus = FocusPauseController(
        pause=lambda: controller.set_paused(True),
        resume=lambda: controller.set_paused(False),
    )

    focus.application_state_changed("inactive", "active")
    assert controller.paused
    focus.application_state_changed("active", "active")
    assert not controller.paused
