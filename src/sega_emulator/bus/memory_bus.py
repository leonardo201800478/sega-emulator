"""Barramento de memória principal do Sega SG-1000."""

from sega_emulator.bus.memory import (
    ADDRESS_SPACE_SIZE,
    RAM_START,
    SG1000Ram,
)

CARTRIDGE_ADDRESS_SPACE_SIZE = RAM_START


class UnmappedMemoryError(LookupError):
    """A CPU acessou uma região sem dispositivo mapeado."""


class ReadOnlyMemoryError(PermissionError):
    """A CPU tentou escrever em uma região de ROM."""


class SG1000MemoryBus:
    """Integra a ROM do cartucho e a RAM interna no espaço de 16 bits.

    A ROM fornecida é mapeada linearmente a partir de 0000h, sem espelhamento
    ou banking implícitos. Regiões do cartucho além dos bytes fornecidos são
    tratadas como não mapeadas até que o decodificador físico seja modelado.
    """

    def __init__(
        self,
        ram: SG1000Ram,
        cartridge_rom: bytes | None = None,
    ) -> None:
        """Cria o barramento com RAM explícita e cartucho opcional."""
        if not isinstance(ram, SG1000Ram):
            raise TypeError("ram deve ser uma instância de SG1000Ram.")

        if cartridge_rom is not None:
            if not isinstance(cartridge_rom, bytes):
                raise TypeError("cartridge_rom deve ser bytes ou None.")
            if not 1 <= len(cartridge_rom) <= CARTRIDGE_ADDRESS_SPACE_SIZE:
                raise ValueError(
                    "A ROM deve conter entre 1 byte e 48 KiB (0xC000 bytes)."
                )

        self._ram = ram
        self._cartridge_rom = cartridge_rom

    @staticmethod
    def _validate_address(address: int) -> None:
        """Valida um endereço de CPU de 16 bits."""
        if type(address) is not int:
            raise TypeError("O endereço deve ser um inteiro.")
        if not 0 <= address < ADDRESS_SPACE_SIZE:
            raise ValueError("O endereço deve estar entre 0000h e FFFFh.")

    def read(self, address: int) -> int:
        """Lê um byte da ROM mapeada ou da RAM interna."""
        self._validate_address(address)

        if address >= RAM_START:
            return self._ram.read(address)

        if self._cartridge_rom is None or address >= len(self._cartridge_rom):
            raise UnmappedMemoryError(
                f"Não há dispositivo mapeado para o endereço {address:04X}h."
            )

        return self._cartridge_rom[address]

    def write(self, address: int, value: int) -> None:
        """Escreve na RAM; rejeita escritas em ROM e regiões não mapeadas."""
        self._validate_address(address)

        if type(value) is not int:
            raise TypeError("O valor deve ser um inteiro.")
        if not 0 <= value <= 0xFF:
            raise ValueError("O valor deve estar entre 00h e FFh.")

        if address >= RAM_START:
            self._ram.write(address, value)
            return

        if self._cartridge_rom is None or address >= len(self._cartridge_rom):
            raise UnmappedMemoryError(
                f"Não há dispositivo mapeado para o endereço {address:04X}h."
            )

        raise ReadOnlyMemoryError(
            f"Não é permitido escrever na ROM do cartucho em {address:04X}h."
        )
