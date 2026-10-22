# Sega Emulator

Projeto de emulação de alta precisão, iniciado com uma implementação de referência em Python para o SG-1000.

## Estado atual

- **Etapa 01 concluída:** infraestrutura Python, pytest, Ruff e pacote executável.
- **Etapa 02 em andamento:** RAM interna, barramento inicial de memória, ROM de cartucho somente leitura e contrato de barramento para a CPU.
- **Etapa 03 em andamento:** subconjunto inicial executável da CPU Z80, com registradores, `NOP`, `LD`, operações de ALU (`ADD A,r`, `ADC A,r`, `SUB r`, `SBC A,r`, `AND r`, `XOR A,r`, `OR r`, `CP r` e suas formas imediatas), incrementos/decrementos de 8 bits (`INC r`, `DEC r`), operações de flags (`DAA`, `CPL`, `SCF`, `CCF`) e rotações do acumulador (`RLCA`, `RRCA`, `RLA`, `RRA`), trocas de registradores (`EX AF,AF'`, `EXX`, `EX DE,HL`) e chamadas a vetores fixos (`RST`), saltos `JP`/`JR` com as oito condições de `JP cc`, `CALL cc` e `RET cc`, e laço `DJNZ`, pilha/sub-rotinas (`PUSH`, `POP`, `CALL`, `RET`) e operações de 16 bits (`LD rr,nn`, `INC rr`, `DEC rr`, `ADD HL,rr`, `ADC/SBC HL,rr` prefixadas por ED) e acessos indiretos via BC/DE (`LD (BC/DE),A`, `LD A,(BC/DE)`) e acessos absolutos (`LD (nn),HL`, `LD HL,(nn)`, `LD (nn),A`, `LD A,(nn)`, `JP (HL)`, `LD SP,HL`), coberto por testes sintéticos.
- Ainda não há uma CPU Z80 completa, mapa de I/O, VDP, PSG ou frontend.

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

## Testes e qualidade

```powershell
.\.venv\Scripts\python.exe -m pytest
.\.venv\Scripts\python.exe -m ruff check .
.\.venv\Scripts\python.exe -m ruff format --check .
```

Consulte [`docs/z80-coverage.md`](docs/z80-coverage.md) para a matriz de cobertura e as lacunas conhecidas.
