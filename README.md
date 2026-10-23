# Sega Emulator

Projeto de emulação de alta precisão, iniciado com uma implementação de referência em Python para o SG-1000.

## Estado atual

- **Etapa 01 concluída:** infraestrutura Python, pytest, Ruff e pacote executável.
- **Etapa 02 em andamento:** RAM interna, barramento inicial de memória, ROM de cartucho somente leitura e contrato de barramento para a CPU.
- **Etapa 03 em andamento:** CPU Z80 em expansão, incluindo operações base/CB/ED/DD/FD e DD/FD-CB, HALT, DI/EI e modelo inicial de NMI/INT. A contagem de ciclos continua nominal por instrução; não existe ainda temporização ciclo a ciclo.
- **Auditoria diferencial Z80 em andamento:** o lote de referência anterior reportou 1.000.000 de casos, 992.309 aprovados e 7.691 divergências. As divergências concentram-se em `BIT b,(HL)`, `SCF/CCF` e `DAA`; correções estão em branch/PR e aguardam testes curtos e nova auditoria. Veja o [relatório diferencial de 23/10/2026](docs/z80-differential-audit-2026-10-23.md).
- A CPU Z80 ainda não está completa nem validada pós-correção contra a suíte diferencial. O estado interno `WZ/MEMPTR` e o latch `Q` têm suporte parcial e precisam de atualização abrangente e validação. O contrato IOBus existe, mas mapa real de portas SG-1000, VDP, PSG e frontend ainda não estão integrados; os componentes atuais não formam uma máquina SG-1000 funcional.

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

O comando do módulo informa que a máquina SG-1000 ainda não está integrada. RAM, barramento e CPU são componentes parciais; a mensagem não indica que exista um emulador funcional completo.

## Documentação

- [Registro de desenvolvimento e ponto de retomada (22/10/2026)](docs/development-handoff-2026-10-22.md)
- [Auditoria geral do código](docs/codebase-audit.md)
- [Arquitetura multi-sistema e estratégia de reutilização](docs/architecture.md)
- [Auditoria de cobertura da CPU Z80](docs/z80-coverage.md)
- [Auditoria diferencial Z80 — 1 milhão de casos (23/10/2026)](docs/z80-differential-audit-2026-10-23.md)
- [Registro de desenvolvimento e retomada (23/10/2026)](docs/development-handoff-2026-10-23.md)
- [Auditoria técnica detalhada do núcleo Z80 (22/10/2026)](docs/z80-code-audit-2026-10-22.md)
- [Pesquisa do núcleo Z80 do MAME](docs/z80-mame-reference.md)
- [Pesquisa comparativa de comportamentos Z80 pouco documentados](docs/z80-undocumented-behavior-research.md)
- [Notas de hardware do SG-1000](docs/hardware/sg1000.md)
- [Inventário de firmware/ROM mantidos apenas localmente](docs/firmware-inventory.md)
- Barramento inicial: `src/sega_emulator/bus/memory_bus.py`
- Contrato de acesso à memória para a CPU: `src/sega_emulator/bus/interfaces.py`

## Decisões pendentes

- Escolher e documentar a licença do projeto antes de qualquer distribuição.
- Documentar o mapa de I/O, expansões de RAM e variantes de hardware antes de integrar os dispositivos.
