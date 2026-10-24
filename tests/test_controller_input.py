from __future__ import annotations

from types import SimpleNamespace

import pytest

from sega_emulator.frontend.controller_input import (
    ControllerInput,
    ControllerInputConfig,
    PlayerInputProfile,
    joystick_value,
)


class FakeController:
    def __init__(self, buttons: set[int] | None = None, axes: dict[int, int] | None = None) -> None:
        self.buttons = buttons or set()
        self.axes = axes or {}
        self.is_attached = True
        self.closed = False

    def attached(self) -> bool:
        return self.is_attached

    def quit(self) -> None:
        self.closed = True

    def get_button(self, button: int) -> bool:
        return button in self.buttons

    def get_axis(self, axis: int) -> int:
        return self.axes.get(axis, 0)


class FakeControllerModule:
    CONTROLLER_BUTTON_A = 0
    CONTROLLER_BUTTON_B = 1
    CONTROLLER_BUTTON_X = 2
    CONTROLLER_BUTTON_Y = 3
    CONTROLLER_BUTTON_DPAD_UP = 4
    CONTROLLER_BUTTON_DPAD_DOWN = 5
    CONTROLLER_BUTTON_DPAD_LEFT = 6
    CONTROLLER_BUTTON_DPAD_RIGHT = 7
    CONTROLLER_BUTTON_LEFTSHOULDER = 8
    CONTROLLER_BUTTON_RIGHTSHOULDER = 9
    CONTROLLER_BUTTON_LEFTSTICK = 10
    CONTROLLER_BUTTON_RIGHTSTICK = 11
    CONTROLLER_BUTTON_BACK = 12
    CONTROLLER_BUTTON_GUIDE = 13
    CONTROLLER_BUTTON_START = 14
    CONTROLLER_AXIS_LEFTX = 0
    CONTROLLER_AXIS_LEFTY = 1

    def __init__(self) -> None:
        self.devices = [FakeController(), FakeController()]
        self.initialized = False

    def get_init(self) -> bool:
        return self.initialized

    def init(self) -> None:
        self.initialized = True

    def get_count(self) -> int:
        return len(self.devices)

    def is_controller(self, index: int) -> bool:
        return 0 <= index < len(self.devices)

    def Controller(self, index: int) -> FakeController:
        return self.devices[index]

    def name_forindex(self, index: int) -> str:
        return f"Fake controller {index}"


def test_active_low_joystick_encoding() -> None:
    assert joystick_value(set()) == 0xFF
    assert joystick_value({"up", "button2"}) == 0xDF
    with pytest.raises(ValueError, match="Ações desconhecidas"):
        joystick_value({"start"})


def test_controller_configuration_round_trip_and_validation() -> None:
    config = ControllerInputConfig(
        players={
            1: PlayerInputProfile(
                gamepad_buttons={"button1": "x", "button2": "y"},
                use_left_stick=False,
            ),
            2: PlayerInputProfile(),
        },
        deadzone=0.2,
    )

    decoded = ControllerInputConfig.from_json(config.to_json())

    assert decoded.deadzone == pytest.approx(0.2)
    assert decoded.players[1].gamepad_buttons == {"button1": "x", "button2": "y"}
    assert decoded.players[1].use_left_stick is False
    with pytest.raises(ValueError, match="não suportada"):
        ControllerInputConfig.from_json('{"schema_version":99,"players":{}}')


def test_sdl_controllers_map_independently_to_two_players() -> None:
    backend = FakeControllerModule()
    backend.devices[0].buttons.update({backend.CONTROLLER_BUTTON_A, backend.CONTROLLER_BUTTON_DPAD_RIGHT})
    backend.devices[1].buttons.add(backend.CONTROLLER_BUTTON_B)
    backend.devices[1].axes[backend.CONTROLLER_AXIS_LEFTX] = -32768
    pygame_stub = SimpleNamespace(_sdl2=SimpleNamespace(controller=backend))

    inputs = ControllerInput(pygame_stub)
    actions = inputs.poll()

    assert actions[1] == {"button1", "right"}
    assert actions[2] == {"button2", "left"}
    assert inputs.connected_names() == ["Fake controller 0", "Fake controller 1"]


def test_disconnected_controller_is_closed_and_does_not_keep_input_pressed() -> None:
    backend = FakeControllerModule()
    backend.devices[0].buttons.add(backend.CONTROLLER_BUTTON_A)
    inputs = ControllerInput(SimpleNamespace(_sdl2=SimpleNamespace(controller=backend)))
    assert inputs.poll()[1] == {"button1"}

    disconnected = backend.devices[0]
    disconnected.is_attached = False
    backend.devices = [backend.devices[1]]

    actions = inputs.poll()
    assert disconnected.closed
    assert actions[1] == set()
