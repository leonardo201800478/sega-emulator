from __future__ import annotations

import pytest

from sega_emulator.frontend.qt.wasapi_audio import WasapiAudioOutput


class FakeStream:
    def __init__(self, **kwargs):
        self.kwargs = kwargs
        self.started = False
        self.stopped = False
        self.closed = False

    def start(self):
        self.started = True

    def stop(self):
        self.stopped = True

    def close(self):
        self.closed = True


class FakeSoundDevice:
    def __init__(self, host_apis=None, devices=None):
        self._host_apis = host_apis or [
            {"name": "MME", "default_output_device": 0},
            {"name": "Windows WASAPI", "default_output_device": 1},
        ]
        self._devices = devices or [
            {"hostapi": 0, "max_output_channels": 2},
            {"hostapi": 1, "max_output_channels": 2},
        ]
        self.streams = []
        self.settings = []

    def query_hostapis(self):
        return self._host_apis

    def query_devices(self):
        return self._devices

    def WasapiSettings(self, **kwargs):
        self.settings.append(kwargs)
        return kwargs

    def RawOutputStream(self, **kwargs):
        stream = FakeStream(**kwargs)
        self.streams.append(stream)
        return stream


def test_opens_default_wasapi_device_in_shared_mode() -> None:
    sounddevice = FakeSoundDevice()
    output = WasapiAudioOutput(48_000, 512, sounddevice_module=sounddevice)

    stream = sounddevice.streams[0]
    assert stream.started
    assert stream.kwargs["device"] == 1
    assert stream.kwargs["samplerate"] == 48_000
    assert stream.kwargs["blocksize"] == 512
    assert stream.kwargs["channels"] == 1
    assert stream.kwargs["dtype"] == "int16"
    assert sounddevice.settings == [{"exclusive": False, "auto_convert": True}]
    output.close()
    assert stream.stopped and stream.closed


def test_requires_wasapi_host_api() -> None:
    sounddevice = FakeSoundDevice(
        host_apis=[{"name": "MME", "default_output_device": 0}],
        devices=[{"hostapi": 0, "max_output_channels": 2}],
    )
    with pytest.raises(RuntimeError, match="WASAPI"):
        WasapiAudioOutput(44_100, 512, sounddevice_module=sounddevice)


def test_callback_drains_pcm_across_callbacks_and_silences_shortage() -> None:
    output = WasapiAudioOutput(44_100, 4, sounddevice_module=FakeSoundDevice())
    output.write_pcm(bytes([1, 2, 3, 4, 5, 6, 7, 8]))

    first = bytearray(4)
    output._callback(first, 2, None, None)
    assert first == bytes([1, 2, 3, 4])

    second = bytearray(8)
    output._callback(second, 4, None, None)
    assert second == bytes([5, 6, 7, 8, 0, 0, 0, 0])
    assert output.underrun_callbacks == 1
    output.close()


def test_volume_is_applied_before_audio_callback() -> None:
    sounddevice = FakeSoundDevice()
    output = WasapiAudioOutput(44_100, 4, 50, sounddevice_module=sounddevice)
    output.write_pcm((1000).to_bytes(2, "little", signed=True))
    target = bytearray(2)
    output._callback(target, 1, None, None)
    assert int.from_bytes(target, "little", signed=True) == 500
    output.set_volume(0)
    output.close()


def test_rejects_invalid_pcm_and_volume() -> None:
    output = WasapiAudioOutput(44_100, 4, sounddevice_module=FakeSoundDevice())
    with pytest.raises(ValueError, match="par de bytes"):
        output.write_pcm(b"\x01")
    with pytest.raises(ValueError, match="volume"):
        output.set_volume(101)
    output.close()


def test_invalid_configuration_fails_before_opening_stream() -> None:
    with pytest.raises(ValueError, match="sample_rate"):
        WasapiAudioOutput(0, 512, sounddevice_module=FakeSoundDevice())
    with pytest.raises(ValueError, match="buffer_size"):
        WasapiAudioOutput(44_100, 0, sounddevice_module=FakeSoundDevice())
