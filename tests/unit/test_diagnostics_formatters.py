"""Regressions for preserving runtime identity in session logs."""

from __future__ import annotations

import json
import logging

from sega_emulator.diagnostics import JsonLineFormatter, SessionTextFormatter


def _runtime_record() -> logging.LogRecord:
    return logging.makeLogRecord(
        {
            "name": "sega_emulator.frontend.qt_frontend",
            "levelno": logging.INFO,
            "levelname": "INFO",
            "msg": "Identidade do runtime que executará a ROM",
            "args": (),
            "event": "emulation_runtime_identity",
            "backend_requested": "rust",
            "backend_effective": "rust_native_machine",
            "machine_type": "sega_emulator.machine.sg1000.SG1000Machine",
            "cpu_type": "sega_z80_core.RustZ80Cpu",
            "psg_type": "sega_z80_core.NativeSn76489",
            "audio_sync_type": "sega_z80_core.NativeAudioSynchronizer",
            "native_binary_path": "C:/venv/Lib/site-packages/sega_z80_core.pyd",
            "native_binary_sha256": "a" * 64,
            "rom_sha256": "b" * 64,
        }
    )


def test_jsonl_formatter_persists_backend_and_audio_identity() -> None:
    payload = json.loads(JsonLineFormatter().format(_runtime_record()))

    assert payload["event"] == "emulation_runtime_identity"
    assert payload["backend_requested"] == "rust"
    assert payload["backend_effective"] == "rust_native_machine"
    assert payload["psg_type"] == "sega_z80_core.NativeSn76489"
    assert payload["audio_sync_type"] == "sega_z80_core.NativeAudioSynchronizer"
    assert payload["native_binary_sha256"] == "a" * 64
    assert payload["rom_sha256"] == "b" * 64


def test_text_formatter_persists_backend_and_psg_identity() -> None:
    rendered = SessionTextFormatter("%(levelname)s %(message)s").format(
        _runtime_record()
    )

    assert "backend_effective=rust_native_machine" in rendered
    assert "psg_type=sega_z80_core.NativeSn76489" in rendered
    assert f"native_binary_sha256={'a' * 64}" in rendered
