"""Testes mínimos para validar a infraestrutura do pacote."""


def test_package_version_is_defined() -> None:
    """Confirma que o pacote expõe sua versão."""
    from sega_emulator import __version__

    assert __version__ == "0.1.0"


def test_entry_point_reports_scaffold_state(capsys) -> None:
    """Confirma que o ponto de entrada não afirma executar o emulador."""
    from sega_emulator.__main__ import main

    main()

    captured = capsys.readouterr()
    assert "máquina SG-1000 ainda não integrada" in captured.out
