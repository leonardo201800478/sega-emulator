"""Ponto de entrada da aplicação Qt para o Sega SG-1000."""

from __future__ import annotations

import argparse
import logging
import sys
from collections.abc import Sequence
from pathlib import Path

from sega_emulator.diagnostics import configure_session_logging
from sega_emulator.rom_loader import list_rom_members


def main(argv: Sequence[str] | None = None) -> int:
    """Abre a interface Qt; o carregador também pode listar ROMs de um ZIP."""
    parser = argparse.ArgumentParser(
        description="Sega Emulator — interface Qt com runtime SG-1000 nativo em Rust",
        formatter_class=argparse.ArgumentDefaultsHelpFormatter,
    )
    parser.add_argument(
        "--version", action="version", version="Sega Emulator SG-1000 0.1.0"
    )
    parser.add_argument(
        "rom",
        type=Path,
        nargs="?",
        help="caminho para ROM .sg/.rom/.bin ou arquivo ZIP contendo ROM",
    )
    parser.add_argument(
        "--log-dir",
        type=Path,
        default=Path("logs"),
        help="diretório de logs da sessão (padrão: ./logs)",
    )
    parser.add_argument(
        "--log-level",
        choices=("DEBUG", "INFO", "WARNING", "ERROR", "CRITICAL"),
        default="INFO",
        type=str.upper,
        help="nível mínimo dos logs",
    )
    parser.add_argument(
        "--list-roms",
        type=Path,
        metavar="ZIP",
        help="lista ROMs compatíveis dentro de um ZIP e encerra",
    )
    args = parser.parse_args(list(sys.argv[1:] if argv is None else argv))

    if args.list_roms is not None:
        try:
            members = list_rom_members(args.list_roms)
        except ValueError as exc:
            parser.error(str(exc))
        import json

        print(
            json.dumps(
                {"zip": str(args.list_roms), "roms": members},
                ensure_ascii=False,
                indent=2,
            )
        )
        return 0

    configure_session_logging(args.log_dir, level=getattr(logging, args.log_level))
    try:
        from sega_emulator.frontend.qt_frontend import run

        run(args.rom)
    except (RuntimeError, ImportError) as exc:
        parser.error(str(exc))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
