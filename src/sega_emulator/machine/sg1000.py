"""Integração inicial da máquina Sega SG-1000 sem subsistema de áudio."""

from __future__ import annotations

import weakref

from sega_emulator.audio.sn76489 import SN76489
from sega_emulator.audio.synchronization import AudioSynchronizer
from sega_emulator.bus.memory import SG1000Ram
from sega_emulator.bus.memory_bus import SG1000MemoryBus
from sega_emulator.cpu.z80 import Z80Cpu
from sega_emulator.machine.cartridge_profiles import CartridgeProfile, cartridge_profile
from sega_emulator.video.tms9918 import TMS9918A


class _CartridgeMirrorBus:
    """Adapta o mapeamento linear atual para espelhamento comum de ROMs menores."""

    def __init__(
        self, bus: SG1000MemoryBus, cartridge_rom: bytes, profile: CartridgeProfile
    ) -> None:
        self._bus = bus
        # Expand the ROM window once so CPU fetches avoid per-read modulo work.
        rom_size = len(cartridge_rom)
        self._rom_window = (
            cartridge_rom * (0xC000 // rom_size) + cartridge_rom[: 0xC000 % rom_size]
        )
        # Some cartridges carry their own RAM at 8000h–BFFFh; it is distinct
        # from the SG-1000's internal RAM at C000h–FFFFh.
        self._cartridge_ram = bytearray(profile.cartridge_ram_size)
        self._cartridge_ram_start = profile.cartridge_ram_start
        self._cartridge_ram_end = profile.cartridge_ram_end
        self._cartridge_ram_mask = profile.cartridge_ram_size - 1

    def read(self, address: int) -> int:
        if (
            self._cartridge_ram
            and self._cartridge_ram_start <= address <= self._cartridge_ram_end
        ):
            offset = (address - self._cartridge_ram_start) & self._cartridge_ram_mask
            return self._cartridge_ram[offset]
        if type(address) is int and 0 <= address < 0xC000:
            return self._rom_window[address]
        return self._bus.read(address)

    def _read_for_cpu(self, address: int) -> int:
        """Read a CPU address already constrained to the 16-bit space."""
        if (
            self._cartridge_ram
            and self._cartridge_ram_start <= address <= self._cartridge_ram_end
        ):
            offset = (address - self._cartridge_ram_start) & self._cartridge_ram_mask
            return self._cartridge_ram[offset]
        if address < 0xC000:
            return self._rom_window[address]
        return self._bus.read(address)

    def write(self, address: int, value: int) -> None:
        if (
            self._cartridge_ram
            and self._cartridge_ram_start <= address <= self._cartridge_ram_end
        ):
            if type(value) is not int:
                raise TypeError("O valor deve ser um inteiro.")
            if not 0 <= value <= 0xFF:
                raise ValueError("O valor deve estar entre 00h e FFh.")
            offset = (address - self._cartridge_ram_start) & self._cartridge_ram_mask
            self._cartridge_ram[offset] = value
            return
        if 0 <= address < 0xC000:
            return
        self._bus.write(address, value)


class SG1000Machine:
    """Conecta CPU Z80, ROM/RAM, portas do VDP e entradas de joystick."""

    def __init__(self, cartridge_rom: bytes, *, cpu_core: str = "python") -> None:
        if cpu_core not in ("python", "rust"):
            raise ValueError("cpu_core deve ser 'python' ou 'rust'.")
        if not isinstance(cartridge_rom, bytes):
            raise TypeError("cartridge_rom deve ser bytes.")
        if not 1 <= len(cartridge_rom) <= 0xC000:
            raise ValueError("A ROM deve conter entre 1 byte e 48 KiB.")
        self.cpu_core = cpu_core
        self.cartridge_rom = cartridge_rom
        profile = cartridge_profile(cartridge_rom)
        self.ram_size = profile.internal_ram_size
        self.cartridge_ram_size = profile.cartridge_ram_size
        ram = SG1000Ram(bytes(profile.internal_ram_size))
        base_bus = SG1000MemoryBus(ram, cartridge_rom)
        self.vdp = TMS9918A()
        self.psg = SN76489()
        self.audio_sync = AudioSynchronizer(self.psg)
        self.joystick1 = 0xFF
        self.joystick2 = 0xFF
        self.total_instructions = 0
        self.total_tstates = 0
        if cpu_core == "python":
            self.memory_bus = _CartridgeMirrorBus(base_bus, cartridge_rom, profile)
            self.cpu = Z80Cpu(self.memory_bus, self)
        else:
            try:
                from sega_z80_core import NativeMemoryBus, RustZ80Cpu
            except ImportError as exc:
                raise RuntimeError(
                    "O núcleo Z80 Rust experimental não está instalado. "
                    "Instale com: python -m pip install maturin e "
                    "maturin develop --manifest-path native/z80_core/Cargo.toml"
                ) from exc
            self.memory_bus = NativeMemoryBus(
                cartridge_rom,
                profile.internal_ram_size,
                profile.cartridge_ram_size,
                profile.cartridge_ram_start,
                profile.cartridge_ram_end,
            )
            self.cpu = RustZ80Cpu.with_native_memory(
                self.memory_bus,
                weakref.proxy(self),
            )

    def read_port(self, address: int) -> int:
        port = address & 0xFF
        vdp_port = port & 0xC1
        if vdp_port in (0x80, 0x81):
            return self.vdp.read_port(vdp_port)
        if port in (0x00, 0xDC):
            return self.joystick1
        if port in (0x01, 0xDD):
            return self.joystick2
        return 0xFF

    def write_port(self, address: int, value: int) -> None:
        port = address & 0xFF
        vdp_port = port & 0xC1
        if vdp_port in (0x80, 0x81):
            self.vdp.write_port(vdp_port, value)
            return
        if 0x40 <= port <= 0x7F:
            # Chamadas externas são efetivas no T-state atual da máquina.
            self.audio_sync.schedule_write(self.total_tstates, value)
            self.audio_sync.advance_to(self.total_tstates)

    def write_port_at(self, address: int, value: int, tstate: int) -> None:
        """Agenda uma escrita no PSG na borda temporal informada pela CPU."""
        port = address & 0xFF
        if 0x40 <= port <= 0x7F:
            self.audio_sync.schedule_write(tstate, value)
            return
        # O VDP permanece no contrato legado até receber sua própria fila
        # de eventos temporizados.
        self.write_port(address, value)

    def set_joystick(self, player: int, value: int) -> None:
        if player not in (1, 2):
            raise ValueError("player deve ser 1 ou 2.")
        if type(value) is not int or not 0 <= value <= 0xFF:
            raise ValueError("value deve estar entre 00h e FFh.")
        if player == 1:
            self.joystick1 = value
        else:
            self.joystick2 = value

    def step(self) -> int:
        """Executa uma instrução e avança o VDP pelos T-states nominais retornados."""
        cycles = self.cpu.step()
        self.total_instructions += 1
        self.total_tstates += cycles
        self.vdp.advance(cycles)
        self.audio_sync.advance_to(self.total_tstates)
        if self.vdp.status & 0x80 and self.vdp.registers[1] & 0x20:
            self.cpu.request_int()
        else:
            self.cpu.clear_int_request()
        return cycles

    def drain_audio_samples(self, max_samples: int | None = None) -> list[float]:
        """Retira amostras mono produzidas pelo PSG desde a última leitura."""
        return self.psg.drain_samples(max_samples)

    def run_frame(self, max_steps: int = 100_000) -> int:
        """Executa um quadro com o laço CPU/VDP local para reduzir chamadas."""
        if type(max_steps) is not int or max_steps < 1:
            raise ValueError("max_steps deve ser um inteiro positivo.")
        start_frame = self.vdp.frame_number
        steps = 0
        cpu = self.cpu
        vdp = self.vdp
        cpu_step = cpu.step
        execute_cpu_instruction = getattr(cpu, "_step_impl", None)
        advance_vdp = vdp.advance
        advance_audio = self.audio_sync.advance_to
        request_int = cpu.request_int
        clear_int = cpu.clear_int_request
        fast_cpu_loop = (
            callable(execute_cpu_instruction)
            and getattr(cpu_step, "__func__", None) is Z80Cpu.step
            and "step" not in self.__dict__
            and type(self).step is SG1000Machine.step
        )
        if fast_cpu_loop:
            # This loop is the machine's normal execution path. Keep Q-latch
            # setup here and call the decoder directly to avoid a Python
            # cpu.step() wrapper and try/finally for every emulated opcode.
            cpu._step_depth = 1
            try:
                while vdp.frame_number == start_frame:
                    cpu._q_previous = cpu.q
                    cpu.q = 0
                    cycles = execute_cpu_instruction()
                    self.total_instructions += 1
                    self.total_tstates += cycles
                    advance_vdp(cycles)
                    advance_audio(self.total_tstates)
                    if vdp.status & 0x80 and vdp.registers[1] & 0x20:
                        request_int()
                    else:
                        clear_int()

                    steps += 1
                    if vdp.frame_number != start_frame:
                        break
                    if steps >= max_steps:
                        raise RuntimeError(
                            "Limite de instruções excedido antes do próximo quadro."
                        )
            finally:
                cpu._step_depth = 0
        else:
            while vdp.frame_number == start_frame:
                cycles = self.step()
                steps += 1
                if vdp.frame_number != start_frame:
                    break
                if steps >= max_steps:
                    raise RuntimeError(
                        "Limite de instruções excedido antes do próximo quadro."
                    )
        return steps

    def run_frames(self, count: int) -> int:
        if type(count) is not int or count < 0:
            raise ValueError("count deve ser um inteiro não negativo.")
        total_steps = 0
        for _ in range(count):
            total_steps += self.run_frame()
        return total_steps
