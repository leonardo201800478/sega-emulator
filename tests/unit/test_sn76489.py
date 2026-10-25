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
    assert psg._noise_counter == 0
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


def test_tone_frequency_is_near_440_hz_at_ntsc_clock() -> None:
    psg = SN76489()
    enable_tone_zero(psg, 0xFE)
    samples = psg.render(11_025)
    transitions = sum(a != b for a, b in zip(samples, samples[1:], strict=False))
    assert 200 <= transitions <= 240


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


def test_noise_rate_write_changes_next_reload_not_active_countdown() -> None:
    psg = SN76489()
    psg.write(0xE0)  # taxa fixa /512; divisor interno 16
    psg._noise_counter = 16
    psg._noise_counter_phase = 0
    initial_lfsr = psg._lfsr

    psg.write(0xE1)  # taxa fixa /1024; divisor interno 32
    assert psg._noise_period == 32
    assert psg._noise_counter == 32  # escrever o controle reinicia o gerador

    # Já com um contador ativo, uma mudança de período só altera o próximo reload.
    psg._noise_counter = 16
    psg.write(0xE2)
    assert psg._noise_period == 64
    assert psg._noise_counter == 16
    for _ in range(15):
        psg._tick_internal_clock()
    assert psg._lfsr == LFSR_SEED
    psg._tick_internal_clock()
    assert psg._noise_counter == 64
    assert psg._lfsr == initial_lfsr or psg._lfsr != 0


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
