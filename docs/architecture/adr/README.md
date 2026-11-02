# ADRs — reorganização do runtime

Decisões arquiteturais da reorganização Rust-first, do motor Z80 compartilhado, das fronteiras host e da infraestrutura temporal.

| ADR | Decisão | Estado |
|---|---|---|
| [ADR-0001 — motor Z80 compartilhado](0001-shared-z80-engine.md) | Um único crate `sega-z80-cpu`, reutilizando ALU e bus contract compartilhados; adaptadores por máquina. | Proposto para implementação |
| [ADR-0002 — scheduler temporal](0002-machine-temporal-scheduler.md) | Um scheduler por instância de máquina, múltiplos domínios de clock e eventos determinísticos. | Proposto para implementação |
| [ADR-0003 — fronteiras host/Rust](0003-host-runtime-boundaries.md) | Qt/Python como frontend/ferramentas; runtime de hardware, input, vídeo e áudio em Rust. | Proposto para implementação |
| [ADR-0012 — dimensões de ordenação temporal](0012-event-ordering-dimensions.md) | Separar timestamp absoluto, hardware phase, platform priority e stable sequence. | Proposto para revisão |
| [ADR-0013 — snapshot/restore determinístico](0013-deterministic-scheduler-snapshot-restore.md) | DTOs versionados, fila canônica, restauração atômica e equivalência de replay. | Proposto para revisão; implementação pendente |
| [ADR-0014 — execução e sincronização temporal de dispositivos](0014-temporal-device-execution-and-synchronization.md) | Contratos extensíveis para múltiplas CPUs, chips temporizados, safe points, domínios independentes e determinismo. | Proposto para revisão |

## Gate de implementação

As ADRs consolidam contratos e decisões; não significam que os componentes já estejam implementados ou validados. Código de runtime deve ser desenvolvido em branches próprias, com testes e workflows no SHA candidato exato.

Baseline de evidência: [F0 — inventário remoto e reconciliação das PRs](../migrations/f0-remote-inventory-2026-10-31.md).

A ADR-0013 registra o progresso temporal das PRs #343–#346, a auditoria independente da Etapa E e a sequência de implementação aprovada. A pilha #343–#346 ainda não está integrada à `main`; consulte o estado remoto antes de continuar.

A ADR-0014 define o contrato futuro de execução e sincronização de dispositivos. Ela não substitui as etapas E4–E7 da ADR-0013 nem autoriza integração temporal prematura de CPU, VDP, PSG ou FM.
