# Sega Emulator

Projeto de emulação de alta precisão com implementação de referência em Python,
começando pelo Sega SG-1000.

## Estado atual

- **Infraestrutura:** Python 3.14, pytest, Ruff e pacote executável.
- **CPU:** núcleo Z80 com ampla cobertura de instruções, mas ainda sujeito a auditoria
  diferencial e sem temporização de barramento ciclo a ciclo.
- **Primeira integração SG-1000 em desenvolvimento:** CPU, ROM/RAM, VDP compatível com
  TMS9918A, portas iniciais de joystick e frontend Pygame opcional.
- **Áudio:** não implementado nesta etapa; PSG será integrado depois do primeiro marco jogável.
- A execução de uma ROM real e a validação da jogabilidade continuam pendentes. A existência
  de uma janela ou a execução de uma ROM sintética não será considerada prova de compatibilidade.

## Requisitos

- Windows x64
- Python 3.14.x x64 (ambiente validado: Python 3.14.7)
- Git for Windows
- PowerShell
- Visual Studio Code (recomendado)

## Instalação

Na raiz do repositório:

```powershell
py -3.14 -m venv .venv
.\.venv\Scripts\python.exe -m pip install --upgrade pip
.\.venv\Scripts\python.exe -m pip install -e ".[dev,frontend]"
```

## Executar uma ROM SG-1000

Use uma ROM homebrew ou legalmente obtida. ROMs comerciais não são distribuídas neste
repositório.

```powershell
.\.venv\Scripts\python.exe -m sega_emulator "C:\caminho\para\jogo.rom"
```

Controles iniciais: setas direcionais, Z/Space para botão 1, X/Enter para botão 2 e Esc
para sair.

Para testar o caminho de vídeo sem depender de uma ROM externa, gere uma ROM homebrew de diagnóstico:

```powershell
.\.venv\Scripts\python.exe tools/create_sg1000_demo_rom.py --output sg1000-demo.rom
.\.venv\Scripts\python.exe -m sega_emulator .\sg1000-demo.rom
```

O gerador imprime o SHA-256 da ROM para que o artefato usado nos testes possa ser identificado. Ela testa o caminho básico de renderização, mas não comprova compatibilidade de sprites, interrupções ou jogos comerciais.

Para diagnóstico sem janela:

```powershell
.\.venv\Scripts\python.exe -m sega_emulator "C:\caminho\para\jogo.rom" --headless-frames 600
```

O valor `--headless-frames 0` carrega e valida a ROM sem executar quadros; é útil para
confirmar o caminho do arquivo antes de iniciar uma sessão.

## Validar

```powershell
.\.venv\Scripts\python.exe -m pytest -q
.\.venv\Scripts\python.exe -m ruff check .
.\.venv\Scripts\python.exe -m ruff format --check .
git diff --check
```

## Documentação

- [Arquitetura multi-sistema](docs/architecture.md)
- [Pesquisa de compatibilidade de hardware Sega: Z80, áudio, M68K, Mega CD e 32X](docs/research/hardware-compatibility-roadmap-2026-10.md)
- [Notas de hardware SG-1000](docs/hardware/sg1000.md)
- [Marco do primeiro jogo executável](docs/sg1000-first-playable.md)
- [Auditoria de cobertura da CPU Z80](docs/z80-coverage.md)
- [Auditoria técnica da CPU Z80](docs/z80-code-audit-2026-10-22.md)
- [Plano de resolução das lacunas Z80](docs/z80-gap-resolution-plan.md)

## Próximas etapas

1. Validar o VDP com testes e imagens de referência.
2. Executar uma ROM real conhecida e corrigir falhas observadas.
3. Refinar o mapa de memória/I/O e a sincronização CPU–VDP.
4. Implementar PSG e sincronização audiovisual depois de obter vídeo e controles funcionais.
5. Ampliar precisão temporal com base em divergências reproduzíveis.
