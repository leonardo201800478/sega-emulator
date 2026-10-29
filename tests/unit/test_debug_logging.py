"""Tests for opt-in asynchronous Debug logging."""

from __future__ import annotations

import json
import logging

from sega_emulator.frontend.qt.debug_logging import DebugLogController


def test_debug_logging_is_opt_in_and_writes_jsonl_off_thread(tmp_path) -> None:
    root = logging.getLogger()
    original_level = root.level
    lines: list[str] = []
    controller = DebugLogController(
        log_dir=tmp_path,
        on_lines=lines.append,
        queue_capacity=32,
        max_bytes=4096,
        backup_count=1,
    )
    assert controller.log_path is None
    assert controller.start()
    path = controller.log_path
    assert path is not None

    logging.getLogger("test.debug.capture").info(
        "diagnostic event", extra={"event": "test_event"}
    )
    controller.stop()
    controller.wait(2.0)

    assert path.exists()
    records = [
        json.loads(line) for line in path.read_text(encoding="utf-8").splitlines()
    ]
    assert any(record["message"] == "diagnostic event" for record in records)
    assert any("diagnostic event" in batch for batch in lines)
    assert root.level == original_level


def test_debug_logging_queue_capacity_is_validated(tmp_path) -> None:
    try:
        DebugLogController(log_dir=tmp_path, on_lines=lambda _: None, queue_capacity=0)
    except ValueError as exc:
        assert "queue_capacity" in str(exc)
    else:
        raise AssertionError("queue_capacity=0 must be rejected")



def test_silent_default_logging_uses_null_handler_without_existing_handlers() -> None:
    from logging import Logger, NullHandler

    from sega_emulator.frontend.qt.debug_logging import install_silent_default_logging

    target = Logger("test.silent.default")
    install_silent_default_logging(target)
    assert len(target.handlers) == 1
    assert isinstance(target.handlers[0], NullHandler)
