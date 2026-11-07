# Status dos componentes — 2026-11-07

**Baseline:** `main` em `3ad1781f84b7824b5aa889a583ae6d4545987ec1`.  
**Regra:** esta tabela descreve código integrado na main. Funcionalidades em PRs são identificadas separadamente.

| Componente | Estado integrado na main | Pendências / evidência |
|---|---|---|
| Produto / toolchain | Rust-only; toolchain fixada em `rust-toolchain.toml` | Gates devem ser reexecutados no HEAD candidato |
| Aplicação egui/eframe | Shell nativo integrado | Integração vertical com SMS segue nas PRs #613/#614 |
| Z80 compartilhado | Um núcleo Rust compartilhado | E7.7 IN_PROGRESS; E8 BLOCKED; consultar scorecard e matriz DD/FD |
| MachineScheduler | Núcleo e testes determinísticos integrados | Stress longo PASS na evidência de 2026-11-07; não equivale a TEMPORAL_PASS do SMS |
| SMS core temporal | Base temporal Rust integrada | Precisão VDP ainda parcial; WAIT por fixture; Save States persistentes completos pendentes |
| SMS runtime worker | Worker Rust puro disponível | Ligação operacional ao shell está em PR #613 |
| Vídeo / D3D12 | Runtime/host Rust disponível | Viewport integrado ao shell está em PR #613; validação física GPU NOT RUN nesta fotografia |
| Áudio / PSG | Runtime SN76489 e pipeline Rust integrados; PR #615 melhorou filtro e reset de ruído | Backend físico WASAPI/XAudio2 no shell está em PR #614; endpoint real NOT RUN conforme descrição da PR |
| Input | Infraestrutura Rust/SDL3 disponível nas mudanças da aplicação | Ligação ao shell em PR #614; gamepad físico/hotplug NOT RUN |
| SG-1000 | Código legado/atômico fora do runtime ativo inicial SMS | Nova etapa P2; preservar evidências e regressões sem criar fallback |
| ZEXDOC / ZEXALL | NOT RUN | E8 bloqueada até fechamento E7 e harness Rust completo |
| Python / Qt / PyO3 / Maturin | Fora da arquitetura ativa; removidos do produto conforme ADR 0010 | Não reintroduzir em runtime, tooling, CI ou distribuição |

## PRs abertas relacionadas

| PR | Estado | Escopo | Observação |
|---|---|---|---|
| [#613](https://github.com/leonardo201800478/sega-emulator/pull/613) | OPEN | Runtime SMS + viewport D3D12 no shell | Não integrado; validação física e CI no HEAD final exigidas |
| [#614](https://github.com/leonardo201800478/sega-emulator/pull/614) | OPEN | WASAPI/XAudio2 + teclado/SDL3 | Empilhada sobre #613; validar em conjunto |
| [#616](https://github.com/leonardo201800478/sega-emulator/pull/616) | OPEN | Auditoria documental de boot/cartuchos | Não altera runtime |

## Evidência do Scheduler

A campanha longa executou 320.000 operações e 4.000.000 dispatches em 16 seeds, com 2 testes PASS e 0 falhas. Os arquivos de teste, manifesto e implementação do Scheduler foram comparados por blob SHA com a main e encontrados idênticos. Ver [registro completo](evidence/scheduler-stress-long-2026-11-07.md).

## Classificação de maturidade

- **IMPLEMENTED:** código existe e está integrado.
- **TESTED:** testes direcionados executados no SHA indicado.
- **INTEGRATION_PENDING:** depende de integração com outro componente/host.
- **PHYSICAL_VALIDATION_PENDING:** depende de dispositivo ou GPU real.
- **NOT RUN / BLOCKED:** teste não executado ou execução impedida; nunca equivale a PASS.

A presença de uma crate ou de um teste não comprova compatibilidade completa. O estado operacional consolidado está em [project-status-2026-11-07.md](project-status-2026-11-07.md).
