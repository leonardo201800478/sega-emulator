"""Ponto de entrada CLI para a máquina SG-1000."""

from __future__ import annotations

import argparse
import json
import logging
import sys
import time
from collections.abc import Sequence
from pathlib import Path

from sega_emulator.diagnostics import configure_session_logging
from sega_emulator.machine.sg1000 import SG1000Machine
from sega_emulator.rom_loader import list_rom_members, load_rom


def main(argv: Sequence[str] | None = None) -> int:
    """Carrega uma ROM e abre a interface, ou executa em modo headless."""
    arguments = list(sys.argv[1:] if argv is None else argv)
    parser = argparse.ArgumentParser(description="Emulador experimental Sega SG-1000", formatter_class=argparse.ArgumentDefaultsHelpFormatter)
    parser.add_argument("--version", action="version", version="Sega Emulator SG-1000 0.1.0")
    parser.add_argument(
        "rom",
        type=Path,
        nargs="?",
        help="caminho para ROM .sg ou arquivo .zip contendo ROM",
    )
    parser.add_argument(
        "--headless-frames",
        type=int,
        default=None,
        metavar="N",
        help="executa N quadros sem abrir janela; N deve ser >= 1",
    )
    parser.add_argument(
        "--ui",
        choices=("qt", "pygame"),
        default="qt",
        help="frontend gráfico (padrão: qt)",
    )
    parser.add_argument(
        "--log-dir",
        type=Path,
        default=Path("logs"),
        help="diretório de logs da sessão (padrão: ./logs)",
    )
    parser.add_argument(
        "--log-level", choices=("DEBUG", "INFO", "WARNING", "ERROR", "CRITICAL"),
        default="INFO", type=str.upper, help="nível mínimo dos logs",
    )
    parser.add_argument("--list-roms", type=Path, metavar="ZIP",
                        help="lista ROMs compatíveis dentro de um ZIP e encerra")
    parser.add_argument("--summary-json", type=Path, metavar="ARQUIVO",
                        help="grava resumo JSON da execução headless")
    args = parser.parse_args(arguments)

    if args.list_roms is not None:
        try:
            members = list_rom_members(args.list_roms)
        except ValueError as exc:
            parser.error(str(exc))
        print(json.dumps({"zip": str(args.list_roms), "roms": members},
                         ensure_ascii=False, indent=2))
        return 0
    if args.headless_frames is not None and args.headless_frames < 1:
        parser.error("--headless-frames deve ser maior ou igual a 1")
    if args.summary_json is not None and args.headless_frames is None:
        parser.error("--summary-json exige --headless-frames")
    if args.rom is None and args.headless_frames is not None:
        parser.error("informe uma ROM quando usar --headless-frames")
    if args.rom is None and args.ui == "pygame":
        parser.error("informe uma ROM ao usar --ui pygame")

    configure_session_logging(args.log_dir, level=getattr(logging, args.log_level))
    logger = logging.getLogger(__name__)

    if args.ui == "qt" and args.headless_frames is None:
        try:
            from sega_emulator.frontend.qt_frontend import run

            run(args.rom)
        except (RuntimeError, ImportError) as exc:
            parser.error(str(exc))
        return 0

    try:
        loaded = load_rom(args.rom)
        logger.info(
            "ROM carregada",
            extra={
                "event": "rom_loaded",
                "rom_path": str(loaded.source_path),
                "rom_member": loaded.member_name,
                "rom_sha256": loaded.sha256,
            },
        )
        machine = SG1000Machine(loaded.data, cpu_core="rust")
        logger.info(
            "Máquina criada",
            extra={
                "event": "machine_created",
                "rom_sha256": loaded.sha256,
                "rom_path": str(loaded.source_path),
            },
        )
        if args.headless_frames is not None:
            started = time.perf_counter()
            start_tstates = machine.total_tstates
            start_instructions = machine.total_instructions
            start_frame = machine.vdp.frame_number
            machine.run_frames(args.headless_frames)
            elapsed_seconds = max(time.perf_counter() - started, 1e-12)
            frame_delta = machine.vdp.frame_number - start_frame
            tstates_delta = machine.total_tstates - start_tstates
            instruction_delta = machine.total_instructions - start_instructions
            speed_percent = tstates_delta / elapsed_seconds / 3_579_545 * 100
            summary = {
                "schema_version": 1,
                "mode": "headless",
                "rom": {"path": str(loaded.source_path), "member": loaded.member_name,
                        "sha256": loaded.sha256, "size_bytes": len(loaded.data)},
                "measurement": {
                    "elapsed_seconds": round(elapsed_seconds, 9),
                    "frames": frame_delta,
                    "frames_per_second": round(frame_delta / elapsed_seconds, 6),
                    "instructions": instruction_delta,
                    "instructions_total": machine.total_instructions,
                    "tstates": tstates_delta,
                    "tstates_total": machine.total_tstates,
                    "speed_percent_of_nominal_z80_clock": round(speed_percent, 6),
                },
                "state": {"frame": machine.vdp.frame_number, "pc": machine.cpu.pc,
                          "scanline": machine.vdp.scanline, "status": machine.vdp.status},
            }
            logger.info("Métricas da execução headless", extra={
                "event": "headless_metrics", "elapsed_ms": round(elapsed_seconds * 1000, 3),
                "frame": frame_delta, "instructions": instruction_delta,
                "tstates": tstates_delta, "fps": summary["measurement"]["frames_per_second"],
                "speed_percent": round(speed_percent, 6), "rom_sha256": loaded.sha256,
            })
            if args.summary_json is not None:
                args.summary_json.parent.mkdir(parents=True, exist_ok=True)
                args.summary_json.write_text(
                    json.dumps(summary, ensure_ascii=False, indent=2) + "\n",
                    encoding="utf-8",
                )
            logger.info(
                "Execução headless concluída",
                extra={
                    "event": "headless_complete",
                    "frame": machine.vdp.frame_number,
                    "pc": machine.cpu.pc,
                    "instructions": machine.total_instructions,
                    "tstates": machine.total_tstates,
                    "scanline": machine.vdp.scanline,
                    "status": machine.vdp.status,
                },
            )
            print(
                f"Execução headless concluída: quadros={frame_delta}, "
                f"instruções={instruction_delta}, PC=0x{machine.cpu.pc:04X}, "
                f"T-states={tstates_delta}, velocidade={speed_percent:.2f}% do clock nominal, "
                f"duração={elapsed_seconds:.3f}s, ROM SHA-256={loaded.sha256}"
            )
            if args.summary_json is not None:
                print(f"Resumo JSON: {args.summary_json.resolve()}")
            return 0

        from sega_emulator.frontend.pygame_frontend import run

        run(machine)
        return 0
    except (OSError, ValueError, RuntimeError, NotImplementedError) as exc:
        logger.exception(
            "Falha ao iniciar sessão",
            extra={
                "event": "startup_failed",
                "rom_path": str(args.rom),
            },
        )
        parser.error(str(exc))
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
