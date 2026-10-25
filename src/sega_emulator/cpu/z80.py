"""Subconjunto inicial e determinístico da CPU Z80."""

from sega_emulator.bus.interfaces import IOBus, MemoryBus

_CONDITION_TABLE = tuple(
    condition
    for flags in range(256)
    for condition in (
        not flags & 0x40,
        bool(flags & 0x40),
        not flags & 0x01,
        bool(flags & 0x01),
        not flags & 0x04,
        bool(flags & 0x04),
        not flags & 0x80,
        bool(flags & 0x80),
    )
)


class UnsupportedOpcodeError(NotImplementedError):
    """A CPU encontrou um opcode ainda não implementado."""


class Z80Cpu:
    """Executa um subconjunto inicial de instruções Z80.

    O estado inicial zerado é uma convenção determinística do emulador, não
    uma afirmação sobre os valores dos registradores após ligar o hardware.
    Os ciclos retornados são os T-states nominais das instruções implementadas;
    ainda não sincronizam CPU, barramento e dispositivos em nível de ciclo.
    """

    def __init__(self, bus: MemoryBus, io_bus: IOBus | None = None) -> None:
        """Inicializa os registradores e associa um barramento de memória."""
        self._bus = bus
        # Machine buses may expose a fast internal fetch path. Keep the
        # generic protocol fallback for standalone CPUs and test buses.
        self._read_for_fetch = getattr(bus, "_read_for_cpu", bus.read)
        self.io_bus = io_bus
        self.reset()

    @property
    def bus(self) -> MemoryBus:
        return self._bus

    @bus.setter
    def bus(self, value: MemoryBus) -> None:
        self._bus = value
        self._read_for_fetch = getattr(value, "_read_for_cpu", value.read)

    def reset(self) -> None:
        """Reinicializa o estado interno determinístico da CPU.

        Esta política do emulador zera todos os registradores e contadores e
        cancela pedidos pendentes. Não representa uma afirmação de que os
        registradores gerais de um Z80 físico tenham valores definidos após
        RESET. Os barramentos e o conteúdo da memória permanecem intactos.
        """
        self._step_depth = 0
        self.q = 0
        self._q_previous = 0
        # P latch: previous instruction was LD A,I or LD A,R.
        self.p = 0
        self.wz = 0
        self.a = 0
        self.f = 0
        self.b = 0
        self.c = 0
        self.d = 0
        self.e = 0
        self.h = 0
        self.l = 0
        self.ix = 0
        self.iy = 0
        self.af_alt = 0
        self.bc_alt = 0
        self.de_alt = 0
        self.hl_alt = 0
        self.i = 0
        self.r = 0
        self.iff1 = False
        self.iff2 = False
        self.interrupt_mode = 0
        self.halted = False
        self._ei_delay = 0
        self._nmi_pending = False
        self._int_pending: int | None = None
        # Opcode fornecido pelo barramento durante acknowledge em IM 0.
        self._im0_opcode: int | None = None
        self.sp = 0
        self.pc = 0
        self.total_cycles = 0

    @property
    def f(self) -> int:
        return self._f

    @f.setter
    def f(self, value: int) -> None:
        self._f = value & 0xFF
        # Durante step(), escritas em F atualizam Q. Escritas externas
        # (setup de testes e atribuições de registradores) não alteram Q.
        if self._step_depth > 0:
            self.q = self._f

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
        # Internal fetches already keep PC in range; bypass the public setter's
        # defensive type/range validation on this per-byte hot path.
        pc = self._pc
        value = self._read_for_fetch(pc)
        self._pc = (pc + 1) & 0xFFFF
        return value

    def _fetch_opcode_byte(self) -> int:
        """Busca opcode M1; IM 0 pode fornecer o primeiro byte externamente."""
        if self._im0_opcode is not None:
            value = self._im0_opcode
            self._im0_opcode = None
            return value
        # Opcode fetch is the commonest memory access. Inline _fetch_byte here
        # to avoid one Python call for every instruction's first byte.
        pc = self._pc
        value = self._read_for_fetch(pc)
        self._pc = (pc + 1) & 0xFFFF
        self.r = (self.r & 0x80) | ((self.r + 1) & 0x7F)
        return value

    def _load_a_from_special_register(self, value: int) -> None:
        """Executa LD A,I/R; P/V reflete IFF2 e C é preservado."""
        self.a = value
        flags = (value & 0xA8) | (self.f & 0x01)
        if value == 0:
            flags |= 0x40
        if self.iff2:
            flags |= 0x04
        self.f = flags

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
        return self._sp

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
            self._sp = value

    def _add_hl(self, value: int) -> None:
        """Executa ADD HL,rr, preservando S, Z e P/V."""
        previous = self.hl
        self.wz = (previous + 1) & 0xFFFF
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
        self.wz = (previous + 1) & 0xFFFF
        carry_in = self.f & 0x01
        if subtract:
            result = previous - value - carry_in
            half_carry = (previous & 0x0FFF) < ((value & 0x0FFF) + carry_in)
            overflow = bool((previous ^ value) & (previous ^ result) & 0x8000)
        else:
            result = previous + value + carry_in
            half_carry = (previous & 0x0FFF) + (value & 0x0FFF) + carry_in > 0x0FFF
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
        if old_half_carry or (accumulator & 0x0F) > 0x09:
            correction |= 0x06
        if old_carry or accumulator > 0x99:
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
        sp = (self._sp - 1) & 0xFFFF
        self._sp = sp
        self.bus.write(sp, (value >> 8) & 0xFF)
        sp = (sp - 1) & 0xFFFF
        self._sp = sp
        self.bus.write(sp, value & 0xFF)

    def _pop_word(self) -> int:
        """Desempilha uma palavra em little-endian a partir de SP."""
        sp = self._sp
        low = self.bus.read(sp)
        sp = (sp + 1) & 0xFFFF
        self._sp = sp
        high = self.bus.read(sp)
        self._sp = (sp + 1) & 0xFFFF
        return (high << 8) | low

    def _condition(self, code: int) -> bool:
        """Avalia os oito códigos de condição definidos pelo Z80."""
        return _CONDITION_TABLE[(self._f << 3) | code]

    def _execute_cb(self, opcode: int) -> int:
        """Executa o conjunto CB: rotações, shifts, BIT, RES e SET."""
        register = opcode & 0x07
        operation = opcode >> 6
        bit_index = (opcode >> 3) & 0x07
        value = self._read_register(register)
        is_memory = register == 6
        if operation == 0:
            carry_in = self.f & 1
            if bit_index == 0:
                carry = value >> 7
                result = ((value << 1) | carry) & 0xFF
            elif bit_index == 1:
                carry = value & 1
                result = (value >> 1) | (carry << 7)
            elif bit_index == 2:
                carry = value >> 7
                result = ((value << 1) | carry_in) & 0xFF
            elif bit_index == 3:
                carry = value & 1
                result = (value >> 1) | (carry_in << 7)
            elif bit_index == 4:
                carry = value >> 7
                result = (value << 1) & 0xFF
            elif bit_index == 5:
                carry = value & 1
                result = (value >> 1) | (value & 0x80)
            elif bit_index == 6:  # SLL, não documentado
                carry = value >> 7
                result = ((value << 1) | 1) & 0xFF
            else:
                carry = value & 1
                result = value >> 1
            self._write_register(register, result)
            self.f = (result & 0xA8) | self._parity_flag(result) | carry
            if result == 0:
                self.f |= 0x40
        elif operation == 1:
            zero = not (value & (1 << bit_index))
            flags = (self.f & 1) | 0x10 | (value & 0x28)
            if zero:
                flags |= 0x44
            if bit_index == 7 and not zero:
                flags |= 0x80
            self.f = flags
        elif operation == 2:
            self._write_register(register, value & ~(1 << bit_index))
        else:
            self._write_register(register, value | (1 << bit_index))
        if operation == 1 and is_memory:
            # BIT b,(HL): F3/F5 vêm do byte alto de WZ/MEMPTR, não de HL.
            self.f = (self.f & ~0x28) | ((self.wz >> 8) & 0x28)
        if is_memory:
            return 12 if operation == 1 else 15
        return 8

    def request_nmi(self) -> None:
        """Solicita NMI para atendimento no próximo step."""
        self._nmi_pending = True

    def request_int(self, data_bus: int = 0xFF) -> None:
        """Solicita INT mascarável; data_bus representa o byte externo."""
        self._validate_byte(data_bus)
        self._int_pending = data_bus

    def clear_int_request(self) -> None:
        """Retira uma INT ainda não aceita quando a linha externa desativa."""
        self._int_pending = None

    def _read_port(self, address: int) -> int:
        """Lê uma porta de 16 bits pelo barramento de I/O separado."""
        if self.io_bus is None:
            raise UnsupportedOpcodeError("Instrução IN requer um IOBus configurado.")
        return self.io_bus.read_port(address & 0xFFFF)

    def _write_port(self, address: int, value: int) -> None:
        """Escreve uma porta de 16 bits pelo barramento de I/O separado."""
        if self.io_bus is None:
            raise UnsupportedOpcodeError("Instrução OUT requer um IOBus configurado.")
        self.io_bus.write_port(address & 0xFFFF, value & 0xFF)

    def _execute_ed(self, opcode: int) -> int:
        """Executa instruções ED documentadas implementadas no Z80 clássico."""
        if opcode in (0x47, 0x4F, 0x57, 0x5F):
            if opcode == 0x47:
                self.i = self.a
            elif opcode == 0x4F:
                self.r = self.a
            else:
                self._load_a_from_special_register(self.i if opcode == 0x57 else self.r)
                self.p = 1
            return 9
        if opcode & 0xCF == 0x43:
            address = self._fetch_byte() | (self._fetch_byte() << 8)
            value = self._read_stack_pair((opcode >> 4) & 3)
            self.bus.write(address, value & 0xFF)
            self.bus.write((address + 1) & 0xFFFF, value >> 8)
            self.wz = (address + 1) & 0xFFFF
            return 20
        if opcode & 0xCF == 0x4B:
            address = self._fetch_byte() | (self._fetch_byte() << 8)
            low = self.bus.read(address)
            high = self.bus.read((address + 1) & 0xFFFF)
            self._write_stack_pair((opcode >> 4) & 3, (high << 8) | low)
            self.wz = (address + 1) & 0xFFFF
            return 20
        if opcode & 0xCF == 0x4A:
            self._add_sub_hl(self._read_stack_pair((opcode >> 4) & 3), False)
            return 15
        if opcode & 0xCF == 0x42:
            self._add_sub_hl(self._read_stack_pair((opcode >> 4) & 3), True)
            return 15
        if opcode & 0xC7 == 0x40:
            register = (opcode >> 3) & 7
            port = self.bc
            value = self._read_port(port)
            self.wz = (port + 1) & 0xFFFF
            if register != 6:
                self._write_register(register, value)
            self.f = (self.f & 1) | (value & 0xA8) | self._parity_flag(value)
            if value == 0:
                self.f |= 0x40
            return 12
        if opcode & 0xC7 == 0x41:
            register = (opcode >> 3) & 7
            port = self.bc
            value = 0 if register == 6 else self._read_register(register)
            self._write_port(port, value)
            self.wz = (port + 1) & 0xFFFF
            return 12
        if opcode in (0x44, 0x4C, 0x54, 0x5C, 0x64, 0x6C, 0x74, 0x7C):
            old_a = self.a
            self.a = 0
            self._sub_a(old_a)
            return 8
        if opcode in (0x45, 0x4D, 0x55, 0x5D, 0x65, 0x6D, 0x75, 0x7D):
            self.pc = self._pop_word()
            self.wz = self.pc
            self.iff1 = self.iff2
            return 14
        if opcode in (0x46, 0x4E, 0x66, 0x6E):
            self.interrupt_mode = 0
            return 8
        if opcode in (0x56, 0x76):
            self.interrupt_mode = 1
            return 8
        if opcode in (0x5E, 0x7E):
            self.interrupt_mode = 2
            return 8
        if opcode in (0x67, 0x6F):
            address = self.hl
            memory_value = self.bus.read(address)
            if opcode == 0x67:
                new_memory = ((self.a & 0x0F) << 4) | (memory_value >> 4)
                self.a = (self.a & 0xF0) | (memory_value & 0x0F)
            else:
                new_memory = ((memory_value << 4) | (self.a & 0x0F)) & 0xFF
                self.a = (self.a & 0xF0) | (memory_value >> 4)
            self.bus.write(address, new_memory)
            self.wz = (address + 1) & 0xFFFF
            self.f = (self.f & 1) | (self.a & 0xA8) | self._parity_flag(self.a)
            if self.a == 0:
                self.f |= 0x40
            return 18
        if opcode in (0xA0, 0xA8, 0xB0, 0xB8):
            direction = -1 if opcode in (0xA8, 0xB8) else 1
            value = self.bus.read(self.hl)
            self.bus.write(self.de, value)
            self.hl = (self.hl + direction) & 0xFFFF
            self.de = (self.de + direction) & 0xFFFF
            self.bc = (self.bc - 1) & 0xFFFF
            total = (self.a + value) & 0xFF
            self.f = (
                (self.f & 0xC1)
                | (0x04 if self.bc else 0)
                | (total & 0x08)
                | ((total & 0x02) << 4)
            )
            if opcode in (0xB0, 0xB8) and self.bc:
                self.pc = (self.pc - 2) & 0xFFFF
                self.wz = (self.pc + 1) & 0xFFFF
                self._set_repeat_block_xy_flags()
                return 21
            return 16
        if opcode in (0xA1, 0xA9, 0xB1, 0xB9):
            direction = -1 if opcode in (0xA9, 0xB9) else 1
            value = self.bus.read(self.hl)
            result = (self.a - value) & 0xFF
            half = (self.a & 0x0F) < (value & 0x0F)
            self.hl = (self.hl + direction) & 0xFFFF
            self.bc = (self.bc - 1) & 0xFFFF
            adjusted = (result - int(half)) & 0xFF
            self.f = (
                (self.f & 1)
                | 0x02
                | (result & 0x80)
                | (0x40 if result == 0 else 0)
                | (0x10 if half else 0)
                | (0x04 if self.bc else 0)
                | (adjusted & 0x08)
                | ((adjusted & 0x02) << 4)
            )
            self.wz = (self.wz + direction) & 0xFFFF
            if opcode in (0xB1, 0xB9) and self.bc and result:
                self.pc = (self.pc - 2) & 0xFFFF
                self.wz = (self.pc + 1) & 0xFFFF
                self._set_repeat_block_xy_flags()
                return 21
            return 16
        if opcode in (0xA2, 0xAA, 0xB2, 0xBA, 0xA3, 0xAB, 0xB3, 0xBB):
            return self._execute_block_io(opcode)
        # No Z80 clássico, bytes ED sem operação atribuída comportam-se como
        # ED-NOP: dois fetches M1, incremento de R por fetch e 8 T-states.
        # Isso não cobre extensões específicas de clones/variantes.
        return 8

    def _set_repeat_block_xy_flags(self) -> None:
        """Use PC.11/PC.13 for F3/F5 on a repeating ED block iteration."""
        self.f = (self.f & ~0x28) | ((self.pc >> 8) & 0x28)

    def _set_repeat_block_io_flags(self, value: int) -> None:
        """Apply the additional flags observed on repeating block-I/O iterations."""
        self.f = (self.f & ~0x28) | ((self.pc >> 8) & 0x28)
        carry = bool(self.f & 0x01)
        pv = bool(self.f & 0x04)
        if carry:
            if value & 0x80:
                pv = pv ^ bool(self._parity_flag((self.b - 1) & 0x07)) ^ True
                half = (self.b & 0x0F) == 0
            else:
                pv = pv ^ bool(self._parity_flag((self.b + 1) & 0x07)) ^ True
                half = (self.b & 0x0F) == 0x0F
        else:
            pv = pv ^ bool(self._parity_flag(self.b & 0x07)) ^ True
            half = False
        self.f = (self.f & ~(0x10 | 0x04)) | (0x10 if half else 0) | (0x04 if pv else 0)

    def _execute_block_io(self, opcode: int) -> int:
        """Executa INI/IND/INIR/INDR e OUTI/OUTD/OTIR/OTDR."""
        input_direction = opcode in (0xA2, 0xAA, 0xB2, 0xBA)
        repeat = opcode in (0xB2, 0xBA, 0xB3, 0xBB)
        direction = -1 if opcode in (0xAA, 0xBA, 0xAB, 0xBB) else 1
        old_bc = self.bc
        if input_direction:
            value = self._read_port(old_bc)
            self.bus.write(self.hl, value)
        else:
            value = self.bus.read(self.hl)
            output_bc = (((self.b - 1) & 0xFF) << 8) | self.c
            self._write_port(output_bc, value)
        self.hl = (self.hl + direction) & 0xFFFF
        self.b = (self.b - 1) & 0xFF
        k = value + (((self.c + direction) & 0xFF) if input_direction else self.l)
        flags = self.b & 0xA8
        if self.b == 0:
            flags |= 0x40
        if value & 0x80:
            flags |= 0x02
        if k > 0xFF:
            flags |= 0x11
        flags |= self._parity_flag(((k & 7) ^ self.b) & 0xFF)
        self.f = flags
        if repeat and self.b:
            self.pc = (self.pc - 2) & 0xFFFF
            self.wz = (self.pc + 1) & 0xFFFF
            self._set_repeat_block_io_flags(value)
            return 21
        wz_base = old_bc if input_direction else self.bc
        self.wz = (wz_base + direction) & 0xFFFF
        return 16

    @staticmethod
    def _signed_byte(value: int) -> int:
        return value - 0x100 if value & 0x80 else value

    def _index_value(self, prefix: int) -> int:
        return self.ix if prefix == 0xDD else self.iy

    def _set_index_value(self, prefix: int, value: int) -> None:
        if prefix == 0xDD:
            self.ix = value & 0xFFFF
        else:
            self.iy = value & 0xFFFF

    def _read_index_register(self, prefix: int, code: int, halves: bool = True) -> int:
        index = self._index_value(prefix)
        if halves and code == 4:
            return index >> 8
        if halves and code == 5:
            return index & 0xFF
        return self._read_register(code)

    def _write_index_register(
        self, prefix: int, code: int, value: int, halves: bool = True
    ) -> None:
        index = self._index_value(prefix)
        value &= 0xFF
        if halves and code == 4:
            self._set_index_value(prefix, (value << 8) | (index & 0xFF))
        elif halves and code == 5:
            self._set_index_value(prefix, (index & 0xFF00) | value)
        else:
            self._write_register(code, value)

    def _step_indexed(self, prefix: int) -> int:
        """Executa instruções DD/FD e trata prefixos ignorados com custo adicional."""
        opcode = self._fetch_opcode_byte()
        index = self._index_value(prefix)
        cycles = 0
        if opcode in (0xDD, 0xFD):
            self.pc = (self.pc - 1) & 0xFFFF
            self.r = (self.r & 0x80) | ((self.r - 1) & 0x7F)
            base_cycles = self.step()
            self.total_cycles += 4
            return base_cycles + 4
        if opcode == 0xCB:
            displacement = self._signed_byte(self._fetch_byte())
            cb_opcode = self._fetch_byte()
            address = (index + displacement) & 0xFFFF
            self.wz = address
            value = self.bus.read(address)
            operation = cb_opcode >> 6
            bit_index = (cb_opcode >> 3) & 7
            register = cb_opcode & 7
            if operation == 0:
                carry_in = self.f & 1
                if bit_index == 0:
                    carry = value >> 7
                    result = ((value << 1) | carry) & 0xFF
                elif bit_index == 1:
                    carry = value & 1
                    result = (value >> 1) | (carry << 7)
                elif bit_index == 2:
                    carry = value >> 7
                    result = ((value << 1) | carry_in) & 0xFF
                elif bit_index == 3:
                    carry = value & 1
                    result = (value >> 1) | (carry_in << 7)
                elif bit_index == 4:
                    carry = value >> 7
                    result = (value << 1) & 0xFF
                elif bit_index == 5:
                    carry = value & 1
                    result = (value >> 1) | (value & 0x80)
                elif bit_index == 6:
                    carry = value >> 7
                    result = ((value << 1) | 1) & 0xFF
                else:
                    carry = value & 1
                    result = value >> 1
                self.bus.write(address, result)
                if register != 6:
                    self._write_register(register, result)
                self.f = (result & 0xA8) | self._parity_flag(result) | carry
                if result == 0:
                    self.f |= 0x40
                cycles = 23
            elif operation == 1:
                zero = not (value & (1 << bit_index))
                flags = (self.f & 1) | 0x10 | ((address >> 8) & 0x28)
                if zero:
                    flags |= 0x44
                if bit_index == 7 and not zero:
                    flags |= 0x80
                self.f = flags
                cycles = 20
            else:
                result = (
                    value & ~(1 << bit_index)
                    if operation == 2
                    else value | (1 << bit_index)
                )
                self.bus.write(address, result)
                if register != 6:
                    self._write_register(register, result)
                cycles = 23
        elif opcode in (
            0x21,
            0x22,
            0x2A,
            0x23,
            0x2B,
            0x09,
            0x19,
            0x29,
            0x39,
            0xE1,
            0xE5,
            0xE3,
            0xE9,
            0xF9,
        ):
            if opcode == 0x21:
                low = self._fetch_byte()
                high = self._fetch_byte()
                self._set_index_value(prefix, low | (high << 8))
                cycles = 14
            elif opcode in (0x22, 0x2A):
                address = self._fetch_byte() | (self._fetch_byte() << 8)
                self.wz = (address + 1) & 0xFFFF
                if opcode == 0x22:
                    self.bus.write(address, index & 0xFF)
                    self.bus.write((address + 1) & 0xFFFF, index >> 8)
                else:
                    low = self.bus.read(address)
                    high = self.bus.read((address + 1) & 0xFFFF)
                    self._set_index_value(prefix, (high << 8) | low)
                cycles = 20
            elif opcode in (0x23, 0x2B):
                self._set_index_value(prefix, index + (1 if opcode == 0x23 else -1))
                cycles = 10
            elif opcode in (0x09, 0x19, 0x29, 0x39):
                code = (opcode >> 4) & 3
                operand = index if code == 2 else self._read_stack_pair(code)
                self.wz = (index + 1) & 0xFFFF
                result = index + operand
                flags = self.f & 0xC4
                flags |= ((result & 0xFFFF) >> 8) & 0x28
                if (index & 0x0FFF) + (operand & 0x0FFF) > 0x0FFF:
                    flags |= 0x10
                if result > 0xFFFF:
                    flags |= 1
                self._set_index_value(prefix, result)
                self.f = flags
                cycles = 15
            elif opcode == 0xE1:
                self._set_index_value(prefix, self._pop_word())
                cycles = 14
            elif opcode == 0xE5:
                self._push_word(index)
                cycles = 15
            elif opcode == 0xE3:
                low = self.bus.read(self.sp)
                high_address = (self.sp + 1) & 0xFFFF
                high = self.bus.read(high_address)
                self.bus.write(high_address, index >> 8)
                self.bus.write(self.sp, index & 0xFF)
                self._set_index_value(prefix, (high << 8) | low)
                self.wz = (high << 8) | low
                cycles = 23
            elif opcode == 0xE9:
                self.pc = index
                cycles = 8
            else:
                self.sp = index
                cycles = 10
        elif opcode in (0x24, 0x25, 0x26, 0x2C, 0x2D, 0x2E):
            code = 4 if opcode in (0x24, 0x25, 0x26) else 5
            if opcode in (0x26, 0x2E):
                self._write_index_register(prefix, code, self._fetch_byte())
                cycles = 11
            else:
                old = self._read_index_register(prefix, code)
                value = (
                    self._inc_register_value(old)
                    if opcode in (0x24, 0x2C)
                    else self._dec_register_value(old)
                )
                self._write_index_register(prefix, code, value)
                cycles = 8
        elif opcode == 0x36:
            displacement = self._signed_byte(self._fetch_byte())
            value = self._fetch_byte()
            address = (index + displacement) & 0xFFFF
            self.wz = address
            self.bus.write(address, value)
            cycles = 19
        elif opcode in (0x34, 0x35):
            displacement = self._signed_byte(self._fetch_byte())
            address = (index + displacement) & 0xFFFF
            self.wz = address
            old = self.bus.read(address)
            value = (
                self._inc_register_value(old)
                if opcode == 0x34
                else self._dec_register_value(old)
            )
            self.bus.write(address, value)
            cycles = 23
        elif 0x40 <= opcode <= 0x7F and opcode != 0x76:
            destination, source = (opcode >> 3) & 7, opcode & 7
            if destination == 6 or source == 6:
                displacement = self._signed_byte(self._fetch_byte())
                address = (index + displacement) & 0xFFFF
                self.wz = address
                if source == 6:
                    value = self.bus.read(address)
                    self._write_index_register(prefix, destination, value, halves=False)
                else:
                    value = self._read_index_register(prefix, source, halves=False)
                    self.bus.write(address, value)
                cycles = 19
            else:
                value = self._read_index_register(prefix, source)
                self._write_index_register(prefix, destination, value)
                cycles = 8
        elif 0x80 <= opcode <= 0xBF:
            source = opcode & 7
            if source == 6:
                displacement = self._signed_byte(self._fetch_byte())
                address = (index + displacement) & 0xFFFF
                self.wz = address
                value = self.bus.read(address)
                cycles = 19
            else:
                value = self._read_index_register(prefix, source)
                cycles = 8
            operation = (opcode >> 3) & 7
            if operation == 0:
                self._add_a(value)
            elif operation == 1:
                self._adc_a(value)
            elif operation == 2:
                self._sub_a(value)
            elif operation == 3:
                self._sbc_a(value)
            elif operation == 4:
                self._and_a(value)
            elif operation == 5:
                self._xor_a(value)
            elif operation == 6:
                self._or_a(value)
            else:
                self._compare_a(value)
        else:
            # O prefixo DD/FD não altera instruções que não usam HL/indexação.
            self.pc = (self.pc - 1) & 0xFFFF
            self.r = (self.r & 0x80) | ((self.r - 1) & 0x7F)
            base_cycles = self.step()
            self.total_cycles += 4
            return base_cycles + 4
        self.total_cycles += cycles
        if self._ei_delay > 0:
            self._ei_delay -= 1
        return cycles

    def step(self) -> int:
        """Executa uma instrução mantendo o latch Q entre instruções."""
        if self._step_depth:
            return self._step_impl()
        self._q_previous = self.q
        self.q = 0
        self._step_depth = 1
        try:
            return self._step_impl()
        finally:
            self._step_depth = 0

    def _step_impl(self) -> int:
        """Executa uma instrução implementada e retorna seus T-states nominais.

        Famílias suportadas no marco atual:
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
        - LD (nn),rr / LD rr,(nn) (ED 43h/53h/63h/73h e ED 4Bh/5Bh/6Bh/7Bh)
        - EX (SP),HL (E3h)
        - LD I,A (ED 47h), LD R,A (ED 4Fh), LD A,I (ED 57h), LD A,R (ED 5Fh)
        - CP r (B8h–BFh)
        - Prefixo CB: rotações, shifts, BIT, RES e SET para registradores e (HL).
        - Prefixos DD/FD: IX/IY, operações indexadas comuns e DD/FD-CB.
        - Prefixo ED: I/O por (C), operações de bloco, RRD/RLD, IM, RETN/RETI,
          transferências de palavras e registradores especiais.
        - IN/OUT imediatos, HALT, DI/EI e solicitações iniciais de NMI/INT.

        INC/DEC de pares e instruções de controle preservam F conforme a
        arquitetura; POP AF restaura F. ADD HL preserva S, Z e P/V.
        """
        if self._nmi_pending:
            # NMI encerra a condição de "instrução anterior LD A,I/R", mas
            # não aplica o erratum de PV reservado ao INT mascarável.
            self.p = 0
            # O acknowledge de interrupção ocupa um ciclo M1 e incrementa R.
            self.r = (self.r & 0x80) | ((self.r + 1) & 0x7F)
            self._nmi_pending = False
            self.halted = False
            self.iff2 = self.iff1
            self.iff1 = False
            self._push_word(self.pc)
            self.pc = 0x0066
            cycles = 11
            self.total_cycles += cycles
            return cycles
        if self._int_pending is not None and self.iff1 and self._ei_delay == 0:
            # No Z80 clássico, INT imediatamente após LD A,I/R força PV=0.
            if self.p:
                self.f &= ~0x04
            self.p = 0
            data_bus = self._int_pending
            # O acknowledge de INT incrementa R uma vez, inclusive em IM 0.
            self.r = (self.r & 0x80) | ((self.r + 1) & 0x7F)
            self._int_pending = None
            self.halted = False
            self.iff1 = self.iff2 = False
            if self.interrupt_mode == 0:
                # Em IM 0, o dispositivo fornece o primeiro opcode. Os bytes
                # seguintes (operandos/prefixos) continuam vindo de PC.
                # O acknowledge substitui o M1 normal e custa dois T-states
                # adicionais sobre a duração nominal da instrução decodificada.
                self._im0_opcode = data_bus
                cycles = self.step() + 2
                self.total_cycles += 2
                return cycles
            self._push_word(self.pc)
            if self.interrupt_mode == 2:
                vector = (self.i << 8) | data_bus
                low = self.bus.read(vector)
                high = self.bus.read((vector + 1) & 0xFFFF)
                self.pc = (high << 8) | low
                cycles = 19
            else:
                self.pc = 0x0038
                cycles = 13
            self.total_cycles += cycles
            return cycles
        # Qualquer instrução normal substitui a condição anterior de LD A,I/R;
        # a própria instrução ED 57/5F pode armar P novamente.
        self.p = 0
        if self.halted:
            self.r = (self.r & 0x80) | ((self.r + 1) & 0x7F)
            self.total_cycles += 4
            return 4
        opcode = self._fetch_opcode_byte()
        if opcode in (0xDD, 0xFD):
            return self._step_indexed(opcode)

        # These two opcodes dominate Wonder Boy's hot loop. Handle them before
        # the general decoder so they do not scan the long conditional chain.
        if opcode == 0x20:  # JR NZ,e
            displacement = self._fetch_byte()
            cycles = 7
            if not self._f & 0x40:
                if displacement & 0x80:
                    displacement -= 0x100
                self._pc = (self._pc + displacement) & 0xFFFF
                self.wz = self._pc
                cycles = 12
            self.total_cycles += cycles
            if self._ei_delay > 0:
                self._ei_delay -= 1
            return cycles
        if opcode == 0x21:  # LD HL,nn
            low = self._fetch_byte()
            high = self._fetch_byte()
            self.h = high
            self.l = low
            cycles = 10
            self.total_cycles += cycles
            if self._ei_delay > 0:
                self._ei_delay -= 1
            return cycles

        if opcode == 0x00:  # NOP
            cycles = 4
        elif opcode == 0xED:
            cycles = self._execute_ed(self._fetch_opcode_byte())
        elif opcode in (0xD3, 0xDB):  # OUT (n),A / IN A,(n)
            port_low = self._fetch_byte()
            port = (self.a << 8) | port_low
            if opcode == 0xD3:
                self._write_port(port, self.a)
                self.wz = (port & 0xFF00) | ((port_low + 1) & 0xFF)
            else:
                self.a = self._read_port(port)
                self.wz = (port + 1) & 0xFFFF
            cycles = 11
        elif opcode == 0xCB:  # família CB
            cycles = self._execute_cb(self._fetch_opcode_byte())
        elif opcode == 0x76:  # HALT
            self.halted = True
            cycles = 4
        elif 0x40 <= opcode <= 0xBF:  # LD r,r' e grupo ALU A,r
            if opcode < 0x80:
                destination = (opcode >> 3) & 0x07
                source = opcode & 0x07
                self._write_register(destination, self._read_register(source))
                cycles = 7 if destination == 6 or source == 6 else 4
            else:
                source = opcode & 0x07
                operation = (opcode >> 3) & 0x07
                value = self._read_register(source)
                if operation == 0:
                    self._add_a(value)
                elif operation == 1:
                    self._adc_a(value)
                elif operation == 2:
                    self._sub_a(value)
                elif operation == 3:
                    self._sbc_a(value)
                elif operation == 4:
                    self._and_a(value)
                elif operation == 5:
                    self._xor_a(value)
                elif operation == 6:
                    self._or_a(value)
                else:
                    self._compare_a(value)
                cycles = 7 if source == 6 else 4
        elif opcode == 0xF3:  # DI
            self.iff1 = self.iff2 = False
            self._ei_delay = 0
            cycles = 4
        elif opcode == 0xFB:  # EI: reconhecimento após a instrução seguinte
            self.iff1 = self.iff2 = True
            self._ei_delay = 2
            cycles = 4
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
            return_address = self.pc
            self._push_word(return_address)
            self.pc = opcode & 0x38
            self.wz = return_address
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
            self.wz = (self.a << 8) | ((address + 1) & 0xFF)
            cycles = 7
        elif opcode in (0x0A, 0x1A):  # LD A,(BC/DE)
            address = self.bc if opcode == 0x0A else self.de
            self.a = self.bus.read(address)
            self.wz = (address + 1) & 0xFFFF
            cycles = 7
        elif opcode in (0x22, 0x2A):  # LD (nn),HL / LD HL,(nn)
            low = self._fetch_byte()
            high = self._fetch_byte()
            address = (high << 8) | low
            next_address = (address + 1) & 0xFFFF
            self.wz = next_address
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
                self.wz = (self.a << 8) | ((address + 1) & 0xFF)
            else:
                self.a = self.bus.read(address)
                self.wz = (address + 1) & 0xFFFF
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
        elif opcode == 0xE3:  # EX (SP),HL
            low_address = self.sp
            high_address = (self.sp + 1) & 0xFFFF
            low_value = self.bus.read(low_address)
            high_value = self.bus.read(high_address)
            previous_hl = self.hl
            self.bus.write(high_address, previous_hl >> 8)
            self.bus.write(low_address, previous_hl & 0xFF)
            self.hl = (high_value << 8) | low_value
            self.wz = self.hl
            cycles = 19
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
            self.wz = self.pc
            cycles = 10
        elif opcode & 0xC7 == 0xC2:  # JP cc,nn
            low = self._fetch_byte()
            high = self._fetch_byte()
            target = (high << 8) | low
            self.wz = target
            if self._condition((opcode >> 3) & 0x07):
                self.pc = target
            cycles = 10
        elif opcode == 0x2F:  # CPL
            self.a ^= 0xFF
            self.f = (self.f & 0xC5) | (self.a & 0x28) | 0x12
            cycles = 4
        elif opcode == 0x37:  # SCF
            undocumented = (self.a | (self._q_previous ^ self.f)) & 0x28
            self.f = (self.f & 0xC4) | undocumented | 0x01
            cycles = 4
        elif opcode == 0x3F:  # CCF
            previous_carry = self.f & 0x01
            undocumented = (self.a | (self._q_previous ^ self.f)) & 0x28
            self.f = (self.f & 0xC4) | undocumented
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
                self.wz = self.pc
                cycles = 13
            else:
                cycles = 8
        elif opcode == 0x18:  # JR e
            displacement = self._fetch_byte()
            if displacement & 0x80:
                displacement -= 0x100
            self.pc = (self.pc + displacement) & 0xFFFF
            self.wz = self.pc
            cycles = 12
        elif opcode in (0x20, 0x28, 0x30, 0x38):  # JR cc,e
            displacement = self._fetch_byte()
            if self._condition((opcode >> 3) & 0x03):
                if displacement & 0x80:
                    displacement -= 0x100
                self.pc = (self.pc + displacement) & 0xFFFF
                self.wz = self.pc
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
            self.wz = self.pc
            cycles = 17
        elif opcode & 0xC7 == 0xC4:  # CALL cc,nn
            low = self._fetch_byte()
            high = self._fetch_byte()
            target = (high << 8) | low
            self.wz = target
            if self._condition((opcode >> 3) & 0x07):
                self._push_word(self.pc)
                self.pc = target
                cycles = 17
            else:
                cycles = 10
        elif opcode == 0xC9:  # RET
            self.pc = self._pop_word()
            self.wz = self.pc
            cycles = 10
        elif opcode & 0xC7 == 0xC0:  # RET cc
            if self._condition((opcode >> 3) & 0x07):
                self.pc = self._pop_word()
                self.wz = self.pc
                cycles = 11
            else:
                cycles = 5
        else:
            raise UnsupportedOpcodeError(
                f"Opcode Z80 {opcode:02X}h não implementado "
                f"no endereço {(self.pc - 1) & 0xFFFF:04X}h."
            )

        self.total_cycles += cycles
        if self._ei_delay > 0:
            self._ei_delay -= 1
        return cycles
