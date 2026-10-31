"""Thin Python adapter for the native Rust Sega Master System core.

CPU, memory, I/O, VDP, PSG and sample generation remain in Rust. This adapter
only validates inputs and forwards operations across the PyO3 boundary.
"""

from __future__ import annotations

import hashlib
import zipfile
from pathlib import Path
from types import SimpleNamespace

from sega_emulator.rom_loader import LoadedRom


class SMSMachine:
    """Instruction-stepping SMS machine with native PSG and optional BIOS boot."""

    cpu_core = "rust"

    def __init__(
        self, cartridge_rom: bytes, *, region: str = "usa", bios: bytes | None = None
    ) -> None:
        if not isinstance(cartridge_rom, bytes):
            raise TypeError("cartridge_rom deve ser bytes.")
        if not cartridge_rom:
            raise ValueError("A ROM do Master System não pode estar vazia.")
        if bios is not None and (not isinstance(bios, bytes) or len(bios) != 0x2000):
            raise ValueError("A BIOS SMS deve conter exatamente 8192 bytes.")
        if region not in {"usa", "brazil", "europe", "japan"}:
            raise ValueError("region deve ser usa, brazil, europe ou japan.")

        try:
            from sega_z80_core import NativeSMSMachine
        except ImportError as exc:
            raise RuntimeError(
                "A extensão nativa Rust do SMS não está instalada. "
                "Construa native/z80_core com Maturin e "
                "--features python-extension --release."
            ) from exc

        self.cartridge_rom = cartridge_rom
        self.region = region
        self.bios = bios
        self.native_machine = NativeSMSMachine(cartridge_rom, region=region, bios=bios)

    @property
    def cpu(self):
        return SimpleNamespace(pc=int(self.native_machine.pc))

    @property
    def vdp(self):
        return _SMSVdpSnapshot(self.native_machine)

    @property
    def bios_enabled(self) -> bool:
        return bool(self.native_machine.bios_enabled)

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

    def request_nmi(self) -> None:
        """Assert the SMS console Pause switch through the Z80 NMI input."""
        self.native_machine.request_nmi()

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

    def configure_audio_output(
        self,
        backend: str,
        sample_rate: int = 48_000,
        buffer_size: int = 512,
        queue_blocks: int = 4,
        volume: int = 100,
        balance: int = 0,
    ) -> None:
        """Configure the native Rust audio session without exposing PCM buffers."""
        self.native_machine.configure_audio_output(
            backend, sample_rate, buffer_size, queue_blocks, volume, balance
        )

    def close_audio_output(self) -> None:
        """Stop and release the native Rust audio session."""
        self.native_machine.close_audio_output()

    @property
    def audio_output_metrics(self) -> tuple[int, int]:
        """Return native underrun and dropped-block counters."""
        return tuple(self.native_machine.audio_output_metrics)

    def run_frame(self, max_steps: int = 100_000) -> int:
        if type(max_steps) is not int or max_steps < 1:
            raise ValueError("max_steps deve ser um inteiro positivo.")
        return int(self.native_machine.run_frame(max_steps))

    def set_controller_actions(self, player: int, actions: set[str]) -> None:
        if player not in (1, 2):
            raise ValueError("player deve ser 1 ou 2.")
        self.native_machine.set_controller(
            player,
            "up" in actions,
            "down" in actions,
            "left" in actions,
            "right" in actions,
            "button1" in actions,
            "button2" in actions,
        )

    def drain_psg_write_events(self) -> list[tuple[int, int]]:
        # SMS VGM event capture is not wired to the native timestamped bus yet.
        return []


class _SMSVdpSnapshot:
    """Read-only view of native SMS VDP state required by Qt presentation."""

    def __init__(self, native_machine) -> None:
        self._native_machine = native_machine

    @property
    def framebuffer(self) -> bytes:
        return bytes(self._native_machine.framebuffer)

    @property
    def frame_number(self) -> int:
        return int(self._native_machine.frame_number)

    @property
    def scanline(self) -> int:
        return int(self._native_machine.scanline)

    @property
    def status(self) -> int:
        return int(self._native_machine.vdp_status)

    @property
    def visible_height(self) -> int:
        return int(self._native_machine.visible_height)


def load_sms_rom(path: str | Path) -> LoadedRom:
    """Load a local SMS cartridge image without applying the SG-1000 size limit."""
    source = Path(path).expanduser()
    allowed = {".sms", ".rom", ".bin"}
    try:
        if source.suffix.casefold() == ".zip":
            with zipfile.ZipFile(source) as archive:
                candidates = [
                    item
                    for item in archive.infolist()
                    if not item.is_dir()
                    and Path(item.filename).suffix.casefold() in allowed
                ]
                if not candidates:
                    raise ValueError("O ZIP não contém ROM .sms, .rom ou .bin.")
                if len(candidates) != 1:
                    raise ValueError(
                        "O ZIP contém várias ROMs; selecione uma ROM SMS individual."
                    )
                item = candidates[0]
                if item.file_size > 0x100000:
                    raise ValueError("A ROM SMS excede o limite atual de 1 MiB.")
                data = archive.read(item)
                name = Path(item.filename).name
                member_name = item.filename
        else:
            if source.suffix.casefold() not in allowed:
                raise ValueError(
                    "Formato SMS não suportado. Use .sms, .rom, .bin ou ZIP."
                )
            if source.stat().st_size > 0x100000:
                raise ValueError("A ROM SMS excede o limite atual de 1 MiB.")
            data = source.read_bytes()
            name = source.name
            member_name = None
    except (OSError, zipfile.BadZipFile) as exc:
        raise ValueError(f"Não foi possível ler a ROM SMS '{source}': {exc}") from exc

    if not data:
        raise ValueError("A ROM do Master System está vazia.")
    if len(data) > 0x100000:
        raise ValueError("A ROM SMS excede o limite atual de 1 MiB.")
    return LoadedRom(
        data=data,
        display_name=name,
        source_path=source,
        member_name=member_name,
        sha256=hashlib.sha256(data).hexdigest(),
    )
