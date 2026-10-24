"""Roteador de portas de I/O para dispositivos da plataforma."""

IO_ADDRESS_MASK = 0xFFFF
IO_PORT_MASK = 0xFF


class UnmappedIOError(LookupError):
    """A CPU acessou uma porta de I/O sem dispositivo associado."""


class SG1000IOBus:
    """Encaminha operações Z80 às portas registradas.

    O Z80 fornece um endereço de I/O de 16 bits; esta primeira implementação
    decodifica o byte inferior. O espelhamento elétrico de portas específicas
    deve ser configurado explicitamente quando confirmado para o dispositivo.
    """

    def __init__(self) -> None:
        self._read_handlers: dict[int, object] = {}
        self._write_handlers: dict[int, object] = {}

    @staticmethod
    def _validate_address(address: int) -> None:
        if type(address) is not int:
            raise TypeError("O endereço de I/O deve ser um inteiro.")
        if not 0 <= address <= IO_ADDRESS_MASK:
            raise ValueError("O endereço de I/O deve estar entre 0000h e FFFFh.")

    @staticmethod
    def _validate_byte(value: int) -> None:
        if type(value) is not int:
            raise TypeError("O valor deve ser um inteiro.")
        if not 0 <= value <= 0xFF:
            raise ValueError("O valor deve estar entre 00h e FFh.")

    def map_device(
        self,
        ports: tuple[int, ...],
        device: object,
    ) -> None:
        """Registra um dispositivo que implementa read_port/write_port."""
        if not callable(getattr(device, "read_port", None)):
            raise TypeError("O dispositivo precisa implementar read_port().")
        if not callable(getattr(device, "write_port", None)):
            raise TypeError("O dispositivo precisa implementar write_port().")
        if not ports:
            raise ValueError("Informe ao menos uma porta.")
        normalized: list[int] = []
        for port in ports:
            if type(port) is not int or not 0 <= port <= IO_PORT_MASK:
                raise ValueError("As portas devem ser inteiros entre 00h e FFh.")
            if port in normalized:
                raise ValueError(f"A porta {port:02X}h foi repetida.")
            if port in self._read_handlers or port in self._write_handlers:
                raise ValueError(f"A porta {port:02X}h já está mapeada.")
            normalized.append(port)
        for port in normalized:
            self._read_handlers[port] = device
            self._write_handlers[port] = device

    def read_port(self, address: int) -> int:
        """Lê uma porta decodificando o byte inferior do endereço Z80."""
        self._validate_address(address)
        port = address & IO_PORT_MASK
        device = self._read_handlers.get(port)
        if device is None:
            raise UnmappedIOError(f"Não há dispositivo na porta {port:02X}h.")
        value = device.read_port(port)  # type: ignore[attr-defined]
        self._validate_byte(value)
        return value

    def write_port(self, address: int, value: int) -> None:
        """Escreve uma porta decodificando o byte inferior do endereço Z80."""
        self._validate_address(address)
        self._validate_byte(value)
        port = address & IO_PORT_MASK
        device = self._write_handlers.get(port)
        if device is None:
            raise UnmappedIOError(f"Não há dispositivo na porta {port:02X}h.")
        device.write_port(port, value)  # type: ignore[attr-defined]
