# Sega Emulator

Projeto de emulação de alta precisão com implementação de referência em Python, começando pelo Sega SG-1000.

## Estado atual

- **CPU:** núcleo Z80 em auditoria diferencial; temporização ainda não é ciclo a ciclo.
- **Máquina SG-1000:** CPU, ROM/RAM, VDP TMS9918A inicial e joystick em desenvolvimento.
- **Interface experimental:** menus Qt 6/PySide6 para abrir ROMs, pausar/continuar, reiniciar, alterar escala e consultar diagnóstico.
- **Carregador:** arquivos `.sg`, `.rom`, `.bin` e ROMs dentro de ZIP; ZIP com múltiplas ROMs exige seleção explícita.
- **Diagnóstico:** logs JSONL/texto por sessão, SHA-256 da ROM, checkpoints de execução e stack traces.
- **Áudio:** PSG ainda não implementado. A compatibilidade de jogos reais continua pendente.

## Requisitos

- Windows x64
- Python 3.14.x x64 (ambiente de desenvolvimento: Python 3.14.7)
- Git for Windows, PowerShell e VS Code (recomendados)

## Instalação no PowerShell

Na raiz do repositório:

```powershell
py -3.14 -m venv .venv
.\.venv\Scripts\python.exe -m pip install --upgrade pip
.\.venv\Scripts\python.exe -m pip install -e ".[dev,qt,frontend]"
```

## Iniciar a interface experimental

```powershell
.\.venv\Scripts\python.exe -m sega_emulator
```

A interface Qt abre sem ROM carregada. Use **Arquivo → Abrir ROM** para selecionar uma ROM local.

Menus disponíveis: **Arquivo**, **Emulação** (pausar/continuar e reiniciar), **Vídeo** (escalas de teste), **Diagnóstico** (logs e estado da máquina) e **Ajuda**.

Para iniciar diretamente uma ROM:

```powershell
.\.venv\Scripts\python.exe -m sega_emulator "C:\caminho\para\jogo.sg"
```

Também é possível apontar para um ZIP:

```powershell
.\.venv\Scripts\python.exe -m sega_emulator "C:\caminho\para\jogo.zip"
```

Se o ZIP contiver várias ROMs compatíveis, a interface permite selecionar explicitamente o membro. O conteúdo não é extraído automaticamente.

Para usar o frontend Pygame:

```powershell
.\.venv\Scripts\python.exe -m sega_emulator "C:\caminho\para\jogo.sg" --ui pygame
```

## Diagnóstico headless

```powershell
.\.venv\Scripts\python.exe -m sega_emulator "C:\caminho\para\jogo.sg" --headless-frames 600
```

O comando informa quadro, PC, instruções, T-states e SHA-256. Isso não prova, por si só, compatibilidade visual ou de jogabilidade.

## Logs da sessão

Por padrão, os arquivos ficam em `logs\session-<data-hora>.jsonl` e `.log`. Para escolher outro diretório:

```powershell
.\.venv\Scripts\python.exe -m sega_emulator --log-dir "C:\temp\sega-logs"
```

O JSONL é destinado à análise automatizada; o log textual é para leitura rápida. Os logs são artefatos locais de execução e não devem ser versionados.

## Testes

```powershell
.\.venv\Scripts\python.exe -m pytest -q
.\.venv\Scripts\python.exe -m ruff check .
.\.venv\Scripts\python.exe -m ruff format --check .
git diff --check
```

## Documentação

- [Arquitetura multi-sistema](docs/architecture.md)
- [Pesquisa de compatibilidade de hardware Sega](docs/research/hardware-compatibility-roadmap-2026-10.md)
- [Notas de hardware SG-1000](docs/hardware/sg1000.md)
- [Modelo e plano de fidelidade do VDP TMS9918A](docs/hardware/tms9918a-model.md)
- [Memória, portas e barramentos SG-1000](docs/hardware/sg1000-memory-buses.md)
- [Catálogo prioritário de compatibilidade SG-1000](docs/hardware/sg1000-game-compatibility.md)
- [Marco do primeiro jogo executável](docs/sg1000-first-playable.md)
- [Auditoria de cobertura da CPU Z80](docs/z80-coverage.md)
- [Auditoria técnica da CPU Z80](docs/z80-code-audit-2026-10-22.md)
- [Plano de resolução das lacunas Z80](docs/z80-gap-resolution-plan.md)

## Limitações e próximos passos

1. Reproduzir e corrigir a renderização incorreta observada com ROM real.
2. Validar a entrada de joystick com hardware e jogos compatíveis.
3. Implementar PSG e sincronização audiovisual.
4. Aumentar a precisão temporal CPU–VDP com regressões reproduzíveis.

ROMs comerciais e logs locais não devem ser incluídos no repositório. Use homebrew ou cópias obtidas legalmente no ambiente local.
