"""Configuração e leitura de controles SDL para os joysticks do SG-1000.

Este módulo não depende de Qt e pode ser testado sem hardware físico.
"""

from __future__ import annotations

import json
import re
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
_RAW_BINDING_PATTERN = re.compile(
    r"raw_(?:button_\d+|hat_\d+_(?:up|down|left|right)|axis_\d+_(?:negative|positive))"
)


def _is_supported_binding(value: str) -> bool:
    return value in SUPPORTED_BUTTONS or bool(_RAW_BINDING_PATTERN.fullmatch(value))


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
        unknown_buttons = {
            button
            for button in self.gamepad_buttons.values()
            if not _is_supported_binding(button)
        }
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
        normalized_keys = [
            key.strip().casefold() for key in self.keyboard_bindings.values()
        ]
        if len(normalized_keys) != len(set(normalized_keys)):
            raise ValueError(
                "Uma tecla não pode estar atribuída a duas ações do mesmo jogador."
            )
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
            raise ValueError(
                "keyboard_bindings deve mapear ações para nomes de teclas."
            )
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
            1: PlayerInputProfile(
                keyboard_bindings=DEFAULT_KEYBOARD_BINDINGS_P1.copy()
            ),
            2: PlayerInputProfile(
                keyboard_bindings=DEFAULT_KEYBOARD_BINDINGS_P2.copy()
            ),
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


def apply_turbo(
    actions: set[str],
    turbo_buttons: dict[str, bool],
    frame_number: int,
) -> set[str]:
    """Aplica turbo de 15 Hz: dois frames ativos e dois inativos por ciclo."""
    result = actions.copy()
    if frame_number % 4 >= 2:
        for action in TURBO_ACTIONS:
            if turbo_buttons.get(action, False):
                result.discard(action)
    return result


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
        self._joystick_module: Any | None = getattr(pygame_module, "joystick", None)
        self._controllers: dict[int, Any] = {}
        self._raw_joysticks: dict[int, Any] = {}
        self._player_device_indices: dict[int, int] = {}
        self._last_device_count: int | None = None
        if pygame_module is not None:
            try:
                if self._joystick_module is None:
                    self._joystick_module = pygame_module.joystick
                if not self._joystick_module.get_init():
                    self._joystick_module.init()
            except AttributeError, RuntimeError:
                self._joystick_module = None
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
        return self._controller_module is not None or self._joystick_module is not None

    def connected_names(self) -> list[str]:
        """Retorna nomes sem depender de APIs específicas de uma versão do SDL."""
        self._sync_controllers()
        if self._controller_module is None and self._joystick_module is None:
            return []
        names: list[str] = []
        name_for_index = getattr(self._controller_module, "name_forindex", None)
        joystick_module = getattr(self._pygame_module, "joystick", None)
        for index, controller in self._controllers.items():
            name = None
            if callable(name_for_index):
                try:
                    name = name_for_index(index)
                except IndexError, RuntimeError, TypeError:
                    name = None
            if not name:
                controller_name = getattr(controller, "name", None)
                if callable(controller_name):
                    try:
                        controller_name = controller_name()
                    except RuntimeError, TypeError:
                        controller_name = None
                name = controller_name
            if not name and joystick_module is not None:
                try:
                    if not joystick_module.get_init():
                        joystick_module.init()
                    joystick = joystick_module.Joystick(index)
                    name = joystick.get_name()
                    joystick.quit()
                except AttributeError, IndexError, RuntimeError, TypeError:
                    name = None
            names.append(str(name or f"Controle {index + 1}"))
        for index, joystick in self._raw_joysticks.items():
            if index in self._controllers:
                continue
            try:
                name = joystick.get_name()
            except AttributeError, IndexError, RuntimeError:
                name = None
            names.append(str(name or f"Joystick {index + 1} (sem mapeamento)"))
        return names

    def _sync_controllers(self) -> None:
        event_module = getattr(self._pygame_module, "event", None)
        if event_module is not None:
            try:
                event_module.pump()
                event_types = [
                    getattr(self._pygame_module, name)
                    for name in (
                        "JOYDEVICEADDED",
                        "JOYDEVICEREMOVED",
                        "CONTROLLERDEVICEADDED",
                        "CONTROLLERDEVICEREMOVED",
                    )
                    if getattr(self._pygame_module, name, None) is not None
                ]
                device_events = event_module.get(event_types) if event_types else []
                if device_events:
                    for device in (
                        *self._controllers.values(),
                        *self._raw_joysticks.values(),
                    ):
                        try:
                            device.quit()
                        except AttributeError, RuntimeError:
                            pass
                    self._controllers.clear()
                    self._raw_joysticks.clear()
                    self._last_device_count = None
            except AttributeError, RuntimeError, TypeError:
                pass
        module = self._controller_module
        joystick_module = self._joystick_module
        if module is None and joystick_module is None:
            return
        count_source = module if module is not None else joystick_module
        count = count_source.get_count()
        if self._last_device_count is not None and count != self._last_device_count:
            # SDL device indices can shift after unplugging a device. Reopen all
            # handles rather than accidentally retaining the old device at index 0.
            for device in (*self._controllers.values(), *self._raw_joysticks.values()):
                try:
                    device.quit()
                except AttributeError, RuntimeError:
                    pass
            self._controllers.clear()
            self._raw_joysticks.clear()
        self._last_device_count = count
        valid_indices = set(range(count))
        controller_indices = {
            index
            for index in valid_indices
            if module is not None and module.is_controller(index)
        }
        for index in list(self._controllers):
            controller = self._controllers[index]
            try:
                attached = controller.attached()
            except AttributeError, RuntimeError:
                attached = False
            if index not in controller_indices or not attached:
                try:
                    controller.quit()
                except AttributeError, RuntimeError:
                    pass
                del self._controllers[index]
        for index in sorted(controller_indices):
            if index not in self._controllers:
                try:
                    self._controllers[index] = module.Controller(index)
                except IndexError, RuntimeError:
                    continue
        for index in list(self._raw_joysticks):
            joystick = self._raw_joysticks[index]
            try:
                attached = joystick.get_init()
            except AttributeError, RuntimeError:
                attached = False
            if (
                index not in valid_indices
                or index in controller_indices
                or not attached
            ):
                try:
                    joystick.quit()
                except AttributeError, RuntimeError:
                    pass
                del self._raw_joysticks[index]
        if joystick_module is not None:
            for index in sorted(valid_indices):
                if index not in self._raw_joysticks:
                    try:
                        self._raw_joysticks[index] = joystick_module.Joystick(index)
                    except IndexError, RuntimeError:
                        continue
        available_indices = sorted(set(self._controllers) | set(self._raw_joysticks))
        self._player_device_indices = {
            player: available_indices[player - 1]
            for player in (1, 2)
            if len(available_indices) >= player
        }

    def read_pressed_button(self, player: int) -> str | None:
        """Retorna o primeiro botão físico pressionado, para captura de mapeamento."""
        if player not in (1, 2):
            raise ValueError("player deve ser 1 ou 2.")
        self._sync_controllers()
        index = self._player_device_indices.get(player)
        controller = self._controllers.get(index) if index is not None else None
        module = self._controller_module
        if controller is None:
            raw = self._raw_joysticks.get(index) if index is not None else None
            return self._read_raw_pressed_input(raw) if raw is not None else None
        if module is None:
            return None
        for button_name in SUPPORTED_BUTTONS:
            constant = getattr(module, f"CONTROLLER_BUTTON_{button_name.upper()}", None)
            if constant is None:
                continue
            try:
                if controller.get_button(constant):
                    return button_name
            except IndexError, RuntimeError:
                return None
        raw = self._raw_joysticks.get(index) if index is not None else None
        return self._read_raw_pressed_input(raw) if raw is not None else None

    @staticmethod
    def _read_raw_pressed_input(joystick: Any) -> str | None:
        """Captura botões, hats e eixos de joysticks sem mapeamento SDL."""
        try:
            for button in range(joystick.get_numbuttons()):
                if joystick.get_button(button):
                    return f"raw_button_{button}"
            for hat in range(joystick.get_numhats()):
                x_value, y_value = joystick.get_hat(hat)
                if y_value > 0:
                    return f"raw_hat_{hat}_up"
                if y_value < 0:
                    return f"raw_hat_{hat}_down"
                if x_value < 0:
                    return f"raw_hat_{hat}_left"
                if x_value > 0:
                    return f"raw_hat_{hat}_right"
            for axis in range(joystick.get_numaxes()):
                value = joystick.get_axis(axis)
                if value <= -0.75:
                    return f"raw_axis_{axis}_negative"
                if value >= 0.75:
                    return f"raw_axis_{axis}_positive"
        except IndexError, RuntimeError:
            return None
        return None

    @staticmethod
    def _raw_binding_pressed(joystick: Any, binding: str) -> bool:
        """Lê uma atribuição genérica de botão, hat ou eixo."""
        try:
            match = re.fullmatch(r"raw_button_(\d+)", binding)
            if match:
                index = int(match.group(1))
                return index < joystick.get_numbuttons() and bool(
                    joystick.get_button(index)
                )
            match = re.fullmatch(r"raw_hat_(\d+)_(up|down|left|right)", binding)
            if match:
                index, direction = int(match.group(1)), match.group(2)
                if index >= joystick.get_numhats():
                    return False
                x_value, y_value = joystick.get_hat(index)
                return {
                    "up": y_value > 0,
                    "down": y_value < 0,
                    "left": x_value < 0,
                    "right": x_value > 0,
                }[direction]
            match = re.fullmatch(r"raw_axis_(\d+)_(negative|positive)", binding)
            if match:
                index, direction = int(match.group(1)), match.group(2)
                if index >= joystick.get_numaxes():
                    return False
                value = joystick.get_axis(index)
                return value <= -0.75 if direction == "negative" else value >= 0.75
        except IndexError, RuntimeError:
            return False
        return False

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
        button_constants = {
            name: getattr(module, f"CONTROLLER_BUTTON_{name.upper()}", None)
            for name in SUPPORTED_BUTTONS
        }
        axis_x = getattr(module, self._AXIS_NAMES["leftx"], None) if module else None
        axis_y = getattr(module, self._AXIS_NAMES["lefty"], None) if module else None
        for player, index in self._player_device_indices.items():
            controller = self._controllers.get(index)
            raw_joystick = self._raw_joysticks.get(index)
            if controller is None and raw_joystick is None:
                continue
            profile = self.config.players[player]
            actions = result[player]
            try:
                for action, button_name in profile.gamepad_buttons.items():
                    if button_name in SUPPORTED_BUTTONS and controller is not None:
                        constant = button_constants.get(button_name)
                        if constant is not None and controller.get_button(constant):
                            actions.add(action)
                    elif raw_joystick is not None and self._raw_binding_pressed(
                        raw_joystick, button_name
                    ):
                        actions.add(action)
                if raw_joystick is not None and controller is None and profile.use_left_stick:
                    threshold = self.config.deadzone
                    for axis_index in range(min(2, raw_joystick.get_numaxes())):
                        value = raw_joystick.get_axis(axis_index)
                        if axis_index == 0:
                            if value <= -threshold:
                                actions.add("left")
                            elif value >= threshold:
                                actions.add("right")
                        else:
                            if value <= -threshold:
                                actions.add("up")
                            elif value >= threshold:
                                actions.add("down")
                    for hat_index in range(raw_joystick.get_numhats()):
                        x_value, y_value = raw_joystick.get_hat(hat_index)
                        if x_value < 0:
                            actions.add("left")
                        elif x_value > 0:
                            actions.add("right")
                        if y_value > 0:
                            actions.add("up")
                        elif y_value < 0:
                            actions.add("down")
                if (
                    controller is not None
                    and profile.use_left_stick
                    and axis_x is not None
                    and axis_y is not None
                ):
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
        for joystick in self._raw_joysticks.values():
            try:
                joystick.quit()
            except AttributeError, RuntimeError:
                pass
        self._controllers.clear()
        self._raw_joysticks.clear()
        self._player_device_indices.clear()
