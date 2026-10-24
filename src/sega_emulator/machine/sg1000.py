"""Composição inicial dos barramentos e VDP do SG-1000."""

from dataclasses import dataclass

from sega_emulator.bus.io_bus import SG1000IOBus
from sega_emulator.bus.memory import RAM_SIZE, SG1000Ram
from sega_emulator.bus.memory_bus import SG1000MemoryBus
from sega_emulator.bus.vram import TMS9918VRAM
from sega_emulator.video.tms9918 import (
    VDP_CONTROL_PORT,
    VDP_DATA_PORT,
    TMS9918,
)


@dataclass(slots=True)
class SG1000MemorySystem:
    """Componentes de memória e portas conectados para uma máquina SG-1000.

    Esta composição ainda não é uma máquina completa: não conecta PSG,
    controladores, renderizador nem relógio de dispositivos.
    """

    ram: SG1000Ram
    vram: TMS9918VRAM
    memory_bus: SG1000MemoryBus
    io_bus: SG1000IOBus
    vdp: TMS9918


def create_memory_system(
    cartridge_rom: bytes | None = None,
    *,
    ram_initial_data: bytes | None = None,
    vram_initial_data: bytes | None = None,
) -> SG1000MemorySystem:
    """Cria memória determinística e registra as portas do VDP."""
    if ram_initial_data is None:
        ram_initial_data = bytes(RAM_SIZE)
    ram = SG1000Ram(ram_initial_data)
    vram = TMS9918VRAM(vram_initial_data)
    vdp = TMS9918(vram)
    memory_bus = SG1000MemoryBus(ram, cartridge_rom)
    io_bus = SG1000IOBus()
    io_bus.map_device((VDP_DATA_PORT, VDP_CONTROL_PORT), vdp)
    return SG1000MemorySystem(ram, vram, memory_bus, io_bus, vdp)
