"""RAM interna e RAM expandida mapeadas na janela do SG-1000."""

RAM_START = 0xC000
RAM_END = 0xFFFF
RAM_SIZE = 0x0400
ADDRESS_SPACE_SIZE = 0x10000


class SG1000Ram:
    """RAM de 1 KiB ou expansão de 8 KiB espelhada em C000h–FFFFh.

    O tamanho físico é determinado pelo estado inicial explícito. A máquina
    seleciona 1 KiB para o SG-1000 comum e 8 KiB apenas para perfis Type-B
    conhecidos; o barramento mantém a janela lógica C000h–FFFFh.
    """

    def __init__(self, initial_data: bytes) -> None:
        """Cria RAM física de 1 KiB ou 8 KiB com estado inicial determinístico."""
        if not isinstance(initial_data, bytes):
            raise TypeError("initial_data deve ser bytes.")
        if len(initial_data) not in (RAM_SIZE, 0x2000):
            raise ValueError("A RAM deve conter exatamente 1 KiB ou 8 KiB.")

        self._data = bytearray(initial_data)
        self.size = len(initial_data)
        self._mask = self.size - 1

    @staticmethod
    def _offset(address: int) -> int:
        """Converte endereço da CPU em offset da RAM física."""
        if type(address) is not int:
            raise TypeError("O endereço deve ser um inteiro.")

        if not 0 <= address < ADDRESS_SPACE_SIZE:
            raise ValueError("O endereço deve estar entre 0000h e FFFFh.")

        if not RAM_START <= address <= RAM_END:
            raise ValueError(f"O endereço {address:04X}h não pertence à região da RAM.")

        return address & self._mask

    def read(self, address: int) -> int:
        """Lê um byte de um endereço mapeado para a RAM."""
        return self._data[self._offset(address)]

    def write(self, address: int, value: int) -> None:
        """Escreve um byte em um endereço mapeado para a RAM."""
        offset = self._offset(address)
        self._validate_value(value)
        self._write_offset(offset, value)

    @staticmethod
    def _validate_value(value: int) -> None:
        """Valida um byte antes de gravá-lo na RAM."""
        if type(value) is not int:
            raise TypeError("O valor deve ser um inteiro.")
        if not 0 <= value <= 0xFF:
            raise ValueError("O valor deve estar entre 00h e FFh.")

    def _read_mapped(self, address: int) -> int:
        """Lê endereço já validado pelo barramento de memória."""
        return self._data[address & self._mask]

    def _write_mapped(self, address: int, value: int) -> None:
        """Grava byte/endereço já validados pelo barramento de memória."""
        self._data[address & self._mask] = value

    def _write_offset(self, offset: int, value: int) -> None:
        """Grava um byte no offset físico já calculado e validado."""
        self._data[offset] = value
