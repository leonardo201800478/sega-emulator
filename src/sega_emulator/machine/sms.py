"""Thin Python adapter for the native Rust Sega Master System core.

CPU, memory, I/O, VDP, PSG and sample generation remain in Rust. This adapter
only validates inputs and forwards operations across the PyO3 boundary.
"""

from __future__ import annotations


class SMSMachine:
    """Instruction-stepping SMS machine with native PSG audio output."""

    cpu_core = "rust"

    def __init__(self, cartridge_rom: bytes, *, pal: bool = False) -> None:
        if not isinstance(cartridge_rom, bytes):
            raise TypeError("cartridge_rom deve ser bytes.")
        if not cartridge_rom:
            raise ValueError("A ROM do Master System não pode estar vazia.")
        if type(pal) is not bool:
            raise TypeError("pal deve ser bool.")

        try:
            from sega_z80_core import NativeSMSMachine
        except ImportError as exc:
            raise RuntimeError(
                "A extensão nativa Rust do SMS não está instalada. "
                "Construa native/z80_core com Maturin e "
                "--features python-extension --release."
            ) from exc

        self.cartridge_rom = cartridge_rom
        self.native_machine = NativeSMSMachine(cartridge_rom, pal)

    @property
    def total_instructions(self) -> int:
        return int(self.native_machine.total_instructions)

    @property
    def total_tstates(self) -> int:
        return int(self.native_machine.total_tstates)

    @property
    def psg_clock_hz(self) -> int:
        return int(self.native_machine.psg_clock_hz)

    @property
    def psg_sample_rate(self) -> int:
        return int(self.native_machine.psg_sample_rate)

    @property
    def psg_tone_periods(self) -> tuple[int, int, int]:
        return tuple(self.native_machine.psg_tone_periods)

    @property
    def psg_attenuations(self) -> tuple[int, int, int, int]:
        return tuple(self.native_machine.psg_attenuations)

    def step(self) -> int:
        """Execute one Z80 instruction and return its nominal T-state count."""
        return int(self.native_machine.step())

    def read_memory(self, address: int) -> int:
        if type(address) is not int or not 0 <= address <= 0xFFFF:
            raise ValueError("address deve estar entre 0000h e FFFFh.")
        return int(self.native_machine.read_memory(address))

    def read_port(self, address: int) -> int:
        if type(address) is not int or not 0 <= address <= 0xFFFF:
            raise ValueError("address deve estar entre 0000h e FFFFh.")
        return int(self.native_machine.read_port(address))

    def drain_audio_samples(self, max_samples: int | None = None) -> list[float]:
        if max_samples is not None and (
            type(max_samples) is not int or max_samples < 0
        ):
            raise ValueError("max_samples deve ser um inteiro não negativo ou None.")
        return list(self.native_machine.drain_audio_samples(max_samples))

    def drain_audio_pcm(self, resampler: object) -> bytes:
        """Drain native PSG samples and convert them to PCM16 in Rust."""
        native_resampler = getattr(resampler, "_native", None)
        if native_resampler is None:
            raise TypeError("resampler deve encapsular o processador PCM nativo.")
        return bytes(self.native_machine.drain_audio_pcm(native_resampler))

    def discard_audio_samples(self) -> None:
        """Discard pending native PSG samples when output is disabled."""
        self.native_machine.discard_audio_samples()
