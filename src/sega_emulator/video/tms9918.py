"""Interface de portas e registradores do VDP TMS9918A/TMS9928A.

Este módulo implementa o caminho CPU–VDP–VRAM e o protocolo básico de
registradores. Não implementa ainda renderização, sprites, colisões ou
temporização de linhas/quadros; o status de VBlank é alimentado externamente
até a integração com o escalonador temporal.
"""

from sega_emulator.bus.vram import TMS9918VRAM

VDP_DATA_PORT = 0xBE
VDP_CONTROL_PORT = 0xBF
REGISTER_COUNT = 8


class TMS9918:
    """Modelo inicial da interface de CPU de um VDP TMS9918 compatível."""

    def __init__(self, vram: TMS9918VRAM | None = None) -> None:
        self.vram = vram if vram is not None else TMS9918VRAM()
        self.registers = bytearray(REGISTER_COUNT)
        self.address = 0
        self.status = 0
        self._control_latch: int | None = None
        self._read_buffer = 0
        self._read_buffer_valid = False
        self._write_mode = False

    def reset(self) -> None:
        """Reinicia o estado lógico da interface, sem apagar a VRAM."""
        self.registers[:] = bytes(REGISTER_COUNT)
        self.address = 0
        self.status = 0
        self._control_latch = None
        self._read_buffer = 0
        self._read_buffer_valid = False
        self._write_mode = False

    def set_vblank(self, active: bool) -> None:
        """Define o bit de VBlank para testes e futura integração temporal."""
        if type(active) is not bool:
            raise TypeError("active deve ser bool.")
        if active:
            self.status |= 0x80
        else:
            self.status &= 0x7F

    def read_port(self, port: int) -> int:
        if port == VDP_DATA_PORT:
            value = self._read_buffer
            if not self._read_buffer_valid:
                value = self.vram.read(self.address)
            self._read_buffer = self.vram.read((self.address + 1) & 0x3FFF)
            self.address = (self.address + 1) & 0x3FFF
            self._read_buffer_valid = True
            self._control_latch = None
            return value
        if port == VDP_CONTROL_PORT:
            value = self.status
            # A leitura de status limpa os flags de status e a primeira metade
            # de um comando de controle pendente.
            self.status &= 0x1F
            self._control_latch = None
            return value
        raise ValueError(f"Porta de VDP inválida: {port:02X}h.")

    def write_port(self, port: int, value: int) -> None:
        if type(value) is not int or not 0 <= value <= 0xFF:
            raise ValueError("O valor do VDP deve estar entre 00h e FFh.")
        if port == VDP_DATA_PORT:
            self.vram.write(self.address, value)
            self.address = (self.address + 1) & 0x3FFF
            self._read_buffer_valid = False
            self._control_latch = None
            return
        if port != VDP_CONTROL_PORT:
            raise ValueError(f"Porta de VDP inválida: {port:02X}h.")
        if self._control_latch is None:
            self._control_latch = value
            return

        first = self._control_latch
        self._control_latch = None
        if value & 0x80:
            register = value & 0x07
            self.registers[register] = first
            return

        self.address = ((value & 0x3F) << 8) | first
        self._write_mode = bool(value & 0x40)
        self._read_buffer_valid = False
        if not self._write_mode:
            # Leitura do VDP: prefetch do byte apontado e avanço do endereço.
            self._read_buffer = self.vram.read(self.address)
            self.address = (self.address + 1) & 0x3FFF
            self._read_buffer_valid = True
