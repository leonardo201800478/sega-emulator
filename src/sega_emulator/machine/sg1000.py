"""Integração inicial da máquina Sega SG-1000 sem subsistema de áudio."""

from __future__ import annotations

from sega_emulator.bus.memory import RAM_SIZE, SG1000Ram
from sega_emulator.bus.memory_bus import SG1000MemoryBus
from sega_emulator.cpu.z80 import Z80Cpu
from sega_emulator.video.tms9918 import TMS9918A


class _CartridgeMirrorBus:
    """Adapta o mapeamento linear atual para espelhamento comum de ROMs menores."""

    def __init__(self, bus: SG1000MemoryBus, rom_size: int) -> None:
        self._bus = bus
        self._rom_size = rom_size

    def read(self, address: int) -> int:
        if 0 <= address < 0xC000:
            return self._bus.read(address % self._rom_size)
        return self._bus.read(address)

    def write(self, address: int, value: int) -> None:
        if 0 <= address < 0xC000:
            return
        self._bus.write(address, value)


class SG1000Machine:
    """Conecta CPU Z80, ROM/RAM, portas do VDP e entradas de joystick."""

    def __init__(self, cartridge_rom: bytes) -> None:
        if not isinstance(cartridge_rom, bytes):
            raise TypeError("cartridge_rom deve ser bytes.")
        if not 1 <= len(cartridge_rom) <= 0xC000:
            raise ValueError("A ROM deve conter entre 1 byte e 48 KiB.")
        self.cartridge_rom = cartridge_rom
        ram = SG1000Ram(bytes(RAM_SIZE))
        base_bus = SG1000MemoryBus(ram, cartridge_rom)
        self.memory_bus = _CartridgeMirrorBus(base_bus, len(cartridge_rom))
        self.vdp = TMS9918A()
        self.joystick1 = 0xFF
        self.joystick2 = 0xFF
        self.total_instructions = 0
        self.total_tstates = 0
        self.cpu = Z80Cpu(self.memory_bus, self)

    def read_port(self, address: int) -> int:
        port = address & 0xFF
        if port in (0xBE, 0xBF):
            return self.vdp.read_port(port)
        if port in (0x00, 0xDC):
            return self.joystick1
        if port in (0x01, 0xDD):
            return self.joystick2
        return 0xFF

    def write_port(self, address: int, value: int) -> None:
        port = address & 0xFF
        if port in (0xBE, 0xBF):
            self.vdp.write_port(port, value)

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
        if self.vdp.interrupt_pending:
            self.cpu.request_int()
        else:
            self.cpu.clear_int_request()
        return cycles

    def run_frame(self, max_steps: int = 100_000) -> int:
        """Executa até o próximo limite de quadro; retorna instruções executadas."""
        if type(max_steps) is not int or max_steps < 1:
            raise ValueError("max_steps deve ser um inteiro positivo.")
        start_frame = self.vdp.frame_number
        steps = 0
        while self.vdp.frame_number == start_frame:
            self.step()
            steps += 1
            if self.vdp.frame_number != start_frame:
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
