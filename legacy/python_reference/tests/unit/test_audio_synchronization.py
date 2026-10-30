"""Testes para a linha temporal de writes do PSG."""

import pytest

from sega_emulator.audio.synchronization import AudioSynchronizer


class RecordingAudioDevice:
    def __init__(self) -> None:
        self.events: list[tuple[str, int]] = []
        self.write_log: list[tuple[int, int]] = []
        self.elapsed_tstates = 0

    def advance_clocks(self, clocks: int) -> None:
        self.events.append(("advance", clocks))
        self.elapsed_tstates += clocks

    def write(self, value: int) -> None:
        self.events.append(("write", value))
        self.write_log.append((self.elapsed_tstates, value))


def test_writes_split_audio_advance_at_their_absolute_tstates() -> None:
    device = RecordingAudioDevice()
    sync = AudioSynchronizer(device)
    sync.schedule_write(4, 0x90)
    sync.schedule_write(4, 0x85)
    sync.schedule_write(9, 0x12)

    sync.advance_to(6)
    sync.advance_to(12)

    assert device.events == [
        ("advance", 4),
        ("write", 0x90),
        ("write", 0x85),
        ("advance", 2),
        ("advance", 3),
        ("write", 0x12),
        ("advance", 3),
    ]
    assert device.write_log == [(4, 0x90), (4, 0x85), (9, 0x12)]
    assert device.elapsed_tstates == 12
    assert sync.current_tstate == 12
    assert sync.pending_events == ()


def test_event_at_current_tstate_is_applied_without_advancing_clock() -> None:
    device = RecordingAudioDevice()
    sync = AudioSynchronizer(device)
    sync.advance_to(5)
    device.events.clear()

    sync.schedule_write(5, 0x9F)
    sync.advance_to(5)

    assert device.events == [("write", 0x9F)]
    assert sync.current_tstate == 5


def test_synchronizer_is_invariant_to_advance_chunking() -> None:
    def run(chunks: tuple[int, ...]) -> tuple[list[tuple[int, int]], int]:
        device = RecordingAudioDevice()
        sync = AudioSynchronizer(device)
        sync.schedule_write(3, 0x90)
        sync.schedule_write(8, 0x85)
        target = 0
        for delta in chunks:
            target += delta
            sync.advance_to(target)
        return device.write_log, device.elapsed_tstates

    assert run((11,)) == run((2, 1, 3, 5))


@pytest.mark.parametrize(
    ("tstate", "value"),
    [(-1, 0x90), (1.5, 0x90), (0, -1), (0, 0x100)],
)
def test_rejects_invalid_event_fields(tstate: int, value: int) -> None:
    sync = AudioSynchronizer(RecordingAudioDevice())
    with pytest.raises(ValueError):
        sync.schedule_write(tstate, value)


def test_rejects_out_of_order_or_past_events() -> None:
    sync = AudioSynchronizer(RecordingAudioDevice())
    sync.schedule_write(8, 0x90)
    with pytest.raises(ValueError):
        sync.schedule_write(7, 0x91)
    sync.advance_to(8)
    with pytest.raises(ValueError):
        sync.schedule_write(7, 0x92)
    with pytest.raises(ValueError):
        sync.advance_to(7)
