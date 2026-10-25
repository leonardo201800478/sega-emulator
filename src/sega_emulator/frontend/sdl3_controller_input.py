"""Backend de entrada baseado diretamente na SDL3 via PySDL3.

Usa a API Gamepad quando o SDL conhece o mapeamento e cai para Joystick
bruto quando o dispositivo (por exemplo, um controle genérico) não tem mapping.
"""

from __future__ import annotations

import ctypes
import logging
import re
from typing import Any

from sega_emulator.frontend.controller_input import (
    SUPPORTED_BUTTONS,
    ControllerInputConfig,
)

logger = logging.getLogger(__name__)

_BUTTON_CONSTANTS = {
    "a": "SDL_GAMEPAD_BUTTON_SOUTH",
    "b": "SDL_GAMEPAD_BUTTON_EAST",
    "x": "SDL_GAMEPAD_BUTTON_WEST",
    "y": "SDL_GAMEPAD_BUTTON_NORTH",
    "dpad_up": "SDL_GAMEPAD_BUTTON_DPAD_UP",
    "dpad_down": "SDL_GAMEPAD_BUTTON_DPAD_DOWN",
    "dpad_left": "SDL_GAMEPAD_BUTTON_DPAD_LEFT",
    "dpad_right": "SDL_GAMEPAD_BUTTON_DPAD_RIGHT",
    "leftshoulder": "SDL_GAMEPAD_BUTTON_LEFT_SHOULDER",
    "rightshoulder": "SDL_GAMEPAD_BUTTON_RIGHT_SHOULDER",
    "leftstick": "SDL_GAMEPAD_BUTTON_LEFT_STICK",
    "rightstick": "SDL_GAMEPAD_BUTTON_RIGHT_STICK",
    "back": "SDL_GAMEPAD_BUTTON_BACK",
    "guide": "SDL_GAMEPAD_BUTTON_GUIDE",
    "start": "SDL_GAMEPAD_BUTTON_START",
    "misc1": "SDL_GAMEPAD_BUTTON_MISC1",
    "paddle1": "SDL_GAMEPAD_BUTTON_RIGHT_PADDLE1",
    "paddle2": "SDL_GAMEPAD_BUTTON_LEFT_PADDLE1",
    "paddle3": "SDL_GAMEPAD_BUTTON_RIGHT_PADDLE2",
    "paddle4": "SDL_GAMEPAD_BUTTON_LEFT_PADDLE2",
    "touchpad": "SDL_GAMEPAD_BUTTON_TOUCHPAD",
}
_AXIS_X = "SDL_GAMEPAD_AXIS_LEFTX"
_AXIS_Y = "SDL_GAMEPAD_AXIS_LEFTY"
_HAT_BITS = {"up": 0x01, "right": 0x02, "down": 0x04, "left": 0x08}


def _decode_name(value: Any) -> str:
    if isinstance(value, bytes):
        return value.decode("utf-8", errors="replace")
    return str(value or "")


class SDL3ControllerInput:
    """Leitor de dois controles SDL3 com hotplug e suporte a joystick sem mapping."""

    def __init__(
        self,
        config: ControllerInputConfig | None = None,
        sdl_module: Any | None = None,
    ) -> None:
        self.config = config or ControllerInputConfig()
        self._sdl = sdl_module
        self._initialized = False
        self._gamepads: dict[int, Any] = {}
        self._joysticks: dict[int, Any] = {}
        self._player_ids: dict[int, int] = {}
        self._gamepad_ids: set[int] = set()
        self._joystick_ids: set[int] = set()
        if self._sdl is None:
            try:
                import sdl3
            except ImportError as exc:
                logger.warning(
                    "PySDL3 indisponível; controles SDL3 desativados: %s", exc
                )
                return
            self._sdl = sdl3
        try:
            flags = (
                self._sdl.SDL_INIT_EVENTS
                | self._sdl.SDL_INIT_GAMEPAD
                | self._sdl.SDL_INIT_JOYSTICK
            )
            if not self._sdl.SDL_InitSubSystem(flags):
                raise RuntimeError(self._error())
            self._initialized = True
            self._sync_devices()
        except (AttributeError, OSError, RuntimeError, TypeError) as exc:
            logger.exception("Não foi possível inicializar a entrada SDL3: %s", exc)
            self._initialized = False

    def _error(self) -> str:
        try:
            return _decode_name(self._sdl.SDL_GetError())
        except (AttributeError, TypeError):
            return "erro SDL3 não detalhado"

    def _list_ids(self, function_name: str) -> list[int]:
        count = ctypes.c_int(0)
        pointer = getattr(self._sdl, function_name)(ctypes.byref(count))
        if not pointer:
            return []
        try:
            return [int(pointer[index]) for index in range(count.value)]
        finally:
            self._sdl.SDL_free(pointer)

    @property
    def available(self) -> bool:
        return self._initialized

    def _close_handles(self) -> None:
        for handle in self._gamepads.values():
            try:
                self._sdl.SDL_CloseGamepad(handle)
            except (AttributeError, OSError, RuntimeError):
                pass
        for handle in self._joysticks.values():
            try:
                self._sdl.SDL_CloseJoystick(handle)
            except (AttributeError, OSError, RuntimeError):
                pass
        self._gamepads.clear()
        self._joysticks.clear()
        self._player_ids.clear()

    def _sync_devices(self) -> None:
        if not self._initialized:
            return
        self._sdl.SDL_PumpEvents()
        gamepad_ids = set(self._list_ids("SDL_GetGamepads"))
        joystick_ids = set(self._list_ids("SDL_GetJoysticks"))
        if gamepad_ids != self._gamepad_ids or joystick_ids != self._joystick_ids:
            self._close_handles()
            self._gamepad_ids = gamepad_ids
            self._joystick_ids = joystick_ids
        for instance_id in sorted(gamepad_ids):
            if instance_id not in self._gamepads:
                handle = self._sdl.SDL_OpenGamepad(instance_id)
                if handle:
                    self._gamepads[instance_id] = handle
        # Keep raw joystick handles for devices SDL3 does not classify as gamepads,
        # and for fallback capture when a gamepad mapping is incomplete.
        for instance_id in sorted(joystick_ids - gamepad_ids):
            if instance_id not in self._joysticks:
                handle = self._sdl.SDL_OpenJoystick(instance_id)
                if handle:
                    self._joysticks[instance_id] = handle
        ids = sorted(self._gamepads.keys() | self._joysticks.keys())
        self._player_ids = {
            player: ids[player - 1] for player in (1, 2) if len(ids) >= player
        }

    def connected_names(self) -> list[str]:
        self._sync_devices()
        names: list[str] = []
        for instance_id in sorted(self._gamepads.keys() | self._joysticks.keys()):
            try:
                if instance_id in self._gamepads:
                    name = self._sdl.SDL_GetGamepadName(self._gamepads[instance_id])
                else:
                    name = self._sdl.SDL_GetJoystickName(self._joysticks[instance_id])
                names.append(_decode_name(name) or f"Controle {instance_id}")
            except (AttributeError, OSError, RuntimeError):
                names.append(f"Controle {instance_id}")
        return names

    def _raw_pressed(self, joystick: Any, binding: str) -> bool:
        match = re.fullmatch(r"raw_button_(\d+)", binding)
        if match:
            return bool(self._sdl.SDL_GetJoystickButton(joystick, int(match[1])))
        match = re.fullmatch(r"raw_hat_(\d+)_(up|down|left|right)", binding)
        if match:
            hat = self._sdl.SDL_GetJoystickHat(joystick, int(match[1]))
            return bool(hat & _HAT_BITS[match[2]])
        match = re.fullmatch(r"raw_axis_(\d+)_(negative|positive)", binding)
        if match:
            value = self._sdl.SDL_GetJoystickAxis(joystick, int(match[1]))
            return value <= -24576 if match[2] == "negative" else value >= 24576
        return False

    def read_pressed_button(self, player: int) -> str | None:
        if player not in (1, 2):
            raise ValueError("player deve ser 1 ou 2.")
        self._sync_devices()
        instance_id = self._player_ids.get(player)
        if instance_id is None:
            return None
        joystick = self._joysticks.get(instance_id)
        if joystick is None and instance_id in self._gamepads:
            joystick = self._sdl.SDL_GetGamepadJoystick(self._gamepads[instance_id])
        if joystick is None:
            return None
        for index in range(self._sdl.SDL_GetNumJoystickButtons(joystick)):
            if self._sdl.SDL_GetJoystickButton(joystick, index):
                return f"raw_button_{index}"
        for index in range(self._sdl.SDL_GetNumJoystickHats(joystick)):
            hat = self._sdl.SDL_GetJoystickHat(joystick, index)
            for direction, bit in _HAT_BITS.items():
                if hat & bit:
                    return f"raw_hat_{index}_{direction}"
        for index in range(min(2, self._sdl.SDL_GetNumJoystickAxes(joystick))):
            value = self._sdl.SDL_GetJoystickAxis(joystick, index)
            if value <= -24576:
                return f"raw_axis_{index}_negative"
            if value >= 24576:
                return f"raw_axis_{index}_positive"
        return None

    def poll(self) -> dict[int, set[str]]:
        result = {1: set(), 2: set()}
        if not self._initialized:
            return result
        try:
            self._sync_devices()
            for player, instance_id in self._player_ids.items():
                profile = self.config.players[player]
                actions = result[player]
                gamepad = self._gamepads.get(instance_id)
                joystick = self._joysticks.get(instance_id)
                if gamepad is not None:
                    for action, name in profile.gamepad_buttons.items():
                        if name in SUPPORTED_BUTTONS:
                            constant_name = _BUTTON_CONSTANTS.get(name)
                            constant = (
                                getattr(self._sdl, constant_name, None)
                                if constant_name
                                else None
                            )
                            if constant is not None and self._sdl.SDL_GetGamepadButton(
                                gamepad, constant
                            ):
                                actions.add(action)
                    if profile.use_left_stick:
                        threshold = round(self.config.deadzone * 32767)
                        x = self._sdl.SDL_GetGamepadAxis(
                            gamepad, getattr(self._sdl, _AXIS_X)
                        )
                        y = self._sdl.SDL_GetGamepadAxis(
                            gamepad, getattr(self._sdl, _AXIS_Y)
                        )
                        if x < 0 and x <= -threshold:
                            actions.add("left")
                        elif x > 0 and x >= threshold:
                            actions.add("right")
                        if y < 0 and y <= -threshold:
                            actions.add("up")
                        elif y > 0 and y >= threshold:
                            actions.add("down")
                    # A mapped controller can still expose raw buttons/axes via its
                    # underlying joystick if a particular mapping omits controls.
                    if any(
                        name.startswith("raw_")
                        for name in profile.gamepad_buttons.values()
                    ):
                        joystick = self._sdl.SDL_GetGamepadJoystick(gamepad)
                if joystick is not None:
                    for action, binding in profile.gamepad_buttons.items():
                        if binding.startswith("raw_") and self._raw_pressed(
                            joystick, binding
                        ):
                            actions.add(action)
                    if profile.use_left_stick and gamepad is None:
                        threshold = round(self.config.deadzone * 32767)
                        for axis in range(
                            min(2, self._sdl.SDL_GetNumJoystickAxes(joystick))
                        ):
                            value = self._sdl.SDL_GetJoystickAxis(joystick, axis)
                            if axis == 0:
                                if value < 0 and value <= -threshold:
                                    actions.add("left")
                                elif value > 0 and value >= threshold:
                                    actions.add("right")
                            elif value < 0 and value <= -threshold:
                                actions.add("up")
                            elif value > 0 and value >= threshold:
                                actions.add("down")
                        for hat_index in range(
                            self._sdl.SDL_GetNumJoystickHats(joystick)
                        ):
                            hat = self._sdl.SDL_GetJoystickHat(joystick, hat_index)
                            if hat & _HAT_BITS["up"]:
                                actions.add("up")
                            if hat & _HAT_BITS["down"]:
                                actions.add("down")
                            if hat & _HAT_BITS["left"]:
                                actions.add("left")
                            if hat & _HAT_BITS["right"]:
                                actions.add("right")
        except (AttributeError, OSError, RuntimeError, TypeError, ValueError):
            logger.exception("Erro ao consultar controles SDL3")
            self._close_handles()
            self._gamepad_ids.clear()
            self._joystick_ids.clear()
        return result

    def close(self) -> None:
        if self._sdl is not None:
            self._close_handles()
            if self._initialized:
                flags = (
                    self._sdl.SDL_INIT_EVENTS
                    | self._sdl.SDL_INIT_GAMEPAD
                    | self._sdl.SDL_INIT_JOYSTICK
                )
                try:
                    self._sdl.SDL_QuitSubSystem(flags)
                except AttributeError:
                    logger.debug("PySDL3 não expõe SDL_QuitSubSystem neste binding")
        self._initialized = False
