"""Testes rápidos da seleção determinística de HL do validador de 16 bits."""

from scripts.validate_z80_alu_16bit import _select_left_values


def test_sequential_sampling_preserves_legacy_order() -> None:
    assert _select_left_values(4, "sequential") == (0, 1, 2, 3)


def test_distributed_single_sample_uses_middle_of_domain() -> None:
    assert _select_left_values(1, "distributed") == (0x8000,)


def test_distributed_sampling_spans_entire_16bit_domain() -> None:
    values = _select_left_values(1024, "distributed")

    assert len(values) == 1024
    assert len(set(values)) == 1024
    assert values[0] == 0x0000
    assert values[-1] == 0xFFFF
    assert values[512] > 0x7FFF


def test_distributed_sampling_includes_important_boundaries() -> None:
    values = set(_select_left_values(13, "distributed"))

    assert values == {
        0x0000,
        0x0001,
        0x000F,
        0x0010,
        0x00FF,
        0x0100,
        0x0FFF,
        0x1000,
        0x7FFF,
        0x8000,
        0x8001,
        0xFFFE,
        0xFFFF,
    }


def test_distributed_sampling_is_deterministic_and_unique() -> None:
    first = _select_left_values(256, "distributed")
    second = _select_left_values(256, "distributed")

    assert first == second
    assert len(first) == len(set(first)) == 256
