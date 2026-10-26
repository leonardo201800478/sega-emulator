from __future__ import annotations

import struct

import pytest

from sega_emulator.audio.vgm_recorder import VgmRecorder


def test_vgm_header_and_timestamped_sn76489_writes() -> None:
    recorder = VgmRecorder()
    recorder.start(1000)
    recorder.record_write(1000, 0x90)
    recorder.record_write(1000 + 3_579_545, 0x9F)
    payload = recorder.encode(1000 + 3_579_545)

    assert payload[:4] == b"Vgm "
    assert struct.unpack_from("<I", payload, 0x08)[0] == 0x171
    assert struct.unpack_from("<I", payload, 0x0C)[0] == 3_579_545
    assert struct.unpack_from("<I", payload, 0x18)[0] == 44_100
    assert struct.unpack_from("<I", payload, 0x04)[0] == len(payload) - 4
    assert struct.unpack_from("<I", payload, 0x34)[0] == 0x0C
    assert payload[0x40:0x42] == bytes((0x50, 0x90))
    assert bytes((0x50, 0x9F)) in payload
    assert payload[-1] == 0x66
    assert not recorder.active


def test_vgm_waits_cover_long_intervals_and_finish_at_requested_time() -> None:
    recorder = VgmRecorder()
    recorder.start(0)
    recorder.record_write(3_579_545 * 2, 0x80)
    payload = recorder.encode(3_579_545 * 3)

    assert struct.unpack_from("<I", payload, 0x18)[0] == 132_300
    assert bytes((0x50, 0x80)) in payload
    assert payload[-1] == 0x66
    assert b"\x61\xff\xff" in payload


def test_vgm_recorder_rejects_invalid_or_out_of_order_events() -> None:
    recorder = VgmRecorder()
    with pytest.raises(ValueError):
        recorder.start(-1)
    recorder.start(10)
    with pytest.raises(ValueError):
        recorder.record_write(9, 0x80)
    with pytest.raises(ValueError):
        recorder.record_write(10, 0x100)
    recorder.record_write(20, 0x80)
    with pytest.raises(ValueError):
        recorder.record_write(19, 0x90)
    with pytest.raises(ValueError):
        recorder.encode(19)


def test_vgm_writer_can_save_file(tmp_path) -> None:
    recorder = VgmRecorder()
    recorder.start(50)
    recorder.record_write(50, 0x9F)
    output = recorder.save(tmp_path / "capture.vgm", 50)

    assert output.read_bytes().startswith(b"Vgm ")
    assert output.stat().st_size >= 0x41
