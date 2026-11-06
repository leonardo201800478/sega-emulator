# Auditoria consolidada de integração de PRs — Sega Emulator

**Data da fotografia:** 2026-11-06  
**Repositório:** `leonardo201800478/sega-emulator`  
**Main auditada após PR #597:** `0fb09e2cad46a92aefe6a98ca07549a5ba91d78c`  
**Critério:** PR aberta/Draft, CI antigo verde ou `mergeable=true` isoladamente não autorizam merge. O CI precisa corresponder ao HEAD atual, a base deve estar reconciliada, os gates SKIPPED são NOT RUN e a revisão técnica precisa cobrir o diff integrado.

## 1. Resumo executivo

- PR #595 — MERGED na main pelo SHA `733152f61b706b2243914ae017643cef949d4ae1`; CI #5178 SUCCESS no HEAD `69ca40ab29c486f26852adb7536a04f95f9dd7ae`.
- PR #596 — MERGED na main pelo SHA `3c0e0548f0f7923ec2e5a500b3a953528266fd2e`; documentação de baseline reconciliada.
- PR #597 — MERGED na main pelo SHA `0fb09e2cad46a92aefe6a98ca07549a5ba91d78c`; CI #5185 SUCCESS no HEAD `30554cc6642dd49d06adf495dca3e80f4822b081`, comprovando worker → NativeAudioSession → PcmSink em memória.
- PR #594 — CLOSED / SUPERSEDED pela #595; a implementação de IRQ foi substituída pela versão que preserva INT externa e estado de acknowledge. Nenhuma alteração exclusiva útil permaneceu sem port.
- PR #537 — CLOSED / SUPERSEDED; aditivo E7 exclusivo foi preservado no relatório histórico, e inventário antigo foi substituído por esta auditoria.
- PRs #538–#541 — CLOSED / SUPERSEDED por #562/#563, já integradas na main; seus limites de evidência permanecem documentados.
- Permanecem 13 PRs abertas na fotografia. Nenhuma das PRs abertas abaixo foi integrada por esta auditoria.
- CI de uma branch antiga é evidência histórica. Para todas as PRs abertas abaixo, a base/HEAD deve ser reconciliada e o CI repetido no novo HEAD antes de qualquer merge.

## 2. PRs resolvidas

| PR | Branch / HEAD | Base registrada | Decisão | CI / evidência | Próxima ação |
|---|---|---|---|---|---|
| #595 — functional A/V baseline | `fix/sms-functional-av-baseline-20261106` @ `69ca40ab29c486f26852adb7536a04f95f9dd7ae` | `main` @ `8bd7f6c78cacd247736268d141e4cc7e30805ccd` | **MERGED** → `733152f61b706b2243914ae017643cef949d4ae1` | CI #5178 SUCCESS; Rust/Python/Windows D3D12/PyO3 PASS; Scheduler stress, Qt smoke e PowerShell NOT RUN | Prosseguir A/V-04, host físico e gameplay; não declarar SMS funcional |
| #594 — VDP IRQ → Z80 INT | `fix/sms-vdp-irq-wiring-20261106` @ `9fc6a669c2d77c90c675f20060e78707d1633504` | `main` @ `8bd7f6c78cacd247736268d141e4cc7e30805ccd` | **SUPERSEDED / CLOSED** | CI antigo CANCELLED; diff comparado com #595 | Nenhuma portabilidade exclusiva identificada; relatório absorvido em `sms-runtime-functional-baseline-2026-11-06.md` |
| #596 — SMS A/V documentation | `docs/reconcile-post-sms-av-baseline-20261106` @ `b6cbdddd73a24c7b9c9e7c3f379ca6872f0eff3d` | `main` @ `733152f61b706b2243914ae017643cef949d4ae1` | **MERGED** → `3c0e0548f0f7923ec2e5a500b3a953528266fd2e` | CI documental SUCCESS no HEAD exato; jobs não aplicáveis SKIPPED | Atualizada novamente nesta branch para refletir PR #597 e main atual |
| #597 — worker audio integration | `test/sms-worker-native-audio-session-20261106` @ `30554cc6642dd49d06adf495dca3e80f4822b081` | `main` @ `3c0e0548f0f7923ec2e5a500b3a953528266fd2e` (merge-base exato) | **MERGED** → `0fb09e2cad46a92aefe6a98ca07549a5ba91d78c` | CI #5185 SUCCESS no HEAD exato; Rust, Python, D3D12 e PyO3 PASS; Scheduler stress, Qt, PowerShell e lock refresh NOT RUN | Comprovado PCM não silencioso no `NativeAudioSession` com sink em memória; backend físico e audibilidade NOT RUN |
| #537 — open PR/Z80 audit docs | `docs/reconcile-open-pr-audit-2026-11-05` @ `0a578ed8c28e481905f2d93bcfa34eddcee65b4b` | `main` @ `1da784f556b9524f5a6f43ce8a7c525e68354f9e` | **SUPERSEDED / CLOSED** | Sem CI útil no HEAD atual; branch divergente | Conteúdo histórico E7 portado; auditoria de PRs substituída por este relatório |

## 3. PRs abertas — fotografia atual

| PR / título | Branch @ HEAD | Base registrada @ SHA | Decisão | CI associado ao HEAD consultado | Dependências, riscos e próxima ação |
|---|---|---|---|---|---|
| #593 — VDP/PSG telemetry | `debug/sms-runtime-signal-telemetry-20261106` @ `ee5ba8925e5b1a1dfe344d6739727c1d9d2e209a` | `main` @ `8bd7f6c78cacd247736268d141e4cc7e30805ccd` | **DRAFT** | Run `42170191385` CANCELLED | Avaliar utilidade de pixels não pretos, Mode 4/Display Enable, VRAM/CRAM, amostras PSG e amplitude PCM. Scans por frame podem impor overhead; medir custo ou tornar coleta desativável. Rebasear na main e repetir CI antes de reconsiderar. |
| #587 — external TH events | `feat/sms-scheduler-external-th-events` @ `381e3cb98c5cc901b5b25953966627575ed907c4` | `main` @ `5211a2f66b04f7489109baf4a52e001632631b0f` | **BLOCKED** | Run `42157641684` SUCCESS no HEAD consultado; não valida a base atual | Base antiga/divergente e mergeable=false após #597. Reconciliar com a main atual `0fb09e2cad46a92aefe6a98ca07549a5ba91d78c`; revisar deadlines, snapshots e causalidade. Não implementa captura H-counter por borda TH, offset de Light Phaser ou precisão dot-level. Reexecutar todos os gates após reconciliação. |
| #566 — frame geometry/overscan metadata | `feat/video-frame-metadata-20261105` @ `490efa3cc7f72a638277e82c84d74db8c40c8429` | `main` @ `b129cd1577fd2b2fd1c1f6b8afb8b40d4f4d59a4` | **BLOCKED** | Run `42098595074` SUCCESS no HEAD consultado | Diff adiciona `VideoRect`, `VideoAspectRatio`, `VideoRasterMetadata` e `VideoFrameMetadata` ao `VideoFrame` em `native/video_runtime/src/lib.rs`, além de ADR/roadmap. Contrato útil para active/visible rect, overscan, aspecto e timestamp; ainda não há consumidor de apresentação demonstrado nesta PR. Base antiga e conflito documental; portar/reconciliar seletivamente, validar API e consumidores D3D12/Qt/mailbox, executar testes de metadados e CI no novo HEAD. Não integrar apenas pelo CI histórico. |
| #543 — Sega mapper SRAM | `feat/sms-cartridge-sram-stack-20261105` @ `3bb07dc981d22d4242e86bd89090ed212d3e4f71` | `fix/sms-io-event-timing-gameplay-20261105` @ `02b8c5526a79513364783904a9300b533fe5669a` | **DRAFT** | Run `42029934531` SUCCESS no HEAD consultado | PR depende de branch de correção de timing de I/O, não aponta para main. Validar integração da SRAM e persistência in-game, capacidade/caminho por DAT e regressão. Reavaliar base/dependência antes de retirar Draft. |
| #533 — permanent completion contract | `project/permanent-completion-contract` @ `bad9978a04bdcce389cd0a5c5af0d76575942e97` | `main` @ `6ffd7c45ba4c1a1ddd8da7cde1145d52f2a2ed9b` | **DRAFT** | Run `41966637150` SUCCESS no HEAD consultado | Contrato de conclusão global. Deve permanecer Draft até o projeto e seus gates estarem efetivamente concluídos; não usar como selo prematuro. |
| #530 — multiplatform temporal contract | `design/z80-multiplatform-temporal-contract` @ `8f1e49d5e199d27a15916e0aaa7f23abd1659cbb` | `main` @ `c8da934d051f69f0fad01d133a923c26248a7250` | **DRAFT** | Run `41962771409` SUCCESS no HEAD consultado | Revisar consistência com ADR-0016 e arquitetura atual; manter Draft enquanto contrato não estiver reconciliado e validado pelas plataformas. |
| #527 — independent E7 audits | `audit/z80-e7-three-agent-20261104` @ `eee762564025140d992ecee4630a6cb966220915` | `main` @ `249d7a3da613dec456f22aa6a7ceaccb2418ee0b` | **DRAFT** | Run `41946088177` SUCCESS no HEAD consultado | Evidência documental E7 antiga; confrontar com scorecard/matriz atuais e main. E7.7 permanece IN_PROGRESS; E8 BLOCKED. Não integrar auditoria sem reconciliação. |
| #526 — MachineInstant | `feat/temporal-core-machine-instant-c1-clippy` @ `a1a47906f4c9f07efe96facc2e0a3cbe628f0fa9` | `main` @ `249d7a3da613dec456f22aa6a7ceaccb2418ee0b` | **DRAFT / BLOCKED** | Run `41947928084` SUCCESS no HEAD consultado | Conflitos e dependência temporal ainda não demonstrada como necessária ao runtime SMS atual. Reconciliar arquitetura, snapshots e Scheduler somente após consumidor observável; repetir CI. |
| #514 — bus M-cycles/T-state edges | `feat/z80-e7-4-1b-bus-tstate-runtime-20261104` @ `ed81de385f7f4a0e7ed50de434b1f4c143cdad9f` | `main` @ `869b402e4c8f2dbd9c5f5fa4a10d10ac26506461` | **DRAFT / BLOCKED** | Run `41896166347` SUCCESS no HEAD consultado | E7.4.1B incompleta semanticamente; revisar WAIT/TW, amostragem e commit de efeitos. Não integrar até fechar os bloqueios semânticos e reexecutar os gates. |
| #512 — resumable M-cycle contract | `feat/z80-e7-4-1a-mcycle-contract-20261104` @ `19866f365b4873603a7dd230121a1127aeca9c2d` | `main` @ `869b402e4c8f2dbd9c5f5fa4a10d10ac26506461` | **DRAFT / BLOCKED** | Run `41891089747` SUCCESS no HEAD consultado | Contrato ainda não conectado ao sequenciador/barramento; WAIT externo durante TW e TW automático I/O permanecem problemas registrados. CI verde não fecha semântica. |
| #447 — DD/FD indexed sequencer | `feature/z80-dd-fd-indexed-sequencer` @ `405944de94b7a90b921867a1ce8a827e658f08b9` | `main` @ `eba8faae3f87baed2e64dfbe96f2009474d9f212` | **BLOCKED** | Run `41665270882` SUCCESS no HEAD consultado | Conflito e lacunas semânticas/diferenciais: matriz DD/FD completa, flags não documentadas, IXH/IXL/IYH/IYL e traces. Não integrar; E4C/E5/E6 já estão integradas e não devem ser reabertas sem divergência comprovada. |
| #424 — SMS cartridge SRAM | `feat/sms-cartridge-ram-mapper` @ `bda933fa1116ff12e6c9a8492ebe163eaa653ceb` | `main` @ `9c9b6dcad052193b81de33845b6bf94bd5f927c0` | **BLOCKED** | Run `41564391709` SUCCESS em revisão de código anterior; verificar HEAD exato atual após reconciliação | Conflito e validação in-game/persistência incompletas; branch contém commits de código/documentação antigos. Comparar com #543 para evitar duplicação, portar somente o trabalho útil e repetir CI no HEAD final. |
| #338 — VDP audit/native graphics backends | `feat/sms-vdp-audit-native-graphics-apis` @ `99d68af51023c3ef87a0ab8c1779f8ffee4dfca3` | `main` @ `0da3098893cce4d80f7a22291d00488e4e7d0f9b` | **DRAFT / BLOCKED** | Run `41347940527` FAILURE no HEAD consultado | Plano amplo de auditoria/backends com conflitos e CI falho. Preservar como planejamento; reavaliar escopo e dependências somente após baseline A/V e VDP funcional estarem comprovados. |

## 4. Gates e decisões de integração

- `SUCCESS` de CI antigo significa somente que aquele SHA passou no workflow indicado. Não autoriza merge depois de mudança de base ou HEAD.
- `CANCELLED`, `FAILURE`, `SKIPPED` e ausência de execução não são PASS.
- PRs `DRAFT` permanecem DRAFT até revisão de escopo, dependências e critérios de aceite.
- Conflitos exigem reconciliação em branch própria, sem force-push ou reset destrutivo; todo novo HEAD precisa de CI novo.
- PR #595 foi integrada como baseline automatizada A/V, não como conclusão funcional SMS.
- PR #594 foi fechada como SUPERSEDED após comprovar que a #595 cobre a ligação IRQ e preserva o comportamento adicional da INT externa.
- Nenhuma PR aberta nesta tabela deve ser mesclada sem reauditoria contra a `main` vigente. Na fotografia desta PR, o SHA remoto confirmado e `0fb09e2cad46a92aefe6a98ca07549a5ba91d78c`.

## 5. Próxima sequência de trabalho

1. Validar `NativeAudioBackend` real/WASAPI/XAudio2 com PCM conhecido e métricas de aceitação/underrun.
2. Validar host Windows físico: janela D3D12, GPU e áudio audível em endpoint real.
3. Executar ROM legal/homebrew e jogo representativo; registrar hash local e comportamento, sem versionar ROM comercial.
4. Reconciliar #587 e #566 somente depois de inspecionar conflitos e consumidores na main atual.
5. Manter #593 como Draft até a telemetria ser opt-in ou ter overhead medido.
6. Manter E7.7 IN_PROGRESS e E8 BLOCKED; consultar scorecard e matriz DD/FD antes de qualquer alteração Z80.
7. Preservar e executar regressões SG-1000 em todas as alterações compartilhadas.

## 6. Limites desta fotografia

Esta auditoria registra metadados, branches, HEADs e os últimos workflows associados aos commits consultados em 2026-11-06. Não representa uma aprovação técnica integral de cada diff aberto. Antes de trabalhar em qualquer PR, buscar novamente estado, HEAD, base, comentários/reviews, arquivos alterados, conflitos e CI no SHA exato. A main do GitHub permanece a única fonte da verdade.