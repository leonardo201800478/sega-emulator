# Status dos componentes — 2026-11-08

**Baseline:** `main` = `9731b37098e7937fa92fffc15007a7c79aac4e4c`.
**Regra:** descreve somente código integrado na main; PRs abertas, evidência manual e validação física são classificadas separadamente.

| Componente | Estado integrado na main | Pendências / evidência |
|---|---|---|
| Produto / toolchain | Rust-only; toolchain fixada em `rust-toolchain.toml` | Gates devem ser reexecutados no HEAD candidato |
| Aplicação egui/eframe | Shell nativo e integração SMS na main | UI é apresentação/comandos coarse-grained; validação Windows ainda parcial |
| Z80 compartilhado | Um núcleo Rust compartilhado | E7.7 IN_PROGRESS; E8 BLOCKED; consultar scorecard e matriz DD/FD |
| MachineScheduler | Núcleo e testes determinísticos integrados | Stress do Scheduler não comprova sincronismo físico do áudio nem TEMPORAL_PASS sistêmico |
| SMS core temporal | Base temporal Rust integrada | Precisão VDP ainda parcial; WAIT por fixture; Save States persistentes completos pendentes |
| SMS runtime worker | Worker Rust puro integrado | Lifecycle e áudio físico precisam de validação no mesmo SHA |
| Vídeo / Slint + WGPU | Frames SMS integrados ao desktop Slint/WGPU (R2) | Validação física Vulkan e campanha de frame pacing continuam exigindo captura Windows Release; ver plano R2 |
| Áudio / PSG | SN76489 e geração PCM no runtime Rust | Saída física CPAL 0.18.2 ainda não integrada (R3); não declarar áudio físico concluído |
| Input | Teclado e SDL3 integrados ao shell | Foco/consumo de teclado requer teste Windows; gamepad físico/hotplug NOT RUN |
| SG-1000 | Implementação preservada fora da trilha de expansão SMS atual | Regressão obrigatória em cada alteração compartilhada; não criar fallback duplicado |
| ZEXDOC / ZEXALL | NOT RUN | E8 bloqueada até fechamento E7 e harness Rust completo |
| Python / Qt / PyO3 / Maturin | Fora da arquitetura ativa | Não reintroduzir em runtime, tooling, CI ou distribuição |

## Prioridade operacional — áudio

Consultar [scorecard de confiabilidade de áudio](audio/audio-reliability-scorecard.md) e [auditoria consolidada](audits/audio-reliability-consolidated-2026-11-08.md).

- AUDIO-00 a AUDIO-05 são a trilha imediata.
- A investigação histórica da qualidade de áudio permanece arquivada; a nova integração física deve validar PCM, formatos, lifecycle, underruns e latência ponta a ponta no CPAL.
- A issue #640 permanece aberta; contagem de underrun isolada não prova causa raiz.
- Não ajustar clocks do PSG/Scheduler, pitch, velocidade, buffer ou prebuffer sem evidência reproduzível.
- Não considerar `consumed_frames` como frames reproduzidos nem `queue_duration_micros` como latência ponta a ponta.

## PRs e issues relevantes

| Referência | Estado em 2026-11-08 | Consequência |
|---|---|---|
| [PR #752](https://github.com/leonardo201800478/sega-emulator/pull/752) | MERGED | Integração de frames SMS no desktop Slint/WGPU (R2) |
| [PR #614](https://github.com/leonardo201800478/sega-emulator/pull/614) | MERGED | Áudio nativo e controles integrados; não é aceite físico |
| [PR #657](https://github.com/leonardo201800478/sega-emulator/pull/657) | CLOSED / NOT MERGED | Não atribuir a esta PR a integração do código atual |
| [PR #658](https://github.com/leonardo201800478/sega-emulator/pull/658) | MERGED | Descarte da cauda do resampler após invalidação de epoch |
| [PR #661](https://github.com/leonardo201800478/sega-emulator/pull/661) | MERGED | Amostragem SN76489 em posições fracionárias do clock de saída |
| [PR #666](https://github.com/leonardo201800478/sega-emulator/pull/666) | OPEN / CI SUCCESS | Correções de falha/telemetria; validação física pendente |
| [PR #643](https://github.com/leonardo201800478/sega-emulator/pull/643) | OPEN / mergeable=false | Base antiga; não integrada |
| [PR #644](https://github.com/leonardo201800478/sega-emulator/pull/644) | OPEN / mergeable=false | Base antiga; não integrada |
| [PR #645](https://github.com/leonardo201800478/sega-emulator/pull/645) | OPEN / mergeable=false | Base antiga; não integrada |
| R3 — CPAL | PENDING | Integração de saída física, dispositivos, formatos, lifecycle e underruns |
| [Issue #659](https://github.com/leonardo201800478/sega-emulator/issues/659) | OPEN | Distorção/descompasso em ambos os backends; causa não determinada |
| [Issue #641](https://github.com/leonardo201800478/sega-emulator/issues/641) | OPEN | Corrupção visual; fora da trilha de áudio |

## CI da baseline

Workflow [42484594809](https://github.com/leonardo201800478/sega-emulator/actions/runs/42484594809), SHA `9731b37098e7937fa92fffc15007a7c79aac4e4c`, conclusão geral SUCCESS.

- **PASS:** Rust quality and unit regression; Documentation patch hygiene; Change impact classification.
- **SKIPPED / NOT RUN:** registrar por SHA e impacto. Checks de compatibilidade de hosts anteriores podem aparecer como SKIPPED; não representam validação do caminho Slint/WGPU, CPAL ou SDL3.

Não interpretar o sucesso geral como validação das suítes ignoradas.

## CI da correção de áudio — PR #666

Workflow [42489225235](https://github.com/leonardo201800478/sega-emulator/actions/runs/42489225235), HEAD `29916f7ddb27a069b385a57e53d04760de5e01ed`, conclusão SUCCESS. PASS: workspace fmt/Clippy/testes; Linux audio-output; host de áudio transitório fmt/Clippy/testes; formatação e Clippy da aplicação Windows; documentação e classificação de impacto. SKIPPED: Scheduler stress, Z80 focused, SDL3, PowerShell e refresh manual de lockfiles. A CI não executa áudio em endpoint físico nem loopback.

## Evidência do Scheduler

A campanha longa anterior executou 320.000 operações e 4.000.000 dispatches em 16 seeds, com 2 testes PASS e 0 falhas, conforme [registro de 2026-11-07](evidence/scheduler-stress-long-2026-11-07.md). Essa evidência é restrita ao Scheduler e não comprova áudio físico.

## Classificação de maturidade

- **IMPLEMENTED:** código existe e está integrado.
- **TESTED:** testes direcionados executados no SHA indicado.
- **INTEGRATION_PENDING:** depende de integração com outro componente/host.
- **PHYSICAL_VALIDATION_PENDING:** depende de dispositivo ou GPU real.
- **NOT RUN / BLOCKED:** teste não executado ou execução impedida; nunca equivale a PASS.
