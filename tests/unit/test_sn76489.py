"""Testes determinísticos do PSG SN76489 inicial."""

import pytest

from sega_emulator.audio.sn76489 import SN76489


def test_power_on_is_muted_and_render_is_deterministic() -> None:
    psg = SN76489()
    assert psg.render(8) == [0.0] * 8
    first = SN76489()
    second = SN76489()
    for chip in (first, second):
        chip.write(0x90)
        chip.write(0x81)
    assert first.render(128) == second.render(128)


def test_latched_tone_period_and_followup_data() -> None:
    psg = SN76489()
    psg.write(0x85)
    psg.write(0x12)
    assert psg.tone_periods[0] == 0x125


def test_volume_registers_are_four_bit_and_15_mutes_channel() -> None:
    psg = SN76489()
    psg.write(0x90)
    assert psg.attenuations[0] == 0
    psg.write(0x9F)
    assert psg.attenuations[0] == 15
    assert psg.render(4) == [0.0] * 4


def test_noise_control_resets_lfsr_and_masks_control_bits() -> None:
    psg = SN76489()
    psg.write(0xE7)
    assert psg.noise_control == 7
    assert psg._lfsr == 0x4000
    psg.write(0xF3)
    assert psg.attenuations[3] == 3


def test_render_validates_sample_count_and_write_validates_byte() -> None:
    psg = SN76489()
    with pytest.raises(ValueError):
        psg.render(-1)
    with pytest.raises(ValueError):
        psg.render(1.5)  # type: ignore[arg-type]
    with pytest.raises(ValueError):
        psg.write(256)
