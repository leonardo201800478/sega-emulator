"""Testes mínimos para validar a infraestrutura do pacote."""

import sys
from types import ModuleType

import pytest


def test_package_version_is_defined() -> None:
    """Confirma que o pacote expõe sua versão."""
    from sega_emulator import __version__

    assert __version__ == "0.1.1"


def test_entry_point_without_rom_opens_qt_frontend_without_starting_gui(
    capsys, monkeypatch, tmp_path
) -> None:
    """Sem ROM, o CLI delega ao frontend Qt sem iniciar uma janela no teste."""
    from sega_emulator.__main__ import main

    calls: list[object] = []

    def fake_run(rom: object) -> None:
        calls.append(rom)

    frontend = ModuleType("sega_emulator.frontend.qt_frontend")
    frontend.run = fake_run
    monkeypatch.setitem(sys.modules, "sega_emulator.frontend.qt_frontend", frontend)

    main(["--log-dir", str(tmp_path)])

    assert calls == [None]
    assert capsys.readouterr().out == ""


def test_cli_version_matches_package_version(capsys) -> None:
    """O argumento --version usa a mesma versão declarada pelo pacote."""
    from sega_emulator import __version__
    from sega_emulator.__main__ import main

    with pytest.raises(SystemExit) as exc_info:
        main(["--version"])

    assert exc_info.value.code == 0
    assert capsys.readouterr().out.strip() == f"Sega Emulator {__version__}"
