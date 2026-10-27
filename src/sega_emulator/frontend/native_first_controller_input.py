"""Native-first low-latency gamepad input for the Windows Qt frontend.

XInput is polled directly through its Windows DLL at the frame input boundary.
SDL3 remains the cross-platform/legacy fallback when no XInput device is present.
The emulation core only receives normalized button snapshots; no OS API is called
from CPU instructions or bus callbacks.
"""

from __future__ import annotations

import ctypes
import logging
import sys
import time
from collections.abc import Callable
from typing import Any

from sega_emulator.frontend.controller_input import (
    SUPPORTED_BUTTONS,
    ControllerInputConfig,
)

logger = logging.getLogger(__name__)

ERROR_SUCCESS = 0
XINPUT_MAX_USERS = 4
DISCOVERY_INTERVAL_SECONDS = 0.5

# XINPUT_GAMEPAD button masks from the Windows XInput contract.
_BUTTON_MASKS = {
    "dpad_up": 0x0001,
    "dpad_down": 0x0002,
    "dpad_left": 0x0004,
    "dpad_right": 0x0008,
    "start": 0x0010,
    "back": 0x0020,
    "leftstick": 0x0040,
    "rightstick": 0x0080,
    "leftshoulder": 0x0100,
    "rightshoulder": 0x0200,
    "a": 0x1000,
    "b": 0x2000,
    "x": 0x4000,
    "y": 0x8000,
}
_ACTION_BINDINGS = frozenset(SUPPORTED_BUTTONS)


class XInputGamepad(ctypes.Structure):
    _fields_ = [
        ("wButtons", ctypes.c_ushort),
        ("bLeftTrigger", ctypes.c_ubyte),
        ("bRightTrigger", ctypes.c_ubyte),
        ("sThumbLX", ctypes.c_short),
        ("sThumbLY", ctypes.c_short),
        ("sThumbRX", ctypes.c_short),
        ("sThumbRY", ctypes.c_short),
    ]


class XInputState(ctypes.Structure):
    _fields_ = [
        ("dwPacketNumber", ctypes.c_ulong),
        ("Gamepad", XInputGamepad),
    ]


def _load_xinput() -> Any | None:
    """Load the first available Windows XInput DLL without failing on other OSes."""
    if sys.platform != "win32":
        return None
    loader = getattr(ctypes, "WinDLL", None)
    if loader is None:
        return None
    for name in ("xinput1_4.dll", "xinput1_3.dll", "xinput9_1_0.dll"):
        try:
            library = loader(name)
            function = library.XInputGetState
            function.argtypes = [ctypes.c_uint, ctypes.POINTER(XInputState)]
            function.restype = ctypes.c_uint
            return library
        except AttributeError, OSError:
            continue
    return None


class XInputControllerInput:
    """Poll connected XInput slots directly and normalize them to game actions."""

    def __init__(
        self,
        config: ControllerInputConfig | None = None,
        *,
        library: Any | None = None,
        clock: Callable[[], float] = time.monotonic,
    ) -> None:
        self.config = config or ControllerInputConfig()
        self._library = _load_xinput() if library is None else library
        self._clock = clock
        self._known_slots: set[int] = set()
        self._last_discovery = float("-inf")
        self._disconnect_detected = False
        self._last_states: dict[int, XInputState] = {}
        self._discover(force=True)

    @property
    def available(self) -> bool:
        return self._library is not None

    def _get_state(self, slot: int) -> XInputState | None:
        if self._library is None:
            return None
        state = XInputState()
        try:
            result = int(self._library.XInputGetState(slot, ctypes.byref(state)))
        except AttributeError, OSError, TypeError, ValueError:
            logger.exception("Falha ao consultar XInput no slot %s", slot)
            return None
        return state if result == ERROR_SUCCESS else None

    def _discover(self, *, force: bool = False) -> bool:
        """Refresh empty slots at a low rate; never probe all slots every frame."""
        if not self.available:
            return False
        now = self._clock()
        if not force and now - self._last_discovery < DISCOVERY_INTERVAL_SECONDS:
            return False
        previous = self._known_slots.copy()
        states: dict[int, XInputState] = {}
        for slot in range(XINPUT_MAX_USERS):
            state = self._get_state(slot)
            if state is not None:
                states[slot] = state
        self._known_slots = set(states)
        self._last_states = states
        self._last_discovery = now
        if previous - self._known_slots:
            self._disconnect_detected = True
        return True

    def connected_names(self) -> list[str]:
        self._discover()
        return [f"XInput controller {slot + 1}" for slot in sorted(self._known_slots)]

    def _current_states(self) -> dict[int, XInputState]:
        discovered = self._discover()
        states = self._last_states.copy() if discovered else {}
        if not discovered:
            for slot in sorted(self._known_slots):
                state = self._get_state(slot)
                if state is None:
                    self._known_slots.discard(slot)
                    self._last_states.pop(slot, None)
                    self._disconnect_detected = True
                else:
                    states[slot] = state
                    self._last_states[slot] = state
        return states

    @staticmethod
    def _button_down(state: XInputState, name: str) -> bool:
        mask = _BUTTON_MASKS.get(name)
        return bool(mask is not None and state.Gamepad.wButtons & mask)

    def _actions_for_player(self, player: int, state: XInputState) -> set[str]:
        profile = self.config.players[player]
        buttons = state.Gamepad.wButtons
        actions = {
            action
            for action, binding in profile.gamepad_buttons.items()
            if binding in _ACTION_BINDINGS
            and (mask := _BUTTON_MASKS.get(binding)) is not None
            and buttons & mask
        }
        if profile.use_left_stick:
            threshold = round(self.config.deadzone * 32767)
            x = state.Gamepad.sThumbLX
            y = state.Gamepad.sThumbLY
            if x <= -threshold:
                actions.add("left")
            elif x >= threshold:
                actions.add("right")
            if y >= threshold:
                actions.add("up")
            elif y <= -threshold:
                actions.add("down")
        return actions

    def poll(self) -> dict[int, set[str]]:
        result = {1: set(), 2: set()}
        states = self._current_states()
        # XInput slot order is stable and explicit; only the first two connected
        # XInput slots map to the two SG-1000 players.
        for player, slot in enumerate(sorted(states)[:2], start=1):
            result[player] = self._actions_for_player(player, states[slot])
        return result

    def read_pressed_button(self, player: int) -> str | None:
        if player not in (1, 2):
            raise ValueError("player deve ser 1 ou 2.")
        states = self._current_states()
        slots = sorted(states)
        if len(slots) < player:
            return None
        state = states[slots[player - 1]]
        for name, mask in _BUTTON_MASKS.items():
            if state.Gamepad.wButtons & mask:
                return name
        return None

    def consume_disconnect_event(self) -> bool:
        detected = self._disconnect_detected
        self._disconnect_detected = False
        return detected

    def close(self) -> None:
        self._known_slots.clear()
        self._last_states.clear()


class NativeFirstControllerInput:
    """Use XInput when present; otherwise delegate to the existing SDL3 backend.

    This avoids double-reading XInput devices through both APIs. In mixed
    XInput/non-XInput setups, XInput takes precedence as a group; SDL is used
    when no XInput controller is connected. This limitation is explicit rather
    than assigning the same physical controller twice to separate players.
    """

    def __init__(
        self,
        config: ControllerInputConfig | None = None,
        *,
        native: XInputControllerInput | None = None,
        fallback: Any | None = None,
        backend_preference: str = "auto",
    ) -> None:
        if backend_preference not in {"auto", "xinput", "sdl3"}:
            raise ValueError("backend_preference must be auto, xinput, or sdl3")
        self.backend_preference = backend_preference
        self._config = config or ControllerInputConfig()
        self.native = native or XInputControllerInput(self._config)
        if fallback is None:
            from sega_emulator.frontend.sdl3_controller_input import SDL3ControllerInput

            fallback = SDL3ControllerInput(self._config)
        self.fallback = fallback
        self.fallback.config = self._config
        self.native.config = self._config
        self._active_backend = "xinput" if self.native.connected_names() else "sdl"
        self._disconnect_pending = False

    @property
    def config(self) -> ControllerInputConfig:
        return self._config

    @config.setter
    def config(self, value: ControllerInputConfig) -> None:
        self._config = value
        if hasattr(self, "native"):
            self.native.config = value
        if hasattr(self, "fallback"):
            self.fallback.config = value

    @property
    def available(self) -> bool:
        if self.backend_preference == "xinput":
            return self.native.available
        if self.backend_preference == "sdl3":
            return bool(self.fallback.available)
        return self.native.available or bool(self.fallback.available)

    def set_backend_preference(self, preference: str) -> None:
        """Switch the host input API without changing the normalized core contract."""
        if preference not in {"auto", "xinput", "sdl3"}:
            raise ValueError("preference must be auto, xinput, or sdl3")
        self.backend_preference = preference
        self._active_backend = (
            "xinput" if preference == "xinput" else "sdl" if preference == "sdl3"
            else "xinput" if self.native.connected_names() else "sdl"
        )
        self._disconnect_pending = False

    def _native_names(self) -> list[str]:
        return self.native.connected_names()

    def connected_names(self) -> list[str]:
        if self.backend_preference == "xinput":
            return self._native_names()
        if self.backend_preference == "sdl3":
            return self.fallback.connected_names()
        native_names = self._native_names()
        return native_names if native_names else self.fallback.connected_names()

    def poll(self) -> dict[int, set[str]]:
        previous_backend = self._active_backend
        if self.backend_preference == "xinput":
            self._active_backend = "xinput"
            return self.native.poll()
        if self.backend_preference == "sdl3":
            self._active_backend = "sdl"
            return self.fallback.poll()
        native_names = self._native_names()
        if native_names:
            actions = self.native.poll()
            # A slot can disappear between enumeration and state polling.
            if self.native.connected_names():
                self._active_backend = "xinput"
                return actions
            self._disconnect_pending |= previous_backend == "xinput"
            self._disconnect_pending |= self.native.consume_disconnect_event()
        elif previous_backend == "xinput":
            self._disconnect_pending = True
        self._active_backend = "sdl"
        self.fallback.config = self.config
        return self.fallback.poll()

    def consume_disconnect_event(self) -> bool:
        if self.backend_preference == "xinput":
            return self.native.consume_disconnect_event()
        if self.backend_preference == "sdl3":
            return self.fallback.consume_disconnect_event()
        if self._disconnect_pending:
            self._disconnect_pending = False
            # Clear the underlying native edge too, so it is not reported twice.
            self.native.consume_disconnect_event()
            return True
        if self._active_backend == "xinput":
            return self.native.consume_disconnect_event()
        return self.fallback.consume_disconnect_event()

    def read_pressed_button(self, player: int) -> str | None:
        if self.backend_preference == "xinput":
            return self.native.read_pressed_button(player)
        if self.backend_preference == "sdl3":
            return self.fallback.read_pressed_button(player)
        if self._native_names():
            return self.native.read_pressed_button(player)
        return self.fallback.read_pressed_button(player)

    def close(self) -> None:
        self.native.close()
        self.fallback.close()
