"""Testes da ferramenta de diagnóstico de runtime do SG-1000."""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path


def test_runtime_diagnostics_emits_baseline_phases_and_frame_hashes(
    tmp_path: Path,
) -> None:
    repository = Path(__file__).resolve().parents[1]
    rom_path = tmp_path / "loop.sg"
    report_path = tmp_path / "runtime.json"
    # JP 0000h: laço determinístico para exercitar CPU, VDP e limites de quadro.
    rom_path.write_bytes(bytes((0xC3, 0x00, 0x00)))

    completed = subprocess.run(
        [
            sys.executable,
            str(repository / "tools" / "benchmarks" / "runtime_diagnostics.py"),
            "--rom",
            str(rom_path),
            "--warmup-frames",
            "0",
            "--frames",
            "2",
            "--output",
            str(report_path),
        ],
        capture_output=True,
        text=True,
        check=False,
    )

    assert completed.returncode == 0, completed.stdout + completed.stderr
    report = json.loads(report_path.read_text(encoding="utf-8"))
    assert report["baseline"]["fps"] > 0
    assert report["instrumented"]["calls"]["cpu.step"] > 0
    assert report["instrumented"]["calls"]["vdp.advance"] > 0
    assert report["instrumented"]["calls"]["vdp._render_scanline"] > 0
    assert len(report["instrumented"]["frames"]) == 2
    assert all(
        len(frame["framebuffer_sha256"]) == 64
        for frame in report["instrumented"]["frames"]
    )
