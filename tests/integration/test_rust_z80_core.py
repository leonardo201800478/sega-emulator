from __future__ import annotations

import pytest

from sega_emulator.cpu.z80 import Z80Cpu
from sega_emulator.machine.sg1000 import SG1000Machine


class MemoryBus:
    def __init__(self, program: bytes) -> None:
        self.memory = bytearray(0x10000)
        self.memory[: len(program)] = program

    def read(self, address: int) -> int:
        return self.memory[address & 0xFFFF]

    def write(self, address: int, value: int) -> None:
        self.memory[address & 0xFFFF] = value & 0xFF


def _rust_cpu():
    native = pytest.importorskip("sega_z80_core")
    return native.RustZ80Cpu


class PortBus:
    def __init__(self) -> None:
        self.ports: dict[int, int] = {}
        self.writes: list[tuple[int, int]] = []

    def read_port(self, address: int) -> int:
        return self.ports.get(address & 0xFFFF, 0xA5)

    def write_port(self, address: int, value: int) -> None:
        self.ports[address & 0xFFFF] = value & 0xFF
        self.writes.append((address & 0xFFFF, value & 0xFF))


def test_rust_core_matches_python_for_supported_instruction_sequence() -> None:
    RustZ80Cpu = _rust_cpu()
    program = bytes(
        [
            0x31,
            0x00,
            0xF0,  # LD SP, F000
            0x21,
            0x00,
            0x20,  # LD HL, 2000
            0x3E,
            0x7F,  # LD A, 7F
            0x06,
            0x01,  # LD B, 01
            0x80,  # ADD A, B
            0x77,  # LD (HL), A
            0x7E,  # LD A, (HL)
            0xFE,
            0x80,  # CP 80
            0x20,
            0x02,  # JR NZ, +2 (not taken)
            0x76,  # HALT
        ]
    )
    python_bus = MemoryBus(program)
    rust_bus = MemoryBus(program)
    python_cpu = Z80Cpu(python_bus)
    rust_cpu = RustZ80Cpu(rust_bus, None)

    attributes = (
        "a",
        "f",
        "b",
        "c",
        "d",
        "e",
        "h",
        "l",
        "sp",
        "pc",
        "ix",
        "iy",
        "i",
        "r",
        "iff1",
        "iff2",
        "interrupt_mode",
        "halted",
        "wz",
        "total_cycles",
        "q",
        "_q_previous",
        "p",
    )
    for _ in range(20):
        python_cycles = python_cpu.step()
        rust_cycles = rust_cpu.step()
        assert rust_cycles == python_cycles
        assert {name: getattr(rust_cpu, name) for name in attributes} == {
            name: getattr(python_cpu, name) for name in attributes
        }
        assert rust_bus.memory == python_bus.memory
        if python_cpu.halted:
            break
    assert python_cpu.halted
    assert rust_cpu.halted


@pytest.mark.parametrize(
    ("opcode", "byte_count", "expected_cycles"),
    [
        (0xA0, 1, 16),  # LDI
        (0xA8, 1, 16),  # LDD
        (0xB0, 2, 21),  # LDIR repeats while BC != 0
        (0xB8, 2, 21),  # LDDR repeats while BC != 0
    ],
)
def test_rust_block_transfer_matches_python(
    opcode: int, byte_count: int, expected_cycles: int
) -> None:
    RustZ80Cpu = _rust_cpu()
    program = bytes([0xED, opcode])
    python_bus = MemoryBus(program)
    rust_bus = MemoryBus(program)
    source_data = bytes(range(0x41, 0x41 + byte_count))
    python_bus.memory[0x1000 : 0x1000 + byte_count] = source_data
    rust_bus.memory[0x1000 : 0x1000 + byte_count] = source_data
    python_cpu = Z80Cpu(python_bus)
    rust_cpu = RustZ80Cpu(rust_bus, None)

    initial_bc = byte_count
    python_cpu.a = rust_cpu.a = 0x10
    python_cpu.f = rust_cpu.f = 0xC1
    python_cpu.hl = rust_cpu.hl = 0x1000
    python_cpu.de = rust_cpu.de = 0x2000
    python_cpu.bc = rust_cpu.bc = initial_bc

    python_cycles = python_cpu.step()
    rust_cycles = rust_cpu.step()

    attributes = (
        "a",
        "f",
        "b",
        "c",
        "d",
        "e",
        "h",
        "l",
        "pc",
        "r",
        "wz",
        "total_cycles",
        "q",
        "_q_previous",
        "p",
    )
    assert python_cycles == rust_cycles == expected_cycles
    assert {name: getattr(rust_cpu, name) for name in attributes} == {
        name: getattr(python_cpu, name) for name in attributes
    }
    assert rust_cpu.bc == python_cpu.bc
    assert rust_cpu.de == python_cpu.de
    assert rust_cpu.hl == python_cpu.hl
    assert rust_bus.memory == python_bus.memory


@pytest.mark.parametrize("opcode", [0x00, 0x77, 0x7F, 0x80, 0xA4, 0xC0])
def test_rust_unassigned_ed_opcode_matches_classic_z80_nop_behavior(
    opcode: int,
) -> None:
    RustZ80Cpu = _rust_cpu()
    program = bytes([0xED, opcode])
    python_cpu = Z80Cpu(MemoryBus(program))
    rust_cpu = RustZ80Cpu(MemoryBus(program), None)

    assert python_cpu.step() == rust_cpu.step() == 8
    assert python_cpu.pc == rust_cpu.pc == 2
    assert python_cpu.r == rust_cpu.r == 2
    assert python_cpu.total_cycles == rust_cpu.total_cycles == 8


def test_rust_core_fails_explicitly_on_assigned_but_unsupported_ed_opcode() -> None:
    RustZ80Cpu = _rust_cpu()
    cpu = RustZ80Cpu(MemoryBus(bytes([0xED, 0x42])), None)
    with pytest.raises(NotImplementedError, match="does not support ED opcode 0x42"):
        cpu.step()


@pytest.mark.parametrize("opcode", range(256))
def test_rust_primary_opcode_map_matches_python(opcode: int) -> None:
    RustZ80Cpu = _rust_cpu()
    program = bytes([opcode, 0x00, 0x00, 0x00, 0x00, 0x00])
    python_bus = MemoryBus(program)
    rust_bus = MemoryBus(program)
    python_io = PortBus()
    rust_io = PortBus()
    python_cpu = Z80Cpu(python_bus, python_io)
    rust_cpu = RustZ80Cpu(rust_bus, rust_io)
    for cpu in (python_cpu, rust_cpu):
        cpu.a = 0x69
        cpu.f = 0xA5
        cpu.b, cpu.c = 0x12, 0x34
        cpu.d, cpu.e = 0x56, 0x78
        cpu.h, cpu.l = 0x20, 0x00
        cpu.ix = 0x2100
        cpu.iy = 0x2200
        cpu.sp = 0x3000
        cpu.af_alt = 0x2468
        cpu.bc_alt = 0x1357
        cpu.de_alt = 0xFEDC
        cpu.hl_alt = 0xBEEF
        cpu.wz = 0x2800

    python_cycles = python_cpu.step()
    rust_cycles = rust_cpu.step()
    attributes = (
        "a",
        "f",
        "b",
        "c",
        "d",
        "e",
        "h",
        "l",
        "sp",
        "pc",
        "ix",
        "iy",
        "i",
        "r",
        "iff1",
        "iff2",
        "interrupt_mode",
        "halted",
        "wz",
        "af_alt",
        "bc_alt",
        "de_alt",
        "hl_alt",
        "total_cycles",
        "q",
        "_q_previous",
        "p",
    )
    assert rust_cycles == python_cycles
    assert {name: getattr(rust_cpu, name) for name in attributes} == {
        name: getattr(python_cpu, name) for name in attributes
    }
    assert rust_bus.memory == python_bus.memory
    assert rust_io.ports == python_io.ports
    assert rust_io.writes == python_io.writes


@pytest.mark.parametrize("prefix", [0xDD, 0xFD])
@pytest.mark.parametrize(
    ("opcode", "operands"),
    [
        (0x21, (0x34, 0x12)),  # LD IX/IY,nn
        (0x22, (0x00, 0x30)),  # LD (nn),IX/IY
        (0x2A, (0x00, 0x30)),  # LD IX/IY,(nn)
        (0x23, ()),
        (0x2B, ()),  # INC/DEC IX/IY
        (0x09, ()),
        (0x19, ()),
        (0x29, ()),
        (0x39, ()),  # ADD IX/IY,rr
        (0x24, ()),
        (0x25, ()),
        (0x26, (0x55,)),  # IXH/IYH
        (0x2C, ()),
        (0x2D, ()),
        (0x2E, (0xAA,)),  # IXL/IYL
        (0x34, (0x05,)),
        (0x35, (0x05,)),  # INC/DEC (IX/IY+d)
        (0x36, (0x05, 0x77)),  # LD (IX/IY+d),n
        (0xE1, ()),
        (0xE5, ()),
        (0xE3, ()),  # POP/PUSH/EX (SP),IX/IY
        (0xE9, ()),
        (0xF9, ()),  # JP (IX/IY), LD SP,IX/IY
        (0x00, ()),  # ignored prefix
    ],
)
def test_rust_indexed_special_instructions_match_python(
    prefix: int, opcode: int, operands: tuple[int, ...]
) -> None:
    RustZ80Cpu = _rust_cpu()
    program = bytes([prefix, opcode, *operands])
    python_bus = MemoryBus(program)
    rust_bus = MemoryBus(program)
    python_bus.memory[0x2005] = rust_bus.memory[0x2005] = 0x7F
    python_bus.memory[0x3000:0x3002] = bytes([0x34, 0x12])
    rust_bus.memory[0x3000:0x3002] = bytes([0x34, 0x12])
    python_cpu = Z80Cpu(python_bus)
    rust_cpu = RustZ80Cpu(rust_bus, None)
    for cpu in (python_cpu, rust_cpu):
        cpu.a = 0x69
        cpu.f = 0xA5
        cpu.bc = 0x1234
        cpu.de = 0x5678
        cpu.hl = 0x2200
        cpu.ix = 0x2000
        cpu.iy = 0x2100
        cpu.sp = 0x3000
        cpu.wz = 0x2800

    python_cycles = python_cpu.step()
    rust_cycles = rust_cpu.step()
    attributes = (
        "a",
        "f",
        "b",
        "c",
        "d",
        "e",
        "h",
        "l",
        "sp",
        "pc",
        "ix",
        "iy",
        "i",
        "r",
        "iff1",
        "iff2",
        "interrupt_mode",
        "halted",
        "wz",
        "total_cycles",
        "q",
        "_q_previous",
        "p",
    )
    assert rust_cycles == python_cycles
    assert {name: getattr(rust_cpu, name) for name in attributes} == {
        name: getattr(python_cpu, name) for name in attributes
    }
    assert rust_bus.memory == python_bus.memory


@pytest.mark.parametrize("prefix", [0xDD, 0xFD])
@pytest.mark.parametrize("opcode", range(0x40, 0xC0))
def test_rust_indexed_register_and_alu_families_match_python(
    prefix: int, opcode: int
) -> None:
    RustZ80Cpu = _rust_cpu()
    uses_displacement = (
        0x40 <= opcode <= 0x7F
        and opcode != 0x76
        and (opcode & 7 == 6 or (opcode >> 3) & 7 == 6)
    ) or (0x80 <= opcode <= 0xBF and opcode & 7 == 6)
    program = bytes([prefix, opcode, *([0x05] if uses_displacement else [])])
    python_bus = MemoryBus(program)
    rust_bus = MemoryBus(program)
    python_bus.memory[0x2005] = rust_bus.memory[0x2005] = 0x96
    python_cpu = Z80Cpu(python_bus)
    rust_cpu = RustZ80Cpu(rust_bus, None)
    for cpu in (python_cpu, rust_cpu):
        cpu.a = 0x69
        cpu.f = 0xA5
        cpu.bc = 0x1234
        cpu.de = 0x5678
        cpu.hl = 0x2200
        cpu.ix = 0x2000
        cpu.iy = 0x2100
        cpu.sp = 0x3000
        cpu.wz = 0x2800

    python_cycles = python_cpu.step()
    rust_cycles = rust_cpu.step()
    attributes = (
        "a",
        "f",
        "b",
        "c",
        "d",
        "e",
        "h",
        "l",
        "sp",
        "pc",
        "ix",
        "iy",
        "i",
        "r",
        "iff1",
        "iff2",
        "interrupt_mode",
        "halted",
        "wz",
        "total_cycles",
        "q",
        "_q_previous",
        "p",
    )
    assert rust_cycles == python_cycles
    assert {name: getattr(rust_cpu, name) for name in attributes} == {
        name: getattr(python_cpu, name) for name in attributes
    }
    assert rust_bus.memory == python_bus.memory


@pytest.mark.parametrize("prefix", [0xDD, 0xFD])
@pytest.mark.parametrize("cb_opcode", range(256))
def test_rust_indexed_cb_opcode_family_matches_python(
    prefix: int, cb_opcode: int
) -> None:
    RustZ80Cpu = _rust_cpu()
    program = bytes([prefix, 0xCB, 0x05, cb_opcode])
    python_bus = MemoryBus(program)
    rust_bus = MemoryBus(program)
    python_bus.memory[0x2005] = rust_bus.memory[0x2005] = 0x96
    python_cpu = Z80Cpu(python_bus)
    rust_cpu = RustZ80Cpu(rust_bus, None)
    for cpu in (python_cpu, rust_cpu):
        cpu.a = 0x69
        cpu.f = 0xA5
        cpu.bc = 0x1234
        cpu.de = 0x5678
        cpu.hl = 0x2200
        cpu.ix = 0x2000
        cpu.iy = 0x2100
        cpu.wz = 0x2800

    python_cycles = python_cpu.step()
    rust_cycles = rust_cpu.step()
    attributes = (
        "a",
        "f",
        "b",
        "c",
        "d",
        "e",
        "h",
        "l",
        "sp",
        "pc",
        "ix",
        "iy",
        "i",
        "r",
        "iff1",
        "iff2",
        "interrupt_mode",
        "halted",
        "wz",
        "total_cycles",
        "q",
        "_q_previous",
        "p",
    )
    assert rust_cycles == python_cycles
    assert {name: getattr(rust_cpu, name) for name in attributes} == {
        name: getattr(python_cpu, name) for name in attributes
    }
    assert rust_bus.memory == python_bus.memory


@pytest.mark.parametrize("opcode", range(256))
def test_rust_cb_opcode_family_matches_python(opcode: int) -> None:
    RustZ80Cpu = _rust_cpu()
    program = bytes([0xCB, opcode])
    python_bus = MemoryBus(program)
    rust_bus = MemoryBus(program)
    python_bus.memory[0x2000] = rust_bus.memory[0x2000] = 0x96
    python_cpu = Z80Cpu(python_bus)
    rust_cpu = RustZ80Cpu(rust_bus, None)

    for cpu in (python_cpu, rust_cpu):
        cpu.a = 0x69
        cpu.f = 0xA5
        cpu.bc = 0x1234
        cpu.de = 0x5678
        cpu.hl = 0x2000
        cpu.wz = 0x2800

    python_cycles = python_cpu.step()
    rust_cycles = rust_cpu.step()
    attributes = (
        "a",
        "f",
        "b",
        "c",
        "d",
        "e",
        "h",
        "l",
        "sp",
        "pc",
        "ix",
        "iy",
        "i",
        "r",
        "iff1",
        "iff2",
        "interrupt_mode",
        "halted",
        "wz",
        "total_cycles",
        "q",
        "_q_previous",
        "p",
    )
    assert rust_cycles == python_cycles
    assert {name: getattr(rust_cpu, name) for name in attributes} == {
        name: getattr(python_cpu, name) for name in attributes
    }
    assert rust_bus.memory == python_bus.memory


@pytest.mark.parametrize(
    ("opcode", "expected_cycles"),
    [
        (0x07, 4),  # RLCA
        (0x0F, 4),  # RRCA
        (0x17, 4),  # RLA
        (0x1F, 4),  # RRA
        (0x27, 4),  # DAA
        (0x2F, 4),  # CPL
        (0x37, 4),  # SCF
        (0x3F, 4),  # CCF
        (0x08, 4),  # EX AF,AF'
        (0xD9, 4),  # EXX
        (0xE3, 19),  # EX (SP),HL
        (0xEB, 4),  # EX DE,HL
    ],
)
def test_rust_base_misc_instructions_match_python(
    opcode: int, expected_cycles: int
) -> None:
    RustZ80Cpu = _rust_cpu()
    program = bytes([opcode])
    python_bus = MemoryBus(program)
    rust_bus = MemoryBus(program)
    python_bus.memory[0x3000:0x3002] = bytes([0xA5, 0x5A])
    rust_bus.memory[0x3000:0x3002] = bytes([0xA5, 0x5A])
    python_cpu = Z80Cpu(python_bus)
    rust_cpu = RustZ80Cpu(rust_bus, None)

    for cpu in (python_cpu, rust_cpu):
        cpu.a = 0x91
        cpu.f = 0x95
        cpu.b, cpu.c = 0x12, 0x34
        cpu.d, cpu.e = 0x56, 0x78
        cpu.h, cpu.l = 0x9A, 0xBC
        cpu.sp = 0x3000
        cpu.af_alt = 0x2468
        cpu.bc_alt = 0x1357
        cpu.de_alt = 0xFEDC
        cpu.hl_alt = 0xBEEF

    python_cycles = python_cpu.step()
    rust_cycles = rust_cpu.step()
    attributes = (
        "a",
        "f",
        "b",
        "c",
        "d",
        "e",
        "h",
        "l",
        "sp",
        "pc",
        "af_alt",
        "bc_alt",
        "de_alt",
        "hl_alt",
        "wz",
        "r",
        "total_cycles",
        "q",
        "_q_previous",
        "p",
    )
    assert python_cycles == rust_cycles == expected_cycles
    assert {name: getattr(rust_cpu, name) for name in attributes} == {
        name: getattr(python_cpu, name) for name in attributes
    }
    assert rust_bus.memory == python_bus.memory


@pytest.mark.parametrize(
    ("opcode", "expected_cycles"),
    [
        (0x42, 15),  # SBC HL,BC
        (0x4A, 15),  # ADC HL,BC
        (0x43, 20),  # LD (nn),BC
        (0x4B, 20),  # LD BC,(nn)
        (0x44, 8),  # NEG
        (0x47, 9),  # LD I,A
        (0x4F, 9),  # LD R,A
        (0x57, 9),  # LD A,I
        (0x5F, 9),  # LD A,R
        (0x67, 18),  # RRD
        (0x6F, 18),  # RLD
    ],
)
def test_rust_ed_arithmetic_and_special_ops_match_python(
    opcode: int, expected_cycles: int
) -> None:
    RustZ80Cpu = _rust_cpu()
    immediate = [0x00, 0x20] if opcode in (0x43, 0x4B) else []
    program = bytes([0xED, opcode, *immediate])
    python_bus = MemoryBus(program)
    rust_bus = MemoryBus(program)
    python_bus.memory[0x2000:0x2002] = bytes([0xCD, 0xAB])
    rust_bus.memory[0x2000:0x2002] = bytes([0xCD, 0xAB])
    python_cpu = Z80Cpu(python_bus)
    rust_cpu = RustZ80Cpu(rust_bus, None)

    for cpu in (python_cpu, rust_cpu):
        cpu.a = 0x59
        cpu.f = 0x15
        cpu.bc = 0x1234
        cpu.de = 0x5678
        cpu.hl = 0x2000
        cpu.sp = 0x3000
        cpu.i = 0x81
        cpu.r = 0x42
        cpu.iff1 = True
        cpu.iff2 = True

    python_cycles = python_cpu.step()
    rust_cycles = rust_cpu.step()
    attributes = (
        "a",
        "f",
        "b",
        "c",
        "d",
        "e",
        "h",
        "l",
        "sp",
        "pc",
        "i",
        "r",
        "iff1",
        "iff2",
        "interrupt_mode",
        "wz",
        "total_cycles",
        "q",
        "_q_previous",
        "p",
    )
    assert python_cycles == rust_cycles == expected_cycles
    assert {name: getattr(rust_cpu, name) for name in attributes} == {
        name: getattr(python_cpu, name) for name in attributes
    }
    assert rust_cpu.bc == python_cpu.bc
    assert rust_cpu.de == python_cpu.de
    assert rust_cpu.hl == python_cpu.hl
    assert rust_bus.memory == python_bus.memory


@pytest.mark.parametrize(
    ("opcode", "expected_cycles"),
    [
        (0xA1, 16),  # CPI
        (0xA9, 16),  # CPD
        (0xB1, 21),  # CPIR repeats when not equal
        (0xB9, 21),  # CPDR repeats when not equal
    ],
)
def test_rust_ed_block_compare_matches_python(
    opcode: int, expected_cycles: int
) -> None:
    RustZ80Cpu = _rust_cpu()
    program = bytes([0xED, opcode])
    python_bus = MemoryBus(program)
    rust_bus = MemoryBus(program)
    python_bus.memory[0x1000] = rust_bus.memory[0x1000] = 0x11
    python_cpu = Z80Cpu(python_bus)
    rust_cpu = RustZ80Cpu(rust_bus, None)
    for cpu in (python_cpu, rust_cpu):
        cpu.a = 0x60
        cpu.f = 0x01
        cpu.hl = 0x1000
        cpu.bc = 2

    assert python_cpu.step() == rust_cpu.step() == expected_cycles
    assert (rust_cpu.a, rust_cpu.f, rust_cpu.hl, rust_cpu.bc, rust_cpu.pc) == (
        python_cpu.a,
        python_cpu.f,
        python_cpu.hl,
        python_cpu.bc,
        python_cpu.pc,
    )
    assert rust_bus.memory == python_bus.memory


def test_rust_ed_block_io_matches_python() -> None:
    RustZ80Cpu = _rust_cpu()
    program = bytes([0xED, 0xA2])  # INI
    python_bus = MemoryBus(program)
    rust_bus = MemoryBus(program)
    python_io = PortBus()
    rust_io = PortBus()
    python_cpu = Z80Cpu(python_bus, python_io)
    rust_cpu = RustZ80Cpu(rust_bus, rust_io)
    for cpu in (python_cpu, rust_cpu):
        cpu.bc = 0x0220
        cpu.hl = 0x1000

    assert python_cpu.step() == rust_cpu.step() == 16
    assert (rust_cpu.b, rust_cpu.c, rust_cpu.f, rust_cpu.hl, rust_cpu.pc) == (
        python_cpu.b,
        python_cpu.c,
        python_cpu.f,
        python_cpu.hl,
        python_cpu.pc,
    )
    assert rust_bus.memory == python_bus.memory
    assert rust_io.ports == python_io.ports
    assert rust_io.writes == python_io.writes


def test_rust_core_fails_explicitly_on_unsupported_opcode() -> None:
    RustZ80Cpu = _rust_cpu()
    bus = MemoryBus(bytes([0xDD]))
    cpu = RustZ80Cpu(bus, None)
    with pytest.raises(NotImplementedError, match="does not support opcode"):
        cpu.step()


def test_machine_can_select_rust_core_for_a_halted_test_cartridge() -> None:
    _rust_cpu()
    machine = SG1000Machine(bytes([0x76]), cpu_core="rust")
    assert machine.cpu_core == "rust"
    machine.run_frames(1)
    assert machine.vdp.frame_number == 1
    assert machine.total_tstates > 0


def test_rust_q_latch_affects_scf_after_flag_preserving_instruction() -> None:
    RustZ80Cpu = _rust_cpu()
    # BIT 0,(HL) takes undocumented X/Y from WZ high byte; NOP clears Q,
    # so SCF must combine A with the retained F bits rather than prior Q.
    program = bytes([0xCB, 0x46, 0x00, 0x37])  # BIT 0,(HL); NOP; SCF
    python_bus = MemoryBus(program)
    rust_bus = MemoryBus(program)
    python_bus.memory[0x2000] = rust_bus.memory[0x2000] = 0x00
    python_cpu = Z80Cpu(python_bus)
    rust_cpu = RustZ80Cpu(rust_bus, None)
    for cpu in (python_cpu, rust_cpu):
        cpu.hl = 0x2000
        cpu.wz = 0x2800
        cpu.a = 0x00
        cpu.f = 0x01

    for _ in range(3):
        assert rust_cpu.step() == python_cpu.step()
        assert (rust_cpu.f, rust_cpu.q, rust_cpu._q_previous, rust_cpu.p) == (
            python_cpu.f,
            python_cpu.q,
            python_cpu._q_previous,
            python_cpu.p,
        )
    assert rust_cpu.f & 0x28 == 0x28


def test_rust_ld_a_i_interrupt_p_latch_matches_python() -> None:
    RustZ80Cpu = _rust_cpu()
    program = bytes([0xED, 0x57])  # LD A,I
    python_cpu = Z80Cpu(MemoryBus(program))
    rust_cpu = RustZ80Cpu(MemoryBus(program), None)
    for cpu in (python_cpu, rust_cpu):
        cpu.i = 0x80
        cpu.iff1 = True
        cpu.iff2 = True
        cpu.interrupt_mode = 1

    assert python_cpu.step() == rust_cpu.step() == 9
    assert python_cpu.p == rust_cpu.p == 1
    assert python_cpu.f & 0x04
    assert rust_cpu.f & 0x04

    python_cpu.request_int(0xFF)
    rust_cpu.request_int(0xFF)
    assert python_cpu.step() == rust_cpu.step() == 13
    assert python_cpu.p == rust_cpu.p == 0
    assert not (python_cpu.f & 0x04)
    assert not (rust_cpu.f & 0x04)
    assert (rust_cpu.pc, rust_cpu.sp, rust_cpu.f, rust_cpu.q) == (
        python_cpu.pc,
        python_cpu.sp,
        python_cpu.f,
        python_cpu.q,
    )


@pytest.mark.parametrize(
    ("data_bus_opcode", "operand", "expected_cycles", "expected_pc", "expected_b"),
    [
        (0x00, 0x5A, 6, 0x0000, 0x00),  # IM 0 NOP: acknowledge + two T-states
        (0x06, 0x5A, 9, 0x0001, 0x5A),  # IM 0 LD B,n: operand comes from PC
        (0xCF, 0x5A, 13, 0x0008, 0x00),  # IM 0 RST 08h
    ],
)
def test_rust_im0_executes_interrupt_supplied_opcode(
    data_bus_opcode: int,
    operand: int,
    expected_cycles: int,
    expected_pc: int,
    expected_b: int,
) -> None:
    RustZ80Cpu = _rust_cpu()
    python_bus = MemoryBus(bytes([operand, 0x00, 0x00]))
    rust_bus = MemoryBus(bytes([operand, 0x00, 0x00]))
    python_cpu = Z80Cpu(python_bus)
    rust_cpu = RustZ80Cpu(rust_bus, None)
    for cpu in (python_cpu, rust_cpu):
        cpu.iff1 = True
        cpu.iff2 = True
        cpu.interrupt_mode = 0
        cpu.sp = 0x3000
        cpu.request_int(data_bus_opcode)

    assert python_cpu.step() == rust_cpu.step() == expected_cycles
    assert (rust_cpu.pc, rust_cpu.b, rust_cpu.sp, rust_cpu.r) == (
        python_cpu.pc,
        python_cpu.b,
        python_cpu.sp,
        python_cpu.r,
    )
    assert rust_cpu.pc == expected_pc
    assert rust_cpu.b == expected_b
    assert rust_bus.memory == python_bus.memory
    assert (rust_cpu.q, rust_cpu.p, rust_cpu.f) == (
        python_cpu.q,
        python_cpu.p,
        python_cpu.f,
    )
