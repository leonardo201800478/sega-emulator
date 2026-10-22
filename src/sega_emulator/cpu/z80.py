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
        self.af_alt = 0
        self.bc_alt = 0
        self.de_alt = 0
        self.hl_alt = 0
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
    def af_alt(self) -> int:
        """Banco alternativo A':F' usado por EX AF,AF'."""
        return self._af_alt

    @af_alt.setter
    def af_alt(self, value: int) -> None:
        self._validate_word(value)
        self._af_alt = value

    @property
    def bc_alt(self) -> int:
        """Banco alternativo B':C' usado por EXX."""
        return self._bc_alt

    @bc_alt.setter
    def bc_alt(self, value: int) -> None:
        self._validate_word(value)
        self._bc_alt = value

    @property
    def de_alt(self) -> int:
        """Banco alternativo D':E' usado por EXX."""
        return self._de_alt

    @de_alt.setter
    def de_alt(self, value: int) -> None:
        self._validate_word(value)
        self._de_alt = value

    @property
    def hl_alt(self) -> int:
        """Banco alternativo H':L' usado por EXX."""
        return self._hl_alt

    @hl_alt.setter
    def hl_alt(self, value: int) -> None:
        self._validate_word(value)
        self._hl_alt = value

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

    def _read_stack_pair(self, code: int) -> int:
        """Lê BC, DE, HL ou SP para instruções de pares de 16 bits."""
        if code == 0:
            return self.bc
        if code == 1:
            return self.de
        if code == 2:
            return self.hl
        return self.sp

    def _write_stack_pair(self, code: int, value: int) -> None:
        """Escreve BC, DE, HL ou SP para instruções de pares de 16 bits."""
        value &= 0xFFFF
        if code == 0:
            self.bc = value
        elif code == 1:
            self.de = value
        elif code == 2:
            self.hl = value
        else:
            self.sp = value

    def _add_hl(self, value: int) -> None:
        """Executa ADD HL,rr, preservando S, Z e P/V."""
        previous = self.hl
        result = previous + value
        result_word = result & 0xFFFF
        flags = self.f & 0xC4  # S, Z e P/V preservados
        flags |= (result_word >> 8) & 0x28  # Y e X do byte alto
        if (previous & 0x0FFF) + (value & 0x0FFF) > 0x0FFF:
            flags |= 0x10
        if result > 0xFFFF:
            flags |= 0x01
        self.hl = result_word
        self.f = flags

    def _add_sub_hl(self, value: int, subtract: bool) -> None:
        """Executa ADC/SBC HL,rr, incluindo carry, overflow e flags de 16 bits."""
        previous = self.hl
        carry_in = self.f & 0x01
        if subtract:
            result = previous - value - carry_in
            half_carry = (previous & 0x0FFF) < ((value & 0x0FFF) + carry_in)
            overflow = bool((previous ^ value) & (previous ^ result) & 0x8000)
        else:
            result = previous + value + carry_in
            half_carry = (
                (previous & 0x0FFF) + (value & 0x0FFF) + carry_in > 0x0FFF
            )
            overflow = bool(~(previous ^ value) & (previous ^ result) & 0x8000)

        result_word = result & 0xFFFF
        flags = (result_word >> 8) & 0xA8
        if result_word == 0:
            flags |= 0x40
        if half_carry:
            flags |= 0x10
        if overflow:
            flags |= 0x04
        if subtract:
            flags |= 0x02
            if result < 0:
                flags |= 0x01
        elif result > 0xFFFF:
            flags |= 0x01

        self.hl = result_word
        self.f = flags

    def _inc_register_value(self, value: int) -> int:
        """Incrementa um byte, atualizando flags e preservando carry."""
        result = (value + 1) & 0xFF
        flags = (self.f & 0x01) | (result & 0xA8)
        if result == 0:
            flags |= 0x40
        if (value & 0x0F) == 0x0F:
            flags |= 0x10
        if value == 0x7F:
            flags |= 0x04
        self.f = flags
        return result

    def _dec_register_value(self, value: int) -> int:
        """Decrementa um byte, atualizando flags e preservando carry."""
        result = (value - 1) & 0xFF
        flags = (self.f & 0x01) | (result & 0xA8) | 0x02
        if result == 0:
            flags |= 0x40
        if (value & 0x0F) == 0:
            flags |= 0x10
        if value == 0x80:
            flags |= 0x04
        self.f = flags
        return result

    def _and_a(self, value: int) -> None:
        """Executa AND e atualiza flags de resultado, half-carry e paridade."""
        self.a &= value
        self.f = (self.a & 0xA8) | 0x10 | self._parity_flag(self.a)
        if self.a == 0:
            self.f |= 0x40

    def _or_a(self, value: int) -> None:
        """Executa OR e atualiza flags de resultado e paridade."""
        self.a |= value
        self.f = (self.a & 0xA8) | self._parity_flag(self.a)
        if self.a == 0:
            self.f |= 0x40

    def _compare_a(self, value: int) -> None:
        """Executa CP value: atualiza flags como SUB sem alterar A."""
        accumulator = self.a
        result_byte = (accumulator - value) & 0xFF
        flags = (value & 0x28) | 0x02  # X/Y vêm do operando em CP
        if result_byte & 0x80:
            flags |= 0x80
        if result_byte == 0:
            flags |= 0x40
        if (accumulator & 0x0F) < (value & 0x0F):
            flags |= 0x10
        if ((accumulator ^ value) & (accumulator ^ result_byte) & 0x80) != 0:
            flags |= 0x04
        if accumulator < value:
            flags |= 0x01
        self.f = flags

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

    def _adc_a(self, value: int) -> None:
        """Executa ADC A,value incluindo o carry de entrada."""
        accumulator = self.a
        carry_in = self.f & 0x01
        result = accumulator + value + carry_in
        result_byte = result & 0xFF
        flags = result_byte & 0xA8
        if result_byte == 0:
            flags |= 0x40
        if (accumulator & 0x0F) + (value & 0x0F) + carry_in > 0x0F:
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

    def _sbc_a(self, value: int) -> None:
        """Executa SBC A,value incluindo o carry/borrow de entrada."""
        accumulator = self.a
        carry_in = self.f & 0x01
        result_byte = (accumulator - value - carry_in) & 0xFF
        flags = (result_byte & 0xA8) | 0x02
        if result_byte == 0:
            flags |= 0x40
        if (accumulator & 0x0F) < ((value & 0x0F) + carry_in):
            flags |= 0x10
        if ((accumulator ^ value) & (accumulator ^ result_byte) & 0x80) != 0:
            flags |= 0x04
        if accumulator < value + carry_in:
            flags |= 0x01
        self.a = result_byte
        self.f = flags

    def _rotate_accumulator(self, opcode: int) -> None:
        """Executa RLCA, RRCA, RLA ou RRA com flags específicas do Z80."""
        accumulator = self.a
        old_carry = self.f & 0x01

        if opcode in (0x07, 0x17):  # RLCA / RLA
            carry_out = (accumulator >> 7) & 0x01
            carry_in = old_carry if opcode == 0x17 else carry_out
            result = ((accumulator << 1) | carry_in) & 0xFF
        else:  # RRCA / RRA
            carry_out = accumulator & 0x01
            carry_in = old_carry if opcode == 0x1F else carry_out
            result = (accumulator >> 1) | (carry_in << 7)

        # Estas quatro instruções preservam S, Z e P/V; H e N são limpos.
        flags = (self.f & 0xC4) | (result & 0x28) | carry_out
        self.a = result
        self.f = flags

    def _daa(self) -> None:
        """Ajusta A para BCD conforme os flags H, N e C anteriores."""
        accumulator = self.a
        old_flags = self.f
        subtract = bool(old_flags & 0x02)
        old_half_carry = bool(old_flags & 0x10)
        old_carry = bool(old_flags & 0x01)

        correction = 0
        if old_half_carry or (not subtract and (accumulator & 0x0F) > 0x09):
            correction |= 0x06
        if old_carry or (not subtract and accumulator > 0x99):
            correction |= 0x60

        if subtract:
            result = (accumulator - correction) & 0xFF
        else:
            result = (accumulator + correction) & 0xFF

        flags = (result & 0xA8) | (old_flags & 0x02)
        if result == 0:
            flags |= 0x40
        if self._parity_flag(result):
            flags |= 0x04
        if (accumulator ^ result) & 0x10:
            flags |= 0x10
        if old_carry or (not subtract and accumulator > 0x99):
            flags |= 0x01

        self.a = result
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
        """Avalia os oito códigos de condição definidos pelo Z80."""
        zero = bool(self.f & 0x40)
        carry = bool(self.f & 0x01)
        parity_overflow = bool(self.f & 0x04)
        sign = bool(self.f & 0x80)
        conditions = (
            not zero,  # NZ
            zero,  # Z
            not carry,  # NC
            carry,  # C
            not parity_overflow,  # PO
            parity_overflow,  # PE
            not sign,  # P
            sign,  # M
        )
        return conditions[code]

    def step(self) -> int:
        """Executa uma instrução implementada e retorna seus T-states nominais.

        Subconjunto atual:
        - NOP (00h)
        - DAA (27h)
        - EX AF,AF' (08h), EXX (D9h), EX DE,HL (EBh)
        - RST 00h, 08h, 10h, 18h, 20h, 28h, 30h e 38h
        - RLCA (07h), RRCA (0Fh), RLA (17h), RRA (1Fh)
        - CPL (2Fh), SCF (37h), CCF (3Fh)
        - LD rr,nn (01h, 11h, 21h, 31h)
        - LD (BC/DE),A (02h, 12h) e LD A,(BC/DE) (0Ah, 1Ah)
        - LD (nn),HL (22h) e LD HL,(nn) (2Ah)
        - JP (HL) (E9h) e LD SP,HL (F9h)
        - LD (nn),A (32h) e LD A,(nn) (3Ah)
        - LD r,n (06h, 0Eh, 16h, 1Eh, 26h, 2Eh, 36h, 3Eh)
        - LD r,r' (40h–7Fh, exceto HALT 76h)
        - ADD A,r (80h–87h), ADC A,r (88h–8Fh)
        - SUB r (90h–97h), SBC A,r (98h–9Fh)
        - Operações ALU imediatas: ADD (C6h), ADC (CEh), SUB (D6h),
          SBC (DEh), AND (E6h), XOR (EEh), OR (F6h), CP (FEh)
        - AND r (A0h–A7h), XOR A,r (A8h–AFh)
        - OR r (B0h–B7h), CP r (B8h–BFh)
        - INC r (04h, 0Ch, 14h, 1Ch, 24h, 2Ch, 34h, 3Ch)
        - DEC r (05h, 0Dh, 15h, 1Dh, 25h, 2Dh, 35h, 3Dh)
        - JP nn (C3h), JP cc,nn (C2h, CAh, D2h, DAh, E2h, EAh, F2h, FAh)
        - DJNZ e (10h), JR e (18h), JR cc,e (20h, 28h, 30h, 38h)
        - PUSH qq (C5h, D5h, E5h, F5h), POP qq (C1h, D1h, E1h, F1h)
        - CALL nn (CDh), CALL cc,nn (C4h, CCh, D4h, DCh, E4h, ECh, F4h, FCh)
        - RET (C9h), RET cc (C0h, C8h, D0h, D8h, E0h, E8h, F0h, F8h)
        - INC rr (03h, 13h, 23h, 33h), DEC rr (0Bh, 1Bh, 2Bh, 3Bh)
        - ADD HL,rr (09h, 19h, 29h, 39h)
        - ADC HL,rr / SBC HL,rr (prefixo EDh)
        - CP r (B8h–BFh)

        INC/DEC de pares e instruções de controle preservam F conforme a
        arquitetura; POP AF restaura F. ADD HL preserva S, Z e P/V.
        """
        opcode = self._fetch_byte()

        if opcode == 0x00:  # NOP
            cycles = 4
        elif opcode == 0xED:  # instruções prefixadas ED implementadas
            extended_opcode = self._fetch_byte()
            if extended_opcode & 0xCF == 0x4A:  # ADC HL,rr
                self._add_sub_hl(
                    self._read_stack_pair((extended_opcode >> 4) & 0x03), False
                )
                cycles = 15
            elif extended_opcode & 0xCF == 0x42:  # SBC HL,rr
                self._add_sub_hl(
                    self._read_stack_pair((extended_opcode >> 4) & 0x03), True
                )
                cycles = 15
            else:
                raise UnsupportedOpcodeError(
                    f"Opcode Z80 ED {extended_opcode:02X}h não implementado "
                    f"no endereço {(self.pc - 2) & 0xFFFF:04X}h."
                )
        elif opcode == 0x08:  # EX AF,AF'
            previous = self.af
            self.af = self.af_alt
            self.af_alt = previous
            cycles = 4
        elif opcode == 0xD9:  # EXX
            self.bc, self.bc_alt = self.bc_alt, self.bc
            self.de, self.de_alt = self.de_alt, self.de
            self.hl, self.hl_alt = self.hl_alt, self.hl
            cycles = 4
        elif opcode == 0xEB:  # EX DE,HL
            self.de, self.hl = self.hl, self.de
            cycles = 4
        elif opcode & 0xC7 == 0xC7:  # RST p
            self._push_word(self.pc)
            self.pc = opcode & 0x38
            cycles = 11
        elif opcode == 0x27:  # DAA
            self._daa()
            cycles = 4
        elif opcode in (0x07, 0x0F, 0x17, 0x1F):  # rotações do acumulador
            self._rotate_accumulator(opcode)
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
        elif 0x88 <= opcode <= 0x8F:  # ADC A,r
            self._adc_a(self._read_register(opcode & 0x07))
            cycles = 7 if opcode & 0x07 == 6 else 4
        elif 0x90 <= opcode <= 0x97:  # SUB r
            self._sub_a(self._read_register(opcode & 0x07))
            cycles = 7 if opcode & 0x07 == 6 else 4
        elif 0x98 <= opcode <= 0x9F:  # SBC A,r
            self._sbc_a(self._read_register(opcode & 0x07))
            cycles = 7 if opcode & 0x07 == 6 else 4
        elif 0xA0 <= opcode <= 0xA7:  # AND r
            self._and_a(self._read_register(opcode & 0x07))
            cycles = 7 if opcode & 0x07 == 6 else 4
        elif 0xA8 <= opcode <= 0xAF:  # XOR A,r
            self._xor_a(self._read_register(opcode & 0x07))
            cycles = 7 if opcode & 0x07 == 6 else 4
        elif 0xB0 <= opcode <= 0xB7:  # OR r
            self._or_a(self._read_register(opcode & 0x07))
            cycles = 7 if opcode & 0x07 == 6 else 4
        elif 0xB8 <= opcode <= 0xBF:  # CP r
            self._compare_a(self._read_register(opcode & 0x07))
            cycles = 7 if opcode & 0x07 == 6 else 4
        elif opcode in (0xC6, 0xCE, 0xD6, 0xDE, 0xE6, 0xEE, 0xF6, 0xFE):
            value = self._fetch_byte()
            if opcode == 0xC6:  # ADD A,n
                self._add_a(value)
            elif opcode == 0xCE:  # ADC A,n
                self._adc_a(value)
            elif opcode == 0xD6:  # SUB n
                self._sub_a(value)
            elif opcode == 0xDE:  # SBC A,n
                self._sbc_a(value)
            elif opcode == 0xE6:  # AND n
                self._and_a(value)
            elif opcode == 0xEE:  # XOR n
                self._xor_a(value)
            elif opcode == 0xF6:  # OR n
                self._or_a(value)
            else:  # CP n
                self._compare_a(value)
            cycles = 7
        elif opcode & 0xC7 == 0x04:  # INC r
            register = (opcode >> 3) & 0x07
            self._write_register(
                register, self._inc_register_value(self._read_register(register))
            )
            cycles = 11 if register == 6 else 4
        elif opcode & 0xC7 == 0x05:  # DEC r
            register = (opcode >> 3) & 0x07
            self._write_register(
                register, self._dec_register_value(self._read_register(register))
            )
            cycles = 11 if register == 6 else 4
        elif opcode & 0xCF == 0x01:  # LD rr,nn
            code = (opcode >> 4) & 0x03
            low = self._fetch_byte()
            high = self._fetch_byte()
            self._write_stack_pair(code, (high << 8) | low)
            cycles = 10
        elif opcode in (0x02, 0x12):  # LD (BC/DE),A
            address = self.bc if opcode == 0x02 else self.de
            self.bus.write(address, self.a)
            cycles = 7
        elif opcode in (0x0A, 0x1A):  # LD A,(BC/DE)
            address = self.bc if opcode == 0x0A else self.de
            self.a = self.bus.read(address)
            cycles = 7
        elif opcode in (0x22, 0x2A):  # LD (nn),HL / LD HL,(nn)
            low = self._fetch_byte()
            high = self._fetch_byte()
            address = (high << 8) | low
            next_address = (address + 1) & 0xFFFF
            if opcode == 0x22:
                self.bus.write(address, self.l)
                self.bus.write(next_address, self.h)
            else:
                low_value = self.bus.read(address)
                high_value = self.bus.read(next_address)
                self.hl = (high_value << 8) | low_value
            cycles = 16
        elif opcode in (0x32, 0x3A):  # LD (nn),A / LD A,(nn)
            low = self._fetch_byte()
            high = self._fetch_byte()
            address = (high << 8) | low
            if opcode == 0x32:
                self.bus.write(address, self.a)
            else:
                self.a = self.bus.read(address)
            cycles = 13
        elif opcode & 0xCF == 0x03:  # INC rr
            code = (opcode >> 4) & 0x03
            self._write_stack_pair(code, self._read_stack_pair(code) + 1)
            cycles = 6
        elif opcode & 0xCF == 0x0B:  # DEC rr
            code = (opcode >> 4) & 0x03
            self._write_stack_pair(code, self._read_stack_pair(code) - 1)
            cycles = 6
        elif opcode & 0xCF == 0x09:  # ADD HL,rr
            self._add_hl(self._read_stack_pair((opcode >> 4) & 0x03))
            cycles = 11
        elif opcode == 0xE9:  # JP (HL)
            self.pc = self.hl
            cycles = 4
        elif opcode == 0xF9:  # LD SP,HL
            self.sp = self.hl
            cycles = 6
        elif opcode == 0xC3:  # JP nn
            low = self._fetch_byte()
            high = self._fetch_byte()
            self.pc = (high << 8) | low
            cycles = 10
        elif opcode & 0xC7 == 0xC2:  # JP cc,nn
            low = self._fetch_byte()
            high = self._fetch_byte()
            if self._condition((opcode >> 3) & 0x07):
                self.pc = (high << 8) | low
            cycles = 10
        elif opcode == 0x2F:  # CPL
            self.a ^= 0xFF
            self.f = (self.f & 0xC5) | (self.a & 0x28) | 0x12
            cycles = 4
        elif opcode == 0x37:  # SCF
            self.f = (self.f & 0xC4) | (self.a & 0x28) | 0x01
            cycles = 4
        elif opcode == 0x3F:  # CCF
            previous_carry = self.f & 0x01
            self.f = (self.f & 0xC4) | (self.a & 0x28)
            if previous_carry:
                self.f |= 0x10
            else:
                self.f |= 0x01
            cycles = 4
        elif opcode == 0x10:  # DJNZ e
            displacement = self._fetch_byte()
            self.b = (self.b - 1) & 0xFF
            if self.b != 0:
                if displacement & 0x80:
                    displacement -= 0x100
                self.pc = (self.pc + displacement) & 0xFFFF
                cycles = 13
            else:
                cycles = 8
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
        elif opcode & 0xC7 == 0xC4:  # CALL cc,nn
            low = self._fetch_byte()
            high = self._fetch_byte()
            if self._condition((opcode >> 3) & 0x07):
                self._push_word(self.pc)
                self.pc = (high << 8) | low
                cycles = 17
            else:
                cycles = 10
        elif opcode == 0xC9:  # RET
            self.pc = self._pop_word()
            cycles = 10
        elif opcode & 0xC7 == 0xC0:  # RET cc
            if self._condition((opcode >> 3) & 0x07):
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
