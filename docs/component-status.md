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
| Vídeo / D3D12 | Runtime/viewport integrados | Validação física GPU e regressões visuais permanecem pendentes; issue #641 aberta |
| Áudio / PSG | SN76489 e pipeline de áudio Rust integrados; melhorias #615/#661 e lifecycle de epoch #658 integradas | **BLOCKED para aceite:** issues #640/#659 abertas; telemetria/semântica e falhas XAudio2 pendentes; validação física dos dois backends NOT RUN nesta baseline |
| Input | Teclado e SDL3 integrados ao shell | Foco/consumo de teclado requer teste Windows; gamepad físico/hotplug NOT RUN |
| SG-1000 | Implementação preservada fora da trilha de expansão SMS atual | Regressão obrigatória em cada alteração compartilhada; não criar fallback duplicado |
| ZEXDOC / ZEXALL | NOT RUN | E8 bloqueada até fechamento E7 e harness Rust completo |
| Python / Qt / PyO3 / Maturin | Fora da arquitetura ativa | Não reintroduzir em runtime, tooling, CI ou distribuição |

## Prioridade operacional — áudio

Consultar [scorecard de confiabilidade de áudio](audio/audio-reliability-scorecard.md) e [auditoria consolidada](audits/audio-reliability-consolidated-2026-11-08.md).

- AUDIO-00 a AUDIO-05 são a trilha imediata.
- A issue #659 demonstra sintomas semelhantes em WASAPI e XAudio2 mesmo sem underruns frequentes; investigar primeiro a cadeia compartilhada de geração PCM/resampler.
- A issue #640 permanece aberta; contagem de underrun isolada não prova causa raiz.
- Não ajustar clocks do PSG/Scheduler, pitch, velocidade, buffer ou prebuffer sem evidência reproduzível.
- Não considerar `consumed_frames` como frames reproduzidos nem `queue_duration_micros` como latência ponta a ponta.

## PRs e issues relevantes

| Referência | Estado em 2026-11-08 | Consequência |
|---|---|---|
| [PR #613](https://github.com/leonardo201800478/sega-emulator/pull/613) | MERGED | Runtime SMS e viewport D3D12 integrados |
| [PR #614](https://github.com/leonardo201800478/sega-emulator/pull/614) | MERGED | Áudio nativo e controles integrados; não é aceite físico |
| [PR #657](https://github.com/leonardo201800478/sega-emulator/pull/657) | CLOSED / NOT MERGED | Não atribuir a esta PR a integração do código atual |
| [PR #658](https://github.com/leonardo201800478/sega-emulator/pull/658) | MERGED | Descarte da cauda do resampler após invalidação de epoch |
| [PR #661](https://github.com/leonardo201800478/sega-emulator/pull/661) | MERGED | Amostragem SN76489 em posições fracionárias do clock de saída |
| [PR #643](https://github.com/leonardo201800478/sega-emulator/pull/643) | OPEN / mergeable=false | Base antiga; não integrada |
| [PR #644](https://github.com/leonardo201800478/sega-emulator/pull/644) | OPEN / mergeable=false | Base antiga; não integrada |
| [PR #645](https://github.com/leonardo201800478/sega-emulator/pull/645) | OPEN / mergeable=false | Base antiga; não integrada |
| [Issue #640](https://github.com/leonardo201800478/sega-emulator/issues/640) | OPEN | Underruns WASAPI; causa não confirmada |
| [Issue #659](https://github.com/leonardo201800478/sega-emulator/issues/659) | OPEN | Distorção/descompasso em ambos os backends; causa não determinada |
| [Issue #641](https://github.com/leonardo201800478/sega-emulator/issues/641) | OPEN | Corrupção visual; fora da trilha de áudio |

## CI da baseline

Workflow [42484594809](https://github.com/leonardo201800478/sega-emulator/actions/runs/42484594809), SHA `9731b37098e7937fa92fffc15007a7c79aac4e4c`, conclusão geral SUCCESS.

- **PASS:** Rust quality and unit regression; Documentation patch hygiene; Change impact classification.
- **SKIPPED / NOT RUN:** Rust workspace regression; Linux audio-output host validation; Windows WASAPI/XAudio2 validation; Windows x64 D3D12 compile; Windows SDL3 input; Scheduler deterministic stress; Z80 focused validation; PowerShell tooling; refresh manual de lockfiles.

Não interpretar o sucesso geral como validação das suítes ignoradas.

## Evidência do Scheduler

A campanha longa anterior executou 320.000 operações e 4.000.000 dispatches em 16 seeds, com 2 testes PASS e 0 falhas, conforme [registro de 2026-11-07](evidence/scheduler-stress-long-2026-11-07.md). Essa evidência é restrita ao Scheduler e não comprova áudio físico.

## Classificação de maturidade

- **IMPLEMENTED:** código existe e está integrado.
- **TESTED:** testes direcionados executados no SHA indicado.
- **INTEGRATION_PENDING:** depende de integração com outro componente/host.
- **PHYSICAL_VALIDATION_PENDING:** depende de dispositivo ou GPU real.
- **NOT RUN / BLOCKED:** teste não executado ou execução impedida; nunca equivale a PASS.
