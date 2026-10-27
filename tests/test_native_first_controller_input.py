from __future__ import annotations

import ctypes

from sega_emulator.frontend.controller_input import ControllerInputConfig
from sega_emulator.frontend.native_first_controller_input import (
    NativeFirstControllerInput,
    XInputControllerInput,
    XInputState,
)


class FakeXInputLibrary:
    def __init__(self, states: dict[int, tuple[int, int, int]]) -> None:
        self.states = states
        self.calls: list[int] = []

    def XInputGetState(self, slot: int, state_pointer: object) -> int:
        self.calls.append(slot)
        sample = self.states.get(slot)
        if sample is None:
            return 1167  # ERROR_DEVICE_NOT_CONNECTED
        buttons, thumb_x, thumb_y = sample
        state = ctypes.cast(state_pointer, ctypes.POINTER(XInputState)).contents
        state.dwPacketNumber = len(self.calls)
        state.Gamepad.wButtons = buttons
        state.Gamepad.sThumbLX = thumb_x
        state.Gamepad.sThumbLY = thumb_y
        return 0


class FakeClock:
    def __init__(self) -> None:
        self.now = 0.0

    def __call__(self) -> float:
        return self.now


class FakeFallback:
    available = True

    def __init__(self) -> None:
        self.config = ControllerInputConfig()
        self.actions = {1: {"button2"}, 2: set()}
        self.names = ["SDL legacy pad"]
        self.disconnect = False
        self.closed = False

    def poll(self) -> dict[int, set[str]]:
        return {player: actions.copy() for player, actions in self.actions.items()}

    def connected_names(self) -> list[str]:
        return self.names.copy()

    def consume_disconnect_event(self) -> bool:
        result = self.disconnect
        self.disconnect = False
        return result

    def read_pressed_button(self, player: int) -> str | None:
        return "b" if self.actions[player] else None

    def close(self) -> None:
        self.closed = True


def test_xinput_maps_buttons_and_left_stick_to_semantic_actions() -> None:
    # XInput A + D-pad right; left stick points up and right.
    library = FakeXInputLibrary({0: (0x1000 | 0x0008, 20000, 20000)})
    backend = XInputControllerInput(library=library, clock=FakeClock())

    assert backend.connected_names() == ["XInput controller 1"]
    assert backend.poll() == {1: {"button1", "right", "up"}, 2: set()}
    assert backend.read_pressed_button(1) == "dpad_right"


def test_xinput_discovery_is_throttled_but_connected_slots_are_polled_each_frame() -> (
    None
):
    clock = FakeClock()
    library = FakeXInputLibrary({0: (0x1000, 0, 0)})
    backend = XInputControllerInput(library=library, clock=clock)

    calls_after_discovery = len(library.calls)
    backend.poll()
    assert len(library.calls) == calls_after_discovery + 1
    assert library.calls[-1] == 0

    library.states[1] = (0x2000, 0, 0)
    backend.poll()
    assert 1 not in backend._known_slots

    clock.now = 0.51
    assert backend.connected_names() == ["XInput controller 1", "XInput controller 2"]


def test_xinput_disconnect_releases_input_and_reports_one_edge() -> None:
    library = FakeXInputLibrary({0: (0x1000, 0, 0)})
    backend = XInputControllerInput(library=library, clock=FakeClock())
    assert backend.poll()[1] == {"button1"}

    library.states.clear()
    assert backend.poll() == {1: set(), 2: set()}
    assert backend.consume_disconnect_event()
    assert not backend.consume_disconnect_event()


def test_native_first_provider_uses_xinput_before_sdl_and_does_not_double_poll() -> (
    None
):
    library = FakeXInputLibrary({0: (0x1000, 0, 0)})
    native = XInputControllerInput(library=library, clock=FakeClock())
    fallback = FakeFallback()
    backend = NativeFirstControllerInput(native=native, fallback=fallback)

    assert backend.connected_names() == ["XInput controller 1"]
    assert backend.poll() == {1: {"button1"}, 2: set()}
    assert fallback.actions[1] == {"button2"}


def test_native_first_provider_uses_sdl_when_no_xinput_device_exists() -> None:
    native = XInputControllerInput(library=FakeXInputLibrary({}), clock=FakeClock())
    fallback = FakeFallback()
    backend = NativeFirstControllerInput(native=native, fallback=fallback)

    assert backend.connected_names() == ["SDL legacy pad"]
    assert backend.poll() == {1: {"button2"}, 2: set()}


def test_native_first_provider_surfaces_xinput_disconnect_before_fallback() -> None:
    library = FakeXInputLibrary({0: (0x1000, 0, 0)})
    native = XInputControllerInput(library=library, clock=FakeClock())
    fallback = FakeFallback()
    backend = NativeFirstControllerInput(native=native, fallback=fallback)
    assert backend.poll()[1] == {"button1"}

    library.states.clear()
    assert backend.poll()[1] == {"button2"}
    assert backend.consume_disconnect_event()
    assert not backend.consume_disconnect_event()
    backend.close()
    assert fallback.closed


def test_explicit_xinput_preference_does_not_fall_back_to_sdl() -> None:
    native = XInputControllerInput(library=FakeXInputLibrary({}), clock=FakeClock())
    fallback = FakeFallback()
    backend = NativeFirstControllerInput(
        native=native, fallback=fallback, backend_preference="xinput"
    )

    assert backend.connected_names() == []
    assert backend.poll() == {1: set(), 2: set()}
    assert backend.available
    assert fallback.actions[1] == {"button2"}


def test_explicit_sdl3_preference_bypasses_xinput() -> None:
    native = XInputControllerInput(
        library=FakeXInputLibrary({0: (0x1000, 0, 0)}), clock=FakeClock()
    )
    fallback = FakeFallback()
    backend = NativeFirstControllerInput(
        native=native, fallback=fallback, backend_preference="sdl3"
    )

    assert backend.connected_names() == ["SDL legacy pad"]
    assert backend.poll() == {1: {"button2"}, 2: set()}


def test_backend_preference_rejects_unknown_api() -> None:
    backend = NativeFirstControllerInput(
        native=XInputControllerInput(library=FakeXInputLibrary({}), clock=FakeClock()),
        fallback=FakeFallback(),
    )
    try:
        backend.set_backend_preference("directinput")
    except ValueError as exc:
        assert "preference" in str(exc)
    else:
        raise AssertionError("unknown input backend should be rejected")
