"""Configuração e leitura de controles SDL para os joysticks do SG-1000.

Este módulo não depende de Qt e pode ser testado sem hardware físico.
"""

from __future__ import annotations

import json
from dataclasses import dataclass, field
from typing import Any

ACTIONS = ("up", "down", "left", "right", "button1", "button2")
ACTION_BITS = {
    "up": 0,
    "down": 1,
    "left": 2,
    "right": 3,
    "button1": 4,
    "button2": 5,
}
SUPPORTED_BUTTONS = (
    "a",
    "b",
    "x",
    "y",
    "dpad_up",
    "dpad_down",
    "dpad_left",
    "dpad_right",
    "leftshoulder",
    "rightshoulder",
    "leftstick",
    "rightstick",
    "back",
    "guide",
    "start",
)
DEFAULT_GAMEPAD_BINDINGS = {
    "up": "dpad_up",
    "down": "dpad_down",
    "left": "dpad_left",
    "right": "dpad_right",
    "button1": "a",
    "button2": "b",
}


@dataclass
class PlayerInputProfile:
    """Mapeamento lógico independente do dispositivo físico."""

    gamepad_buttons: dict[str, str] = field(
        default_factory=lambda: DEFAULT_GAMEPAD_BINDINGS.copy()
    )
    use_left_stick: bool = True

    def __post_init__(self) -> None:
        unknown_actions = set(self.gamepad_buttons) - set(ACTIONS)
        if unknown_actions:
            raise ValueError(f"Ações desconhecidas: {sorted(unknown_actions)}")
        unknown_buttons = set(self.gamepad_buttons.values()) - set(SUPPORTED_BUTTONS)
        if unknown_buttons:
            raise ValueError(f"Botões SDL desconhecidos: {sorted(unknown_buttons)}")
        if type(self.use_left_stick) is not bool:
            raise ValueError("use_left_stick deve ser bool.")

    def to_dict(self) -> dict[str, Any]:
        return {
            "gamepad_buttons": self.gamepad_buttons.copy(),
            "use_left_stick": self.use_left_stick,
        }

    @classmethod
    def from_dict(cls, value: dict[str, Any]) -> PlayerInputProfile:
        if not isinstance(value, dict):
            raise ValueError("O perfil do jogador deve ser um objeto.")
        bindings = value.get("gamepad_buttons", DEFAULT_GAMEPAD_BINDINGS)
        if not isinstance(bindings, dict) or not all(
            isinstance(key, str) and isinstance(button, str)
            for key, button in bindings.items()
        ):
            raise ValueError("gamepad_buttons deve mapear strings para strings.")
        return cls(
            gamepad_buttons=dict(bindings),
            use_left_stick=value.get("use_left_stick", True),
        )


@dataclass
class ControllerInputConfig:
    """Perfis independentes dos dois jogadores e zona morta analógica."""

    players: dict[int, PlayerInputProfile] = field(
        default_factory=lambda: {
            1: PlayerInputProfile(),
            2: PlayerInputProfile(),
        }
    )
    deadzone: float = 0.35

    def __post_init__(self) -> None:
        if set(self.players) != {1, 2}:
            raise ValueError("A configuração deve conter os jogadores 1 e 2.")
        if not 0.0 <= self.deadzone < 1.0:
            raise ValueError("deadzone deve estar no intervalo [0, 1).")
        if any(not isinstance(profile, PlayerInputProfile) for profile in self.players.values()):
            raise ValueError("Cada jogador deve ter um PlayerInputProfile.")

    def to_json(self) -> str:
        payload = {
            "schema_version": 1,
            "deadzone": self.deadzone,
            "players": {str(player): profile.to_dict() for player, profile in self.players.items()},
        }
        return json.dumps(payload, sort_keys=True, separators=(",", ":"))

    @classmethod
    def from_json(cls, serialized: str) -> ControllerInputConfig:
        try:
            payload = json.loads(serialized)
        except (TypeError, json.JSONDecodeError) as exc:
            raise ValueError("Configuração de controles JSON inválida.") from exc
        if not isinstance(payload, dict) or payload.get("schema_version") != 1:
            raise ValueError("Versão de configuração de controles não suportada.")
        players_payload = payload.get("players")
        if not isinstance(players_payload, dict) or set(players_payload) != {"1", "2"}:
            raise ValueError("A configuração deve conter os jogadores 1 e 2.")
        try:
            players = {
                int(player): PlayerInputProfile.from_dict(profile)
                for player, profile in players_payload.items()
            }
            deadzone = payload.get("deadzone", 0.35)
            if isinstance(deadzone, bool) or not isinstance(deadzone, (int, float)):
                raise ValueError("deadzone deve ser numérica.")
            return cls(players=players, deadzone=float(deadzone))
        except (TypeError, ValueError) as exc:
            raise ValueError(f"Configuração de controles inválida: {exc}") from exc


def joystick_value(actions: set[str] | frozenset[str]) -> int:
    """Converte ações pressionadas para os seis bits ativos em nível baixo."""
    unknown = set(actions) - set(ACTION_BITS)
    if unknown:
        raise ValueError(f"Ações desconhecidas: {sorted(unknown)}")
    value = 0xFF
    for action in actions:
        value &= ~(1 << ACTION_BITS[action])
    return value


class ControllerInput:
    """Poller opcional SDL GameController; ausência de suporte/dispositivo é válida."""

    _AXIS_NAMES = {
        "leftx": "CONTROLLER_AXIS_LEFTX",
        "lefty": "CONTROLLER_AXIS_LEFTY",
    }

    def __init__(
        self,
        pygame_module: Any | None,
        config: ControllerInputConfig | None = None,
    ) -> None:
        self.config = config or ControllerInputConfig()
        self._controller_module: Any | None = None
        self._controllers: dict[int, Any] = {}
        self._player_device_indices: dict[int, int] = {}
        if pygame_module is not None:
            try:
                controller_module = getattr(
                    getattr(pygame_module, "_sdl2", None), "controller", None
                )
                if controller_module is None:
                    from pygame._sdl2 import controller as controller_module
                if not controller_module.get_init():
                    controller_module.init()
                self._controller_module = controller_module
            except (ImportError, AttributeError, RuntimeError):
                # Pygame audio/video may be usable even if SDL GameController is not.
                self._controller_module = None

    @property
    def available(self) -> bool:
        return self._controller_module is not None

    def connected_names(self) -> list[str]:
        self._sync_controllers()
        return [
            str(self._controller_module.name_forindex(index) or f"Controle {index + 1}")
            for index in self._controllers
        ] if self._controller_module is not None else []

    def _sync_controllers(self) -> None:
        module = self._controller_module
        if module is None:
            return
        count = module.get_count()
        valid_indices = {
            index for index in range(count) if module.is_controller(index)
        }
        for index in list(self._controllers):
            controller = self._controllers[index]
            try:
                attached = controller.attached()
            except (AttributeError, RuntimeError):
                attached = False
            if index not in valid_indices or not attached:
                try:
                    controller.quit()
                except (AttributeError, RuntimeError):
                    pass
                del self._controllers[index]
        for index in sorted(valid_indices):
            if index not in self._controllers:
                try:
                    self._controllers[index] = module.Controller(index)
                except (IndexError, RuntimeError):
                    continue
        available_indices = sorted(self._controllers)
        self._player_device_indices = {
            player: available_indices[player - 1]
            for player in (1, 2)
            if len(available_indices) >= player
        }

    def poll(self) -> dict[int, set[str]]:
        self._sync_controllers()
        result = {1: set(), 2: set()}
        module = self._controller_module
        if module is None:
            return result

        button_constants = {
            name: getattr(module, f"CONTROLLER_BUTTON_{name.upper()}", None)
            for name in SUPPORTED_BUTTONS
        }
        axis_x = getattr(module, self._AXIS_NAMES["leftx"], None)
        axis_y = getattr(module, self._AXIS_NAMES["lefty"], None)
        for player, index in self._player_device_indices.items():
            controller = self._controllers.get(index)
            if controller is None:
                continue
            profile = self.config.players[player]
            actions = result[player]
            try:
                for action, button_name in profile.gamepad_buttons.items():
                    constant = button_constants.get(button_name)
                    if constant is not None and controller.get_button(constant):
                        actions.add(action)
                if profile.use_left_stick and axis_x is not None and axis_y is not None:
                    threshold = round(self.config.deadzone * 32767)
                    x_value = controller.get_axis(axis_x)
                    y_value = controller.get_axis(axis_y)
                    if x_value <= -threshold:
                        actions.add("left")
                    elif x_value >= threshold:
                        actions.add("right")
                    if y_value <= -threshold:
                        actions.add("up")
                    elif y_value >= threshold:
                        actions.add("down")
            except (IndexError, RuntimeError):
                # A device can disappear while being polled. The next poll resyncs.
                continue
        return result

    def close(self) -> None:
        for controller in self._controllers.values():
            try:
                controller.quit()
            except (AttributeError, RuntimeError):
                pass
        self._controllers.clear()
        self._player_device_indices.clear()
