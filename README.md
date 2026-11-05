# Sega Emulator

Emulador multi-sistema Sega com runtime de hardware em Rust e frontend Python/Qt. O objetivo é oferecer **fidelidade funcional e temporal próxima ao console original**, com execução eficiente e comportamento consistente em jogos e periféricos.

O projeto não busca simulação elétrica integral, reprodução de transistores ou atualização de todos os pinos físicos. A precisão é aplicada onde influencia o comportamento observável do console. Consulte a [ADR-0016 — fidelidade observável](docs/architecture/adr/0016-observable-fidelity-and-proportional-timing.md).

## Estado do projeto

- **Plataformas prioritárias:** Master System e SG-1000; preservar regressões SG-1000 durante a consolidação do SMS.
- **CPU:** núcleo Z80 compartilhado em Rust. Correção funcional de instruções, registradores, flags e interrupções é prioritária; detalhamento de ciclos e sinais só é exigido quando uma plataforma depender dele.
- **Vídeo e áudio:** componentes específicos por console, com sincronização na granularidade necessária aos efeitos observáveis.
- **Scheduler:** coordena o tempo da máquina e os eventos relevantes; não exige um evento por pino, tick ou pixel.
- **Frontend:** Python/PySide6 para interface, launcher, biblioteca, configurações, logs e telemetria.
- **Integração:** PyO3 coarse-grained, sem callbacks Python por instrução.
- **Precisão elétrica/analógica:** fora do escopo.

O estado exato de cada componente e das PRs deve ser conferido na [main](https://github.com/leonardo201800478/sega-emulator) e em [Estado dos componentes](docs/component-status.md). Documentos históricos não prevalecem sobre o código integrado nem sobre a ADR-0016.

## Arquitetura

| Camada | Responsabilidade |
|---|---|
| Rust | CPUs, ALUs, memória, barramentos, mappers, VDPs, áudio, interrupções, sincronização e Scheduler |
| Python / PySide6 | GUI, launcher, biblioteca, configurações, logs, diagnóstico e automação |
| PyO3 | Comandos de alto nível e telemetria; sem callbacks por instrução |

Componentes são compartilhados quando representam hardware compatível. Diferenças reais de console, revisão, região ou periférico permanecem em perfis/adaptadores próprios.

A menor granularidade suficiente deve ser preferida: execução por instrução, transação, scanline, amostra ou evento, conforme o comportamento que precisa ser reproduzido. M-cycles, T-states, WAIT, arbitragem e sinais são modelados quando necessários ao hardware suportado, não como objetivos independentes.

## Requisitos de desenvolvimento

- Windows x64
- Python 3.15 x64
- Rust toolchain definida pelo projeto
- Git for Windows e PowerShell
- Visual Studio Build Tools com MSVC para compilar a extensão nativa no Windows

## Instalação no PowerShell

Na raiz do repositório:

```powershell
py -3.15 -m venv .venv
.\.venv\Scripts\python.exe -m pip install --upgrade pip
.\.venv\Scripts\python.exe -m pip install -e ".[dev,qt]"
```

## Executar

```powershell
.\.venv\Scripts\python.exe -m sega_emulator
```

A interface Qt abre sem ROM carregada. Use **Arquivo → Abrir ROM** para selecionar uma ROM local.

Para iniciar diretamente uma ROM:

```powershell
.\.venv\Scripts\python.exe -m sega_emulator "C:\caminho\para\jogo.sms"
```

O carregador também aceita formatos suportados pelo console e arquivos ZIP. Se houver múltiplas ROMs compatíveis no ZIP, a seleção é explícita; o conteúdo não é extraído automaticamente.

## Compilar e testar

Compilação rápida Rust no Windows/PowerShell:

```powershell
.\scripts\compile-fast.ps1
```

Compilar e instalar a extensão PyO3 (a partir da raiz):

```powershell
python -m pip install maturin
Push-Location native/z80_core
maturin develop --release --features python-extension
Pop-Location
```

Executar a interface com uma ROM local após instalar a extensão:

```powershell
.\scripts\test-rust-in-game.ps1 "C:\caminho\para\jogo.sms"
```

Gates de qualidade:

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
python -m pytest
python -m ruff check .
python -m ruff format --check .
```

Uma suíte ignorada ou não executada não conta como PASS. Resultados devem ser vinculados ao SHA exato.

## Documentação normativa

- [Roadmap mestre — fases e prioridades](docs/project-roadmap.md)
- [Escopo e princípios arquiteturais](docs/project-scope.md)
- [ADR-0016 — fidelidade observável e precisão proporcional](docs/architecture/adr/0016-observable-fidelity-and-proportional-timing.md)
- [Estado dos componentes](docs/component-status.md)
- [Índice de ADRs](docs/architecture/adr/README.md)
- [Estado do VDP SMS](docs/architecture/sms-vdp.md)
- [Perfis de hardware Z80](docs/hardware/z80-silicon-platform-profiles.md)

## Segurança e licença

Use somente ROMs e firmware que você tenha direito de executar. ROMs comerciais, BIOS proprietárias e dumps protegidos não devem ser versionados.

O projeto é distribuído sob a licença MIT. Consulte [LICENSE](LICENSE). Sega e nomes de consoles/chips são marcas de seus respectivos titulares; o projeto é independente e não afiliado à SEGA.
