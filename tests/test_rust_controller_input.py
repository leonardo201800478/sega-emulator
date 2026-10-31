from __future__ import annotations

import sys
from types import SimpleNamespace

import pytest

from sega_emulator.frontend.controller_profiles import ControllerInputConfig
from sega_emulator.frontend.rust_controller_input import RustControllerInput


class FakeNativeInputManager:
    def __init__(self) -> None:
        self.available = True
        self.backend_error = None
        self.profiles: dict[int, tuple[object, ...]] = {}
        self.frames: list[int] = []
        self.closed = False
        self.disconnect = True

    def set_backend_preference(self, preference: str) -> None:
        self.preference = preference

    def set_profile(self, player, bindings, use_left_stick, deadzone, turbo1, turbo2):
        self.profiles[player] = (
            dict(bindings),
            use_left_stick,
            deadzone,
            turbo1,
            turbo2,
        )

    def poll_actions(self, frame_number: int, keyboard_player1: int, keyboard_player2: int):
        self.frames.append(frame_number)
        return 0b0001_0001 | keyboard_player1, 0b0000_1000 | keyboard_player2

    def read_pressed_button(self, player: int):
        return "raw_button_4" if player == 1 else None

    def connected_names(self):
        return ["Test HID Controller"]

    def consume_disconnect_event(self):
        result = self.disconnect
        self.disconnect = False
        return result

    def close(self):
        self.closed = True


def install_fake_extension(monkeypatch: pytest.MonkeyPatch) -> FakeNativeInputManager:
    native = FakeNativeInputManager()
    monkeypatch.setitem(
        sys.modules,
        "sega_z80_core",
        SimpleNamespace(NativeInputManager=lambda: native),
    )
    return native


def test_qt_adapter_sends_profiles_to_rust_and_receives_semantic_actions(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    native = install_fake_extension(monkeypatch)
    config = ControllerInputConfig()
    controller = RustControllerInput(config, backend_preference="sdl3")

    assert native.preference == "sdl3"
    assert native.profiles[1][0] == config.players[1].gamepad_buttons
    assert native.profiles[2][2] == config.deadzone
    assert controller.poll_masks(123, {1: {"left"}, 2: {"button2"}}) == {
        1: 0b0001_1001,
        2: 0b0010_1000,
    }
    assert native.frames == [123]
    assert controller.read_pressed_button(1) == "raw_button_4"
    assert controller.connected_names() == ["Test HID Controller"]
    assert controller.consume_disconnect_event()
    assert not controller.consume_disconnect_event()


def test_qt_adapter_updates_native_profiles_when_ui_configuration_changes(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    native = install_fake_extension(monkeypatch)
    controller = RustControllerInput(ControllerInputConfig())
    updated = ControllerInputConfig()
    updated.deadzone = 0.25
    updated.players[1].gamepad_buttons["button1"] = "x"
    controller.config = updated

    assert native.profiles[1][0]["button1"] == "x"
    assert native.profiles[1][2] == 0.25


def test_qt_adapter_degrades_to_keyboard_only_if_extension_is_missing(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setitem(sys.modules, "sega_z80_core", None)
    controller = RustControllerInput(ControllerInputConfig())

    assert not controller.available
    assert controller.backend_error
    assert controller.poll(1) == {1: set(), 2: set()}
    assert controller.connected_names() == []
    assert controller.read_pressed_button(1) is None
    assert not controller.consume_disconnect_event()


def test_qt_adapter_closes_native_manager(monkeypatch: pytest.MonkeyPatch) -> None:
    native = install_fake_extension(monkeypatch)
    controller = RustControllerInput(ControllerInputConfig())
    controller.close()
    assert native.closed
