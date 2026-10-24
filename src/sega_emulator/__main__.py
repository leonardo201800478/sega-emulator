"""Ponto de entrada CLI para a máquina SG-1000."""

from __future__ import annotations

import argparse
import sys
from collections.abc import Sequence
from pathlib import Path

from sega_emulator.machine.sg1000 import SG1000Machine


def main(argv: Sequence[str] | None = None) -> None:
    """Carrega uma ROM e abre o frontend, ou executa em modo headless."""
    arguments = list(sys.argv[1:] if argv is None else argv)
    if not arguments:
        print(
            "Sega Emulator — SG-1000 ainda em desenvolvimento. "
            "Informe uma ROM para iniciar: python -m sega_emulator <jogo.rom> "
            "[--headless-frames N]"
        )
        return

    parser = argparse.ArgumentParser(description="Emulador experimental Sega SG-1000")
    parser.add_argument("rom", type=Path, help="caminho para uma ROM SG-1000")
    parser.add_argument(
        "--headless-frames",
        type=int,
        default=None,
        metavar="N",
        help="executa N quadros sem abrir janela (para diagnóstico)",
    )
    args = parser.parse_args(arguments)

    try:
        rom = args.rom.read_bytes()
    except OSError as exc:
        parser.error(f"não foi possível ler a ROM: {exc}")

    try:
        machine = SG1000Machine(rom)
        if args.headless_frames is not None:
            if args.headless_frames < 0:
                parser.error("--headless-frames não pode ser negativo")
            steps = machine.run_frames(args.headless_frames)
            print(
                f"Execução headless concluída: quadros={machine.vdp.frame_number}, "
                f"instruções={steps}, PC=0x{machine.cpu.pc:04X}, "
                f"T-states={machine.cpu.total_cycles}"
            )
            return

        from sega_emulator.frontend.pygame_frontend import run

        run(machine)
    except (ValueError, RuntimeError, NotImplementedError) as exc:
        parser.error(str(exc))


if __name__ == "__main__":
    main()
