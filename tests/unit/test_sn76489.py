"""Testes determinísticos e funcionais do PSG SN76489AN para SG-1000."""

import pytest

from sega_emulator.audio.sn76489 import LFSR_MASK, LFSR_SEED, SN76489


def enable_tone_zero(psg: SN76489, period: int = 0xFE) -> None:
    psg.write(0x90)
    psg.write(0x80 | (period & 0x0F))
    psg.write((period >> 4) & 0x3F)


def test_power_on_is_muted_and_render_is_deterministic() -> None:
    psg = SN76489()
    assert psg.render(8) == [0.0] * 8
    first, second = SN76489(), SN76489()
    for chip in (first, second):
        enable_tone_zero(chip)
    assert first.render(128) == second.render(128)


def test_latch_and_data_bytes_update_ten_bit_tone_period() -> None:
    psg = SN76489()
    psg.write(0x85)
    psg.write(0x12)
    assert psg.tone_periods[0] == 0x125
    psg.write(0x8F)
    assert psg.tone_periods[0] == 0x12F


def test_volume_is_four_bit_and_fifteen_mutes_channel() -> None:
    psg = SN76489()
    psg.write(0x90)
    assert psg.attenuations[0] == 0
    psg.write(0x9F)
    assert psg.attenuations[0] == 15
    assert psg.render(4) == [0.0] * 4


def test_noise_write_resets_fifteen_bit_lfsr_and_divider_phase() -> None:
    psg = SN76489()
    psg.advance_clocks(1000)
    psg.write(0xE7)
    assert psg.noise_control == 7
    assert psg._lfsr == LFSR_SEED
    assert psg._noise_counter == psg._noise_period
    assert psg._noise_counter_phase == 0
    assert psg._lfsr & ~LFSR_MASK == 0


def test_white_noise_uses_sg1000_feedback_taps_and_stays_fifteen_bit() -> None:
    psg = SN76489()
    psg.write(0xE4)
    psg._lfsr = 0b01
    psg._shift_noise_register()
    assert psg._lfsr == LFSR_SEED
    for _ in range(20_000):
        psg._shift_noise_register()
        assert 0 < psg._lfsr <= LFSR_MASK


def test_tone_period_zero_and_one_follow_documented_sg1000_profile() -> None:
    for period in (0, 1):
        psg = SN76489()
        psg.write(0x90)
        psg.write(0x80 | (period & 0x0F))
        assert psg._tone_outputs[0] == 1
        assert all(sample > 0 for sample in psg.render(128))


@pytest.mark.parametrize("period", (127, 254, 511))
def test_rendered_tone_frequency_matches_clock_and_divider(period: int) -> None:
    """Check the measured fundamental against f = input_clock / (32 * period)."""
    psg = SN76489()
    enable_tone_zero(psg, period)
    samples = psg.render(22_050)  # 0.5 s, enough to resolve low-frequency periods.
    transitions = sum(
        (a < 0.0) != (b < 0.0) for a, b in zip(samples, samples[1:], strict=False)
    )

    expected_frequency = psg.clock_hz / (2 * 16 * period)
    measured_frequency = transitions * psg.sample_rate / (2 * (len(samples) - 1))

    # Each missed/extra transition shifts the estimate by about 1 Hz over 0.5 s.
    assert abs(measured_frequency - expected_frequency) <= 2.5, (
        f"period={period}: expected {expected_frequency:.3f} Hz, "
        f"measured {measured_frequency:.3f} Hz from {transitions} transitions"
    )


def test_rendering_in_chunks_matches_single_render() -> None:
    whole, chunked = SN76489(), SN76489()
    for chip in (whole, chunked):
        enable_tone_zero(chip, 0xFE)
        chip.write(0xE5)
        chip.write(0xF2)
    assert whole.render(997) == chunked.render(123) + chunked.render(874)


def test_render_samples_are_not_duplicated_into_drain_buffer() -> None:
    psg = SN76489()
    enable_tone_zero(psg)
    rendered = psg.render(128)
    assert len(rendered) == 128
    assert psg.drain_samples() == []


def test_advance_clocks_is_chunk_invariant() -> None:
    whole, chunked = SN76489(), SN76489()
    for chip in (whole, chunked):
        chip.write(0xE5)
        chip.write(0xF2)
    whole.advance_clocks(123_457)
    chunked.advance_clocks(12_345)
    chunked.advance_clocks(111_112)
    assert whole._lfsr == chunked._lfsr
    assert whole._noise_counter == chunked._noise_counter
    assert whole._noise_counter_phase == chunked._noise_counter_phase
    assert whole._tone_counters == chunked._tone_counters
    assert whole._clock_divider_phase == chunked._clock_divider_phase
    assert whole._output_phase == chunked._output_phase
    assert whole.drain_samples() == chunked.drain_samples()


def test_noise_period_cache_tracks_rate_and_tone_three_writes() -> None:
    psg = SN76489()
    psg.write(0xE2)
    assert psg._noise_period == 0x40
    psg.write(0xE3)
    assert psg._noise_period == 1
    psg.write(0xC7)
    psg.write(0x12)
    assert psg.tone_periods[2] == 0x127
    assert psg._noise_period == 0x127
    psg.write(0xE0)
    assert psg._noise_period == 0x10


def test_enabling_tone_after_period_zero_or_one_loads_initial_countdown() -> None:
    for initial_period in (0, 1):
        psg = SN76489()
        psg.write(0x90)  # canal 0 audível
        psg.write(0x80 | initial_period)
        assert psg.tone_periods[0] == initial_period

        # Programa período 8; o primeiro semiperíodo também deve durar 8 ticks.
        psg.write(0x88)
        psg.write(0x00)
        assert psg.tone_periods[0] == 8
        assert psg._tone_counters[0] == 8
        initial_output = psg._tone_outputs[0]

        for _ in range(7):
            psg._tick_internal_clock()
            assert psg._tone_outputs[0] == initial_output
        psg._tick_internal_clock()
        assert psg._tone_outputs[0] != initial_output


def test_tone_period_write_does_not_restart_active_countdown() -> None:
    psg = SN76489()
    psg.write(0x90)
    psg.write(0x84)
    psg.write(0x06)  # período inicial 0x064
    psg._tone_counters[0] = 100
    initial_output = psg._tone_outputs[0]

    psg.write(0x88)
    psg.write(0x00)  # novo período 8, sem reiniciar o contador ativo
    assert psg.tone_periods[0] == 8
    assert psg._tone_counters[0] == 100

    for _ in range(99):
        psg._tick_internal_clock()
    assert psg._tone_outputs[0] == initial_output
    psg._tick_internal_clock()
    assert psg._tone_outputs[0] != initial_output


def test_noise_tone_link_write_preserves_active_countdown() -> None:
    psg = SN76489()
    psg.write(0xC0)
    psg.write(0x01)  # período de tom 2 = 0x010
    psg.write(0xE3)  # ruído sincronizado ao tom 2; reinicia o divisor
    psg._noise_counter = 16
    psg._noise_counter_phase = 0

    psg.write(0xC0)
    psg.write(0x04)  # período de tom 2 passa a 0x040
    assert psg._noise_period == 64
    assert psg._noise_counter == 16

    for _ in range(15):
        psg._tick_internal_clock()
    assert psg._noise_counter == 1
    psg._tick_internal_clock()
    assert psg._noise_counter == 64


def test_sample_buffer_drops_oldest_samples_and_reports_count() -> None:
    psg = SN76489(clock_hz=1600, sample_rate=100)
    psg.advance_clocks(16 * 250)
    samples = psg.drain_samples()
    assert len(samples) == 100
    assert psg.dropped_samples == 150


def test_reset_returns_to_deterministic_muted_state() -> None:
    psg = SN76489()
    enable_tone_zero(psg)
    psg.write(0xE7)
    psg.render(256)
    psg.reset()
    assert psg.tone_periods == [0, 0, 0]
    assert psg.attenuations == [15, 15, 15, 15]
    assert psg._lfsr == LFSR_SEED
    assert psg._noise_period == 0x10
    assert psg.dropped_samples == 0
    assert psg.render(16) == [0.0] * 16


def test_input_validation() -> None:
    with pytest.raises(ValueError):
        SN76489(clock_hz=0)
    with pytest.raises(ValueError):
        SN76489(sample_rate=0)
    with pytest.raises(ValueError):
        SN76489().render(-1)
    with pytest.raises(ValueError):
        SN76489().render(1.5)  # type: ignore[arg-type]
    with pytest.raises(ValueError):
        SN76489().write(256)
    with pytest.raises(ValueError):
        SN76489().advance_clocks(-1)
    with pytest.raises(ValueError):
        SN76489().drain_samples(-1)


def test_clocked_audio_samples_are_buffered_and_chunk_invariant() -> None:
    whole, chunked = SN76489(), SN76489()
    for chip in (whole, chunked):
        enable_tone_zero(chip, 0xFE)
    whole.advance_clocks(357_954)
    chunked.advance_clocks(123_456)
    chunked.advance_clocks(234_498)

    whole_samples = whole.drain_samples()
    chunked_samples = chunked.drain_samples()
    assert whole_samples == chunked_samples
    assert 4_400 <= len(whole_samples) <= 4_420


def test_drain_samples_respects_limit_and_fifo_order() -> None:
    psg = SN76489(clock_hz=1600, sample_rate=100)
    psg.advance_clocks(16 * 10)
    first = psg.drain_samples(3)
    rest = psg.drain_samples()
    assert len(first) == 3
    assert len(rest) == 7
    assert psg.drain_samples() == []



def test_antialias_filter_suppresses_tone_above_nyquist() -> None:
    """A 55.9 kHz tone should not fold into the audible band at 44.1 kHz."""
    aliased, in_band = SN76489(), SN76489()
    enable_tone_zero(aliased, 2)
    enable_tone_zero(in_band, 8)

    high_tone = aliased.render(4096)[1024:]
    audible_tone = in_band.render(4096)[1024:]
    high_rms = (sum(sample * sample for sample in high_tone) / len(high_tone)) ** 0.5
    audible_rms = (
        sum(sample * sample for sample in audible_tone) / len(audible_tone)
    ) ** 0.5

    assert audible_rms > 0.2
    assert high_rms < audible_rms * 0.1, (
        f"out-of-band tone RMS {high_rms:.6f} was not sufficiently attenuated "
        f"relative to in-band tone RMS {audible_rms:.6f}"
    )


def test_antialias_filter_state_is_reset_and_chunk_deterministic() -> None:
    whole, chunked = SN76489(), SN76489()
    for chip in (whole, chunked):
        enable_tone_zero(chip, 8)
    expected = whole.render(1024)
    actual = chunked.render(127) + chunked.render(897)
    assert actual == expected

    chunked.reset()
    assert chunked._filter_position == 0
    assert chunked._filter_history == [0.0] * len(chunked._filter_history)
    assert chunked.render(64) == SN76489().render(64)
