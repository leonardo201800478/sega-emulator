"""Integration tests for the optional native Z80 migration slice."""

import pytest

native = pytest.importorskip("sega_z80_native")
Z80Subset = native.Z80Subset


def test_native_subset_executes_a_batch_without_per_opcode_python_calls() -> None:
    cpu = Z80Subset()
    cpu.load_program(0x0000, bytes([0x3E, 0x7F, 0x3C, 0x76]))

    assert cpu.run_steps(3) == 3
    assert cpu.a == 0x80
    assert cpu.f & 0xD4 == 0xD4
    assert cpu.halted
    assert cpu.total_tstates == 15
    assert cpu.state() == (0x80, cpu.f, 4, True, 15)


def test_native_subset_rejects_unknown_opcodes() -> None:
    cpu = Z80Subset()
    cpu.load_program(0, bytes([0x01]))

    with pytest.raises(ValueError, match="outside the current native Z80 subset"):
        cpu.step()


def test_native_subset_validates_memory_size_and_program_bounds() -> None:
    with pytest.raises(ValueError, match="65536"):
        Z80Subset(bytes(32))

    cpu = Z80Subset()
    with pytest.raises(ValueError, match="without wrapping"):
        cpu.load_program(0xFFFF, bytes([0x00, 0x00]))
