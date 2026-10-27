"""Integração SG-1000 com backend de referência Python ou runtime nativo Rust."""

from __future__ import annotations

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
        self.joystick1 = 0xFF
        self.joystick2 = 0xFF
        self.total_instructions = 0
        self.total_tstates = 0
        self._psg_write_events: list[tuple[int, int]] = []
        if cpu_core == "python":
            ram = SG1000Ram(bytes(profile.internal_ram_size))
            base_bus = SG1000MemoryBus(ram, cartridge_rom)
            self.vdp = TMS9918A()
            self.psg = SN76489()
            self.audio_sync = AudioSynchronizer(self.psg)
            self.memory_bus = _CartridgeMirrorBus(base_bus, cartridge_rom, profile)
            self.cpu = Z80Cpu(self.memory_bus, self)
        else:
            try:
                from sega_z80_core import NativeSG1000Machine
            except ImportError as exc:
                raise RuntimeError(
                    "O runtime SG-1000 Rust experimental não está instalado. "
                    "Instale com: python -m pip install maturin e "
                    "maturin develop --release --manifest-path "
                    "native/z80_core/Cargo.toml"
                ) from exc
            self.native_machine = NativeSG1000Machine(
                cartridge_rom,
                profile.internal_ram_size,
                profile.cartridge_ram_size,
                profile.cartridge_ram_start,
                profile.cartridge_ram_end,
            )
            self.vdp = self.native_machine.vdp
            self.psg = self.native_machine.psg
            self.audio_sync = self.native_machine.audio_sync
            self.memory_bus = self.native_machine.memory_bus
            self.cpu = self.native_machine.cpu

    def set_sprite_limit_enabled(self, enabled: bool) -> None:
        """Configure the VDP sprite-per-line limit on the selected backend."""
        if type(enabled) is not bool:
            raise TypeError("enabled deve ser bool.")
        if self.cpu_core == "python":
            self.vdp.set_sprite_limit_enabled(enabled)
            return
        setter = getattr(self.native_machine, "set_sprite_limit_enabled", None)
        if callable(setter):
            setter(enabled)
            return
        vdp_setter = getattr(self.vdp, "set_sprite_limit_enabled", None)
        if callable(vdp_setter):
            vdp_setter(enabled)
            return
        raise RuntimeError(
            "O runtime Rust atual ainda não expõe set_sprite_limit_enabled; "
            "a opção está disponível no backend Python, mas não pode ser aplicada "
            "com segurança ao backend Rust nesta versão."
        )

    def read_port(self, address: int) -> int:
        if self.cpu_core == "rust":
            return self.native_machine.read_port(address)
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
        if self.cpu_core == "rust":
            self.native_machine.write_port(address, value)
            return
        port = address & 0xFF
        vdp_port = port & 0xC1
        if vdp_port in (0x80, 0x81):
            self.vdp.write_port(vdp_port, value)
            return
        if 0x40 <= port <= 0x7F:
            # Chamadas externas são efetivas no T-state atual da máquina.
            self._psg_write_events.append((self.total_tstates, value))
            self.audio_sync.schedule_write(self.total_tstates, value)
            self.audio_sync.advance_to(self.total_tstates)

    def write_port_at(self, address: int, value: int, tstate: int) -> None:
        """Agenda uma escrita no PSG na borda temporal informada pela CPU."""
        if self.cpu_core == "rust":
            self.native_machine.write_port_at(address, value, tstate)
            return
        port = address & 0xFF
        if 0x40 <= port <= 0x7F:
            self._psg_write_events.append((tstate, value))
            self.audio_sync.schedule_write(tstate, value)
            return
        # O VDP permanece no contrato legado até receber sua própria fila
        # de eventos temporizados.
        self.write_port(address, value)

    def drain_psg_write_events(self) -> list[tuple[int, int]]:
        """Retira escritas PSG temporizadas desde a última leitura (para VGM)."""
        if self.cpu_core == "rust":
            return [
                (int(tstate), int(value))
                for tstate, value in self.native_machine.drain_psg_write_events()
            ]
        events = self._psg_write_events
        self._psg_write_events = []
        return events

    def set_joystick(self, player: int, value: int) -> None:
        if player not in (1, 2):
            raise ValueError("player deve ser 1 ou 2.")
        if type(value) is not int or not 0 <= value <= 0xFF:
            raise ValueError("value deve estar entre 00h e FFh.")
        if self.cpu_core == "rust":
            self.native_machine.set_joystick(player, value)
        if player == 1:
            self.joystick1 = value
        else:
            self.joystick2 = value

    def step(self) -> int:
        """Executa uma instrução e avança o VDP pelos T-states nominais retornados."""
        if self.cpu_core == "rust":
            cycles = self.native_machine.step()
            self.total_instructions = self.native_machine.total_instructions
            self.total_tstates = self.native_machine.total_tstates
            return cycles
        cycles = self.cpu.step()
        self.total_instructions += 1
        self.total_tstates += cycles
        self.vdp.advance(cycles)
        self.audio_sync.advance_to(self.total_tstates)
        if self.vdp.interrupt_pending:
            self.cpu.request_int()
        else:
            self.cpu.clear_int_request()
        return cycles

    def drain_audio_samples(self, max_samples: int | None = None) -> list[float]:
        """Retira amostras mono produzidas pelo PSG desde a última leitura."""
        return self.psg.drain_samples(max_samples)

    def run_frame(self, max_steps: int = 100_000) -> int:
        """Executa um quadro; o backend Rust coordena CPU e dispositivos nativamente."""
        if type(max_steps) is not int or max_steps < 1:
            raise ValueError("max_steps deve ser um inteiro positivo.")
        if self.cpu_core == "rust":
            steps = self.native_machine.run_frame(max_steps)
            self.total_instructions = self.native_machine.total_instructions
            self.total_tstates = self.native_machine.total_tstates
            return steps
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
