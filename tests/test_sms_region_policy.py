"""Tests for SMS region preference order and nominal frame pacing."""

from __future__ import annotations

import pytest

from sega_emulator.machine.sms_region import (
    normalize_sms_region_preference,
    sms_frame_period_seconds,
    sms_region_candidates,
    sms_region_candidates_for_rom,
)


@pytest.mark.parametrize(
    ("preference", "expected"),
    [
        ("auto", ("usa", "europe", "japan")),
        ("usa", ("usa",)),
        ("europe", ("europe",)),
        ("japan", ("japan",)),
        ("invalid", ("usa", "europe", "japan")),
    ],
)
def test_region_candidates_follow_explicit_priority(preference, expected):
    assert sms_region_candidates(preference) == expected


def test_invalid_persisted_region_defaults_to_auto():
    assert normalize_sms_region_preference(None) == "auto"
    assert normalize_sms_region_preference("invalid") == "auto"


def test_pal_frame_pacing_is_slower_than_ntsc():
    assert sms_frame_period_seconds("europe") > sms_frame_period_seconds("usa")
    assert 0.019 < sms_frame_period_seconds("europe") < 0.021
    assert 0.016 < sms_frame_period_seconds("usa") < 0.018


def test_unknown_concrete_region_is_rejected():
    with pytest.raises(ValueError, match="region"):
        sms_frame_period_seconds("auto")



def test_automatic_region_candidates_follow_trusted_sms_header():
    japanese = bytearray(0x8000)
    japanese[0x7FF0 : 0x7FF8] = b"TMR SEGA"
    japanese[0x7FFF] = 0x3C
    export = bytearray(0x8000)
    export[0x7FF0 : 0x7FF8] = b"TMR SEGA"
    export[0x7FFF] = 0x4C

    assert sms_region_candidates_for_rom("auto", bytes(japanese)) == ("japan",)
    assert sms_region_candidates_for_rom("auto", bytes(export)) == ("usa", "europe")


def test_unknown_header_keeps_legacy_auto_order_without_guessing():
    assert sms_region_candidates_for_rom("auto", b"\\x00" * 0x8000) == (
        "usa",
        "europe",
        "japan",
    )


def test_manual_region_preference_overrides_cartridge_header():
    japanese = bytearray(0x8000)
    japanese[0x7FF0 : 0x7FF8] = b"TMR SEGA"
    japanese[0x7FFF] = 0x3C
    assert sms_region_candidates_for_rom("europe", bytes(japanese)) == ("europe",)
