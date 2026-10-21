# Sega Emulator

Projeto de emulação de alta precisão, iniciado com uma implementação de referência em Python para o SG-1000.

## Estado atual

Etapa 01 — infraestrutura inicial. Ainda não há CPU, barramento, VDP ou PSG implementados.

## Requisitos

- Windows x64
- Python 3.13 x64
- Git for Windows
- PowerShell
- Visual Studio Code (recomendado)

## Criar o ambiente local

Na raiz do repositório, execute no PowerShell:

```powershell
py -3.13 --version
git --version
py -3.13 -m venv .venv
.\.venv\Scripts\python.exe -m pip install --upgrade pip
.\.venv\Scripts\python.exe -m pip install -e ".[dev]"
```

A ativação do ambiente virtual é opcional. Os comandos acima chamam diretamente o Python do `.venv`, evitando depender da política de execução do PowerShell.

## Validar a infraestrutura

```powershell
.\.venv\Scripts\python.exe --version
.\.venv\Scripts\python.exe -m pytest
.\.venv\Scripts\python.exe -m ruff check .
.\.venv\Scripts\python.exe -m ruff format --check .
.\.venv\Scripts\python.exe -m sega_emulator
```

O comando do módulo deve informar que a infraestrutura existe e que o núcleo ainda não foi implementado.

## Fluxo Git inicial

Depois de validar os comandos:

```powershell
git status --short
git add .gitignore .python-version pyproject.toml README.md src tests
git commit -m "chore: establish initial Python project scaffold"
```

## Decisões pendentes

- Escolher e documentar a licença do projeto antes de publicar o repositório.
- Criar a integração contínua na Etapa 02.
- Documentar os mapas de memória, I/O e as variantes do SG-1000 antes de implementar os dispositivos.
