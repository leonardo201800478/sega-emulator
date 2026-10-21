"""Subconjunto inicial e determinístico da CPU Z80."""

from sega_emulator.bus.interfaces import MemoryBus


class UnsupportedOpcodeError(NotImplementedError):
    """A CPU encontrou um opcode ainda não implementado."""


class Z80Cpu:
    """Executa um subconjunto inicial de instruções Z80.

    O estado inicial zerado é uma convenção determinística do emulador, não
    uma afirmação sobre os valores dos registradores após ligar o hardware.
    Os ciclos retornados são os T-states nominais das instruções implementadas;
    ainda não sincronizam CPU, barramento e dispositivos em nível de ciclo.
    """

    def __init__(self, bus: MemoryBus) -> None:
        """Inicializa os registradores e associa um barramento de memória."""
        self.bus = bus
        self.a = 0
        self.f = 0
        self.b = 0
        self.c = 0
        self.d = 0
        self.e = 0
        self.h = 0
        self.l = 0
        self.sp = 0
        self.pc = 0
        self.total_cycles = 0

    @staticmethod
    def _validate_byte(value: int) -> None:
        if type(value) is not int:
            raise TypeError("O registrador deve receber um inteiro.")
        if not 0 <= value <= 0xFF:
            raise ValueError("O registrador deve estar entre 00h e FFh.")

    @staticmethod
    def _validate_word(value: int) -> None:
        if type(value) is not int:
            raise TypeError("O registrador deve receber um inteiro.")
        if not 0 <= value <= 0xFFFF:
            raise ValueError("O registrador deve estar entre 0000h e FFFFh.")

    @property
    def af(self) -> int:
        """Par de registradores A:F."""
        return (self.a << 8) | self.f

    @af.setter
    def af(self, value: int) -> None:
        self._validate_word(value)
        self.a, self.f = value >> 8, value & 0xFF

    @property
    def bc(self) -> int:
        """Par de registradores B:C."""
        return (self.b << 8) | self.c

    @bc.setter
    def bc(self, value: int) -> None:
        self._validate_word(value)
        self.b, self.c = value >> 8, value & 0xFF

    @property
    def de(self) -> int:
        """Par de registradores D:E."""
        return (self.d << 8) | self.e

    @de.setter
    def de(self, value: int) -> None:
        self._validate_word(value)
        self.d, self.e = value >> 8, value & 0xFF

    @property
    def hl(self) -> int:
        """Par de registradores H:L."""
        return (self.h << 8) | self.l

    @hl.setter
    def hl(self, value: int) -> None:
        self._validate_word(value)
        self.h, self.l = value >> 8, value & 0xFF

    @property
    def pc(self) -> int:
        """Contador de programa de 16 bits."""
        return self._pc

    @pc.setter
    def pc(self, value: int) -> None:
        self._validate_word(value)
        self._pc = value

    @property
    def sp(self) -> int:
        """Ponteiro de pilha de 16 bits."""
        return self._sp

    @sp.setter
    def sp(self, value: int) -> None:
        self._validate_word(value)
        self._sp = value

    def _fetch_byte(self) -> int:
        value = self.bus.read(self.pc)
        self.pc = (self.pc + 1) & 0xFFFF
        return value

    def _read_register(self, code: int) -> int:
        if code == 0:
            return self.b
        if code == 1:
            return self.c
        if code == 2:
            return self.d
        if code == 3:
            return self.e
        if code == 4:
            return self.h
        if code == 5:
            return self.l
        if code == 6:
            return self.bus.read(self.hl)
        return self.a

    def _write_register(self, code: int, value: int) -> None:
        if code == 6:
            self.bus.write(self.hl, value)
        elif code == 0:
            self.b = value
        elif code == 1:
            self.c = value
        elif code == 2:
            self.d = value
        elif code == 3:
            self.e = value
        elif code == 4:
            self.h = value
        elif code == 5:
            self.l = value
        else:
            self.a = value

    def step(self) -> int:
        """Executa uma instrução implementada e retorna seus T-states nominais.

        Subconjunto atual:
        - NOP (00h)
        - LD r,n (06h, 0Eh, 16h, 1Eh, 26h, 2Eh, 36h, 3Eh)
        - LD r,r' (40h–7Fh, exceto HALT 76h)

        As operações LD não alteram o registrador de flags F.
        """
        opcode = self._fetch_byte()

        if opcode == 0x00:  # NOP
            cycles = 4
        elif opcode & 0xC7 == 0x06:  # LD r,n
            destination = (opcode >> 3) & 0x07
            value = self._fetch_byte()
            self._write_register(destination, value)
            cycles = 10 if destination == 6 else 7
        elif 0x40 <= opcode <= 0x7F and opcode != 0x76:  # LD r,r'
            destination = (opcode >> 3) & 0x07
            source = opcode & 0x07
            value = self._read_register(source)
            self._write_register(destination, value)
            cycles = 7 if destination == 6 or source == 6 else 4
        else:
            raise UnsupportedOpcodeError(
                f"Opcode Z80 {opcode:02X}h não implementado "
                f"no endereço {(self.pc - 1) & 0xFFFF:04X}h."
            )

        self.total_cycles += cycles
        return cycles
