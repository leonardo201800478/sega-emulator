"""Thin Python adapter for the Scheduler-driven Rust SMS machine.

All CPU, bus, mapper, VDP, PSG, interrupt and timing work stays in Rust.
This module only validates inputs and forwards coarse-grained session calls.
"""

from __future__ import annotations


class TemporalSMSMachine:
    """SMS session adapter backed exclusively by NativeSMSTemporalMachine."""

    cpu_core = "rust"

    def __init__(self, cartridge_rom: bytes, *, region: str = "usa") -> None:
        if not isinstance(cartridge_rom, bytes):
            raise TypeError("cartridge_rom deve ser bytes.")
        if not cartridge_rom:
            raise ValueError("A ROM do Master System não pode estar vazia.")
        if region not in {"usa", "brazil", "europe", "japan"}:
            raise ValueError("region deve ser usa, brazil, europe ou japan.")

        try:
            from sega_z80_core import NativeSMSTemporalMachine
        except ImportError as exc:
            raise RuntimeError(
                "A extensão nativa Rust com SmsTemporalMachine não está instalada. "
                "Construa native/z80_core com Maturin e "
                "--features python-extension --release."
            ) from exc

        self.cartridge_rom = cartridge_rom
        self.region = region
        self._audio_config: dict[str, object] | None = None
        self.native_machine = NativeSMSTemporalMachine(cartridge_rom, region=region)

    @property
    def frame_number(self) -> int:
        """Return the completed VDP frame counter."""
        return int(self.native_machine.frame_number)

    @property
    def total_instructions(self) -> int:
        """Return the number of CPU instructions executed by frame stepping."""
        return int(self.native_machine.total_instructions)

    @property
    def is_running(self) -> bool:
        """Return whether the session is marked as running."""
        return bool(self.native_machine.is_running)

    def load_rom(self, cartridge_rom: bytes) -> None:
        """Replace the current cartridge and rebuild the Rust machine state."""
        if not isinstance(cartridge_rom, bytes):
            raise TypeError("cartridge_rom deve ser bytes.")
        if not cartridge_rom:
            raise ValueError("A ROM do Master System não pode estar vazia.")
        self.native_machine.load_rom(cartridge_rom)
        self.cartridge_rom = cartridge_rom
        self._restore_audio_output()

    def start(self) -> None:
        """Mark the session as active for a future native execution host."""
        self.native_machine.start()

    def pause(self) -> None:
        """Pause the native session without changing hardware state."""
        self.native_machine.pause()

    def stop(self) -> None:
        """Stop frame execution while retaining the loaded cartridge."""
        self.native_machine.stop()

    def reset(self) -> None:
        """Reset the loaded cartridge to the machine's power-on state."""
        self.native_machine.reset()
        self._restore_audio_output()

    def step_frame(self, max_instructions: int = 100_000) -> int:
        """Execute one complete VDP frame through the Rust Scheduler."""
        if type(max_instructions) is not int or max_instructions <= 0:
            raise ValueError("max_instructions deve ser um inteiro positivo.")
        return int(self.native_machine.step_frame(max_instructions))

    def set_controller(
        self,
        player: int,
        *,
        up: bool = False,
        down: bool = False,
        left: bool = False,
        right: bool = False,
        button_1: bool = False,
        button_2: bool = False,
    ) -> None:
        """Update controller levels without advancing emulation time."""
        if type(player) is not int or player not in {1, 2}:
            raise ValueError("player deve ser 1 ou 2.")
        buttons = (up, down, left, right, button_1, button_2)
        if any(type(button) is not bool for button in buttons):
            raise TypeError("Os estados dos botões devem ser bool.")
        self.native_machine.set_controller(player, *buttons)

    def request_nmi(self) -> None:
        """Assert the SMS Pause switch through the Z80 NMI input."""
        self.native_machine.request_nmi()

    def configure_audio_output(
        self,
        backend: str,
        sample_rate: int = 48_000,
        buffer_size: int = 512,
        queue_blocks: int = 4,
        volume: int = 100,
        balance: int = 0,
    ) -> None:
        """Configure native audio output without exposing PCM buffers to Python."""
        self.native_machine.configure_audio_output(
            backend, sample_rate, buffer_size, queue_blocks, volume, balance
        )
        self._audio_config = {
            "backend": backend,
            "sample_rate": sample_rate,
            "buffer_size": buffer_size,
            "queue_blocks": queue_blocks,
            "volume": volume,
            "balance": balance,
        }

    def _restore_audio_output(self) -> None:
        """Reopen the previous native audio session after machine replacement."""
        if self._audio_config is not None:
            self.native_machine.configure_audio_output(**self._audio_config)

    def open_video_output_in_viewport(self, window_handle: int) -> None:
        """Attach the Rust D3D12 host to an existing native GUI widget handle."""
        if type(window_handle) is not int or window_handle <= 0:
            raise ValueError("window_handle deve ser um HWND inteiro positivo.")
        self.native_machine.open_video_output_in_viewport(window_handle)

    def close_video_output(self) -> None:
        """Detach and release the native D3D12 presentation session."""
        self.native_machine.close_video_output()

    def close_audio_output(self) -> None:
        """Stop and release the native audio output session."""
        self.native_machine.close_audio_output()
        self._audio_config = None

    @property
    def audio_output_error(self) -> str | None:
        """Return the last native audio output error, if any."""
        error = self.native_machine.audio_output_error
        return str(error) if error is not None else None

    @property
    def audio_output_metrics(self) -> tuple[int, int]:
        """Return native audio underrun and dropped-block counters."""
        return tuple(self.native_machine.audio_output_metrics)
