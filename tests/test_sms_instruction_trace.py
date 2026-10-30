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
        json.loads(line)
        for line in machine.drain_instruction_trace_json().splitlines()
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
