"""Windows WASAPI PCM output for the Qt frontend.

The device is opened through PortAudio's WASAPI host API via sounddevice.
The emulated PSG remains independent from this real-time output backend.
"""

from __future__ import annotations

import logging
import queue
from array import array
from typing import Any

logger = logging.getLogger(__name__)


class WasapiAudioOutput:
    """Bounded, mono PCM16 output stream using the Windows WASAPI host API.

    PCM is queued by the emulation/UI thread. PortAudio's callback only drains
    queued bytes and fills shortages with silence; it never runs emulation.
    """

    def __init__(
        self,
        sample_rate: int,
        buffer_size: int,
        volume: int = 100,
        *,
        queue_blocks: int = 8,
        sounddevice_module: Any | None = None,
    ) -> None:
        if type(sample_rate) is not int or sample_rate <= 0:
            raise ValueError("sample_rate deve ser um inteiro positivo")
        if type(buffer_size) is not int or buffer_size <= 0:
            raise ValueError("buffer_size deve ser um inteiro positivo")
        if type(volume) is not int or not 0 <= volume <= 100:
            raise ValueError("volume deve estar entre 0 e 100")
        if type(queue_blocks) is not int or queue_blocks < 1:
            raise ValueError("queue_blocks deve ser um inteiro positivo")

        if sounddevice_module is None:
            try:
                import sounddevice as sounddevice_module
            except ImportError as exc:
                raise RuntimeError(
                    "Saída WASAPI indisponível: instale a dependência sounddevice."
                ) from exc

        self._sd = sounddevice_module
        self.sample_rate = sample_rate
        self.buffer_size = buffer_size
        self._volume = volume / 100.0
        self._queue: queue.Queue[bytes] = queue.Queue(maxsize=queue_blocks)
        self._current_block = b""
        self._current_offset = 0
        self.underrun_callbacks = 0
        self.dropped_blocks = 0
        self._closed = False

        device = self._find_wasapi_default_output()
        try:
            extra_settings = self._sd.WasapiSettings(
                exclusive=False,
                auto_convert=True,
            )
            self._stream = self._sd.RawOutputStream(
                samplerate=sample_rate,
                blocksize=buffer_size,
                device=device,
                channels=1,
                dtype="int16",
                callback=self._callback,
                extra_settings=extra_settings,
            )
            self._stream.start()
        except Exception as exc:
            raise RuntimeError(f"Não foi possível abrir a saída WASAPI: {exc}") from exc

        logger.info(
            "Saída WASAPI aberta",
            extra={
                "event": "wasapi_output_opened",
                "device_index": device,
                "sample_rate": sample_rate,
                "buffer_size": buffer_size,
                "shared_mode": True,
            },
        )

    def _find_wasapi_default_output(self) -> int:
        """Resolve the default output device specifically from the WASAPI host."""
        try:
            host_apis = self._sd.query_hostapis()
            devices = self._sd.query_devices()
        except Exception as exc:
            raise RuntimeError(
                f"Falha ao enumerar dispositivos de áudio: {exc}"
            ) from exc

        for host_index, host_api in enumerate(host_apis):
            if "wasapi" not in str(host_api.get("name", "")).casefold():
                continue
            device_index = host_api.get("default_output_device", -1)
            if isinstance(device_index, int) and device_index >= 0:
                if device_index < len(devices):
                    return device_index
            for index, device in enumerate(devices):
                if (
                    device.get("hostapi") == host_index
                    and device.get("max_output_channels", 0) > 0
                ):
                    return index
        raise RuntimeError(
            "O host WASAPI não foi encontrado ou não possui dispositivo de saída."
        )

    def write_pcm(self, pcm: bytes) -> None:
        """Queue one mono signed-16-bit PCM block without waiting for the device."""
        if self._closed or not pcm:
            return
        if len(pcm) % 2:
            raise ValueError("PCM16 deve conter um número par de bytes")

        if self._volume != 1.0:
            samples = array("h")
            samples.frombytes(pcm)
            for index, sample in enumerate(samples):
                samples[index] = max(-32768, min(32767, round(sample * self._volume)))
            pcm = samples.tobytes()

        try:
            self._queue.put_nowait(pcm)
        except queue.Full:
            # Prefer recent audio to unbounded latency after a slow frame.
            try:
                self._queue.get_nowait()
                self.dropped_blocks += 1
            except queue.Empty:
                pass
            try:
                self._queue.put_nowait(pcm)
            except queue.Full:
                self.dropped_blocks += 1

    def set_volume(self, volume: int) -> None:
        """Set output gain without touching PSG state or clocks."""
        if type(volume) is not int or not 0 <= volume <= 100:
            raise ValueError("volume deve estar entre 0 e 100")
        self._volume = volume / 100.0

    def _callback(
        self,
        outdata: Any,
        frames: int,
        time_info: Any,
        status: Any,
    ) -> None:
        del time_info
        underflow_reported = bool(status and getattr(status, "output_underflow", False))
        required = frames * 2
        output = memoryview(outdata).cast("B")
        output[:] = b"\\x00" * required
        written = 0
        while written < required:
            if self._current_offset >= len(self._current_block):
                try:
                    self._current_block = self._queue.get_nowait()
                    self._current_offset = 0
                except queue.Empty:
                    underflow_reported = True
                    break
            available = len(self._current_block) - self._current_offset
            count = min(required - written, available)
            start = self._current_offset
            output[written : written + count] = self._current_block[
                start : start + count
            ]
            self._current_offset += count
            written += count
        if underflow_reported:
            self.underrun_callbacks += 1

    def close(self) -> None:
        """Stop and release the WASAPI stream; safe to call more than once."""
        if self._closed:
            return
        self._closed = True
        try:
            self._stream.stop()
        finally:
            self._stream.close()
        logger.info(
            "Saída WASAPI encerrada",
            extra={
                "event": "wasapi_output_closed",
                "underrun_callbacks": self.underrun_callbacks,
                "dropped_blocks": self.dropped_blocks,
            },
        )

    def __enter__(self) -> WasapiAudioOutput:
        return self

    def __exit__(self, exc_type: Any, exc: Any, traceback: Any) -> None:
        self.close()
