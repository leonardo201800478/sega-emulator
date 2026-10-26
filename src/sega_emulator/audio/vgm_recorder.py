"""Deterministic VGM writer for timestamped SN76489 writes.

This module records chip-register traffic, not rendered PCM. Timestamps are
SG-1000 Z80 T-states; the VGM stream uses the standard 44.1 kHz sample clock.
"""

from __future__ import annotations

import struct
from pathlib import Path

VGM_CLOCK_HZ = 3_579_545
VGM_SAMPLE_RATE = 44_100
VGM_VERSION = 0x00000171
VGM_HEADER_SIZE = 0x40


class VgmRecorder:
    """Collect PSG writes and serialize them as a VGM 1.71 file."""

    def __init__(self, clock_hz: int = VGM_CLOCK_HZ) -> None:
        if type(clock_hz) is not int or clock_hz <= 0:
            raise ValueError("clock_hz deve ser um inteiro positivo.")
        self.clock_hz = clock_hz
        self._events: list[tuple[int, int]] = []
        self._start_tstate = 0
        self._last_tstate = 0
        self._active = False

    @property
    def active(self) -> bool:
        return self._active

    @property
    def event_count(self) -> int:
        return len(self._events)

    def start(self, start_tstate: int) -> None:
        if type(start_tstate) is not int or start_tstate < 0:
            raise ValueError("start_tstate deve ser um inteiro não negativo.")
        self._events.clear()
        self._start_tstate = start_tstate
        self._last_tstate = start_tstate
        self._active = True

    def record_write(self, tstate: int, value: int) -> None:
        if not self._active:
            return
        if type(tstate) is not int or tstate < self._start_tstate:
            raise ValueError("O timestamp deve ser >= ao início da gravação.")
        if type(value) is not int or not 0 <= value <= 0xFF:
            raise ValueError("O valor PSG deve ser um byte.")
        if tstate < self._last_tstate:
            raise ValueError("Os eventos PSG devem estar em ordem temporal.")
        self._events.append((tstate, value))
        self._last_tstate = tstate

    @staticmethod
    def _wait_commands(samples: int) -> bytes:
        commands = bytearray()
        while samples:
            if 1 <= samples <= 16:
                commands.append(0x70 + samples - 1)
                samples = 0
            else:
                chunk = min(samples, 0xFFFF)
                commands.extend((0x61, chunk & 0xFF, chunk >> 8))
                samples -= chunk
        return bytes(commands)

    def encode(self, end_tstate: int) -> bytes:
        if not self._active:
            raise RuntimeError("Não há gravação VGM ativa.")
        if type(end_tstate) is not int or end_tstate < self._last_tstate:
            raise ValueError("O fim deve ser posterior ao último evento PSG.")

        data = bytearray()
        cursor_samples = 0
        for tstate, value in self._events:
            target_samples = (
                (tstate - self._start_tstate) * VGM_SAMPLE_RATE // self.clock_hz
            )
            data.extend(self._wait_commands(target_samples - cursor_samples))
            data.extend((0x50, value))
            cursor_samples = target_samples

        total_samples = (
            (end_tstate - self._start_tstate) * VGM_SAMPLE_RATE // self.clock_hz
        )
        data.extend(self._wait_commands(total_samples - cursor_samples))
        data.append(0x66)

        header = bytearray(VGM_HEADER_SIZE)
        header[0:4] = b"Vgm "
        struct.pack_into("<I", header, 0x08, VGM_VERSION)
        struct.pack_into("<I", header, 0x0C, self.clock_hz)
        struct.pack_into("<I", header, 0x18, total_samples)
        struct.pack_into("<I", header, 0x34, VGM_HEADER_SIZE - 0x34)
        file_data = header + data
        struct.pack_into("<I", file_data, 0x04, len(file_data) - 4)
        self._active = False
        return bytes(file_data)

    def save(self, path: str | Path, end_tstate: int) -> Path:
        output = Path(path)
        payload = self.encode(end_tstate)
        try:
            output.write_bytes(payload)
        except OSError:
            self._active = True
            raise
        return output
