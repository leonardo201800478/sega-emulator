# P-0 — Baseline e inventário arquitetural

**Data da auditoria:** 2026-10-31  
**Repositório:** `leonardo201800478/sega-emulator`  
**Branch de auditoria:** `audit/p0-baseline-20261031`  
**Base:** branch remota `main`, consultada no momento da criação desta branch  
**Último commit encontrado na busca global do repositório:** `c31e4183b8d0b36d95b4f58582938186417652f8` — `docs(re): document SMS BIOS boot and slot detection`  
**Natureza:** inventário documental; nenhum runtime ou comportamento de emulação foi alterado.

> Limite de proveniência: a integração disponível permitiu criar a branch a partir de `main`, mas não expôs diretamente o SHA do ponteiro da branch no momento da leitura. O commit acima é o mais recente retornado pela busca global de commits, não deve ser tratado isoladamente como prova do HEAD. O SHA exato deve ser registrado pelo workflow/checkout da branch candidata.

## 1. Resumo executivo

A P-0 confirma que o repositório já contém runtimes Rust separados para SG-1000 e SMS, ALU/bus Z80, áudio nativo e integração Python/Qt. A reorganização deve ser incremental. Não há justificativa para substituir os consoles ou recriar subsistemas existentes.

A principal dívida estrutural identificada é a duplicação do executor Z80 entre SG-1000 e SMS, combinada com contratos de barramento distintos. A extração de um motor compartilhado precisa preservar os adaptadores específicos de cada máquina e portar os testes antes de remover os executores antigos.

A fundação temporal ainda não equivale a um scheduler de máquina integrado. A PR #315 propõe primitivas temporais, mas declara explicitamente que não implementa `MachineScheduler`, micro-operações, bus cycles, WAIT, contenção ou integração PSG. A contagem agregada de T-states por instrução não pode ser promovida a precisão ciclo a ciclo.

## 2. Inventário do workspace Rust

O `Cargo.toml` raiz declara resolver 3 e os seguintes membros ativos:

| Crate | Pacote | Papel observado |
|---|---|---|
| `native/sg1000_core` | `sega-sg1000-core` | Runtime SG-1000 |
| `native/sms_core` | `sega-sms-core` | Runtime SMS |
| `native/z80_alu` | `sega-z80-alu` | ALU e vetores Z80 |
| `native/z80_bus` | `sega-z80-bus` | Contratos/utilitários de barramento Z80 |
| `native/audio_runtime` | `sega-audio-runtime` | Infraestrutura de áudio Rust |

O workspace exclui `native/z80_core`, `native/sega_emu_core` e `native/audio_output`. A exclusão não autoriza apagar esses diretórios: é necessário inventariar consumidores, histórico, testes e papel de compatibilidade antes de qualquer remoção.

Manifestos consultados:
- `native/sms_core/Cargo.toml`: depende de `sega-z80-bus`.
- `native/sg1000_core/Cargo.toml`: não declara dependências adicionais no manifesto consultado.
- `native/z80_alu/Cargo.toml`: inclui binário `z80-alu-vectors`.
- `native/z80_bus/Cargo.toml`: crate de biblioteca.
- `native/audio_runtime/Cargo.toml`: crate de biblioteca.

O workspace ativo não corresponde à árvore idealizada no plano de arquitetura (pastas `cpus/`, `video/`, `systems/`, `scheduler/`, `sega_ffi/`). A migração deve reutilizar e renomear/mover crates somente quando houver ganho comprovado, preservando APIs e histórico; não criar uma segunda árvore paralela.

## 3. Inventário Python e fronteira nativa

O `pyproject.toml` consultado declara:
- Python `>=3.14,<3.15`;
- setuptools `84.0.0`;
- pytest `9.1.1`;
- Ruff `0.17.1`;
- extras Qt com PySide6, NumPy e PySDL3;
- descoberta de pacotes sob `src`;
- testes em `tests`.

O escopo normativo determina Python/Qt para interface, carregamento de mídia/perfis, configuração, logs, diagnóstico e ferramentas; Rust permanece o runtime de hardware. PyO3/Maturin deve permanecer uma fronteira fina, sem callbacks Python por instrução, pixel ou amostra PCM.

**Pendência de inventário:** mapear todos os módulos Python ativos e seus imports de extensões nativas; confirmar quais caminhos SDL/PySDL3 permanecem necessários após a PR #313; auditar timers Qt, `run_frame`, cópias de framebuffer e ciclo de vida de áudio/vídeo no frontend. A leitura de manifestos não comprova por si só o comportamento em runtime.

## 4. Estado conhecido por subsistema

| Subsistema | Evidência disponível | Estado P-0 / pendência |
|---|---|---|
| SG-1000 | Runtime Rust e marco funcional descritos no README e roadmap | Preservar como baseline; repetir regressões no SHA candidato |
| SMS | `native/sms_core` integra CPU, memória/cartucho, I/O, VDP Mode 4 e PSG | IMPLEMENTED nos limites documentados; ampliar validação |
| Z80 | Implementações em `native/sg1000_core/src/cpu.rs` e `native/sms_core/src/cpu.rs`; ALU/bus compartilhados | Duplicação confirmada; extrair motor único após mapear contratos/testes |
| VDP SG-1000 | TMS9918A separado | Preservar semântica específica |
| VDP SMS | Mode 4, scanline-oriented no status técnico | Efeitos intralinha/dot timing permanecem pendentes |
| PSG | Runtime de áudio Rust integrado | Preservar pipeline e distinguir chip digital de saída host |
| FM/YM2413 SMS | Status técnico SMS declara síntese não implementada | INTEGRATION_PENDING |
| Input | PR #313 propõe backend Rust SDL3 e ponte Qt; #300 é proposta anterior | Reconciliar diffs; não integrar duas soluções em duplicidade |
| Vídeo/apresentação | PR #301 propõe migração nativa | Verificar estado/diff/checks antes de adotar |
| Scheduler | PR #315 propõe Temporal Core | Não equivale a scheduler integrado nem a TEMPORAL_PASS |
| BIOS SMS | Código/documentação de BIOS requer confirmação por revisão e evidência binária | Separar parser, mapper, firmware e política de slots; não presumir suporte integral |
| Mappers SMS | PR #324 é documental e define perfis/fail-closed | Não significa que os mappers estejam implementados no runtime |

## 5. PRs que precisam ser reconciliadas antes de refatorações

- [#324 — compatibilidade de mappers SMS](https://github.com/leonardo201800478/sega-emulator/pull/324): documentação/catálogo; não altera runtime.
- [#315 — Rust Temporal Core](https://github.com/leonardo201800478/sega-emulator/pull/315): primitivas de tempo, domínios e fila; não implementa scheduler completo.
- [#314 — wraparound de LD (nn),HL](https://github.com/leonardo201800478/sega-emulator/pull/314): teste Z80; não altera runtime de produção. A descrição registra PASS no SHA candidato `7ff733f...`, mas esse resultado não é transferível para outros SHAs.
- [#313 — input Rust/SDL3](https://github.com/leonardo201800478/sega-emulator/pull/313): proposta mais recente de input nativo; revisar limitações de backend, slots e testes físicos.
- [#301 — vídeo Rust](https://github.com/leonardo201800478/sega-emulator/pull/301): migração do pipeline de vídeo/apresentação.
- [#300 — input nativo anterior](https://github.com/leonardo201800478/sega-emulator/pull/300): comparar com #313 e preservar qualquer trabalho único antes de decidir o destino.

Não fechar, substituir ou mesclar PRs somente por título/descrição. A decisão exige comparar base/head, arquivos, commits, comentários, checks e alterações únicas.

## 6. Arquitetura consolidada — decisões e divergências

1. Rust é o único runtime de hardware.
2. Qt/PySide6 permanece shell de janela, menus, configuração, diagnóstico e comandos coarse-grained.
3. PyO3/Maturin é fronteira fina; buffers grandes não devem trafegar por Python no caminho crítico.
4. Deve existir um scheduler por instância de máquina, com múltiplos domínios de clock; não um scheduler independente por chip.
5. Z80 pode ser compartilhado; VDPs, mapas de memória, I/O, PSG e mappers permanecem específicos quando o hardware divergir.
6. SG-1000 e SMS são prioridades conjuntas; SG-1000 é baseline obrigatório de regressão.
7. Não declarar ciclo a ciclo com base em T-states nominais agregados.

A proposta de janela nativa Rust constante no plano inicial precisa ser reconciliada com a decisão normativa vigente de Qt como shell de janela e fornecedor de surface handle. Não mudar esse contrato sem ADR explícita e análise de impacto.

## 7. CI e validação — estado desta auditoria

A consulta de status combinado para o commit `c31e4183b8d0b36d95b4f58582938186417652f8` retornou lista vazia de statuses; a consulta de workflow runs associadas também não retornou execuções. Isso é **NOT VERIFIED**, não PASS nem FAIL.

A busca de código por caminhos de workflow não retornou resultados. Portanto, os nomes e filtros de todos os workflows ainda precisam ser extraídos do checkout real da branch e auditados.

Comandos obrigatórios no SHA candidato:
- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets --locked -- -D warnings`
- `cargo test --workspace --locked`
- `python -m pytest`
- `python -m ruff check .`
- `python -m ruff format --check .`

Também verificar build Release/Maturin, `python -m compileall`, `python -m pip check` e integração Qt/PyO3 conforme o escopo alterado.

**Execução local nesta sessão:** BLOCKED — o ambiente de execução não conseguiu resolver `github.com` ao tentar `git ls-remote`; não há checkout local do repositório para executar os comandos. Nenhum PASS local foi declarado.

## 8. Matriz de lacunas e próxima ação

| Lacuna | Classe | Próxima ação | Gate |
|---|---|---|---|
| SHA exato de main e estado de refs | Evidência/base | Capturar `git rev-parse HEAD`, `git status`, refs e upstream no checkout | P0 |
| Inventário de workflows e filtros | CI | Listar `.github/workflows`, triggers, paths, required checks e SHA dos runs | P0 |
| Módulos e ownership Rust | Arquitetura | `cargo metadata --locked`, árvore de módulos e dependências | P0 |
| Fronteira Python/PyO3 | Integração | Mapear imports, métodos expostos, buffers e chamadas por frame | P0 |
| Testes e oráculos | Validação | Catalogar suíte por crate/componente, licença, ancestralidade e cobertura | P0 |
| Duplicação Z80 | Código | Comparar implementações normalizadas e APIs; plano de extração compatível | P1 |
| PRs concorrentes | Integração | Comparar diffs/checks de #300/#313, #301 e #315 | P0 |
| Smoke SG-1000/SMS | Evidência funcional | Executar ROMs legais/homebrew identificadas localmente; registrar SHA-256 e checkpoints sem versionar ROM | P0 |
| Scheduler temporal | Arquitetura/implementação | Avaliar #315 e projetar integração somente após baseline e traces | Posterior |

## 9. Critério de saída da P-0

P-0 só será encerrada quando houver:
- SHA exato e árvore limpa do checkout de referência;
- inventário reproduzível de crates, módulos, dependências e APIs;
- mapa dos módulos Python ativos e fronteiras PyO3;
- lista completa de workflows, triggers, filtros e checks obrigatórios;
- matriz de testes/oráculos e resultados vinculados ao SHA;
- baseline SG-1000 e SMS reproduzível com mídia legal/homebrew;
- reconciliação documentada das PRs concorrentes;
- matriz componente × sistema × implementação × teste × evidência × pendência.

**Nenhuma reestruturação ampla de runtime está autorizada por esta fotografia isolada.** A etapa seguinte é concluir o inventário executável e então propor fatias de refatoração pequenas, com branch/PR e regressões preservadas.
