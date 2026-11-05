# Sega Emulator

Emulador multi-sistema Sega com runtime de hardware em Rust e uma janela Qt mínima para operação do emulador. O objetivo é oferecer **fidelidade funcional e temporal próxima ao console original**, com execução eficiente e comportamento consistente em jogos e periféricos.

O projeto não busca simulação elétrica integral, reprodução de transistores ou atualização de todos os pinos físicos. A precisão é aplicada onde influencia o comportamento observável do console. Consulte a [ADR-0016 — fidelidade observável](docs/architecture/adr/0016-observable-fidelity-and-proportional-timing.md).

## Estado do projeto

- **Plataformas prioritárias:** Master System e SG-1000; preservar regressões SG-1000 durante a consolidação do SMS.
- **CPU:** núcleo Z80 compartilhado em Rust. Correção funcional de instruções, registradores, flags e interrupções é prioritária; detalhamento de ciclos e sinais só é exigido quando uma plataforma depender dele.
- **Vídeo e áudio:** componentes específicos por console, com sincronização na granularidade necessária aos efeitos observáveis.
- **Scheduler:** coordena o tempo da máquina e os eventos relevantes; não exige um evento por pino, tick ou pixel.
- **Qt inicial:** Python/PySide6 apenas para viewport, abertura/fechamento de ROM, controles de sessão e entrada; biblioteca, capas, perfis, abas de diagnóstico e configurações avançadas ficam fora do escopo inicial.
- **Integração:** PyO3 coarse-grained, sem callbacks Python por instrução.
- **Precisão elétrica/analógica:** fora do escopo.

O estado exato de cada componente e das PRs deve ser conferido na [main](https://github.com/leonardo201800478/sega-emulator) e em [Estado dos componentes](docs/component-status.md). Documentos históricos não prevalecem sobre o código integrado nem sobre a ADR-0016.

## Fonte da verdade, roadmap e conclusão

- **GitHub `main` é a fonte da verdade.** Código em branches e PRs ainda não mescladas não está integrado, mesmo quando a PR está aberta, aprovada ou com CI verde.
- **Existe um único roadmap normativo:** [Roadmap mestre](docs/project-roadmap.md). Ele define a ordem P0–P9. Não criar cronogramas, listas de sprints ou roadmaps paralelos.
- [Estado dos componentes](docs/component-status.md), scorecards e auditorias podem conter fotografias históricas. Antes de iniciar trabalho, conferir o SHA atual da `main`, o código integrado, as PRs abertas e os workflows; não tratar um SHA antigo ou o estado de uma PR como estado integrado.
- A [PR permanente de conclusão #533](https://github.com/leonardo201800478/sega-emulator/pull/533) é o gate final do projeto. Ela permanece Draft e aberta durante o desenvolvimento e só poderá ser finalizada quando todas as fases P0–P9, os critérios funcionais/temporais, a compatibilidade, o determinismo, os save states/replays e os gates finais estiverem comprovados no SHA final da `main`. Ela não substitui o roadmap nem autoriza antecipar fases.

### Ordem obrigatória das plataformas

P0 Infraestrutura/Hosts → P1 Master System → P2 SG-1000 → P3 Game Gear → P4 Mega Drive/Genesis → P5 Sega CD → P6 32X/Sega CD 32X → P7 Frontend Qt/PyO3 → P8 Compatibilidade/Auditoria → P9 Validação sistêmica.

A prioridade atual dentro dessa sequência é consolidar o Master System e o Z80 compartilhado, preservando o SG-1000. Uma prioridade não significa que a fase esteja concluída.

## Política de precisão

A semântica funcional é exata em todas as instruções e operações suportadas. A precisão temporal recebe prioridade nos caminhos de instrução e processos cuja ordem ou duração possa alterar gameplay — por exemplo, I/O, raster/VDP, áudio, mapper, interrupções, DMA e contenção comprovada. Operações sem efeito intermediário observável usam execução atômica determinística, preservando resultado, flags, estado, efeitos colaterais e duração nominal conhecida. M-cycles, T-states, WAIT e suspensão/retomada são exigidos apenas quando uma dependência concreta estiver demonstrada. Não se exige cycle-accuracy universal, simulação elétrica nem precisão por pino. A [ADR-0016](docs/architecture/adr/0016-observable-fidelity-and-proportional-timing.md) define os critérios.

## Arquitetura

| Camada | Responsabilidade |
|---|---|
| Rust | CPUs, ALUs, memória, barramentos, mappers, VDPs, áudio, interrupções, sincronização e Scheduler |
| Python / PySide6 | Janela mínima, viewport, diálogos de ROM, comandos de sessão e entrada; sem lógica de hardware |
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

A janela Qt mínima abre sem ROM carregada. Use **Abrir ROM** na barra superior para selecionar uma ROM local. O caminho SMS usa `SmsTemporalMachine`; para manter o SG-1000 no host atual, inicie com `--system sg1000`.

Para iniciar diretamente uma ROM:

```powershell
.\.venv\Scripts\python.exe -m sega_emulator "C:\caminho\para\jogo.sms"
```

A janela SMS mínima aceita ROM local `.sms`, `.bin` ou `.rom`. A seleção de região é explícita por `--region usa|brazil|europe|japan`; o padrão é `usa`. O fluxo ZIP e o frontend anterior permanecem no caminho SG-1000/legado até a migração ser validada.

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
