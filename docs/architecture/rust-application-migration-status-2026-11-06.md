# Status da migração integral da aplicação para Rust — 2026-11-06

- **Repositório:** https://github.com/leonardo201800478/sega-emulator
- **ADR normativa:** [ADR 0009 — Aplicação integralmente Rust com egui/eframe](../adr/0009-full-rust-application-egui-eframe.md)
- **Inventário de origem:** [M0.2 — Inventário de migração](rust-application-migration-inventory.md)
- **Roadmap normativo:** [Roadmap mestre](../project-roadmap.md)
- **Main confirmada nesta atualização:** `333900ae35a87b2430caedd2edf26218f272c8a6`
- **Situação geral:** migração iniciada; M0 documentado e M1a integrado. A migração funcional da aplicação ainda não foi concluída.

Este documento é o registro factual do que foi integrado até o SHA indicado. PR aberta, código compilável, CI verde e validação física são estados diferentes; não devem ser tratados como equivalentes.

## 1. Resumo executivo

A direção arquitetural foi alterada para que o produto final seja uma aplicação integralmente Rust, com interface desktop egui/eframe. Python, PySide6, NumPy, PyO3 e Maturin deixam de ser dependências do produto final, mas permanecem no repositório durante a transição até que cada fluxo tenha substituto Rust e paridade demonstrada.

**O que já está integrado na main:**

1. ADR 0009 aceita a arquitetura de aplicação Rust/egui/eframe e define migração incremental, sem reimplementar hardware.
2. Inventário M0.2 registra 40 módulos Python, fluxo de execução SMS, serviços, persistência, recursos, testes e fronteiras Rust/PyO3.
3. O executável inicial `native/sega_app` foi adicionado com eframe 0.37.0, workspace e lockfile próprios, shell egui e teste headless.
4. A CI foi ajustada para priorizar gates Rust obrigatórios e manter Python/Qt/PyO3 como verificações manuais/opt-in durante a migração.
5. A PR #601 foi integrada na main após todos os jobs obrigatórios da CI #5262 concluírem com sucesso no HEAD exato.

**O que ainda não foi migrado:** biblioteca/índice de ROMs, carregamento e auditoria de mídia, firmware/região, preferências e importação QSettings, histórico, perfis de controle, captura de input, configuração de áudio, VGM, logging/diagnóstico, ciclo de vida da máquina, camada de aplicação e integração funcional com SMS/SG-1000. O worker SMS ainda está no crate PyO3. Nenhum código Python foi removido.

## 2. Linha do tempo de integração

| PR | Estado | Conteúdo relevante |
|---|---|---|
| [#600](https://github.com/leonardo201800478/sega-emulator/pull/600) | CLOSED — superseded, não merged | Proposta inicial substituída pela #601. Não considerar código da #600 integrado por si só. |
| [#602](https://github.com/leonardo201800478/sega-emulator/pull/602) | MERGED | CI Rust-first; Python lint/regressão, Qt e PyO3 passam a ser opt-in/manual na transição. Merge SHA `1a10779a37cf4da512186104bb5041552559a000`. |
| [#601](https://github.com/leonardo201800478/sega-emulator/pull/601) | MERGED | ADR 0009, inventário M0.2, atualização do roadmap e shell egui/eframe M1a. Merge SHA `333900ae35a87b2430caedd2edf26218f272c8a6`. |
| [#603](https://github.com/leonardo201800478/sega-emulator/pull/603) | OPEN — não integrada | M1b: viewport D3D12 filho hospedado no shell eframe. CI automatizada passou no HEAD indicado, mas validação física Windows/GPU e reconciliação FIFO/mailbox estão pendentes. |
| [#599](https://github.com/leonardo201800478/sega-emulator/pull/599) | OPEN — não integrada | Aceitação do áudio nativo em endpoints WASAPI/XAudio2 reais. CI automatizada passou, mas testes em endpoint físico continuam NOT RUN. É uma evidência de host/runtime adjacente à migração, não uma entrega de frontend Rust. |

A PR #601 foi mergeada com expectativa de HEAD `86a51e1217aa588764c1559152765d0931beccb7`; GitHub confirmou o merge e criou `333900ae35a87b2430caedd2edf26218f272c8a6`. O commit de merge tem como primeiro pai a main anterior `1a10779a37cf4da512186104bb5041552559a000`.

## 3. M0 — auditoria e decisão arquitetural (integrado)

### 3.1 ADR 0009

A ADR 0009 estabelece:

- Aplicação final nativa Rust com egui/eframe.
- Separação entre apresentação egui, casos de uso da aplicação, serviços de host e máquinas/chips compartilhados.
- UI sem execução de hardware, sem avanço de clocks e sem callbacks por instrução.
- Scheduler como autoridade única de clocks e eventos emulados.
- Migração incremental; o frontend Python legado é baseline/rollback até a paridade ser provada.
- Preservação do renderer D3D12 existente. O renderer wgpu/eframe atende a UI; não substitui o caminho de pixels do emulador.
- Nenhuma duplicação de CPU, VDP, PSG, áudio, Scheduler ou worker.

### 3.2 Inventário M0.2

O inventário de origem identificou **40 módulos Python** sob `src/sega_emulator/` e separou o fluxo ativo do código dormente. O caminho padrão observado inicia a janela SMS `MinimalSmsWindow`; o frontend SG-1000 Qt antigo está desativado e falha explicitamente, portanto sua presença no repositório não prova operação.

Famílias de responsabilidade mapeadas para Rust:

- Entry point e composição da aplicação.
- Preferências e persistência QSettings.
- Biblioteca, indexação, histórico e catálogo DAT.
- Carregamento de ROM/ZIP, hashes, auditoria e firmware.
- Perfis de cartucho e região.
- Fachadas de SMS/SG-1000 e worker temporal.
- Input, controles e perfis.
- Configuração de áudio e gravação VGM.
- Diagnóstico, logging e telemetria.
- UI Qt, diálogos, viewport, localização e filtros.

O inventário também registra contratos de dados a preservar: caminhos, membros ZIP, tamanho, hashes, timestamps, histórico de ROMs, chaves de preferências, recuperação de índice corrompido, assets e arquivos DAT. Há diferenças ainda a reconciliar entre a implementação histórica e o contrato normativo, incluindo histórico de 12 versus 15 ROMs, política de regiões e opções Light Phaser que atualmente são apenas preferências não consumidas pelo runtime.

### 3.3 Fronteira PyO3 crítica

`native/z80_core/src/sms_temporal_worker.rs` contém worker, construção da máquina temporal SMS, comandos de ciclo de vida e integração de áudio/vídeo, mas o crate também é uma extensão PyO3.

A estratégia aprovada é **extrair a implementação para uma biblioteca Rust comum e manter PyO3 como adaptador transitório**, ambos consumindo o mesmo worker. Não criar um segundo worker, uma segunda máquina ou um Scheduler paralelo.

## 4. M1a — shell egui/eframe (integrado)

A PR #601 adicionou:

- `native/sega_app/Cargo.toml`: binário `sega-app`, workspace isolado nesta fase e dependência exata `eframe = 0.37.0`.
- `native/sega_app/Cargo.lock`: lockfile próprio, gerado pela toolchain Rust 1.99.1.
- `native/sega_app/src/main.rs`: entry point nativo Windows.
- `native/sega_app/src/app.rs`: janela egui inicial, estado dos subsistemas e encerramento.
- `native/sega_app/src/lib.rs`: biblioteca da aplicação e exportação do shell.
- `native/sega_app/README.md`: comandos de build e limites da fatia.
- Workflow CI: validação de metadados/lockfile, fmt, check, Clippy e teste headless do app; compilação/testes Windows do vídeo D3D12 e áudio nativo.

**Limites intencionais do shell integrado:**

- Não abre ROM.
- Não instancia SMS ou SG-1000.
- Não inicia CPU, Scheduler, áudio ou input.
- Não apresenta framebuffer real.
- Não implementa ainda biblioteca, configurações, menus funcionais ou ciclo de sessão.
- Não substitui funcionalmente o frontend Qt.

O teste headless prova renderização do shell egui sem janela nativa ou hardware; não prova integração de host.

## 5. CI e validação já executadas

### 5.1 Política CI — PR #602

A PR #602 tornou o pipeline normal Rust-first. Os jobs de Python lint/regressão, build/import PyO3 e Qt smoke ficam fora do caminho obrigatório padrão e podem ser disparados manualmente/opt-in. Isso é uma política de transição, **não** prova de que Python, Qt ou PyO3 foram removidos ou continuam validados a cada PR.

### 5.2 PR #601 — HEAD `86a51e1217aa588764c1559152765d0931beccb7`

CI run #5262 / run id `42233907235`: conclusão SUCCESS.

- Rust quality and unit regression: PASS.
- Scheduler deterministic stress: PASS.
- Windows PowerShell tooling syntax: PASS.
- Windows x64 D3D12 compile: PASS, incluindo fmt/check/Clippy/teste headless do app, compilação/Clippy/testes do video runtime e compilação/Clippy/testes do áudio WASAPI/XAudio2.
- Python lint and regression: SKIPPED por política opt-in.
- Windows PyO3 extension build: SKIPPED por política opt-in.
- Windows Qt smoke: SKIPPED por política opt-in.
- Refresh manual de lockfiles: SKIPPED, não aplicável à execução normal.

Os três SKIPPED acima não são PASS. O gate automatizado exigido para merge da PR #601 estava concluído e verde no HEAD exato. Execução manual do aplicativo em Windows físico e paridade funcional: NOT RUN.

### 5.3 PRs adjacentes ainda abertas

- PR #603: CI #5259 SUCCESS no HEAD `d591c1648dd13bb384ae07cf3e2b8c9842202cec`. Validação de HWND filho, foco/teclado/mouse, z-order/clipping, resize/DPI, minimização/restauração, fechamento e device loss em Windows físico/GPU real: NOT RUN. A semântica latest-frame de `FrameMailbox` também precisa ser reconciliada com o FIFO lossless da ADR-0012 antes da integração vertical.
- PR #599: CI #5206 SUCCESS no HEAD `bb88e96cdb45d703f66d23acde42a2ead74fadd4`. Testes físicos ignorados por padrão que verificam submissão de PCM a endpoint WASAPI/XAudio2: NOT RUN. A/V-04b, A/V-05 e A/V-06 continuam sem comprovação física.

CI verde prova somente os checks executados no SHA correspondente. Não substitui validação de hardware, paridade funcional ou evidência de usuário.

## 6. Estado por área funcional

| Área | Estado em main | Próxima evidência necessária |
|---|---|---|
| Decisão de arquitetura Rust/egui | INTEGRADO | Revisar somente se houver nova evidência técnica |
| Inventário Python M0.2 | INTEGRADO como auditoria estática | Confirmar consumidores residuais em checkout Windows e atualizar durante a migração |
| Shell egui/eframe M1a | INTEGRADO | Executar manualmente em Windows e ampliar a UI |
| CI Rust-first | INTEGRADO | Manter checks obrigatórios no HEAD final |
| Viewport D3D12 filho | NÃO INTEGRADO; PR #603 aberta | Resolver FIFO/mailbox e executar validação física |
| Worker SMS independente de PyO3 | PENDENTE | Extrair para biblioteca comum sem duplicação |
| Camada `sega-application` | PENDENTE | Comandos tipados, snapshots e ciclo de vida coarse-grained |
| ROM/ZIP/DAT/firmware/biblioteca | PENDENTE | Portar serviços e provar preservação de dados |
| Preferências/QSettings | PENDENTE | Importação versionada, backup e round-trip |
| Input/perfis/SDL3 | PENDENTE | Portar captura e persistência com regressão |
| Configuração de áudio/VGM | PENDENTE | Portar serviços; preservar runtime e PCM Rust |
| Logging/telemetria/diagnóstico | PENDENTE | Portar agregação e UI sem bloquear event loop |
| Frontend Qt/PySide6 | AINDA PRESENTE | Remover somente após paridade funcional por fluxo |
| PyO3/Maturin | AINDA PRESENTES | Remover após todos os consumidores migrarem para APIs Rust |
| Python de produção | AINDA PRESENTE | Remover após substituição e auditoria de empacotamento |
| Save states e determinismo integrado | NÃO COMPROVADO pela migração da UI | Testes de snapshot/replay e execução contínua versus blocos |

## 7. Próximas tarefas — ordem recomendada

1. **Fechar M1b sem antecipar integração física:** atualizar a branch da PR #603 sobre a main atual, reconciliar o contrato FIFO lossless da ADR-0012 com a mailbox de apresentação e executar a matriz física Windows/GPU. Manter a PR aberta até evidência reproduzível.
2. **Fechar a aceitação de áudio:** executar os testes ignorados da PR #599 em Windows com endpoint real; registrar buffers aceitos, descartes, erros assíncronos e resultado de audição manual separado.
3. **Extrair o worker temporal SMS:** criar biblioteca Rust comum, mantendo o adaptador PyO3 chamando a mesma implementação durante a transição. Não duplicar worker nem Scheduler.
4. **Criar a camada `sega-application`:** contratos tipados e operações coarse-grained como `start()`, `pause()`, `stop()`, `reset()`, `load_rom()` e `step_frame()`; snapshots/eventos imutáveis; UI sem callbacks por instrução.
5. **Integrar sessão SMS verticalmente:** abrir ROM legal/sintética, ciclo de vida, vídeo D3D12, áudio nativo, input, telemetria e encerramento ordenado. Preservar SG-1000 e o Z80 compartilhado.
6. **Portar serviços de aplicação por fatias:** mídia/catalogação/firmware; preferências; biblioteca/histórico; controles; áudio/VGM; diagnóstico; UI.
7. **Provar paridade e preservação:** comparar fluxos antigos e novos, validar dados persistidos, ZIPs, hashes, histórico, preferências, perfis, dispositivos e erros.
8. **Remover dependências legadas somente no gate final:** pesquisar imports, packaging, workflows, scripts e ferramentas; retirar PySide6, NumPy, PyO3 e Maturin apenas quando nenhum fluxo de produção depender deles.
9. **Validar release:** build Release reproduzível Windows, testes de instalação/atualização e regressões sistêmicas. Não declarar migração completa antes da paridade.

## 8. Critérios de conclusão da migração

A migração somente poderá ser declarada concluída quando todos os itens abaixo forem evidenciados no HEAD final:

- Aplicação distribuída executa sem interpretador Python, PySide6, NumPy, PyO3 ou Maturin.
- Biblioteca, carregamento de mídia, firmware, configurações, histórico, input, áudio, diagnóstico e logging têm implementações Rust funcionais.
- Worker SMS é biblioteca Rust comum; adaptadores temporários não são necessários.
- UI egui/eframe não executa hardware nem controla clocks.
- Renderer D3D12 original permanece único caminho de apresentação do framebuffer, salvo ADR substitutiva fundamentada.
- FIFO lossless e política de descarte de apresentação estão explicitamente definidos; nenhuma operação de emulação é descartada.
- Start/pause/stop/reset/load_rom/step_frame e encerramento são determinísticos e coarse-grained.
- Save states/replay preservam integralmente os estados necessários da máquina e do Scheduler.
- SG-1000 e SMS mantêm regressões e não há duplicação de CPUs ou componentes compartilhados.
- Paridade funcional dos fluxos migrados foi comparada com a baseline antiga.
- Cargo fmt, Clippy com `-D warnings`, testes do workspace, testes Windows, e testes de integração aplicáveis passam no HEAD exato.
- Testes físicos obrigatórios foram executados; nenhum NOT RUN/BLOCKED é mascarado como PASS.
- Documentação, inventário, ADRs e roadmap refletem o código final.

## 9. Referências GitHub

- [PR #599 — aceitação áudio físico](https://github.com/leonardo201800478/sega-emulator/pull/599)
- [PR #601 — shell egui/eframe, MERGED](https://github.com/leonardo201800478/sega-emulator/pull/601)
- [PR #602 — CI Rust-first, MERGED](https://github.com/leonardo201800478/sega-emulator/pull/602)
- [PR #603 — viewport D3D12 filho](https://github.com/leonardo201800478/sega-emulator/pull/603)
- [Merge #601 — `333900ae35a87b2430caedd2edf26218f272c8a6`](https://github.com/leonardo201800478/sega-emulator/commit/333900ae35a87b2430caedd2edf26218f272c8a6)


## 10. Diretriz adicional — eliminar a suíte de testes Python

A decisão do projeto passa a ser retirar integralmente os testes Python ativos. Os comportamentos essenciais serão comprovados por testes Rust; a interface egui/eframe terá testes próprios de aplicação e integrações nativas Windows. Não serão adicionados novos testes Python.

A auditoria inicial está registrada em [python-test-retirement-inventory-2026-11-06.md](python-test-retirement-inventory-2026-11-06.md): 42 arquivos, 272 funções de teste declaradas. A execução local reportada pelo usuário coletou 352 casos (351 aprovados, 1 falha), mas o log não informa o SHA e não deve ser atribuído à baseline GitHub.

**Status nesta etapa:** inventário estático PASS; equivalência Python/Rust NOT AUDITED; execução Rust nesta etapa NOT RUN; remoção de testes NOT STARTED. A suíte não será apagada em lote. Cada domínio será migrado e validado antes da retirada dos respectivos arquivos, da configuração pytest e dos jobs manuais Python na CI.
