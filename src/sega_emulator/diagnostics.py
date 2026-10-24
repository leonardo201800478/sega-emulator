"""Logs estruturados para diagnóstico reproduzível do emulador."""

from __future__ import annotations

import json
import logging
import time
from pathlib import Path
from typing import Any

_session_handlers: list[logging.Handler] = []


class JsonLineFormatter(logging.Formatter):
    """Serializa eventos em JSON Lines, adequado para análise automatizada."""

    def format(self, record: logging.LogRecord) -> str:
        payload: dict[str, Any] = {
            "timestamp_utc": time.strftime(
                "%Y-%m-%dT%H:%M:%S", time.gmtime(record.created)
            )
            + f".{int(record.msecs):03d}Z",
            "level": record.levelname,
            "logger": record.name,
            "message": record.getMessage(),
        }
        for key in (
            "event",
            "rom_path",
            "rom_member",
            "rom_sha256",
            "frame",
            "pc",
            "instructions",
            "instructions_total",
            "tstates",
            "tstates_delta",
            "fps",
            "speed_percent",
            "scanline",
            "registers",
            "status",
            "elapsed_ms",
            "exception",
            "log_json",
            "log_text",
            "paused",
        ):
            if hasattr(record, key):
                payload[key] = getattr(record, key)
        if record.exc_info:
            payload["traceback"] = self.formatException(record.exc_info)
        return json.dumps(payload, ensure_ascii=False, separators=(",", ":"))


def configure_session_logging(
    log_dir: str | Path = "logs", level: int = logging.INFO
) -> Path:
    """Cria log JSONL por sessão e um log textual legível."""
    directory = Path(log_dir)
    directory.mkdir(parents=True, exist_ok=True)
    stamp = time.strftime("%Y%m%d-%H%M%S", time.localtime())
    json_path = directory / f"session-{stamp}.jsonl"
    text_path = directory / f"session-{stamp}.log"

    root = logging.getLogger()
    root.setLevel(level)
    # Reconfiguração idempotente sem remover handlers de pytest ou da aplicação.
    global _session_handlers
    for handler in _session_handlers:
        root.removeHandler(handler)
        handler.close()
    _session_handlers = []

    json_handler = logging.FileHandler(json_path, encoding="utf-8")
    json_handler.setFormatter(JsonLineFormatter())
    text_handler = logging.FileHandler(text_path, encoding="utf-8")
    text_handler.setFormatter(
        logging.Formatter("%(asctime)s %(levelname)s %(name)s: %(message)s")
    )
    console_handler = logging.StreamHandler()
    console_handler.setFormatter(logging.Formatter("%(levelname)s: %(message)s"))
    _session_handlers = [json_handler, text_handler, console_handler]
    for handler in _session_handlers:
        root.addHandler(handler)
    logging.getLogger(__name__).info(
        "Logging de sessão inicializado",
        extra={
            "event": "session_start",
            "log_json": str(json_path),
            "log_text": str(text_path),
        },
    )
    return json_path
