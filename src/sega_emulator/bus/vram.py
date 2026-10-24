"""VRAM dedicada do VDP da família TMS9918 para o SG-1000."""

VRAM_SIZE = 0x4000
VRAM_ADDRESS_MASK = VRAM_SIZE - 1


class TMS9918VRAM:
    """Armazena os 16 KiB de VRAM, separados do espaço de memória da CPU.

    O VDP acessa esta memória por endereços de 14 bits. A CPU não pode ler
    esta classe diretamente pelo barramento principal; deve usar as portas
    de dados/controle do VDP.
    """

    def __init__(self, initial_data: bytes | None = None) -> None:
        if initial_data is None:
            initial_data = bytes(VRAM_SIZE)
        if not isinstance(initial_data, bytes):
            raise TypeError("initial_data deve ser bytes.")
        if len(initial_data) != VRAM_SIZE:
            raise ValueError(f"A VRAM deve conter exatamente {VRAM_SIZE} bytes.")
        self._data = bytearray(initial_data)

    @staticmethod
    def _validate_address(address: int) -> None:
        if type(address) is not int:
            raise TypeError("O endereço da VRAM deve ser um inteiro.")
        if not 0 <= address <= VRAM_ADDRESS_MASK:
            raise ValueError("O endereço da VRAM deve estar entre 0000h e 3FFFh.")

    @staticmethod
    def _validate_byte(value: int) -> None:
        if type(value) is not int:
            raise TypeError("O valor deve ser um inteiro.")
        if not 0 <= value <= 0xFF:
            raise ValueError("O valor deve estar entre 00h e FFh.")

    def read(self, address: int) -> int:
        """Lê um byte por endereço físico de 14 bits."""
        self._validate_address(address)
        return self._data[address]

    def write(self, address: int, value: int) -> None:
        """Escreve um byte por endereço físico de 14 bits."""
        self._validate_address(address)
        self._validate_byte(value)
        self._data[address] = value

    def snapshot(self) -> bytes:
        """Retorna uma cópia imutável para depuração e testes."""
        return bytes(self._data)
