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

    @staticmethod
    def _parity_flag(value: int) -> int:
        """Retorna PV quando o byte possui paridade par."""
        return 0x04 if value.bit_count() % 2 == 0 else 0

    def _add_a(self, value: int) -> None:
        """Executa ADD A,value e atualiza as flags documentadas e XY."""
        accumulator = self.a
        result = accumulator + value
        result_byte = result & 0xFF
        flags = result_byte & 0xA8  # S, Y e X
        if result_byte == 0:
            flags |= 0x40
        if (accumulator & 0x0F) + (value & 0x0F) > 0x0F:
            flags |= 0x10
        if (~(accumulator ^ value) & (accumulator ^ result_byte) & 0x80) != 0:
            flags |= 0x04
        if result > 0xFF:
            flags |= 0x01
        self.a = result_byte
        self.f = flags

    def _sub_a(self, value: int) -> None:
        """Executa SUB value e atualiza as flags documentadas e XY."""
        accumulator = self.a
        result_byte = (accumulator - value) & 0xFF
        flags = (result_byte & 0xA8) | 0x02  # S, Y, X e N
        if result_byte == 0:
            flags |= 0x40
        if (accumulator & 0x0F) < (value & 0x0F):
            flags |= 0x10
        if ((accumulator ^ value) & (accumulator ^ result_byte) & 0x80) != 0:
            flags |= 0x04
        if accumulator < value:
            flags |= 0x01
        self.a = result_byte
        self.f = flags

    def _xor_a(self, value: int) -> None:
        """Executa XOR value e atualiza S, Z, XY e paridade."""
        self.a ^= value
        self.f = (self.a & 0xA8) | self._parity_flag(self.a)
        if self.a == 0:
            self.f |= 0x40

    def _read_register_pair(self, code: int) -> int:
        """Lê BC, DE, HL ou AF conforme o campo qq."""
        if code == 0:
            return self.bc
        if code == 1:
            return self.de
        if code == 2:
            return self.hl
        return self.af

    def _write_register_pair(self, code: int, value: int) -> None:
        """Escreve BC, DE, HL ou AF conforme o campo qq."""
        if code == 0:
            self.bc = value
        elif code == 1:
            self.de = value
        elif code == 2:
            self.hl = value
        else:
            self.af = value

    def _push_word(self, value: int) -> None:
        """Empilha uma palavra: byte alto primeiro, depois byte baixo."""
        self.sp = (self.sp - 1) & 0xFFFF
        self.bus.write(self.sp, (value >> 8) & 0xFF)
        self.sp = (self.sp - 1) & 0xFFFF
        self.bus.write(self.sp, value & 0xFF)

    def _pop_word(self) -> int:
        """Desempilha uma palavra em little-endian a partir de SP."""
        low = self.bus.read(self.sp)
        self.sp = (self.sp + 1) & 0xFFFF
        high = self.bus.read(self.sp)
        self.sp = (self.sp + 1) & 0xFFFF
        return (high << 8) | low

    def _condition(self, code: int) -> bool:
        """Avalia NZ, Z, NC ou C conforme o campo de condição do opcode."""
        zero = bool(self.f & 0x40)
        carry = bool(self.f & 0x01)
        if code == 0:
            return not zero
        if code == 1:
            return zero
        if code == 2:
            return not carry
        return carry

    def step(self) -> int:
        """Executa uma instrução implementada e retorna seus T-states nominais.

        Subconjunto atual:
        - NOP (00h)
        - LD r,n (06h, 0Eh, 16h, 1Eh, 26h, 2Eh, 36h, 3Eh)
        - LD r,r' (40h–7Fh, exceto HALT 76h)
        - ADD A,r (80h–87h), SUB r (90h–97h), XOR A,r (A8h–AFh)
        - JP nn (C3h), JP cc,nn (C2h, CAh, D2h, DAh)
        - JR e (18h), JR cc,e (20h, 28h, 30h, 38h)
        - PUSH qq (C5h, D5h, E5h, F5h), POP qq (C1h, D1h, E1h, F1h)
        - CALL nn (CDh), CALL cc,nn (C4h, CCh, D4h, DCh)
        - RET (C9h), RET cc (C0h, C8h, D0h, D8h)

        As operações LD, saltos e instruções de pilha/controle não alteram
        F, exceto as instruções que restauram o par AF via POP AF.
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
        elif 0x80 <= opcode <= 0x87:  # ADD A,r
            self._add_a(self._read_register(opcode & 0x07))
            cycles = 7 if opcode & 0x07 == 6 else 4
        elif 0x90 <= opcode <= 0x97:  # SUB r
            self._sub_a(self._read_register(opcode & 0x07))
            cycles = 7 if opcode & 0x07 == 6 else 4
        elif 0xA8 <= opcode <= 0xAF:  # XOR A,r
            self._xor_a(self._read_register(opcode & 0x07))
            cycles = 7 if opcode & 0x07 == 6 else 4
        elif opcode == 0xC3:  # JP nn
            low = self._fetch_byte()
            high = self._fetch_byte()
            self.pc = (high << 8) | low
            cycles = 10
        elif opcode in (0xC2, 0xCA, 0xD2, 0xDA):  # JP cc,nn
            low = self._fetch_byte()
            high = self._fetch_byte()
            if self._condition((opcode >> 3) & 0x03):
                self.pc = (high << 8) | low
            cycles = 10
        elif opcode == 0x18:  # JR e
            displacement = self._fetch_byte()
            if displacement & 0x80:
                displacement -= 0x100
            self.pc = (self.pc + displacement) & 0xFFFF
            cycles = 12
        elif opcode in (0x20, 0x28, 0x30, 0x38):  # JR cc,e
            displacement = self._fetch_byte()
            if self._condition((opcode >> 3) & 0x03):
                if displacement & 0x80:
                    displacement -= 0x100
                self.pc = (self.pc + displacement) & 0xFFFF
                cycles = 12
            else:
                cycles = 7
        elif opcode & 0xCF == 0xC5:  # PUSH qq
            self._push_word(self._read_register_pair((opcode >> 4) & 0x03))
            cycles = 11
        elif opcode & 0xCF == 0xC1:  # POP qq
            self._write_register_pair((opcode >> 4) & 0x03, self._pop_word())
            cycles = 10
        elif opcode == 0xCD:  # CALL nn
            low = self._fetch_byte()
            high = self._fetch_byte()
            self._push_word(self.pc)
            self.pc = (high << 8) | low
            cycles = 17
        elif opcode in (0xC4, 0xCC, 0xD4, 0xDC):  # CALL cc,nn
            low = self._fetch_byte()
            high = self._fetch_byte()
            if self._condition((opcode >> 3) & 0x03):
                self._push_word(self.pc)
                self.pc = (high << 8) | low
                cycles = 17
            else:
                cycles = 10
        elif opcode == 0xC9:  # RET
            self.pc = self._pop_word()
            cycles = 10
        elif opcode in (0xC0, 0xC8, 0xD0, 0xD8):  # RET cc
            if self._condition((opcode >> 3) & 0x03):
                self.pc = self._pop_word()
                cycles = 11
            else:
                cycles = 5
        else:
            raise UnsupportedOpcodeError(
                f"Opcode Z80 {opcode:02X}h não implementado "
                f"no endereço {(self.pc - 1) & 0xFFFF:04X}h."
            )

        self.total_cycles += cycles
        return cycles
