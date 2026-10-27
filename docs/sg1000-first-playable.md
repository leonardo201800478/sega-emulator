> **Estado vigente (2026-10-27):** o hardware SG-1000 ativo é Rust-only. O frontend Pygame, o runner headless e o gerador Python de ROM sintética foram arquivados em `legacy/`; os comandos históricos não são executados pelo pacote ativo.

# SG-1000 — marco do primeiro jogo executável

## Estado atual

A aplicação suporta a interface Qt/PySide6 e carrega ROMs `.sg`, `.rom`, `.bin` e membros compatíveis de ZIPs. A execução da CPU, mapa de memória, portas, VDP TMS9918A, PSG SN76489AN, entrada da máquina e coordenação dos quadros pertencem ao runtime Rust em `native/sg1000_core`. Python mantém a interface, o carregamento de ROMs/perfis e uma fachada fina para a extensão PyO3.

A validação do núcleo usa testes Rust e testes de integração da extensão. O teste `tests/test_sg1000_native_machine.py` verifica execução de instruções, portas/joystick, eventos PSG, limite de sprites e avanço de quadro. Isso não substitui a validação manual de ROMs reais.

## Executar no Windows

Instale dependências e a aplicação:

```powershell
py -3.14 -m venv .venv
.\.venv\Scripts\python.exe -m pip install -e ".[dev,qt]"
Push-Location native/z80_core
maturin develop --release --features python-extension
Pop-Location
.\.venv\Scripts\python.exe -m sega_emulator "C:\caminho\para\jogo.sg"
```

A interface Qt também pode ser iniciada sem ROM; use **Arquivo → Abrir ROM**. Para listar ROMs dentro de um ZIP sem iniciar a interface:

```powershell
.\.venv\Scripts\python.exe -m sega_emulator --list-roms ".\roms\colecao.zip"
```

## Limitações conhecidas

- A CPU retorna T-states nominais por instrução; o modelo não é ciclo a ciclo.
- A compatibilidade deve ser validada por ROM e comportamento observável; a compilação da extensão não comprova jogabilidade universal.
- A comparação visual/sonora com ARES e a execução das oito ROMs da issue #136 continuam sendo verificações separadas até que haja evidência registrada no SHA candidato.
- Não distribua ROMs comerciais. Use homebrew ou dumps legalmente obtidos.
