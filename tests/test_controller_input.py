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
    def __init__(
        self,
        buttons: set[int] | None = None,
        axes: dict[int, int] | None = None,
    ) -> None:
        self.buttons = buttons or set()
        self.axes = axes or {}
        self.is_attached = True
        self.closed = False
        self.name = ""

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
    CONTROLLER_BUTTON_MISC1 = 15
    CONTROLLER_BUTTON_PADDLE1 = 16
    CONTROLLER_BUTTON_PADDLE2 = 17
    CONTROLLER_BUTTON_PADDLE3 = 18
    CONTROLLER_BUTTON_PADDLE4 = 19
    CONTROLLER_BUTTON_TOUCHPAD = 20
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
        self.devices[index].name = f"Fake controller {index}"
        return self.devices[index]

    def name_forindex(self, index: int) -> str:
        return f"Fake controller {index}"


def test_active_low_joystick_encoding() -> None:
    assert joystick_value(set()) == 0xFF
    assert joystick_value({"up", "button2"}) == 0xDE
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
    assert decoded.players[1].gamepad_buttons["button1"] == "x"
    assert decoded.players[1].gamepad_buttons["button2"] == "y"
    assert decoded.players[1].use_left_stick is False
    assert decoded.players[1].gamepad_buttons["hold"] == "start"
    assert decoded.players[1].keyboard_bindings["button1"] == "Z"
    assert decoded.players[2].keyboard_bindings["button1"] == "Z"
    assert decoded.players[1].turbo_buttons == {"button1": False, "button2": False}
    with pytest.raises(ValueError, match="não suportada"):
        ControllerInputConfig.from_json('{"schema_version":99,"players":{}}')


def test_default_deadzone_is_fifteen_percent() -> None:
    config = ControllerInputConfig()
    assert config.deadzone == pytest.approx(0.15)


def test_legacy_config_gets_keyboard_defaults_and_fifteen_percent_deadzone() -> None:
    serialized = (
        '{"schema_version":1,"players":{"1":{"gamepad_buttons":{"button1":"x"}},'
        '"2":{"gamepad_buttons":{"button2":"y"}}}}'
    )
    config = ControllerInputConfig.from_json(serialized)
    assert config.deadzone == pytest.approx(0.15)
    assert config.players[1].keyboard_bindings["button1"] == "Z"
    assert config.players[2].keyboard_bindings["button1"] == "F"
    assert config.players[1].gamepad_buttons["button1"] == "x"
    assert config.players[2].gamepad_buttons["button2"] == "y"


def test_keyboard_mapping_and_turbo_options_round_trip() -> None:
    config = ControllerInputConfig()
    config.players[1].keyboard_bindings["button1"] = "J"
    config.players[1].turbo_buttons["button1"] = True
    config.players[2].keyboard_bindings["hold"] = "K"
    config.players[2].turbo_buttons["button2"] = True

    decoded = ControllerInputConfig.from_json(config.to_json())

    assert decoded.players[1].keyboard_bindings["button1"] == "J"
    assert decoded.players[1].turbo_buttons == {"button1": True, "button2": False}
    assert decoded.players[2].keyboard_bindings["hold"] == "K"
    assert decoded.players[2].turbo_buttons == {"button1": False, "button2": True}


def test_keyboard_mapping_rejects_duplicate_keys_for_one_player() -> None:
    with pytest.raises(ValueError, match="mesma tecla"):
        PlayerInputProfile(
            keyboard_bindings={
                "up": "J",
                "down": "j",
            }
        )


def test_sdl_controllers_map_independently_to_two_players() -> None:
    backend = FakeControllerModule()
    backend.devices[0].buttons.update(
        {backend.CONTROLLER_BUTTON_A, backend.CONTROLLER_BUTTON_DPAD_RIGHT}
    )
    backend.devices[1].buttons.add(backend.CONTROLLER_BUTTON_B)
    backend.devices[1].axes[backend.CONTROLLER_AXIS_LEFTX] = -32768
    pygame_stub = SimpleNamespace(_sdl2=SimpleNamespace(controller=backend))

    inputs = ControllerInput(pygame_stub)
    actions = inputs.poll()

    assert actions[1] == {"button1", "right"}
    assert actions[2] == {"button2", "left"}
    assert inputs.connected_names() == ["Fake controller 0", "Fake controller 1"]


def test_analog_deadzone_filters_small_axis_drift() -> None:
    backend = FakeControllerModule()
    backend.devices[0].axes[backend.CONTROLLER_AXIS_LEFTX] = 4000
    backend.devices[0].axes[backend.CONTROLLER_AXIS_LEFTY] = -4000
    inputs = ControllerInput(SimpleNamespace(_sdl2=SimpleNamespace(controller=backend)))
    assert inputs.poll()[1] == set()

    backend.devices[0].axes[backend.CONTROLLER_AXIS_LEFTX] = 6000
    backend.devices[0].axes[backend.CONTROLLER_AXIS_LEFTY] = -6000
    assert inputs.poll()[1] == {"right", "up"}


def test_connected_names_supports_pygame_ce_without_name_forindex() -> None:
    backend = FakeControllerModule()
    backend.name_forindex = None
    inputs = ControllerInput(SimpleNamespace(_sdl2=SimpleNamespace(controller=backend)))
    assert inputs.connected_names() == ["Fake controller 0", "Fake controller 1"]


def test_hold_switch_is_separate_from_the_six_joystick_lines() -> None:
    backend = FakeControllerModule()
    backend.devices[0].buttons.add(backend.CONTROLLER_BUTTON_START)
    inputs = ControllerInput(SimpleNamespace(_sdl2=SimpleNamespace(controller=backend)))

    actions = inputs.poll()

    assert actions[1] == {"hold"}
    # Hold is the console NMI switch, not one of the joystick port bits.
    assert (
        joystick_value(
            actions[1] & {"up", "down", "left", "right", "button1", "button2"}
        )
        == 0xFF
    )

    inputs.config.players[1].gamepad_buttons["hold"] = "back"
    backend.devices[0].buttons.clear()
    backend.devices[0].buttons.add(backend.CONTROLLER_BUTTON_BACK)
    assert inputs.poll()[1] == {"hold"}


def test_modern_auxiliary_gamepad_buttons_are_supported() -> None:
    backend = FakeControllerModule()
    backend.devices[0].buttons.update(
        {
            backend.CONTROLLER_BUTTON_MISC1,
            backend.CONTROLLER_BUTTON_PADDLE1,
            backend.CONTROLLER_BUTTON_PADDLE2,
            backend.CONTROLLER_BUTTON_PADDLE3,
            backend.CONTROLLER_BUTTON_PADDLE4,
            backend.CONTROLLER_BUTTON_TOUCHPAD,
        }
    )
    inputs = ControllerInput(SimpleNamespace(_sdl2=SimpleNamespace(controller=backend)))
    profile = inputs.config.players[1]
    profile.gamepad_buttons = {
        "button1": "misc1",
        "button2": "paddle1",
        "up": "paddle2",
        "down": "paddle3",
        "left": "paddle4",
        "right": "touchpad",
    }

    assert inputs.poll()[1] == {"button1", "button2", "up", "down", "left", "right"}


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
