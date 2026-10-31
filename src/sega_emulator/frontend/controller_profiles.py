"""Configuração e normalização dos perfis de controles usados pelo frontend Qt."""

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
    r"raw_(?:button_(?:[0-9]|[1-8][0-9]|9[0-5])|"
    r"hat_[0-7]_(?:up|down|left|right)|"
    r"axis_(?:[0-9]|1[0-5])_(?:negative|positive))"
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
