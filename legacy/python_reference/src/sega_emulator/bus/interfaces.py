"""Contrato mínimo de acesso à memória para componentes de hardware."""

from typing import Protocol


class MemoryBus(Protocol):
    """Interface de leitura/escrita consumida pela CPU.

    O contrato representa apenas operações lógicas de byte. Não modela ciclos,
    espera, contenção, arbitragem nem efeitos colaterais temporais do hardware.
    """

    def read(self, address: int) -> int:
        """Lê um byte no endereço de 16 bits."""
        ...

    def write(self, address: int, value: int) -> None:
        """Escreve um byte no endereço de 16 bits."""
        ...


class IOBus(Protocol):
    """Contrato separado para portas de I/O de 16 bits do Z80."""

    def read_port(self, address: int) -> int:
        """Lê um byte da porta de I/O."""
        ...

    def write_port(self, address: int, value: int) -> None:
        """Escreve um byte na porta de I/O."""
        ...
