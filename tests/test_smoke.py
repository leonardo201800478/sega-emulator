"""Testes mínimos para validar a infraestrutura do pacote."""


def test_package_version_is_defined() -> None:
    """Confirma que o pacote expõe sua versão."""
    from sega_emulator import __version__

    assert __version__ == "0.1.0"


def test_entry_point_without_rom_explains_how_to_start(capsys) -> None:
    """Sem argumentos, o CLI informa como carregar uma ROM sem afirmar que joga."""
    from sega_emulator.__main__ import main

    main([])

    captured = capsys.readouterr()
    assert "Informe uma ROM para iniciar" in captured.out
