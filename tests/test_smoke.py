"""Testes mínimos para validar a infraestrutura do pacote."""

import sys
from types import ModuleType

import pytest


def test_package_version_is_defined() -> None:
    """Confirma que o pacote expõe sua versão."""
    from sega_emulator import __version__

    assert __version__ == "0.1.1"


def test_entry_point_without_rom_opens_minimal_sms_control_window(
    capsys, monkeypatch, tmp_path
) -> None:
    """Sem ROM, o CLI delega à janela mínima sem iniciar GUI no teste."""
    from sega_emulator.__main__ import main

    calls: list[tuple[object, ...]] = []

    def fake_run(rom: object, *, region: str) -> None:
        calls.append((rom, region))

    frontend = ModuleType("sega_emulator.frontend.qt.minimal_sms_window")
    frontend.run_minimal_sms = fake_run
    monkeypatch.setitem(
        sys.modules,
        "sega_emulator.frontend.qt.minimal_sms_window",
        frontend,
    )

    main(["--log-dir", str(tmp_path)])

    assert calls == [(None, "usa")]
    assert capsys.readouterr().out == ""


def test_cli_version_matches_package_version(capsys) -> None:
    """O argumento --version usa a mesma versão declarada pelo pacote."""
    from sega_emulator import __version__
    from sega_emulator.__main__ import main

    with pytest.raises(SystemExit) as exc_info:
        main(["--version"])

    assert exc_info.value.code == 0
    assert capsys.readouterr().out.strip() == f"Sega Emulator {__version__}"


def test_sg1000_legacy_python_video_frontend_is_blocked(capsys, tmp_path) -> None:
    """The SG-1000 core stays available, but Python Qt video is not launchable."""
    from sega_emulator.__main__ import main

    with pytest.raises(SystemExit) as exc_info:
        main(["--system", "sg1000", "--log-dir", str(tmp_path)])

    assert exc_info.value.code == 2
    assert "desativado por ADR-0012" in capsys.readouterr().err
