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


---

## 10. Atualização de evidências — checkout, testes e CI (2026-10-31)

Esta seção complementa e prevalece sobre as pendências de proveniência e CI descritas nas seções anteriores.

### 10.1 SHA e estado do checkout

- `main` foi consultada diretamente no GitHub e aponta para `c31e4183b8d0b36d95b4f58582938186417652f8`.
- A PR #325 permanece Draft e sua branch `audit/p0-baseline-20261031` parte exatamente desse SHA.
- O usuário recriou o clone Windows e informou `git status --short --branch` como `## main...origin/main`, sem alterações locais, e `git rev-parse HEAD` igual ao SHA acima.
- `main` e `origin/main` foram reportadas no mesmo SHA; o clone também contém a branch remota da auditoria.
- O estado local foi informado pelo usuário; não foi executado um checkout Windows controlado nesta sessão.

### 10.2 Baseline Python no Windows

No clone em Windows 11, Python 3.14.7:

| Verificação | Resultado | Estado |
|---|---:|---|
| Instalação editável `pip install -e ".[dev,qt]"` | concluída | PASS |
| Imports `sega_emulator`, `numpy`, `PySide6` | OK | PASS |
| `python -m pip check` | No broken requirements found | PASS |
| `python -m pytest` | 313 passed, 8 skipped | PASS, com ressalva |
| `python -m ruff check .` | All checks passed | PASS |
| `python -m ruff format --check .` | 204 files already formatted | PASS |
| `python -m pip list --outdated` | somente pytest e Ruff | inventário |
| `cargo tree --locked` | cinco crates do workspace; SMS depende de Z80 bus | inventário |
| `cargo tree -d` | nothing to print | PASS, sem duplicações |

Os oito skips do pytest são associados à ausência do módulo nativo `sega_z80_core`: dois testes de seleção de runtime/áudio SMS, cinco testes SG-1000 nativos e um teste de integração VGM. Portanto, a suíte Python local passou, mas não validou os caminhos de integração que exigem a extensão. Não instalar pacote homônimo de origem não verificada.

As versões das dependências foram mantidas. O inventário encontrou apenas pytest 9.1.1 e Ruff 0.17.1 abaixo das versões listadas como latest pelo pip; nenhuma atualização foi aplicada.

### 10.3 CI real no SHA de main

A consulta direta ao GitHub Actions encontrou o check `quality` no SHA `c31e4183b8d0b36d95b4f58582938186417652f8`:

- Workflow run: `41243525506`.
- Job: `quality`, ID `124662133536`.
- Conclusão: `success`.
- Todas as etapas concluídas com sucesso: checkout, toolchains, instalação, Ruff, build e instalação da extensão Rust via Maturin, compileall, pip check, CLI de boot SMS e pytest.
- O workflow compilou `native/z80_core` com `maturin build --locked --manifest-path native/z80_core/Cargo.toml --release --features python-extension` e instalou a wheel.
- Ambiente CI: Linux, Python 3.14.9, pytest 9.1.1.
- Resultado da suíte CI: **233 passed, 11 skipped**.

Esse resultado demonstra que a extensão nativa foi compilada e importada no runner Linux e que a suíte Python desse ambiente passou. Não transfere aprovação para Windows nem substitui os três gates completos do workspace Rust. O job `quality` não executou `cargo fmt --all`, `cargo clippy --workspace` ou `cargo test --workspace`.

### 10.4 Inventário dos dez workflows no SHA de main

| Workflow | Gatilho e escopo observado | Validação principal |
|---|---|---|
| `ci.yml` | Todo PR, push em main e manual | Ruff (arquivos Python alterados em PR/push), build Maturin, compileall, pip check, CLI SMS e pytest |
| `agent-functional-validation.yml` | PR/push com alterações Python relevantes, diário e manual | Ruff e regressão Python |
| `agent-integrity.yml` | PR/push Python, diário e manual | compileall, pip check e duas execuções pytest |
| `qt-frontend-validation.yml` | PR com paths Qt/frontend/testes e manual | Qt headless, input e suíte Python |
| `qt-rust-z80-core.yml` | PR com paths da extensão/runtime/áudio e push em branch específica | Windows, Maturin, Clippy/testes de crates isolados e integração |
| `rust-sg1000-core.yml` | PR/push com paths SG-1000, ALU, bus e áudio | fmt, Clippy workspace, testes workspace e scaffold |
| `sg1000-three-agent-validation.yml` | PR/push SG-1000 e manual | testes focados sequenciais e gates Rust workspace |
| `sms-core-fast.yml` | PR/push com paths SMS/Z80 bus e manual | fmt workspace, Clippy SMS e testes SMS |
| `sms-z80-platform-validation.yml` | PR/push com paths CPU/ALU/bus/SMS e manual | fmt, Clippy e testes workspace; Z80 full e CCF externos |
| `ui-controller-validation.yml` | PR com paths de controle/UI, push em branch específica e manual | Ruff, compileall e testes de input/UI |

Observações de cobertura:
- `ci.yml` é amplo e executa pytest, mas não substitui os gates Rust do workspace.
- Os workflows de Rust têm filtros por paths diferentes. Uma alteração isolada em SMS pode acionar `sms-core-fast` e/ou `sms-z80-platform-validation`, mas não necessariamente `rust-sg1000-core`.
- Os workflows com `workflow_dispatch` podem ser executados manualmente; a existência do workflow não comprova que tenha rodado no SHA analisado.
- O check `quality` foi o único check-run encontrado associado diretamente ao SHA de main nesta consulta.

### 10.5 Reconciliação inicial das PRs concorrentes

As PRs continuam abertas e seus snapshots apontam bases anteriores ao SHA atual de main:

| PR | Head | Base SHA observado | Arquivos | Conclusão P-0 |
|---|---|---|---:|---|
| #300 input migration | `eeb8b8a...` | `2318fb3...` | 1 | Documento de proposta; não contém implementação |
| #301 vídeo migration | `d2d1543...` | `2318fb3...` | 1 | Documento de proposta; não contém implementação |
| #313 input Rust/SDL3 | `1c64734...` | `3cf846d...` | 30 | Implementação substancial de input nativo; requer rebase/reconciliação e validação |
| #315 Temporal Core | `bb1aec7...` | `3cf846d...` | 11 | Primitivas temporais isoladas; integração com máquinas continua pendente |
| #324 BIOS/mappers SMS | `51fd75e...` | `767d89b...` | 3 | Documentação e pesquisa; evidência binária de BIOS explicitamente não analisada |

A #313 é a implementação de input; #300 é somente o documento de migração. Portanto, não são duas implementações equivalentes, embora a proposta #300 deva ser atualizada para refletir a direção já implementada em #313. #301 também é somente documento de migração, não port de vídeo. #315 adiciona o crate `native/temporal_core`, mas não integra scheduler à CPU/barramento/VDP/PSG. #324 contém documentação BIOS/mappers e não altera runtime.

Todas essas PRs precisam ser comparadas/rebaseadas contra o `main` atual antes de qualquer decisão de integração. Não foram mescladas nem fechadas nesta auditoria.

### 10.6 Gates ainda pendentes para fechar P-0

| Gate | Estado | Evidência / ação necessária |
|---|---|---|
| SHA e clone limpo | PASS informado pelo usuário | SHA e `git status` no clone Windows |
| Python tests | PASS com ressalva | 313 passed; 8 skips por extensão ausente |
| Ruff check | PASS | execução local informada |
| Ruff format | PASS | execução local informada |
| pip check | PASS | execução local informada |
| Inventário Cargo tree | PASS | cinco crates e sem duplicações |
| CI quality no SHA main | PASS | check-run #124662133536; 233 passed, 11 skipped em Linux |
| cargo fmt workspace | NOT RUN | executar no clone Windows |
| cargo clippy workspace com `-D warnings` | NOT RUN | executar no clone Windows |
| cargo test workspace | NOT RUN | executar no clone Windows |
| Build/import da extensão no Windows | NOT RUN | usar o fluxo Maturin do CI |
| Baseline SG-1000 nativo em Windows | BLOCKED | extensão ausente no ambiente local atual |
| Baseline SMS nativo em Windows | BLOCKED | extensão ausente no ambiente local atual |
| ZEXALL/ZEXDOC ou oráculos Z80 | NOT RUN nesta fotografia | vincular execução e resultado a SHA específico |

**Conclusão atualizada:** a P-0 avançou de inventário documental para baseline parcialmente executado e CI comprovado. Ainda não pode ser encerrada: faltam os três gates Rust locais, build/import da extensão em Windows e smoke tests nativos SG-1000/SMS com resultados vinculados ao SHA.


---

## 11. Fechamento dos gates locais reportados — Windows (2026-10-31)

O usuário enviou a saída integral dos comandos de validação executados no clone Windows em `C:\Users\leost\sega-emulator`.

### 11.1 Gates Rust do workspace

| Comando | Evidência observada | Estado |
|---|---|---|
| `cargo fmt --all -- --check` | Retornou ao prompt sem diff/erro | PASS |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | `Finished dev profile`, sem warnings/erros | PASS |
| `cargo test --workspace --locked` | Todos os executáveis de teste e doc-tests concluíram; nenhuma falha | PASS |

O log mostra testes unitários para `sega_audio_runtime` (13), `sega_sg1000_core` (115), `sega_sms_core` (152), `sega_z80_alu` (7), `sega_z80_bus` (3), além de vários alvos de integração/exaustivos/referência do Z80. Entre os alvos explícitos: vetores exaustivos de ALU, CB, ED, controle condicional, prefixos indexados, interrupções, traces temporais ED, integração de memória Z80/SMS e vetores single-step. Todos os resultados visíveis são PASS.

### 11.2 Build e instalação da extensão nativa no Windows

O procedimento alinhado ao workflow CI foi executado:

- Maturin instalado em `1.15.0`.
- Build com `--locked`, manifest `native/z80_core/Cargo.toml`, perfil release e feature `python-extension`.
- Compilação dos crates locais e de PyO3 concluída.
- Wheel produzida: `sega_z80_core-0.1.0-cp314-cp314-win_amd64.whl`.
- Wheel instalada com pip.
- `import sega_z80_core` executado com sucesso.

Estado: **PASS** para build, instalação e import da extensão no Windows/Python 3.14.7.

### 11.3 Suíte Python final no Windows

| Verificação | Resultado | Estado |
|---|---:|---|
| `python -m pytest` | **330 passed, 0 skipped, 0 failed** | PASS |
| `python -m ruff check .` | All checks passed | PASS |
| `python -m ruff format --check .` | 204 files already formatted | PASS |
| `python -m pip check` | No broken requirements found | PASS |

A execução do pytest coletou 330 testes, incluindo seleção de runtime Rust, máquina nativa SG-1000, áudio nativo SMS, firmware SMS, integração VGM e filtros de vídeo. Os oito skips da execução anterior foram eliminados após compilar e instalar a extensão correta do próprio repositório.

### 11.4 Proveniência e ressalvas

- O log anexado identifica o caminho do clone, ambiente Python 3.14.7 e comandos executados, mas **não contém `git rev-parse HEAD` nem `git status`**.
- Em uma verificação anterior, o usuário informou que o clone estava limpo em `main`, no SHA `c31e4183b8d0b36d95b4f58582938186417652f8`. Não há no log recém-enviado prova direta de que o HEAD permaneceu nesse SHA durante todas as execuções.
- Portanto, os resultados são PASS como execução local reportada; para vinculá-los formalmente ao SHA candidato da P-0, falta apenas confirmar novamente `git rev-parse HEAD` e `git status --short --branch` no mesmo clone.
- A wheel e a pasta `native/z80_core/dist` são artefatos locais de build. Não devem ser adicionados ao Git; verificar `git status` antes de qualquer commit.
- A evidência não equivale a ZEXALL/ZEXDOC nem a validação temporal integral do hardware. Os vetores e testes registrados aumentam a cobertura semântica, mas não justificam declarar o Z80 cycle-accurate ou o scheduler temporal concluído.

### 11.5 Estado atualizado da P-0

Com os novos logs, os três gates Rust obrigatórios, a build/import da extensão Windows, os testes Python e Ruff estão todos PASS como execuções reportadas. O bloqueio de ambiente Python foi removido.

Permanecem para fechamento formal:
1. Confirmar SHA e árvore limpa após a geração da wheel.
2. Vincular as execuções locais ao SHA candidato.
3. Reconciliar os estados/bases das PRs concorrentes e registrar a decisão de integração, sem fazer merge nesta auditoria.
4. Verificar se os critérios P-0 definidos no documento inicial estão integralmente satisfeitos.

Não declarar TEMPORAL_PASS: o sucesso dos testes de workspace não prova integração de scheduler mestre com barramento, CPU, VDP e áudio em precisão cycle-accurate.


---

## 12. Reconciliação remota das PRs concorrentes — atualização 2026-10-31

A branch `main` foi consultada novamente e permanece no SHA `c31e4183b8d0b36d95b4f58582938186417652f8`. A comparação foi realizada usando o merge-base real de cada PR, não apenas a idade informada no campo base.

### 12.1 PR #325 — CI da auditoria

- Head: `f1b085ef22b9fa5d833ab8909461a25b1c33db43`.
- Base: `c31e4183b8d0b36d95b4f58582938186417652f8`.
- Workflow run `41246275232`, job `quality`: **SUCCESS**.
- Etapas concluídas: checkout, toolchains, instalação, formatação, lint, build/instalação da extensão nativa, compileall, pip check, smoke test SMS boot matrix CLI e pytest.
- A PR continua Draft; o sucesso do CI não fecha os critérios arquiteturais restantes.

### 12.2 PR #313 — implementação input Rust/SDL3

- Head: `1c64734c56413276b27ab0d85d9a8ec6978f3c41`.
- Merge-base com main: `3cf846d688798080d35f997f6e0d34680372d7e3`.
- Main avançou 36 commits desde o merge-base; a PR tem 106 commits à frente. O compare retornou estado `diverged`.
- GitHub informa `mergeable=true`, mas isso não substitui atualização/revisão da base nem os testes no head reconciliado.
- Comparação por caminho: nenhum dos 36 arquivos alterados em main desde o merge-base coincide com os 36 caminhos alterados pela PR #313. Isso reduz a evidência de conflito textual direto, mas não exclui conflito semântico ou de dependência.
- Workflows observados no SHA da PR concluíram SUCCESS: CI, Qt Frontend Rebuild Validation, UI and Controller Input Validation, SG-1000 sequencial, Qt Rust SG-1000 integration, SMS Core Fast Validation, Agent Functional Validation, SMS Z80 Platform Validation, Rust SG-1000 Workspace e Agent Integrity/Determinism.
- A PR contém implementação substancial: crate `native/input_runtime`, backend SDL3 Windows, ponte PyO3/Qt, migração de chamadas de input físico e testes. Limitações declaradas incluem backend Windows inicial, dois slots automáticos, ausência de rumble/FFB e ausência de medição input-to-photon.
- Decisão: **candidata prioritária de implementação**, mas não integrada. Exigir rebase/atualização controlada e repetir gates no SHA reconciliado; validar dispositivos físicos antes de promover a compatibilidade de hardware.

### 12.3 PR #315 — Temporal Core

- Head: `bb1aec75cf99c1ffd49f4e4c591250de43f1c0cb`.
- Merge-base com main: `3cf846d688798080d35f997f6e0d34680372d7e3`.
- Main avançou 36 commits; PR está 15 commits à frente; estado de comparação `diverged`.
- GitHub informa `mergeable=false`.
- Há sobreposição de caminhos com mudanças posteriores de main em `docs/architecture.md`, `docs/architecture/migrations/native-master-scheduler.md` e `docs/architecture/native-master-scheduler.md`.
- Workflows observados no head concluíram SUCCESS: CI, SMS Z80 Platform Validation, SG-1000 sequencial, Rust SG-1000 Workspace, Qt Rust SG-1000 integration e SMS Core Fast Validation.
- Decisão: não tentar merge/rebase automático. Resolver primeiro as divergências documentais e verificar cuidadosamente `Cargo.toml`/lockfile; em seguida reexecutar validações. Mesmo integrada, a crate temporal não autoriza TEMPORAL_PASS sem scheduler de máquina conectado aos componentes reais.

### 12.4 PR #324 — BIOS e mappers SMS

- Head: `51fd75e1d280ebf7a8180275ac60974604debbbe`.
- Merge-base com main: o próprio SHA atual `c31e4183b8d0b36d95b4f58582938186417652f8`.
- Main não possui commits exclusivos desde o merge-base; PR está 6 commits à frente.
- GitHub informa `mergeable=true`; workflow CI run `41243587738`, job `quality`: SUCCESS.
- Alterações: dois documentos de auditoria e inclusão no índice `docs/project-scope.md`; nenhum runtime alterado.
- A documentação explicita limites: sem análise de dumps locais, sem boot/gameplay executado e sem afirmar mapper implementado. Classificações desconhecidas permanecem UNKNOWN.
- Decisão: candidata documental tecnicamente integrável após revisão editorial final. Não foi mesclada nesta auditoria; a integração requer decisão explícita separada.

### 12.5 PRs #300 e #301 — propostas documentais

- #300 adiciona somente `docs/architecture/migrations/native-input-runtime.md`; seu escopo é uma proposta de migração, não implementação.
- #301 adiciona somente `docs/architecture/migrations/native-video-runtime.md`; seu escopo é uma proposta, não port de vídeo.
- Ambas têm base antiga (`2318fb35715c49c26e63f71debadacdb1816c79c`).
- Decisão: preservar os documentos como propostas e atualizar sua orientação quando apropriado; não tratá-las como PRs de runtime nem duplicar a implementação de input da #313. Não fechar automaticamente.

### 12.6 Matriz de decisão atualizada

| PR | Estado técnico | Base/merge | CI observado | Próxima ação |
|---|---|---|---|---|
| #300 | Documento de proposta | Base antiga | Não consultado nesta rodada | Manter como referência; reconciliar com #313 |
| #301 | Documento de proposta | Base antiga | Não consultado nesta rodada | Manter como referência; não declarar vídeo portado |
| #313 | Implementação input Rust/SDL3 | Diverged; sem caminhos alterados coincidentes detectados | Workflows listados SUCCESS no head antigo | Reconciliar branch e repetir gates |
| #315 | Temporal Core isolado | Diverged; mergeable=false e 3 caminhos documentais coincidentes | Workflows listados SUCCESS no head antigo | Resolver conflitos manualmente e revisar arquitetura |
| #324 | Documentação BIOS/mappers | Ahead 6, mergeable=true | CI SUCCESS | Revisão final; integração documental separada |
| #325 | Baseline P-0 documental | Base exata main | CI quality SUCCESS | Manter Draft até critérios P-0 fecharem |

**Limite de execução:** nenhuma branch foi rebaseada, nenhum arquivo de runtime foi alterado, nenhuma PR foi fechada ou mesclada nesta atualização. As comparações são evidência de estado remoto no momento da consulta; checks SUCCESS pertencem aos SHAs listados, não a futuros SHAs reconciliados.

### 12.7 Pendências finais de P-0

1. Confirmar a saída final de `git status --short --branch` após remover o artefato local `cargo-metadata.json` identificado pelo usuário. A remoção foi sugerida, mas ainda não confirmada.
2. Atualizar/reconciliar #313 e #315 em branches dedicadas, repetindo todos os gates nos novos heads.
3. Decidir separadamente a integração documental de #324.
4. Consolidar matriz de workflows e critérios de aceite por componente; não confundir CI verde com cobertura de hardware físico.
5. Preservar ZEXALL/ZEXDOC, testes externos independentes, traces temporais e validação física como evidências distintas; não declarar TEMPORAL_PASS.



---

## 13. Revalidação remota dos checks — 2026-10-31

Consulta posterior à seção 12, diretamente nos refs e check-runs do GitHub. O SHA de `main` permanece `c31e4183b8d0b36d95b4f58582938186417652f8`.

### 13.1 PR #325 — head atual da auditoria

- Head atual: `ae4216aa44a8ec77489ec19e9040e9b0e1ecc4bb`.
- Base: `c31e4183b8d0b36d95b4f58582938186417652f8`.
- Estado: aberta, Draft, `mergeable=true`.
- Check-run `quality`, ID `124671366410`, workflow run `41246652697`: **completed / success** no SHA exato `ae4216a...`.
- O status combinado legado continua `pending`, com zero status entries; isso não invalida o check-run de Actions, mas também não deve ser descrito como status combinado SUCCESS.
- O check informa duas annotations; seus detalhes não foram recuperados nesta consulta. Não inferir que sejam warnings de compilação nem ignorá-las sem leitura posterior.

### 13.2 PR #313 — checks completos no head existente

Head consultado: `1c64734c56413276b27ab0d85d9a8ec6978f3c41`. A PR continua aberta e Draft, com base antiga `3cf846d...`; comparação contra `main` permanece `diverged` (106 commits à frente, 36 atrás; merge-base `3cf846d...`).

Foram consultados os check-runs associados diretamente ao head. Todos os seguintes concluíram **SUCCESS** nesse SHA:
- Agent 3 — Workspace quality and complete Rust checks;
- Agent 2 — Memory and deterministic machine;
- Agent 1 — CPU instruction tests;
- Python 3.14 quality and regression suite;
- Pinned external Z80 conformance suite;
- Rust formatting;
- Workspace Rust tests;
- Workspace Clippy;
- Qt tests;
- Repeatability, imports and package integrity;
- Windows;
- SMS focused format, lint and tests;
- Python UI;
- Quality.

Isso eleva a evidência da PR #313: os gates completos de formatação, Clippy e testes do workspace, além da suíte externa Z80 pinada, passaram no **head original**. Não transfere esses resultados para um SHA atualizado/reconciliado com main.

A revisão de patch confirma que a implementação adiciona `sega-input-runtime`, backend SDL3 Windows, fachada PyO3 `NativeInputManager`, adaptador Qt/Python e snapshots de ações para SG-1000/SMS. Os testes de review consultados não retornaram review threads nem reviews submetidas. A comparação por caminhos continua sem sobreposição direta com os 36 caminhos modificados por main desde o merge-base, mas há alterações em manifests/lockfiles, workflows e integração nativa que exigem revisão semântica.

**Decisão mantida:** #313 é a candidata prioritária de implementação de input, porém permanece não integrada. Não foi criado branch vazio nem feita atualização automática: as ferramentas disponíveis permitem criar refs e objetos Git, mas não oferecem uma operação de cherry-pick/rebase com resolução e validação local reproduzível. A reconciliação deve ocorrer em checkout Git real, preservando a branch original e sem force-push.

### 13.3 PR #315 — Temporal Core

Head: `bb1aec75cf99c1ffd49f4e4c591250de43f1c0cb`. A PR continua Draft e diverged (15 commits à frente, 36 atrás), com `mergeable=false`.

Os check-runs consultados no head original concluíram **SUCCESS** para:
- Agent 3 — Workspace quality and complete Rust checks;
- Agent 2 — Memory and deterministic machine;
- Workspace Clippy;
- Workspace Rust tests;
- Pinned external Z80 conformance suite;
- Rust formatting;
- Agent 1 — CPU instruction tests;
- SMS focused format, lint and tests;
- Windows (dois jobs);
- Quality.

Os resultados comprovam qualidade do head original, mas não removem a divergência documental nem demonstram scheduler integrado à máquina. Não promover para TEMPORAL_PASS.

### 13.4 PR #324 — documentação BIOS/mappers

Head `51fd75e1d280ebf7a8180275ac60974604debbbe`, seis commits à frente de main, sem commits atrás e `mergeable=true`. Os checks `Sourcery review` e `quality` concluíram SUCCESS nesse head. O escopo permanece documental; nenhum runtime foi alterado.

### 13.5 Próxima decisão operacional

1. **#313:** atualizar em checkout Git dedicado a partir do main atual, sem modificar a branch original; revisar conflitos semânticos em manifests/lockfiles, extensão PyO3, workflows e fronteira Qt; executar novamente todos os gates no novo SHA.
2. **#315:** manter separado até resolução explícita dos conflitos nos três documentos de arquitetura e revisão do Cargo workspace/lockfile.
3. **#324:** manter como candidata documental separada; não mesclar automaticamente.
4. **#325:** manter Draft. A CI no head atual passou, mas ainda faltam a confirmação local pós-build de SHA/árvore limpa e o fechamento dos critérios de inventário e smoke tests definidos nas seções 9 e 11.

Nenhum runtime foi alterado, nenhuma branch de integração foi criada, e nenhuma PR foi fechada ou mesclada nesta atualização.



---

## 14. Revalidação CI e inspeção semântica do input — 2026-10-31

### 14.1 PR #325 — check confirmado no head atual

Nova consulta após a seção 13 confirmou:
- Head: `9c22e297a4ac11de184c5a5ebdef41ad5823655c`.
- Check-run `quality`, job `124671846261`, workflow run `41246815479`.
- Estado: `completed`, conclusão: **success**, SHA correspondente exatamente ao head atual.
- PR #325: aberta, Draft, base `c31e4183b8d0b36d95b4f58582938186417652f8`, `mergeable=true`.
- `main` permanece em `c31e4183b8d0b36d95b4f58582938186417652f8`.

A pendência de check-run do novo head registrada na seção 13 foi resolvida. A PR continua Draft por decisão de escopo; CI verde não encerra os critérios P-0.

### 14.2 Inspeção de código da PR #313 — fronteira do input

Foi feita leitura direta dos arquivos no head original `1c64734c56413276b27ab0d85d9a8ec6978f3c41`, incluindo:
- `native/input_runtime/src/lib.rs`;
- `native/input_runtime/src/sdl3_backend.rs`;
- `native/z80_core/src/input.rs`;
- `src/sega_emulator/frontend/rust_controller_input.py`;
- `src/sega_emulator/frontend/qt_frontend.py`;
- `scripts/install_sdl3_runtime.ps1`;
- manifests Cargo e adaptadores SG-1000/SMS.

O desenho principal é coerente com a fronteira arquitetural: SDL3 e mapeamento físico em Rust; PyO3 expõe máscaras compactas e nomes; Qt traduz teclado e aplica snapshots nos adaptadores de console. O backend SDL3 é explicitamente Windows-only nesta implementação inicial. Rumble/FFB e medições input-to-photon não são implementados nem reivindicados.

### 14.3 Achado de revisão — polling físico em callbacks de teclado

Em `qt_frontend.py`, `KeyboardInputFilter.eventFilter` chama `update_joystick()` em KeyPress e KeyRelease. Essa função chama `controller_input.poll_masks(...)`, que atravessa PyO3 e executa `NativeInputManager.poll_actions()`, incluindo `poll_backend()` e polling SDL3.

A mesma função também é chamada na entrada de `show_frame()`, imediatamente antes de `machine.run_frame()`.

Consequências e avaliação:
- O polling SDL3 não está restrito a uma única fronteira explícita por frame: também ocorre em eventos de teclado.
- Como Qt e a emulação são executados sequencialmente na mesma thread, a chamada de evento não interrompe uma instrução Z80 em andamento; portanto, não há evidência aqui de callback host dentro de CPU/barramento.
- Ainda assim, o mesmo snapshot de input é atualizado por dois caminhos de chamada, com frequência dependente da atividade do teclado. Isso dificulta reproduzir/medir a cadência de amostragem e torna o contrato de entrada menos simples do que o desejado.
- A recomendação é separar a atualização de teclado do polling físico: KeyPress/KeyRelease devem apenas atualizar o conjunto de teclas pressionadas; a leitura física SDL3 e a composição teclado+gamepad devem ocorrer uma única vez na fronteira do frame (ou em uma fronteira explícita do scheduler, quando existir). O tratamento de NMI por transição de HOLD deve consumir o snapshot composto nessa mesma fronteira.
- Adicionar testes que contem chamadas de polling por frame e provem que eventos Qt de teclado não acionam polling SDL3 nem avançam estado de turbo.

Classificação: **REVIEW FINDING — correção recomendada antes da integração**. Não é classificado como violação de execução dentro de instrução nem como bug confirmado de timing da CPU; é uma inconsistência de fronteira/cadência de amostragem.

### 14.4 Outras verificações semânticas observadas

- `poll_actions` usa `frame_number` para fasear turbo; a fase é determinística para uma sequência de números de frame, mas deve ser testada para pausas, reinício de ROM e chamadas extras de polling.
- A API `set_backend_preference` atualmente valida `auto`/ `sdl3`, mas não seleciona backends diferentes; isso é aceitável somente enquanto SDL3 for o único backend suportado e a UI não prometer XInput/GameInput.
- A alocação automática atribui os primeiros dois dispositivos enumerados aos jogadores 1 e 2. Não equivale a configuração persistente de identidade/dispositivo e deve permanecer descrita como atribuição automática básica.
- O SDL3 runtime é carregado dinamicamente e o script fixa versão `3.4.18` com SHA-256 esperado. O arquivo DLL é runtime externo local e está no `.gitignore`; não deve ser versionado.

### 14.5 Decisão operacional

A PR #313 continua candidata prioritária, mas o achado 14.3 deve ser corrigido no branch de integração antes de solicitar aprovação. A reconciliação Git real e os gates no novo SHA ainda não foram executados. Não houve criação de branch de integração, alteração de runtime, comentário/review remoto ou merge nesta etapa.


---

## 15. Correção inicial da fronteira de polling da PR #313 — 2026-10-31

Foi aplicado commit diretamente na branch original da PR #313, sem force-push:
- Branch: `feat/native-input-runtime-foundation`.
- Commit: `fc6463f986fc83e24d1024e6e4e2ad795b484f85`.
- Arquivo: `src/sega_emulator/frontend/qt_frontend.py`.
- Mudança: KeyPress/KeyRelease agora apenas atualizam `pressed_keys`; removidas as chamadas a `update_joystick()` nesses eventos. A composição e polling continuam no início de `show_frame()`, antes de `machine.run_frame()`.

A alteração remove o caminho de polling físico disparado por eventos de teclado, mantendo a aplicação do snapshot na fronteira do frame. Não altera o comportamento de Escape nem os contratos de teclado/controle.

Validação remota imediatamente após o commit:
- PR #313 head confirmado em `fc6463f...`.
- Compare com main: 107 commits à frente, 36 atrás, estado `diverged`.
- GitHub informou `mergeable=false` após o commit.
- A consulta imediata de check-runs retornou lista vazia; nenhum PASS foi atribuído ao novo SHA.
- Não houve checkout local, execução de testes, reconciliação com main, criação de branch de integração ou merge.

Próximo passo obrigatório: aguardar/consultar checks do SHA `fc6463f...`, criar branch de integração a partir do `main` atual e reconciliar a PR sem alterar a branch original. A divergência de 36 commits e a nova condição `mergeable=false` impedem qualquer tentativa de merge direto.


## 16. Revalidação da correção de input e diagnóstico do CI — 2026-10-31

Estado remoto confirmado:
- `main`: `c31e4183b8d0b36d95b4f58582938186417652f8` (inalterado).
- PR #313: head `fc6463f986fc83e24d1024e6e4e2ad795b484f85`, aberta e Draft.
- PR #313 compare: 107 commits ahead, 36 behind, `diverged`; mergeability retornou `true` nesta consulta, mas a divergência permanece e não autoriza merge.
- PR #325: aberta, Draft; não alterada nesta etapa.

Checks no SHA `fc6463f...`:
- PASS: Agent 2, Repeatability/import/package integrity, python-ui, quality, qt-tests, Python 3.14 quality/regression, Agent 1, Workspace Rust tests, pinned external Z80 conformance, Rust formatting, SMS focused format/lint/tests, Workspace Clippy e outro job Windows.
- IN PROGRESS na consulta: Agent 3 — Workspace quality and complete Rust checks.
- FAIL: job Windows `124672749170`, no passo `Install official SDL3 runtime for native input smoke tests`.

Diagnóstico do FAIL:
- A instalação de dependências Python terminou normalmente.
- O instalador falhou em `scripts/install_sdl3_runtime.ps1:18`, durante `Invoke-WebRequest -Uri $uri -OutFile $archivePath`.
- Erro reportado: timeout/conexão ao host remoto não respondeu.
- As etapas seguintes foram skipped; portanto, esse job não validou o build Maturin nem o smoke test SDL3.
- Classificação: falha de infraestrutura/download externo, não evidência de defeito no código SDL3. Ainda assim, o gate Windows do SHA não está aprovado.

Ação tomada:
- Solicitado rerun apenas do job Windows com falha, ID `124672749170`; GitHub confirmou `success=true` para a solicitação de rerun. Resultado do novo attempt ainda pendente.
- Nenhum código adicional foi alterado após `fc6463f...`.
- O ambiente de trabalho continuou sem DNS para github.com; `git ls-remote` falhou por resolução de host. Não foi possível checkout/rebase local.

Próxima decisão:
- Aguardar resultado do rerun Windows e do Agent 3.
- Depois, fazer reconciliação real em branch baseada no `main` atual. Não usar GitHub Contents API para simular rebase de uma PR com 107 commits; não alterar a branch original e não abrir PR de integração sem uma árvore reconciliada e validada.
