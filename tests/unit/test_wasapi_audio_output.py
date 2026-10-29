from __future__ import annotations

import sys
from types import SimpleNamespace

import pytest

from sega_emulator.frontend.qt.wasapi_audio import NativeAudioOutput, WasapiAudioOutput


class FakeNativeOutput:
    def __init__(self, sample_rate, buffer_size, queue_blocks, balance=0):
        self.sample_rate = sample_rate
        self.buffer_size = buffer_size
        self.queue_blocks = queue_blocks
        self.balance = balance
        self.underrun_count = 3
        self.dropped_block_count = 2
        self.blocks = []
        self.closed = False
        self.close_calls = 0

    def write_pcm(self, pcm):
        self.blocks.append(pcm)

    def close(self):
        self.close_calls += 1
        self.closed = True


@pytest.fixture
def native_outputs(monkeypatch):
    instances = []

    def factory(sample_rate, buffer_size, queue_blocks, balance=0):
        instance = FakeNativeOutput(sample_rate, buffer_size, queue_blocks, balance)
        instances.append(instance)
        return instance

    monkeypatch.setitem(
        sys.modules,
        "sega_z80_core",
        SimpleNamespace(
            NativeWasapiOutput=factory,
            NativeXAudio2Output=factory,
        ),
    )
    return instances


def test_constructs_native_wasapi_output(native_outputs):
    output = WasapiAudioOutput(48_000, 512, queue_blocks=6)

    native = native_outputs[0]
    assert native.sample_rate == 48_000
    assert native.buffer_size == 512
    assert native.queue_blocks == 6
    assert native.balance == 0
    assert output.sample_rate == 48_000
    assert output.buffer_size == 512
    output.close()
    assert native.closed


def test_pcm_is_forwarded_unchanged_to_native_output(native_outputs):
    output = WasapiAudioOutput(44_100, 256)
    pcm = b"\x01\x02\x03\x04"

    output.write_pcm(pcm)

    assert native_outputs[0].blocks == [pcm]
    assert output.underrun_callbacks == 3
    assert output.dropped_blocks == 2
    output.close()


def test_rejects_non_bytes_pcm(native_outputs):
    output = WasapiAudioOutput(44_100, 256)
    with pytest.raises(TypeError, match="bytes"):
        output.write_pcm(bytearray(b"\x00\x00"))
    output.close()


@pytest.mark.parametrize(
    ("sample_rate", "buffer_size", "queue_blocks", "message"),
    [
        (0, 512, 8, "sample_rate"),
        (44_100, 0, 8, "buffer_size"),
        (44_100, 512, 0, "queue_blocks"),
    ],
)
def test_invalid_configuration_fails_before_native_open(
    sample_rate, buffer_size, queue_blocks, message
):
    with pytest.raises(ValueError, match=message):
        WasapiAudioOutput(sample_rate, buffer_size, queue_blocks=queue_blocks)


def test_selects_native_xaudio2_when_requested(native_outputs):
    output = NativeAudioOutput(48_000, 512, backend="xaudio2", balance=25)
    native = native_outputs[0]
    assert native.sample_rate == 48_000
    assert native.buffer_size == 512
    assert native.balance == 25
    output.close()


def test_rejects_unimplemented_backend_before_native_open(native_outputs):
    with pytest.raises(ValueError, match="Backend"):
        NativeAudioOutput(48_000, 512, backend="sdl3")
    assert native_outputs == []


def test_close_is_idempotent_and_logs_session_metrics(native_outputs, caplog) -> None:
    with caplog.at_level("INFO", logger="sega_emulator.frontend.qt.wasapi_audio"):
        output = NativeAudioOutput(
            48_000, 512, backend="xaudio2", queue_blocks=4, balance=-20
        )
        output.close()
        output.close()

    native = native_outputs[0]
    assert native.close_calls == 1
    initialized = next(
        record
        for record in caplog.records
        if record.message == "Native audio backend initialized"
    )
    assert initialized.audio_backend == "xaudio2"
    assert initialized.audio_sample_rate == 48_000
    assert initialized.audio_balance == -20
    metrics = [
        record
        for record in caplog.records
        if record.message == "Native audio backend session metrics"
    ]
    assert len(metrics) == 1
    assert metrics[0].audio_underruns == 3
    assert metrics[0].audio_dropped_blocks == 2
