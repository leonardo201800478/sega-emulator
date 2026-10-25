from __future__ import annotations

import ctypes
from types import SimpleNamespace

from sega_emulator.frontend.controller_input import ControllerInputConfig
from sega_emulator.frontend.sdl3_controller_input import SDL3ControllerInput


class FakeSDL3:
    SDL_INIT_EVENTS = 1
    SDL_INIT_GAMEPAD = 2
    SDL_INIT_JOYSTICK = 4
    SDL_GAMEPAD_BUTTON_SOUTH = 0
    SDL_GAMEPAD_BUTTON_EAST = 1
    SDL_GAMEPAD_BUTTON_DPAD_UP = 11
    SDL_GAMEPAD_BUTTON_DPAD_DOWN = 12
    SDL_GAMEPAD_BUTTON_DPAD_LEFT = 13
    SDL_GAMEPAD_BUTTON_DPAD_RIGHT = 14
    SDL_GAMEPAD_AXIS_LEFTX = 0
    SDL_GAMEPAD_AXIS_LEFTY = 1

    def __init__(self) -> None:
        self.gamepad_ids: list[int] = [10]
        self.joystick_ids: list[int] = [10]
        self.gamepads: dict[int, SimpleNamespace] = {
            10: SimpleNamespace(
                name=b"Fake SDL3 gamepad",
                buttons={0, 14},
                axes={0: 0, 1: 0},
                joystick=SimpleNamespace(
                    buttons=set(),
                    axes={},
                    hats={},
                    num_buttons=8,
                    num_axes=2,
                    num_hats=1,
                ),
            )
        }
        self.joysticks: dict[int, SimpleNamespace] = {}
        self.closed_gamepads: list[int] = []
        self.closed_joysticks: list[int] = []

    def SDL_InitSubSystem(self, flags: int) -> bool:
        assert flags == 7
        return True

    def SDL_GetError(self) -> bytes:
        return b"fake error"

    def _ids(self, ids: list[int], count_pointer: object) -> object:
        ctypes.cast(count_pointer, ctypes.POINTER(ctypes.c_int)).contents.value = len(ids)
        return (ctypes.c_int * len(ids))(*ids)

    def SDL_GetGamepads(self, count_pointer: object) -> object:
        return self._ids(self.gamepad_ids, count_pointer)

    def SDL_GetJoysticks(self, count_pointer: object) -> object:
        return self._ids(self.joystick_ids, count_pointer)

    def SDL_free(self, pointer: object) -> None:
        pass

    def SDL_PumpEvents(self) -> None:
        pass

    def SDL_OpenGamepad(self, instance_id: int) -> object:
        return self.gamepads[instance_id]

    def SDL_OpenJoystick(self, instance_id: int) -> object:
        return self.joysticks[instance_id]

    def SDL_CloseGamepad(self, handle: object) -> None:
        self.closed_gamepads.append(id(handle))

    def SDL_CloseJoystick(self, handle: object) -> None:
        self.closed_joysticks.append(id(handle))

    def SDL_GetGamepadName(self, handle: SimpleNamespace) -> bytes:
        return handle.name

    def SDL_GetGamepadButton(self, handle: SimpleNamespace, button: int) -> bool:
        return button in handle.buttons

    def SDL_GetGamepadAxis(self, handle: SimpleNamespace, axis: int) -> int:
        return handle.axes.get(axis, 0)

    def SDL_GetGamepadJoystick(self, handle: SimpleNamespace) -> SimpleNamespace:
        return handle.joystick

    def SDL_GetJoystickName(self, handle: SimpleNamespace) -> bytes:
        return handle.name

    def SDL_GetNumJoystickButtons(self, handle: SimpleNamespace) -> int:
        return handle.num_buttons

    def SDL_GetNumJoystickAxes(self, handle: SimpleNamespace) -> int:
        return handle.num_axes

    def SDL_GetNumJoystickHats(self, handle: SimpleNamespace) -> int:
        return handle.num_hats

    def SDL_GetJoystickButton(self, handle: SimpleNamespace, button: int) -> bool:
        return button in handle.buttons

    def SDL_GetJoystickAxis(self, handle: SimpleNamespace, axis: int) -> int:
        return handle.axes.get(axis, 0)

    def SDL_GetJoystickHat(self, handle: SimpleNamespace, hat: int) -> int:
        return handle.hats.get(hat, 0)


def test_sdl3_mapped_gamepad_reads_buttons_and_dpad() -> None:
    sdl = FakeSDL3()
    inputs = SDL3ControllerInput(sdl_module=sdl)

    assert inputs.available
    assert inputs.connected_names() == ["Fake SDL3 gamepad"]
    assert inputs.poll()[1] == {"button1", "right"}


def test_sdl3_hotplug_reopens_handles_and_clears_disconnected_input() -> None:
    sdl = FakeSDL3()
    sdl.gamepads[10].buttons.add(sdl.SDL_GAMEPAD_BUTTON_SOUTH)
    inputs = SDL3ControllerInput(sdl_module=sdl)
    assert inputs.poll()[1] == {"button1", "right"}

    sdl.gamepad_ids = []
    sdl.joystick_ids = []
    assert inputs.poll() == {1: set(), 2: set()}
    assert sdl.closed_gamepads

    sdl.gamepad_ids = [22]
    sdl.joystick_ids = [22]
    sdl.gamepads[22] = SimpleNamespace(
        name=b"Reconnected gamepad",
        buttons={sdl.SDL_GAMEPAD_BUTTON_EAST},
        axes={0: 0, 1: 0},
        joystick=SimpleNamespace(
            buttons=set(), axes={}, hats={}, num_buttons=8, num_axes=2, num_hats=1
        ),
    )
    assert inputs.poll()[1] == {"button2"}
    assert inputs.connected_names() == ["Reconnected gamepad"]


def test_sdl3_unmapped_joystick_uses_raw_buttons_axes_and_hat() -> None:
    sdl = FakeSDL3()
    sdl.gamepad_ids = []
    sdl.joystick_ids = [31]
    raw = SimpleNamespace(
        name=b"Generic USB controller",
        buttons={3},
        axes={0: -30000, 1: 0},
        hats={0: 0x02},
        num_buttons=8,
        num_axes=2,
        num_hats=1,
    )
    sdl.joysticks[31] = raw
    config = ControllerInputConfig()
    config.players[1].gamepad_buttons = {"button1": "raw_button_3"}

    inputs = SDL3ControllerInput(config, sdl_module=sdl)

    assert inputs.connected_names() == ["Generic USB controller"]
    assert inputs.poll()[1] == {"button1", "left", "right"}


def test_sdl3_missing_dependency_is_a_safe_noop() -> None:
    inputs = SDL3ControllerInput(sdl_module=None)
    # The installed test environment may or may not contain PySDL3; this verifies
    # the public contract only when construction failed to load the binding.
    if not inputs.available:
        assert inputs.poll() == {1: set(), 2: set()}
        assert inputs.connected_names() == []
