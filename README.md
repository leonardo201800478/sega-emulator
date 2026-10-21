# Sega Emulator

Projeto de emulação de alta precisão, iniciado com uma implementação de referência em Python para o SG-1000.

## Estado atual

- **Etapa 01 concluída:** infraestrutura Python, pytest, Ruff e pacote executável.
- **Etapa 02 em andamento:** RAM interna, barramento inicial de memória, ROM de cartucho somente leitura e testes unitários.
- Ainda não há CPU Z80, mapa completo de I/O, VDP, PSG ou frontend.

## Requisitos

- Windows x64
- Python 3.14.x x64 (versão validada no ambiente de desenvolvimento: 3.14.7)
- Git for Windows
- PowerShell
- Visual Studio Code (recomendado)

## Criar o ambiente local

Na raiz do repositório, execute no PowerShell:

```powershell
py -3.14 --version
git --version
py -3.14 -m venv .venv
.\.venv\Scripts\python.exe -m pip install --upgrade pip
.\.venv\Scripts\python.exe -m pip install -e ".[dev]"
```

A ativação do ambiente virtual é opcional. Os comandos chamam diretamente o Python do `.venv`, evitando depender da política de execução do PowerShell.

## Validar o projeto

```powershell
.\.venv\Scripts\python.exe --version
.\.venv\Scripts\python.exe -m pytest
.\.venv\Scripts\python.exe -m ruff check .
.\.venv\Scripts\python.exe -m ruff format --check .
.\.venv\Scripts\python.exe -m sega_emulator
```

O comando do módulo ainda informa que o núcleo não está implementado; a RAM é o primeiro componente isolado, não um emulador funcional completo.

## Documentação

- [Notas de hardware do SG-1000](docs/hardware/sg1000.md)
- Barramento inicial: `src/sega_emulator/bus/memory_bus.py`

## Decisões pendentes

- Escolher e documentar a licença do projeto antes de qualquer distribuição.
- Criar a integração contínua.
- Documentar o mapa de I/O, expansões de RAM e variantes de hardware antes de ampliar o barramento.
