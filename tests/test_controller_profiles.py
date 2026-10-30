"""Tests for the active Qt controller-profile model."""

from __future__ import annotations

import pytest

from sega_emulator.frontend.controller_profiles import (
    ACTION_BITS,
    ControllerInputConfig,
    PlayerInputProfile,
    apply_turbo,
    joystick_value,
)


def test_default_profiles_round_trip_through_json() -> None:
    config = ControllerInputConfig()
    decoded = ControllerInputConfig.from_json(config.to_json())
    assert decoded == config
    assert set(decoded.players) == {1, 2}


def test_invalid_profile_schema_is_rejected() -> None:
    with pytest.raises(ValueError, match="Versão"):
        ControllerInputConfig.from_json('{"schema_version":99,"players":{}}')


def test_player_profile_defaults_and_turbo_are_deterministic() -> None:
    profile = PlayerInputProfile()
    assert profile.turbo_buttons == {"button1": False, "button2": False}
    actions = {"button1", "button2"}
    assert apply_turbo(actions, {"button1": True}, 0) == actions
    assert "button1" not in apply_turbo(actions, {"button1": True}, 2)
    assert "button2" in apply_turbo(actions, {"button1": True}, 2)


def test_joystick_active_low_bits_match_sg1000_input_contract() -> None:
    assert ACTION_BITS["up"] == 0
    assert joystick_value(set()) == 0xFF
    assert joystick_value({"up", "button1"}) == 0xEE
    with pytest.raises(ValueError, match="Ações desconhecidas"):
        joystick_value({"unknown"})



def test_console_pause_is_persisted_but_never_becomes_a_joystick_bit() -> None:
    config = ControllerInputConfig()
    decoded = ControllerInputConfig.from_json(config.to_json())
    assert decoded.players[1].keyboard_bindings["console_pause"] == "Pause"
    assert decoded.players[2].keyboard_bindings["console_pause"] == "ScrollLock"
    assert decoded.players[1].gamepad_buttons["console_pause"] == "back"
    from sega_emulator.frontend.controller_profiles import JOYSTICK_ACTIONS

    assert "console_pause" not in JOYSTICK_ACTIONS
    assert joystick_value({"up", "button1"}) == 0xEE


def test_legacy_profiles_gain_console_pause_default_without_losing_bindings() -> None:
    legacy = PlayerInputProfile.from_dict(
        {
            "gamepad_buttons": {"up": "x"},
            "keyboard_bindings": {"up": "I"},
        }
    )
    assert legacy.gamepad_buttons["up"] == "x"
    assert legacy.gamepad_buttons["console_pause"] == "back"
    assert legacy.keyboard_bindings["up"] == "I"
    assert legacy.keyboard_bindings["console_pause"] == "Pause"
