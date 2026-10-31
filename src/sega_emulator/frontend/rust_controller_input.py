"""Thin Qt adapter for the Rust-owned physical controller runtime.

This module performs no device discovery, polling, normalization or physical
button mapping. It only transfers UI configuration and semantic actions across
the PyO3 boundary.
"""

from __future__ import annotations

from sega_emulator.frontend.controller_profiles import ControllerInputConfig


class RustControllerInput:
    """Frontend facade over sega_z80_core.NativeInputManager."""

    def __init__(
        self,
        config: ControllerInputConfig,
        *,
        backend_preference: str = "sdl3",
    ) -> None:
        self._config = config
        self._native = None
        self._import_error: str | None = None
        try:
            from sega_z80_core import NativeInputManager
        except ImportError as exc:
            self._import_error = (
                "Extensão Rust de input indisponível; reconstrua "
                "native/z80_core com Maturin."
            )
            self._import_error = f"{self._import_error} ({exc})"
        else:
            self._native = NativeInputManager()
            self.set_backend_preference(backend_preference)
            self._sync_profiles()

    @property
    def config(self) -> ControllerInputConfig:
        return self._config

    @config.setter
    def config(self, value: ControllerInputConfig) -> None:
        if not isinstance(value, ControllerInputConfig):
            raise TypeError("config deve ser ControllerInputConfig.")
        self._config = value
        self._sync_profiles()

    def _sync_profiles(self) -> None:
        if self._native is None:
            return
        for player in (1, 2):
            profile = self._config.players[player]
            self._native.set_profile(
                player,
                profile.gamepad_buttons,
                profile.use_left_stick,
                self._config.deadzone,
                False,
                False,
            )

    @property
    def available(self) -> bool:
        return bool(self._native is not None and self._native.available)

    @property
    def backend_error(self) -> str | None:
        if self._native is not None:
            return self._native.backend_error
        return self._import_error

    def set_backend_preference(self, preference: str) -> None:
        if preference not in {"auto", "sdl3"}:
            raise ValueError("backend_preference deve ser 'auto' ou 'sdl3'.")
        if self._native is not None:
            self._native.set_backend_preference(preference)

    def poll(self, frame_number: int = 0) -> dict[int, set[str]]:
        if self._native is None:
            return {1: set(), 2: set()}
        player1, player2 = self._native.poll_actions(frame_number)
        return {1: set(player1), 2: set(player2)}

    def read_pressed_button(self, player: int) -> str | None:
        if self._native is None:
            return None
        return self._native.read_pressed_button(player)

    def connected_names(self) -> list[str]:
        if self._native is None:
            return []
        return list(self._native.connected_names())

    def consume_disconnect_event(self) -> bool:
        if self._native is None:
            return False
        return bool(self._native.consume_disconnect_event())

    def close(self) -> None:
        if self._native is not None:
            self._native.close()
