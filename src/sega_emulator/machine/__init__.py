"""Composições de hardware para sistemas Sega."""

from sega_emulator.machine.sg1000 import (
    SG1000MemorySystem,
    create_memory_system,
)

__all__ = ["SG1000MemorySystem", "create_memory_system"]
