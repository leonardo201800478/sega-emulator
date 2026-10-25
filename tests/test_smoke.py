"""Testes mínimos para validar a infraestrutura do pacote."""

import sys
from types import ModuleType


def test_package_version_is_defined() -> None:
    """Confirma que o pacote expõe sua versão."""
    from sega_emulator import __version__

    assert __version__ == "0.1.0"


def test_entry_point_without_rom_opens_qt_frontend_without_starting_gui(
    capsys, monkeypatch, tmp_path
) -> None:
    """Sem ROM, o CLI delega ao frontend Qt sem iniciar uma janela no teste."""
    from sega_emulator.__main__ import main

    calls: list[object] = []

    def fake_run(
        rom: object, *, cpu_core: str | None = None, vdp_core: str | None = None
    ) -> None:
        calls.append((rom, cpu_core, vdp_core))

    frontend = ModuleType("sega_emulator.frontend.qt_frontend")
    frontend.run = fake_run
    monkeypatch.setitem(sys.modules, "sega_emulator.frontend.qt_frontend", frontend)

    main(["--log-dir", str(tmp_path)])

    assert calls == [(None, None, "python")]
    assert capsys.readouterr().out == ""
