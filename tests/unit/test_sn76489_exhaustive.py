"""Exhaustive invariant and golden-vector tests for the SG-1000 SN76489AN profile.

These tests establish internal contracts and fixed digital vectors. They do not
claim hardware fidelity until the vectors are independently corroborated.
"""

import pytest

from sega_emulator.audio.sn76489 import LFSR_MASK, LFSR_SEED, SN76489

# Fixed states calculated from the documented 15-bit SG-1000 feedback recurrence.
# Kept literal so the test does not derive its expected values from the DUT.
WHITE_NOISE_GOLDEN_STATES = (
    0x4000,
    0x2000,
    0x1000,
    0x0800,
    0x0400,
    0x0200,
    0x0100,
    0x0080,
    0x0040,
    0x0020,
    0x0010,
    0x0008,
    0x0004,
    0x0002,
    0x4001,
    0x6000,
    0x3000,
    0x1800,
    0x0C00,
    0x0600,
    0x0300,
    0x0180,
    0x00C0,
    0x0060,
    0x0030,
    0x0018,
    0x000C,
    0x0006,
    0x4003,
    0x2001,
    0x5000,
    0x2800,
)


def write_tone_period(psg: SN76489, channel: int, period: int) -> None:
    """Program one 10-bit tone divider through the public latch/data protocol."""
    if channel not in (0, 1, 2):
        raise ValueError("channel must be 0, 1, or 2")
    if not 0 <= period <= 0x3FF:
        raise ValueError("period must fit in ten bits")
    register = channel * 2
    psg.write(0x80 | (register << 4) | (period & 0x0F))
    psg.write((period >> 4) & 0x3F)


@pytest.mark.parametrize("register", range(8))
@pytest.mark.parametrize("nibble", range(16))
def test_every_latch_register_and_nibble_is_decoded(register: int, nibble: int) -> None:
    """Exhaust all 128 latch-byte register/data combinations."""
    psg = SN76489()
    psg.write(0x80 | (register << 4) | nibble)

    assert psg._latched_register == register
    if register in (0, 2, 4):
        assert psg.tone_periods[register // 2] == nibble
    elif register in (1, 3, 5, 7):
        assert psg.attenuations[register // 2] == nibble
    else:
        assert psg.noise_control == nibble & 0x07


@pytest.mark.parametrize("register", range(8))
@pytest.mark.parametrize("value", range(128))
def test_every_data_byte_is_masked_to_the_selected_register(
    register: int, value: int
) -> None:
    """Exhaust all 1,024 DATA-byte/register combinations."""
    psg = SN76489()
    psg.write(0x80 | (register << 4) | 0x0F)
    psg.write(value)

    assert psg._latched_register == register
    assert all(0 <= period <= 0x3FF for period in psg.tone_periods)
    assert all(0 <= attenuation <= 0x0F for attenuation in psg.attenuations)
    assert 0 <= psg.noise_control <= 0x07
    assert 0 < psg._lfsr <= LFSR_MASK


@pytest.mark.parametrize("channel", range(3))
@pytest.mark.parametrize("period", range(0x400))
def test_all_tone_periods_have_the_declared_state_and_boundary_behavior(
    channel: int, period: int
) -> None:
    """Exhaust all 3,072 channel/period combinations, including 0 and 1."""
    psg = SN76489()
    write_tone_period(psg, channel, period)

    assert psg.tone_periods[channel] == period
    if period <= 1:
        initial_output = psg._tone_outputs[channel]
        for _ in range(32):
            psg._tick_internal_clock()
            assert psg._tone_outputs[channel] == 1
        assert initial_output == 1
    else:
        # Isolate divider behavior from the intermediate state caused by the
        # two sequential register writes. That intermediate timing remains a
        # separate hardware-validation question, not an assumed oracle here.
        psg._tone_counters[channel] = period
        initial_output = psg._tone_outputs[channel]
        for _ in range(period - 1):
            psg._tick_internal_clock()
            assert psg._tone_outputs[channel] == initial_output
        psg._tick_internal_clock()
        assert psg._tone_outputs[channel] != initial_output


@pytest.mark.parametrize("channel", range(4))
@pytest.mark.parametrize("attenuation", range(16))
def test_all_channel_attenuation_codes_remain_four_bit(
    channel: int, attenuation: int
) -> None:
    """Exhaust all 64 channel/attenuation register combinations."""
    psg = SN76489()
    psg.write(0x80 | ((channel * 2 + 1) << 4) | attenuation)
    assert psg.attenuations[channel] == attenuation

    for _ in range(64):
        psg._tick_internal_clock()
    assert psg.attenuations[channel] == attenuation


def test_white_noise_lfsr_matches_fixed_initial_golden_vector() -> None:
    """Check a literal sequence instead of deriving expectations from the DUT."""
    psg = SN76489()
    psg.write(0xE4)  # Select white noise and reset the generator.
    assert psg._lfsr == LFSR_SEED

    observed = []
    for _ in WHITE_NOISE_GOLDEN_STATES:
        observed.append(psg._lfsr)
        psg._shift_noise_register()

    assert tuple(observed) == WHITE_NOISE_GOLDEN_STATES


def test_white_noise_lfsr_visits_every_nonzero_fifteen_bit_state_once() -> None:
    """Verify the recurrence period and nonzero-state invariant."""
    psg = SN76489()
    psg.write(0xE4)
    seen: set[int] = set()

    for _ in range(LFSR_MASK):
        assert psg._lfsr not in seen
        assert 0 < psg._lfsr <= LFSR_MASK
        seen.add(psg._lfsr)
        psg._shift_noise_register()

    assert len(seen) == LFSR_MASK
    assert psg._lfsr == LFSR_SEED


@pytest.mark.parametrize("noise_rate", range(4))
def test_noise_divider_rates_have_expected_nominal_period(noise_rate: int) -> None:
    """Check all fixed noise rates and the tone-2-linked rate separately."""
    psg = SN76489()
    psg.write(0xE0 | noise_rate)

    if noise_rate < 3:
        assert psg._noise_period == (0x10 << noise_rate)
    else:
        assert psg._noise_period == max(1, psg.tone_periods[2])


def test_noise_tone_link_tracks_all_tone_two_period_values() -> None:
    """Exhaust the linked noise period for every possible tone-2 divider."""
    psg = SN76489()
    psg.write(0xE3)

    for period in range(0x400):
        write_tone_period(psg, 2, period)
        assert psg._noise_period == max(1, period)


@pytest.mark.parametrize("initial_phase", range(16))
def test_clock_divider_preserves_every_possible_phase(initial_phase: int) -> None:
    """Check all phases of the divide-by-16 input clock."""
    psg = SN76489()
    psg.advance_clocks(initial_phase)
    assert psg._clock_divider_phase == initial_phase

    psg.advance_clocks(16 - initial_phase if initial_phase else 16)
    assert psg._clock_divider_phase == 0


@pytest.mark.parametrize(
    ("first", "second", "third"),
    [
        (1, 15, 101),
        (15, 1, 16_000),
        (16, 16, 16),
        (17, 31, 257),
        (123, 456, 789),
        (15_999, 1, 17),
    ],
)
def test_clock_advancement_is_invariant_under_arbitrary_chunking(
    first: int, second: int, third: int
) -> None:
    """Compare state and PCM output after equivalent clock partitions."""
    whole = SN76489()
    chunked = SN76489()
    for psg in (whole, chunked):
        write_tone_period(psg, 0, 0x155)
        write_tone_period(psg, 1, 0x2AA)
        psg.write(0xE5)
        psg.write(0xF3)

    total = first + second + third
    whole.advance_clocks(total)
    chunked.advance_clocks(first)
    chunked.advance_clocks(second)
    chunked.advance_clocks(third)

    assert chunked.tone_periods == whole.tone_periods
    assert chunked._tone_counters == whole._tone_counters
    assert chunked._tone_outputs == whole._tone_outputs
    assert chunked._lfsr == whole._lfsr
    assert chunked._noise_counter == whole._noise_counter
    assert chunked._noise_counter_phase == whole._noise_counter_phase
    assert chunked._clock_divider_phase == whole._clock_divider_phase
    assert chunked._output_phase == whole._output_phase
    assert chunked.drain_samples() == whole.drain_samples()


def test_reset_clears_every_temporal_phase_and_pending_audio_queue() -> None:
    """Reset after active sound must restore the documented initial state."""
    psg = SN76489()
    write_tone_period(psg, 0, 0x123)
    psg.write(0x90)
    psg.write(0xE7)
    psg.advance_clocks(123_457)
    psg.render(257)
    psg.reset()

    assert psg.tone_periods == [0, 0, 0]
    assert psg.attenuations == [15, 15, 15, 15]
    assert psg.noise_control == 0
    assert psg._latched_register == 0
    assert psg._tone_counters == [1, 1, 1]
    assert psg._tone_outputs == [1, 1, 1]
    assert psg._noise_counter == 0x10
    assert psg._noise_counter_phase == 0
    assert psg._lfsr == LFSR_SEED
    assert psg._clock_divider_phase == 0
    assert psg._output_phase == 0
    assert psg._noise_period == 0x10
    assert psg.dropped_samples == 0
    assert psg.drain_samples() == []
    assert psg.render(32) == [0.0] * 32


@pytest.mark.parametrize(
    ("clock_hz", "sample_rate", "clocks"),
    [
        (3_579_545, 44_100, 357_954),
        (3_579_545, 48_000, 1_000_003),
        (1_000, 100, 16_000),
        (160, 1_000, 160),
        (1, 16, 256),
    ],
)
def test_sample_count_is_rationally_determined_by_elapsed_clocks(
    clock_hz: int, sample_rate: int, clocks: int
) -> None:
    """Check output sample count against exact integer phase arithmetic."""
    psg = SN76489(clock_hz=clock_hz, sample_rate=sample_rate)
    psg.advance_clocks(clocks)

    internal_ticks = clocks // 16
    expected_samples = (internal_ticks * sample_rate * 16) // clock_hz
    expected_samples = min(expected_samples, sample_rate)
    assert len(psg.drain_samples()) == expected_samples


def test_noise_mixer_outputs_the_bit_shifted_out_of_the_lfsr() -> None:
    """The mixer must see the outgoing LFSR bit, not the post-shift low bit."""
    psg = SN76489()
    psg.write(0xE0)  # Periodic mode: the seed's single one circulates through 15 bits.
    psg.write(0xF0)  # Make only the noise channel audible.

    outgoing_bits = []
    observed = []
    for _ in range(15):
        outgoing_bits.append(psg._lfsr & 1)
        psg._shift_noise_register()
        observed.append(psg._mix())

    expected_bits = [0] * 14 + [1]
    expected_samples = [-0.25] * 14 + [0.25]
    assert outgoing_bits == expected_bits
    assert observed == expected_samples
