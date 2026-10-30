"""Integration tests for opt-in native SMS boot instruction tracing."""

from __future__ import annotations

import json

import pytest

from sega_emulator.machine.sms import SMSMachine


def test_native_sms_instruction_trace_is_bounded_and_emits_jsonl():
    machine = SMSMachine(b"\x00" * 0x4000)

    assert machine.enable_instruction_trace(2) == 2
    assert machine.step() == 4
    assert machine.step() == 4

    records = [
        json.loads(line) for line in machine.drain_instruction_trace_json().splitlines()
    ]
    assert len(records) == 2
    assert records[0]["instruction_index"] == 1
    assert records[0]["opcode_bytes"] == ["00"]
    assert records[0]["before"]["pc"] == 0
    assert records[0]["after"]["pc"] == 1
    assert records[1]["instruction_index"] == 2
    assert machine.drain_instruction_trace_json() == ""


@pytest.mark.parametrize("limit", [0, 4097])
def test_native_sms_instruction_trace_rejects_invalid_limits(limit):
    machine = SMSMachine(b"\x00" * 0x4000)

    with pytest.raises(ValueError, match="max_instructions"):
        machine.enable_instruction_trace(limit)


def test_python_sms_trace_observes_synthetic_bios_overlay_handoff():
    bios = bytearray(0x2000)
    bios[:4] = b"\x3e\x08\xd3\x3e"
    machine = SMSMachine(b"\x76" * 0x4000, bios=bytes(bios))

    assert machine.bios_enabled
    machine.enable_instruction_trace(2)
    machine.step()
    machine.step()

    records = [
        json.loads(line) for line in machine.drain_instruction_trace_json().splitlines()
    ]
    assert not machine.bios_enabled
    assert [record["instruction_bytes"] for record in records] == [
        ["3E", "08"],
        ["D3", "3E"],
    ]


def test_python_sms_trace_can_start_after_a_deterministic_instruction_window():
    machine = SMSMachine(b"\\x00" * 0x4000)
    machine.enable_instruction_trace(2, start_after_instructions=3)

    for _ in range(5):
        machine.step()

    records = [
        json.loads(line) for line in machine.drain_instruction_trace_json().splitlines()
    ]
    assert len(records) == 2
    assert records[0]["before"]["pc"] == 3
    assert records[0]["after"]["pc"] == 4
    assert records[1]["before"]["pc"] == 4
