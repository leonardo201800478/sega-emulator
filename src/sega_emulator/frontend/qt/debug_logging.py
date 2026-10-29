"""Opt-in asynchronous application log writer for the Qt Debug tab.

The logging handler only performs a non-blocking queue insertion. Formatting and
file I/O happen on a dedicated daemon thread, never on the emulation path.
"""

from __future__ import annotations

import logging
import queue
import threading
import time
from logging.handlers import RotatingFileHandler
from pathlib import Path
from typing import Callable

from sega_emulator.diagnostics import JsonLineFormatter


class _BoundedQueueHandler(logging.Handler):
    """Keep producer-side logging bounded and free of formatting/file I/O."""

    def __init__(
        self,
        records: queue.Queue[logging.LogRecord],
        on_drop: Callable[[], None],
    ) -> None:
        super().__init__(logging.DEBUG)
        self._records = records
        self._on_drop = on_drop

    def emit(self, record: logging.LogRecord) -> None:
        try:
            self._records.put_nowait(record)
        except queue.Full:
            self._on_drop()


class DebugLogController:
    """Session-scoped opt-in logger with a bounded queue and rotating JSONL file."""

    def __init__(
        self,
        *,
        log_dir: str | Path = "logs",
        on_lines: Callable[[str], None],
        queue_capacity: int = 2048,
        max_bytes: int = 8 * 1024 * 1024,
        backup_count: int = 3,
    ) -> None:
        if queue_capacity < 1:
            raise ValueError("queue_capacity must be positive")
        if max_bytes < 1:
            raise ValueError("max_bytes must be positive")
        if backup_count < 0:
            raise ValueError("backup_count cannot be negative")
        self.log_dir = Path(log_dir)
        self.on_lines = on_lines
        self.queue_capacity = queue_capacity
        self.max_bytes = max_bytes
        self.backup_count = backup_count
        self.log_path: Path | None = None
        self._records: queue.Queue[logging.LogRecord] = queue.Queue(
            maxsize=queue_capacity
        )
        self._stop_event = threading.Event()
        self._thread: threading.Thread | None = None
        self._handler: _BoundedQueueHandler | None = None
        self._root = logging.getLogger()
        self._previous_level = self._root.level
        self._drop_lock = threading.Lock()
        self._dropped_records = 0
        self._reported_drops = 0

    @property
    def dropped_records(self) -> int:
        with self._drop_lock:
            return self._dropped_records

    def _count_drop(self) -> None:
        with self._drop_lock:
            self._dropped_records += 1

    def start(self) -> bool:
        """Start capture only on explicit user request; return whether active."""
        if self._handler is not None:
            return True
        if self._thread is not None and self._thread.is_alive():
            self._thread.join(timeout=0.05)
            if self._thread.is_alive():
                return False

        self._records = queue.Queue(maxsize=self.queue_capacity)
        self._stop_event = threading.Event()
        self._dropped_records = 0
        self._reported_drops = 0
        stamp = time.strftime("%Y%m%d-%H%M%S", time.localtime())
        self.log_path = self.log_dir / f"debug-{stamp}.jsonl"
        self._previous_level = self._root.level
        handler = _BoundedQueueHandler(self._records, self._count_drop)
        self._handler = handler
        self._root.addHandler(handler)
        self._root.setLevel(logging.DEBUG)
        self._thread = threading.Thread(
            target=self._writer_loop,
            name="sega-debug-log-writer",
            daemon=True,
        )
        self._thread.start()
        return True

    def stop(self) -> None:
        """Detach the producer immediately and let the writer drain asynchronously."""
        handler = self._handler
        if handler is None:
            return
        self._root.removeHandler(handler)
        handler.close()
        self._handler = None
        if self._root.level == logging.DEBUG:
            self._root.setLevel(self._previous_level)
        self._stop_event.set()

    def wait(self, timeout: float | None = None) -> None:
        """Wait for writer shutdown, intended for application exit only."""
        thread = self._thread
        if thread is not None and thread.is_alive():
            thread.join(timeout)

    def _writer_loop(self) -> None:
        assert self.log_path is not None
        file_handler: RotatingFileHandler | None = None
        try:
            self.log_dir.mkdir(parents=True, exist_ok=True)
            file_handler = RotatingFileHandler(
                self.log_path,
                maxBytes=self.max_bytes,
                backupCount=self.backup_count,
                encoding="utf-8",
            )
            file_handler.setFormatter(JsonLineFormatter())
            display_formatter = logging.Formatter(
                "%(asctime)s %(levelname)s %(name)s: %(message)s",
                datefmt="%H:%M:%S",
            )
            batch: list[str] = []
            last_emit = time.monotonic()
            while not self._stop_event.is_set() or not self._records.empty():
                try:
                    record = self._records.get(timeout=0.1)
                except queue.Empty:
                    record = None
                if record is not None:
                    file_handler.emit(record)
                    batch.append(display_formatter.format(record))
                dropped = self.dropped_records
                if dropped > self._reported_drops:
                    delta = dropped - self._reported_drops
                    self._reported_drops = dropped
                    warning = (
                        f"WARNING debug queue full: {delta} record(s) discarded; "
                        "this capture is incomplete."
                    )
                    batch.append(warning)
                    dropped_record = logging.LogRecord(
                        name="sega_emulator.debug",
                        level=logging.WARNING,
                        pathname=__file__,
                        lineno=0,
                        msg=warning,
                        args=(),
                        exc_info=None,
                    )
                    dropped_record.event = "debug_records_dropped"
                    dropped_record.dropped_records = delta
                    file_handler.emit(dropped_record)
                now = time.monotonic()
                if batch and (len(batch) >= 64 or now - last_emit >= 0.1):
                    self.on_lines("\n".join(batch))
                    batch.clear()
                    last_emit = now
            if batch:
                self.on_lines("\n".join(batch))
        except OSError as exc:
            self.on_lines(f"ERROR: não foi possível gravar logs de Debug: {exc}")
        finally:
            if file_handler is not None:
                file_handler.flush()
                file_handler.close()
