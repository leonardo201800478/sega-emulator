# Estado do projeto — 2026-11-11

**Repositório:** https://github.com/leonardo201800478/sega-emulator
**Main auditada:** `82ccad8eb9cd11d78dfddd168f307f90012426b5`
**Escopo:** fotografia do GitHub, PRs, workflows e CI. Não representa execução física local de Vulkan.

## Resumo

O runtime Rust, o Scheduler e a integração SMS/SG-1000 na `main` devem ser preservados. A arquitetura desktop alvo definida na ADR 0011 é Slint 1.18.x + WGPU 30.x + CPAL 0.18.2, Vulkan inicial no Windows x64 e SDL3 após auditoria.

A `main` ainda contém egui/eframe, D3D12 e WASAPI/XAudio2 como baseline integrada. A nova stack está em fase de POC R1, ainda não integrada à `main`. A execução física Vulkan e a composição real Slint/WGPU ainda precisam ser comprovadas.

## Baseline integrada

- PR #743 integrada; worker SMS e comandos limitados fazem parte da baseline.
- Runtime SMS Rust com `SmsTemporalWorker`.
- MachineScheduler e componentes compartilhados preservados.
- Shell desktop atual egui/eframe.
- Host gráfico atual D3D12.
- Saída física atual WASAPI/XAudio2.
- PR #745 — ADR 0011 e reconciliação documental — MERGED em 2026-11-11 04:36 UTC.
- A baseline de runtime não foi alterada pela PR de higienização da CI.

## PRs abertas relevantes

| PR | Estado | Escopo e observações |
|---|---|---|
| [#750](https://github.com/leonardo201800478/sega-emulator/pull/750) | OPEN, Draft | Higienização CI, filtros de impacto, Actionlint, auditoria de workspaces e documentação. Sem alterações de hardware/runtime. HEAD atual deve ser reconsultado antes de merge. |
| [#747](https://github.com/leonardo201800478/sega-emulator/pull/747) | OPEN, Draft | POC R1 Slint/WGPU/Vulkan. O gate Clippy do desktop falhou no HEAD observado por conflito `forbid(unsafe_code)`/macro Slint; testes e build desktop ficaram SKIPPED. Vulkan físico: NOT RUN. |
| [#749](https://github.com/leonardo201800478/sega-emulator/pull/749) | OPEN, Draft, stacked sobre #747 | Documentação auxiliar de ownership/lockfile R1; não é PR independente contra main. |
| [#746](https://github.com/leonardo201800478/sega-emulator/pull/746) | OPEN, Draft | FrameMailbox portátil. Base antiga `392a5c6`, 19 commits atrás da main auditada. CI verde informado refere-se ao HEAD histórico, não comprova WGPU/Vulkan. |
| [#744](https://github.com/leonardo201800478/sega-emulator/pull/744) | OPEN, Draft | Fallback FPS egui. Base antiga `392a5c6`, 19 commits atrás da main auditada. |
| [#619](https://github.com/leonardo201800478/sega-emulator/pull/619) | OPEN | Auditoria documental SMS; base atual `82ccad8`. Revalidar gates e conteúdo antes de merge. |

### PRs abertas que exigem reconciliação

As PRs #645, #644, #643 e #639 ainda apontam para a base `f47f718`, aproximadamente 184 commits atrás da main auditada. A PR #533 aponta para `6ffd7c4`, aproximadamente 666 commits atrás.

Não fazer merge dessas PRs sem revisar o diff contra a main atual, reconciliar conflitos e reexecutar os gates. Não descartar branches ou commits automaticamente.

## Higienização da CI — PR #750

A PR #750 separa as responsabilidades da CI e estabelece:

- Regressão Rust do workspace quando workflows mudam.
- Gates focados para Z80, PSG, VDP, input e Scheduler.
- Slint/WGPU Windows x64 independente, condicionado à presença real do crate.
- Hosts egui/D3D12 e WASAPI/XAudio2 somente quando afetados ou por opt-in.
- CPAL e SDL3 sem gates fictícios antes da integração.
- Actionlint v1.7.12 com checksum verificado.
- Refresh de lockfiles como artefato revisável, sem commit/push automático.
- Auditoria dos workspaces raiz e independentes via `cargo metadata`.
- Host GUI `native/sms_debugger` explicitamente `NOT RUN`; `debugger_core` continua coberto pelo workspace Rust.

Inventário de integração encontrado: 87 arquivos — 30 Z80 CPU, 48 Z80/SMS, 4 SMS temporais/estado/interrupções, 3 Scheduler, 1 VDP e 1 SMS runtime API. Testes unitários internos não estão incluídos nessa contagem.

Na execução observada da PR #750:
- Actionlint: PASS.
- Rust workspace: fmt e Clippy PASS; testes em execução no snapshot observado.
- PowerShell tooling: PASS; auditoria registrou PASS=9, FAIL=0, BLOCKED=0, NOT RUN=5; lock metadata PASS=3.
- Higiene documental: PASS após correção de espaços finais.
- Gates não pertinentes: SKIPPED, não PASS.

Reconsultar o HEAD e os workflows da PR #750 antes de integrar; os resultados acima são vinculados aos SHAs individuais dos runs e não substituem o resultado final do HEAD.

## Classificação da migração

| Entrega | Estado |
|---|---|
| Decisão arquitetural | DOCUMENTADA na ADR 0011 |
| R1 — janela e composição Slint/WGPU | PENDENTE de validação final |
| Device/queue compartilhados em execução | NOT RUN |
| Vulkan real Windows x64 | NOT RUN |
| Textura externa atualizada e composição | NOT RUN |
| Frames reais SMS | NOT RUN |
| CPAL físico | NOT RUN |
| SDL3 na nova arquitetura | NOT RUN |
| Paridade/corte da baseline | NOT RUN |

CI de compilação não comprova execução física Vulkan. A PR #747 não deve ser declarada R1 concluída apenas por build.

## Próxima sequência

1. Concluir os gates da PR #750 no HEAD exato.
2. Revisar e integrar a higienização da CI na `main`.
3. Atualizar a PR #747 para consumir a CI higienizada, sem reintroduzir workflows antigos.
4. Corrigir o gate Clippy da POC Slint sem alterar o runtime.
5. Validar build Release e execução física Vulkan no Windows x64.
6. Somente após R1 aprovada, avançar para R2.

## Evidências

Classificar cada suíte PASS, FAIL, BLOCKED, NOT RUN ou SKIPPED. SKIPPED nunca é PASS. Registrar SHA, ambiente, comando, versões, backend efetivo e limitações. Builds de aplicação e avaliações de desempenho devem ser Release.
