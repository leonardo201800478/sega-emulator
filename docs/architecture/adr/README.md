# ADRs — reorganização do runtime

Decisões arquiteturais da reorganização Rust-first, do motor Z80 compartilhado, da fronteira host/Rust e da precisão temporal.

| ADR | Decisão | Estado |
|---|---|---|
| [ADR-0001 — motor Z80 compartilhado](0001-shared-z80-engine.md) | Um único crate `sega-z80-cpu`, reutilizando ALU e bus contract compartilhados; adaptadores por máquina. | Proposto para implementação |
| [ADR-0002 — scheduler temporal](0002-machine-temporal-scheduler.md) | Um scheduler por instância de máquina, múltiplos domínios de clock e eventos determinísticos. | Proposto para implementação |
| [ADR-0003 — fronteiras host/Rust](0003-host-runtime-boundaries.md) | Qt/Python como frontend/ferramentas; runtime de hardware, input, vídeo e áudio em Rust. | Proposto para implementação |
| [ADR-0011 — modelo temporal cycle-accurate](0011-cycle-accurate-temporal-model.md) | Precisão temporal como requisito arquitetural; scheduler orientado a eventos, CPU micro-op, bus transactions e VDP temporal. | Aceito; implementação incremental pendente |

## Documentos de arquitetura

- [Modelo temporal cycle-accurate](../cycle-accurate-system-model.md)
- [Arquitetura temporal do VDP SMS](../sms-vdp-cycle-accurate-design.md)
- [Registro de evidências e pesquisas pendentes](../cycle-accuracy-evidence-register.md)

## Gate de implementação

As ADRs consolidam contratos e decisões; não significam que componentes estejam implementados ou validados. Código de runtime deve ser desenvolvido em branches próprias, com testes e workflows no SHA candidato exato.

Baseline de evidência: [F0 — inventário remoto e reconciliação das PRs](../../migrations/f0-remote-inventory-2026-10-31.md).
