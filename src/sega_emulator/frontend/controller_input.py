"""Configuração e leitura de controles SDL para os joysticks do SG-1000.

Este módulo não depende de Qt e pode ser testado sem hardware físico.
"""

from __future__ import annotations

import json
from dataclasses import dataclass, field
from typing import Any

ACTIONS = ("up", "down", "left", "right", "button1", "button2", "hold")
JOYSTICK_ACTIONS = frozenset(ACTIONS) - {"hold"}
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
    "misc1",
    "paddle1",
    "paddle2",
    "paddle3",
    "paddle4",
    "touchpad",
)
DEFAULT_GAMEPAD_BINDINGS = {
    "up": "dpad_up",
    "down": "dpad_down",
    "left": "dpad_left",
    "right": "dpad_right",
    "button1": "a",
    "button2": "b",
    "hold": "start",
}
DEFAULT_KEYBOARD_BINDINGS_P1 = {
    "up": "Up",
    "down": "Down",
    "left": "Left",
    "right": "Right",
    "button1": "Z",
    "button2": "X",
    "hold": "P",
}
DEFAULT_KEYBOARD_BINDINGS_P2 = {
    "up": "W",
    "down": "S",
    "left": "A",
    "right": "D",
    "button1": "F",
    "button2": "G",
    "hold": "H",
}
DEFAULT_DEADZONE = 0.15
TURBO_ACTIONS = frozenset({"button1", "button2"})


@dataclass
class PlayerInputProfile:
    """Mapeamento lógico independente do dispositivo físico."""

    gamepad_buttons: dict[str, str] = field(
        default_factory=lambda: DEFAULT_GAMEPAD_BINDINGS.copy()
    )
    use_left_stick: bool = True
    keyboard_bindings: dict[str, str] = field(
        default_factory=lambda: DEFAULT_KEYBOARD_BINDINGS_P1.copy()
    )
    turbo_buttons: dict[str, bool] = field(
        default_factory=lambda: {"button1": False, "button2": False}
    )

    def __post_init__(self) -> None:
        unknown_actions = set(self.gamepad_buttons) - set(ACTIONS)
        if unknown_actions:
            raise ValueError(f"Ações desconhecidas: {sorted(unknown_actions)}")
        unknown_buttons = set(self.gamepad_buttons.values()) - set(SUPPORTED_BUTTONS)
        if unknown_buttons:
            raise ValueError(f"Botões SDL desconhecidos: {sorted(unknown_buttons)}")
        if type(self.use_left_stick) is not bool:
            raise ValueError("use_left_stick deve ser bool.")
        unknown_keyboard_actions = set(self.keyboard_bindings) - set(ACTIONS)
        if unknown_keyboard_actions:
            raise ValueError(
                f"Ações de teclado desconhecidas: {sorted(unknown_keyboard_actions)}"
            )
        if not all(
            isinstance(key, str) and key.strip()
            for key in self.keyboard_bindings.values()
        ):
            raise ValueError("Cada tecla deve ser um nome não vazio.")
        normalized_keys = [key.strip().casefold() for key in self.keyboard_bindings.values()]
        if len(normalized_keys) != len(set(normalized_keys)):
            raise ValueError("Uma tecla não pode estar atribuída a duas ações do mesmo jogador.")
        if set(self.turbo_buttons) - TURBO_ACTIONS:
            raise ValueError("Turbo só pode ser configurado para button1 e button2.")
        if any(type(enabled) is not bool for enabled in self.turbo_buttons.values()):
            raise ValueError("As opções de turbo devem ser booleanas.")

    def to_dict(self) -> dict[str, Any]:
        return {
            "gamepad_buttons": self.gamepad_buttons.copy(),
            "use_left_stick": self.use_left_stick,
            "keyboard_bindings": self.keyboard_bindings.copy(),
            "turbo_buttons": self.turbo_buttons.copy(),
        }

    @classmethod
    def from_dict(
        cls,
        value: dict[str, Any],
        default_keyboard_bindings: dict[str, str] | None = None,
    ) -> PlayerInputProfile:
        if not isinstance(value, dict):
            raise ValueError("O perfil do jogador deve ser um objeto.")
        bindings = value.get("gamepad_buttons", DEFAULT_GAMEPAD_BINDINGS)
        if not isinstance(bindings, dict) or not all(
            isinstance(key, str) and isinstance(button, str)
            for key, button in bindings.items()
        ):
            raise ValueError("gamepad_buttons deve mapear strings para strings.")
        # Merge defaults so older profiles retain custom mappings and gain new actions.
        merged_bindings = DEFAULT_GAMEPAD_BINDINGS.copy()
        merged_bindings.update(bindings)
        keyboard_payload = value.get("keyboard_bindings", {})
        if not isinstance(keyboard_payload, dict) or not all(
            isinstance(key, str) and isinstance(button, str)
            for key, button in keyboard_payload.items()
        ):
            raise ValueError("keyboard_bindings deve mapear ações para nomes de teclas.")
        keyboard_bindings = (
            default_keyboard_bindings or DEFAULT_KEYBOARD_BINDINGS_P1
        ).copy()
        keyboard_bindings.update(keyboard_payload)
        turbo_payload = value.get("turbo_buttons", {})
        if not isinstance(turbo_payload, dict) or not all(
            isinstance(key, str) and type(enabled) is bool
            for key, enabled in turbo_payload.items()
        ):
            raise ValueError("turbo_buttons deve mapear ações para booleanos.")
        turbo_buttons = {"button1": False, "button2": False}
        turbo_buttons.update(turbo_payload)
        return cls(
            gamepad_buttons=merged_bindings,
            use_left_stick=value.get("use_left_stick", True),
            keyboard_bindings=keyboard_bindings,
            turbo_buttons=turbo_buttons,
        )


@dataclass
class ControllerInputConfig:
    """Perfis independentes dos dois jogadores e zona morta analógica."""

    players: dict[int, PlayerInputProfile] = field(
        default_factory=lambda: {
            1: PlayerInputProfile(keyboard_bindings=DEFAULT_KEYBOARD_BINDINGS_P1.copy()),
            2: PlayerInputProfile(keyboard_bindings=DEFAULT_KEYBOARD_BINDINGS_P2.copy()),
        }
    )
    deadzone: float = DEFAULT_DEADZONE

    def __post_init__(self) -> None:
        if set(self.players) != {1, 2}:
            raise ValueError("A configuração deve conter os jogadores 1 e 2.")
        if not 0.0 <= self.deadzone < 1.0:
            raise ValueError("deadzone deve estar no intervalo [0, 1).")
        if any(
            not isinstance(profile, PlayerInputProfile)
            for profile in self.players.values()
        ):
            raise ValueError("Cada jogador deve ter um PlayerInputProfile.")

    def to_json(self) -> str:
        payload = {
            "schema_version": 1,
            "deadzone": self.deadzone,
            "players": {
                str(player): profile.to_dict()
                for player, profile in self.players.items()
            },
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
                int(player): PlayerInputProfile.from_dict(
                    profile,
                    DEFAULT_KEYBOARD_BINDINGS_P1
                    if player == "1"
                    else DEFAULT_KEYBOARD_BINDINGS_P2,
                )
                for player, profile in players_payload.items()
            }
            deadzone = payload.get("deadzone", DEFAULT_DEADZONE)
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
        self._pygame_module = pygame_module
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
            except ImportError, AttributeError, RuntimeError:
                # Pygame audio/video may be usable even if SDL GameController is not.
                self._controller_module = None

    @property
    def available(self) -> bool:
        return self._controller_module is not None

    def connected_names(self) -> list[str]:
        """Retorna nomes sem depender de APIs específicas de uma versão do SDL."""
        self._sync_controllers()
        if self._controller_module is None:
            return []
        names: list[str] = []
        name_for_index = getattr(self._controller_module, "name_forindex", None)
        joystick_module = getattr(self._pygame_module, "joystick", None)
        for index, controller in self._controllers.items():
            name = None
            if callable(name_for_index):
                try:
                    name = name_for_index(index)
                except (IndexError, RuntimeError, TypeError):
                    name = None
            if not name:
                controller_name = getattr(controller, "name", None)
                if callable(controller_name):
                    try:
                        controller_name = controller_name()
                    except (RuntimeError, TypeError):
                        controller_name = None
                name = controller_name
            if not name and joystick_module is not None:
                try:
                    if not joystick_module.get_init():
                        joystick_module.init()
                    joystick = joystick_module.Joystick(index)
                    name = joystick.get_name()
                    joystick.quit()
                except (AttributeError, IndexError, RuntimeError, TypeError):
                    name = None
            names.append(str(name or f"Controle {index + 1}"))
        return names

    def _sync_controllers(self) -> None:
        module = self._controller_module
        if module is None:
            return
        count = module.get_count()
        valid_indices = {index for index in range(count) if module.is_controller(index)}
        for index in list(self._controllers):
            controller = self._controllers[index]
            try:
                attached = controller.attached()
            except AttributeError, RuntimeError:
                attached = False
            if index not in valid_indices or not attached:
                try:
                    controller.quit()
                except AttributeError, RuntimeError:
                    pass
                del self._controllers[index]
        for index in sorted(valid_indices):
            if index not in self._controllers:
                try:
                    self._controllers[index] = module.Controller(index)
                except IndexError, RuntimeError:
                    continue
        available_indices = sorted(self._controllers)
        self._player_device_indices = {
            player: available_indices[player - 1]
            for player in (1, 2)
            if len(available_indices) >= player
        }

    def poll(self) -> dict[int, set[str]]:
        event_module = getattr(self._pygame_module, "event", None)
        if event_module is not None:
            try:
                event_module.pump()
            except Exception:
                # Input must remain optional if a backend/device fails mid-session.
                pass
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
                    if x_value < 0 and x_value <= -threshold:
                        actions.add("left")
                    elif x_value > 0 and x_value >= threshold:
                        actions.add("right")
                    if y_value < 0 and y_value <= -threshold:
                        actions.add("up")
                    elif y_value > 0 and y_value >= threshold:
                        actions.add("down")
            except IndexError, RuntimeError:
                # A device can disappear while being polled. The next poll resyncs.
                continue
        return result

    def close(self) -> None:
        for controller in self._controllers.values():
            try:
                controller.quit()
            except AttributeError, RuntimeError:
                pass
        self._controllers.clear()
        self._player_device_indices.clear()
