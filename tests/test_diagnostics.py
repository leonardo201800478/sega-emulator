"""Regression tests for actionable text-session diagnostics."""

import logging

from sega_emulator.diagnostics import SessionTextFormatter


def test_text_formatter_includes_frame_performance_metrics() -> None:
    """Keep the human-readable log useful without requiring the JSONL sidecar."""
    record = logging.LogRecord(
        name="sega_emulator.frontend.qt_frontend",
        level=logging.INFO,
        pathname=__file__,
        lineno=1,
        msg="Resumo de desempenho",
        args=(),
        exc_info=None,
    )
    record.event = "frame_checkpoint"
    record.frame = 120
    record.fps = 59.8
    record.speed_percent = 99.7
    record.instructions = 12345
    record.tstates_delta = 214772
    record.elapsed_ms = 1001.2
    record.phase_avg_ms = {"emulation": 2.1, "image_copy": 0.4}
    record.phase_max_ms = {"emulation": 4.8, "image_copy": 1.2}
    record.pc = 0x1234

    formatted = SessionTextFormatter("%(message)s").format(record)

    assert "event=frame_checkpoint" in formatted
    assert "frame=120" in formatted
    assert "fps=59.8" in formatted
    assert "speed_percent=99.7" in formatted
    assert "phase_avg_ms=" in formatted
    assert "phase_max_ms=" in formatted
    assert "pc=4660" in formatted
