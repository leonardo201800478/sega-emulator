# Sega Emulator

Projeto de emulação de alta precisão com implementação de referência em Python,
começando pelo Sega SG-1000.

## Estado atual

- **CPU:** núcleo Z80 em auditoria diferencial; temporização ainda não é ciclo a ciclo.
- **Máquina SG-1000:** CPU, ROM/RAM, VDP TMS9918A inicial e joystick em desenvolvimento.
- **Interface experimental:** menus Qt 6/PySide6 para abrir ROMs, pausar/continuar, reiniciar,
  alterar escala e consultar estado de diagnóstico.
- **Carregador:** arquivos `.sg` e ROMs dentro de `.zip`; ZIP com múltiplas ROMs exige seleção.
- **Logs:** cada sessão cria um arquivo JSON Lines (`.jsonl`) e um log legível (`.log`) em
  `./logs`, incluindo eventos de carregamento, SHA-256, checkpoints de quadro e exceções.
- **Áudio:** PSG ainda não implementado. Ainda não foi confirmado um jogo real funcional.

## Requisitos

- Windows x64
- Python 3.14.x x64 (ambiente de desenvolvimento: Python 3.14.7)
- Git for Windows, PowerShell e VS Code (recomendados)

## Instalação no PowerShell

```powershell
py -3.14 -m venv .venv
.\.venv\Scripts\python.exe -m pip install --upgrade pip
.\.venv\Scripts\python.exe -m pip install -e ".[dev,qt,frontend]"
```

## Iniciar os menus experimentais

```powershell
.\.venv\Scripts\python.exe -m sega_emulator
```

Menus iniciais: **Arquivo** (abrir ROM, sair), **Emulação** (pausar/continuar, reiniciar),
**Vídeo** (escalas de teste) e **Diagnóstico** (pasta de logs e estado da máquina).

Para iniciar diretamente uma ROM:

```powershell
.\.venv\Scripts\python.exe -m sega_emulator "C:\caminho\para\SuperUwol-SG.sg"
```

Também é possível apontar para o ZIP que contém a ROM:

```powershell
.\.venv\Scripts\python.exe -m sega_emulator "C:\caminho\para\SuperUwol-SG.zip"
```

Se o ZIP contiver mais de uma ROM compatível, o carregador interrompe a abertura e informa
os membros encontrados em vez de escolher silenciosamente um arquivo. A interface ainda
precisa concluir o diálogo de seleção de membro para esse caso.

## Diagnóstico headless

```powershell
.\.venv\Scripts\python.exe -m sega_emulator "C:\caminho\para\SuperUwol-SG.sg" --headless-frames 600
```

O comando informa quadro, PC, T-states e SHA-256. Não prova, por si só, compatibilidade visual
ou de jogabilidade.

## Logs da sessão

Por padrão, os arquivos ficam em `logs\session-<data-hora>.jsonl` e `.log`. Para escolher
outro diretório:

```powershell
.\.venv\Scripts\python.exe -m sega_emulator --log-dir "C:\temp\sega-logs"
```

O JSONL é destinado a análise automatizada; o log textual é para leitura rápida. Não são
registrados dados pessoais do usuário. A captura detalhada de cada instrução ainda não está
ativada porque geraria alto volume e afetaria o desempenho; primeiro coletamos checkpoints,
estado de CPU/VDP e exceções, e depois adicionaremos rastreamento de instruções opcional.

## Testes

```powershell
.\.venv\Scripts\python.exe -m pytest -q
.\.venv\Scripts\python.exe -m ruff check .
.\.venv\Scripts\python.exe -m ruff format --check .
git diff --check
```

## Limitações e próximos passos

1. Corrigir a renderização incorreta observada na ROM de diagnóstico e validar Super Uwol.
2. Finalizar seleção de ROM dentro de ZIP com múltiplos candidatos.
3. Validar os eventos do 8BitDo M-30 e então integrar SDL3.
4. Avaliar renderização D3D11 após o caminho de vídeo estar testado.
5. Implementar PSG e aumentar a precisão CPU–VDP com regressões reproduzíveis.

ROMs de terceiros não são incluídas no repositório.
