"""Ponto de entrada CLI para a máquina SG-1000."""

from __future__ import annotations

import argparse
import logging
import sys
from collections.abc import Sequence
from pathlib import Path

from sega_emulator.diagnostics import configure_session_logging
from sega_emulator.machine.sg1000 import SG1000Machine
from sega_emulator.rom_loader import load_rom


def main(argv: Sequence[str] | None = None) -> None:
    """Carrega uma ROM e abre a interface, ou executa em modo headless."""
    arguments = list(sys.argv[1:] if argv is None else argv)
    parser = argparse.ArgumentParser(description="Emulador experimental Sega SG-1000")
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
        help="executa N quadros sem abrir janela (para diagnóstico)",
    )
    parser.add_argument(
        "--ui",
        choices=("qt", "pygame"),
        default="qt",
        help="frontend gráfico (padrão: qt)",
    )
    parser.add_argument(
        "--cpu-core",
        choices=("python", "rust"),
        default=None,
        help="núcleo Z80 (padrão: preferência salva no Qt ou Python no modo headless)",
    )
    parser.add_argument(
        "--vdp-core",
        choices=("python", "rust"),
        default=None,
        help="núcleo TMS9918A (Rust exige extensão experimental compatível)",
    )
    parser.add_argument(
        "--log-dir",
        type=Path,
        default=Path("logs"),
        help="diretório de logs da sessão (padrão: ./logs)",
    )
    args = parser.parse_args(arguments)
    configure_session_logging(args.log_dir)
    logger = logging.getLogger(__name__)

    if args.rom is None and args.headless_frames is not None:
        parser.error("informe uma ROM quando usar --headless-frames")
    if args.rom is None and args.ui == "pygame":
        parser.error("informe uma ROM ao usar --ui pygame")

    if args.ui == "qt" and args.headless_frames is None:
        try:
            from sega_emulator.frontend.qt_frontend import run

            if args.cpu_core is None and args.vdp_core is None:
                run(args.rom)
            else:
                run(args.rom, cpu_core=args.cpu_core, vdp_core=args.vdp_core)
        except (RuntimeError, ImportError) as exc:
            parser.error(str(exc))
        return

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
        machine = SG1000Machine(
            loaded.data,
            cpu_core=args.cpu_core or "python",
            vdp_core=args.vdp_core or "python",
        )
        logger.info(
            "Máquina criada",
            extra={
                "event": "machine_created",
                "rom_sha256": loaded.sha256,
                "rom_path": str(loaded.source_path),
            },
        )
        if args.headless_frames is not None:
            if args.headless_frames < 0:
                parser.error("--headless-frames não pode ser negativo")
            steps = machine.run_frames(args.headless_frames)
            logger.info(
                "Execução headless concluída",
                extra={
                    "event": "headless_complete",
                    "frame": machine.vdp.frame_number,
                    "pc": machine.cpu.pc,
                    "instructions": steps,
                    "tstates": machine.cpu.total_cycles,
                    "scanline": machine.vdp.scanline,
                    "status": machine.vdp.status,
                },
            )
            print(
                f"Execução headless concluída: quadros={machine.vdp.frame_number}, "
                f"instruções={steps}, PC=0x{machine.cpu.pc:04X}, "
                f"T-states={machine.cpu.total_cycles}, ROM SHA-256={loaded.sha256}"
            )
            return

        from sega_emulator.frontend.pygame_frontend import run

        run(machine)
    except (OSError, ValueError, RuntimeError, NotImplementedError) as exc:
        logger.exception(
            "Falha ao iniciar sessão",
            extra={
                "event": "startup_failed",
                "rom_path": str(args.rom),
            },
        )
        parser.error(str(exc))


if __name__ == "__main__":
    main()
