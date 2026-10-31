"""Tests for the active Qt controller-profile model."""

from __future__ import annotations

import pytest

from sega_emulator.frontend.controller_profiles import (
    ACTION_BITS,
    ControllerInputConfig,
    PlayerInputProfile,
)


def test_default_profiles_round_trip_through_json() -> None:
    config = ControllerInputConfig()
    decoded = ControllerInputConfig.from_json(config.to_json())
    assert decoded == config
    assert set(decoded.players) == {1, 2}


def test_invalid_profile_schema_is_rejected() -> None:
    with pytest.raises(ValueError, match="Versão"):
        ControllerInputConfig.from_json('{"schema_version":99,"players":{}}')


def test_turbo_preferences_are_configuration_only_and_round_trip() -> None:
    config = ControllerInputConfig()
    config.players[1].turbo_buttons = {"button1": True, "button2": False}
    decoded = ControllerInputConfig.from_json(config.to_json())

    assert decoded.players[1].turbo_buttons == {
        "button1": True,
        "button2": False,
    }


def test_sg1000_port_action_order_is_stable() -> None:
    assert ACTION_BITS == {
        "up": 0,
        "down": 1,
        "left": 2,
        "right": 3,
        "button1": 4,
        "button2": 5,
    }


def test_hold_binding_is_a_dedicated_action_outside_controller_port_bits() -> None:
    config = ControllerInputConfig()
    assert config.players[1].keyboard_bindings["hold"] == "P"
    assert config.players[2].keyboard_bindings["hold"] == "H"
    assert config.players[1].gamepad_buttons["hold"] == "start"
    assert "hold" not in ACTION_BITS
