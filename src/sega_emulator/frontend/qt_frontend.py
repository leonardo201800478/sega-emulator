"""Interface experimental Qt 6/PySide6 para menus e diagnóstico do SG-1000."""

from __future__ import annotations

import hashlib
import logging
from pathlib import Path

from sega_emulator.frontend.qt.preferences import FrontendPreferences

logger = logging.getLogger(__name__)


def _runtime_diagnostic(machine: object, rom_sha256: str) -> dict[str, object]:
    """Return verifiable identity of the loaded emulator runtime and native binary."""
    import os
    import sys

    diagnostic: dict[str, object] = {
        "backend_requested": "rust",
        "machine_type": type(machine).__module__ + "." + type(machine).__qualname__,
        "cpu_type": type(machine.cpu).__module__ + "." + type(machine.cpu).__qualname__,
        "vdp_type": type(machine.vdp).__module__ + "." + type(machine.vdp).__qualname__,
        "psg_type": (
            type(machine.psg).__module__ + "." + type(machine.psg).__qualname__
            if hasattr(machine, "psg")
            else "native_sms_psg"
        ),
        "rom_sha256": rom_sha256,
        "python_executable": sys.executable,
        "python_version": sys.version,
        "process_id": os.getpid(),
    }
    native = getattr(machine, "native_machine", None)
    diagnostic["backend_effective"] = (
        "rust_native_machine"
        if native is not None
        and type(native).__name__ in {"NativeSG1000Machine", "NativeSMSMachine"}
        else "unexpected_backend_state"
    )
    diagnostic["native_machine_type"] = (
        type(native).__module__ + "." + type(native).__qualname__
        if native is not None
        else None
    )
    try:
        import sega_z80_core

        binary_path = Path(sega_z80_core.__file__).resolve()
        diagnostic["native_module"] = sega_z80_core.__name__
        diagnostic["native_binary_path"] = str(binary_path)
        diagnostic["native_binary_size"] = binary_path.stat().st_size
        diagnostic["native_binary_sha256"] = hashlib.sha256(
            binary_path.read_bytes()
        ).hexdigest()
        diagnostic["native_binary_mtime_ns"] = binary_path.stat().st_mtime_ns
    except Exception as exc:
        diagnostic["native_binary_inspection_error"] = (
            type(exc).__name__ + ": " + str(exc)
        )
    if native is not None:
        diagnostic["native_instruction_count"] = native.total_instructions
        diagnostic["native_tstates"] = native.total_tstates
        diagnostic["native_vdp_frame"] = (
            native.frame_number
            if hasattr(native, "frame_number")
            else native.vdp.frame_number
        )
    return diagnostic


def _configure_machine_audio(
    machine: object | None,
    preferences: FrontendPreferences,
    backend: str,
) -> str | None:
    """Configure the machine-owned Rust audio session; PCM never enters Python."""
    if machine is None:
        return None
    if not preferences.audio_enabled or backend == "disabled":
        machine.close_audio_output()
        return None
    try:
        machine.configure_audio_output(
            backend,
            preferences.audio_sample_rate,
            preferences.audio_buffer_size,
            preferences.audio_queue_blocks,
            preferences.audio_volume,
            preferences.audio_balance,
        )
    except Exception as exc:
        logger.warning("Saída de áudio %s indisponível: %s", backend, exc)
        try:
            machine.close_audio_output()
        except Exception:
            logger.exception("Falha ao encerrar a sessão nativa de áudio")
        return f"Áudio {backend.upper()} indisponível; consulte o log para detalhes."
    return None


def _audio_output_warning(
    preferences: FrontendPreferences,
    audio_active: bool,
    backend: str = "wasapi",
) -> str | None:
    """Describe a requested native audio session that is not active."""
    if preferences.audio_enabled and backend != "disabled" and not audio_active:
        backend_label = {"wasapi": "WASAPI", "xaudio2": "XAudio2"}.get(backend, backend)
        return f"Áudio {backend_label} indisponível; consulte o log para detalhes."
    return None


def run(initial_path: Path | None = None) -> None:
    """Reject the retired Python video frontend; presentation is Rust-owned."""
    del initial_path
    raise RuntimeError(
        "O frontend Qt legado foi desativado por ADR-0012. Python/PySide6 "
        "não recebe pixels, não executa stepping nem renderiza vídeo. "
        "O host D3D12 nativo em Rust precisa ser conectado antes de habilitar "
        "a apresentação do SG-1000."
    )
