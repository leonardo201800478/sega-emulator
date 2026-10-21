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
