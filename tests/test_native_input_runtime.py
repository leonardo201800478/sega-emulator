from __future__ import annotations

import os

import pytest

try:
    from sega_z80_core import NativeInputManager
except ImportError:
    pytest.skip("extensão PyO3 não instalada", allow_module_level=True)


def test_native_input_manager_exposes_stable_frontend_contract() -> None:
    manager = NativeInputManager()
    try:
        manager.set_profile(
            1,
            {
                "up": "dpad_up",
                "down": "dpad_down",
                "left": "dpad_left",
                "right": "dpad_right",
                "button1": "a",
                "button2": "b",
                "hold": "start",
            },
            True,
            0.15,
            False,
            False,
        )
        manager.set_profile(
            2,
            {
                "up": "raw_hat_0_up",
                "down": "raw_hat_0_down",
                "left": "raw_hat_0_left",
                "right": "raw_hat_0_right",
                "button1": "raw_button_0",
                "button2": "raw_button_1",
                "hold": "raw_button_2",
            },
            False,
            0.15,
            False,
            False,
        )
        actions = manager.poll_actions(0)
        assert isinstance(actions, tuple)
        assert len(actions) == 2
        assert all(isinstance(player_mask, int) for player_mask in actions)
        assert isinstance(manager.connected_names(), list)
        captured = manager.read_pressed_button(1)
        assert captured is None or isinstance(captured, str)
        if os.environ.get("SEGA_INPUT_REQUIRE_SDL3") == "1":
            assert manager.available, manager.backend_error
            assert manager.backend_error is None
    finally:
        manager.close()
