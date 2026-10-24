"""Testes do adaptador SingleStepTests usando vetores sintéticos mínimos."""

import json

import pytest

from sega_emulator.testing.z80_singlestep import load_vectors, run_vector


def vector_nop() -> dict:
    return {
        "name": "00 NOP fixture",
        "initial": {
            "pc": 0x1000,
            "sp": 0xFF00,
            "r": 0,
            "f": 0xA5,
            "ram": [[0x1000, 0x00]],
        },
        "final": {
            "pc": 0x1001,
            "sp": 0xFF00,
            "r": 1,
            "f": 0xA5,
            "q": 0,
            "ram": [[0x1000, 0x00]],
        },
        "cycles": [],
        "ports": [],
    }


def test_adapter_passes_minimal_nop_vector() -> None:
    result = run_vector(vector_nop())

    assert result.status == "passed"
    assert result.mismatches == []
    assert result.cycles_returned == 4


def test_adapter_reports_first_state_difference() -> None:
    vector = vector_nop()
    vector["final"]["r"] = 0

    result = run_vector(vector)

    assert result.status == "failed"
    assert any("r: esperado 0, obtido 1" in mismatch for mismatch in result.mismatches)


def test_adapter_loads_operands_from_flat_ram_and_compares_state() -> None:
    vector = {
        "name": "06 LD B,n fixture",
        "initial": {
            "pc": 0x2000,
            "b": 0x99,
            "ram": [[0x2000, 0x06], [0x2001, 0x42]],
        },
        "final": {
            "pc": 0x2002,
            "b": 0x42,
            "r": 1,
            "ram": [[0x2000, 0x06], [0x2001, 0x42]],
        },
        "cycles": [],
        "ports": [],
    }

    result = run_vector(vector)

    assert result.status == "passed"
    assert result.cycles_returned == 7


def test_adapter_marks_nonzero_unmodelled_state_as_unsupported() -> None:
    vector = vector_nop()
    vector["initial"]["p"] = 1

    result = run_vector(vector)

    assert result.status == "unsupported"
    assert result.unsupported_fields == ["p"]


def test_adapter_replays_io_read_value_and_checks_transaction() -> None:
    vector = {
        "name": "DB IN A,(n) fixture",
        "initial": {
            "pc": 0x3000,
            "a": 0x12,
            "ram": [[0x3000, 0xDB], [0x3001, 0x34]],
        },
        "final": {
            "pc": 0x3002,
            "a": 0x56,
            "r": 1,
            "ram": [[0x3000, 0xDB], [0x3001, 0x34]],
        },
        "cycles": [],
        "ports": [[0x1234, 0x56, "r"]],
    }

    result = run_vector(vector)

    assert result.status == "passed"
    assert result.cycles_returned == 11


def test_load_vectors_requires_a_json_array(tmp_path) -> None:
    path = tmp_path / "invalid.json"
    path.write_text(json.dumps({"name": "not a list"}), encoding="utf-8")

    with pytest.raises(ValueError, match="lista JSON"):
        load_vectors(path)


def test_adapter_derives_io_read_value_from_single_step_cycle_trace() -> None:
    vector = {
        "name": "ED 40 IN B,(C) cycle-derived input fixture",
        "initial": {
            "pc": 0x4000,
            "b": 0x12,
            "c": 0x34,
            "ram": [[0x4000, 0xED], [0x4001, 0x40]],
        },
        "final": {
            "pc": 0x4002,
            "b": 0x56,
            "r": 2,
            "wz": 0x1235,
            "ram": [[0x4000, 0xED], [0x4001, 0x40]],
        },
        "cycles": [
            [0x1234, None, "r--i"],
            [0x1234, 0x56, "----"],
        ],
    }

    result = run_vector(vector)

    assert result.status == "passed"
    assert result.mismatches == []


def test_out_immediate_updates_wz_to_port_plus_one() -> None:
    vector = {
        "name": "D3 OUT (n),A WZ fixture",
        "initial": {
            "pc": 0x5000,
            "a": 0x9C,
            "wz": 0x1111,
            "ram": [[0x5000, 0xD3], [0x5001, 0x7C]],
        },
        "final": {
            "pc": 0x5002,
            "wz": 0x9C7D,
            "ram": [[0x5000, 0xD3], [0x5001, 0x7C]],
        },
        "cycles": [],
        "ports": [[0x9C7C, 0x9C, "w"]],
    }

    result = run_vector(vector)

    assert result.status == "passed"
    assert result.mismatches == []


def test_out_immediate_wz_wraps_low_byte_without_carrying_to_high_byte() -> None:
    vector = {
        "name": "D3 OUT (FFh),A WZ wrap fixture",
        "initial": {
            "pc": 0x5100,
            "a": 0x9A,
            "ram": [[0x5100, 0xD3], [0x5101, 0xFF]],
        },
        "final": {
            "pc": 0x5102,
            "wz": 0x9A00,
            "ram": [[0x5100, 0xD3], [0x5101, 0xFF]],
        },
        "cycles": [],
        "ports": [[0x9AFF, 0x9A, "w"]],
    }

    result = run_vector(vector)

    assert result.status == "passed"
    assert result.mismatches == []
