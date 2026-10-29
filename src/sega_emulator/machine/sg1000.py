"""Thin application adapter for the native Rust SG-1000 runtime.

All emulated hardware state and execution live in `sega_sg1000_core`.
This module only resolves the cartridge profile and forwards operations across
the PyO3 boundary for the Qt frontend.
"""

from __future__ import annotations

from sega_emulator.machine.cartridge_profiles import cartridge_profile


class SG1000Machine:
    """Thin Python facade over the native Rust SG-1000 machine."""

    cpu_core = "rust"

    def __init__(self, cartridge_rom: bytes) -> None:
        if not isinstance(cartridge_rom, bytes):
            raise TypeError("cartridge_rom deve ser bytes.")
        if not 1 <= len(cartridge_rom) <= 0xC000:
            raise ValueError("A ROM deve conter entre 1 byte e 48 KiB.")

        profile = cartridge_profile(cartridge_rom)
        self.cartridge_rom = cartridge_rom
        self.ram_size = profile.internal_ram_size
        self.cartridge_ram_size = profile.cartridge_ram_size
        try:
            from sega_z80_core import NativeSG1000Machine
        except ImportError as exc:
            raise RuntimeError(
                "A extensão nativa Rust do SG-1000 não está instalada. "
                "Instale Maturin e construa native/z80_core com "
                "--features python-extension --release."
            ) from exc

        self.native_machine = NativeSG1000Machine(
            cartridge_rom,
            profile.internal_ram_size,
            profile.cartridge_ram_size,
            profile.cartridge_ram_start if profile.cartridge_ram_size else 0,
            profile.cartridge_ram_end if profile.cartridge_ram_size else 0,
        )

    @property
    def cpu(self):
        """Snapshot dos registradores da CPU Rust."""
        return self.native_machine.cpu

    @property
    def vdp(self):
        """Snapshot do estado do VDP Rust."""
        return self.native_machine.vdp

    @property
    def psg(self):
        """Snapshot do estado do PSG Rust."""
        return self.native_machine.psg

    @property
    def total_instructions(self) -> int:
        return int(self.native_machine.total_instructions)

    @property
    def total_tstates(self) -> int:
        return int(self.native_machine.total_tstates)

    def request_nmi(self) -> None:
        self.native_machine.request_nmi()

    def set_sprite_limit_enabled(self, enabled: bool) -> None:
        if type(enabled) is not bool:
            raise TypeError("enabled deve ser bool.")
        self.native_machine.set_sprite_limit_enabled(enabled)

    def read_port(self, address: int) -> int:
        return int(self.native_machine.read_port(address))

    def write_port(self, address: int, value: int) -> None:
        self.native_machine.write_port(address, value)

    def write_port_at(self, address: int, value: int, tstate: int) -> None:
        self.native_machine.write_port_at(address, value, tstate)

    def drain_psg_write_events(self) -> list[tuple[int, int]]:
        return [
            (int(tstate), int(value))
            for tstate, value in self.native_machine.drain_psg_write_events()
        ]

    def set_joystick(self, player: int, value: int) -> None:
        if player not in (1, 2):
            raise ValueError("player deve ser 1 ou 2.")
        if type(value) is not int or not 0 <= value <= 0xFF:
            raise ValueError("value deve estar entre 00h e FFh.")
        self.native_machine.set_joystick(player, value)

    def step(self) -> int:
        return int(self.native_machine.step())

    def drain_audio_samples(self, max_samples: int | None = None) -> list[float]:
        return list(self.native_machine.drain_audio_samples(max_samples))

    def drain_audio_pcm(self, resampler) -> bytes:
        """Drain and convert PSG audio inside the Rust runtime without a Python sample list."""
        native_resampler = getattr(resampler, "_native", None)
        if native_resampler is None:
            raise TypeError("resampler deve encapsular o processador PCM nativo")
        return bytes(self.native_machine.drain_audio_pcm(native_resampler))

    def discard_audio_samples(self) -> None:
        """Discard queued PSG samples natively when audio output is disabled."""
        self.native_machine.discard_audio_samples()

    def run_frame(self, max_steps: int = 100_000) -> int:
        if type(max_steps) is not int or max_steps < 1:
            raise ValueError("max_steps deve ser um inteiro positivo.")
        return int(self.native_machine.run_frame(max_steps))

    def run_frames(self, count: int) -> int:
        if type(count) is not int or count < 0:
            raise ValueError("count deve ser um inteiro não negativo.")
        return int(self.native_machine.run_frames(count))
