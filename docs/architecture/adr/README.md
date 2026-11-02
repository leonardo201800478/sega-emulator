# ADRs — reorganização do runtime

Decisões arquiteturais da reorganização Rust-first, do motor Z80 compartilhado, das fronteiras host e da infraestrutura temporal.

| ADR | Decisão | Estado |
|---|---|---|
| [ADR-0001 — motor Z80 compartilhado](0001-shared-z80-engine.md) | Um único crate `sega-z80-cpu`, reutilizando ALU e bus contract compartilhados; adaptadores por máquina. | Proposto para implementação |
| [ADR-0002 — scheduler temporal](0002-machine-temporal-scheduler.md) | Um scheduler por instância de máquina, múltiplos domínios de clock e eventos determinísticos. | Proposto para implementação |
| [ADR-0003 — fronteiras host/Rust](0003-host-runtime-boundaries.md) | Qt/Python como frontend/ferramentas; runtime de hardware, input, vídeo e áudio em Rust. | Proposto para implementação |
| [ADR-0011 — modelo temporal cycle-accurate](0011-cycle-accurate-temporal-model.md) | Precisão temporal como requisito arquitetural; scheduler orientado a eventos, CPU micro-op, bus transactions e VDP temporal. | Aceito como direção arquitetural; integração não concluída |
| [ADR-0012 — dimensões de ordenação temporal](0012-event-ordering-dimensions.md) | Separar timestamp absoluto, hardware phase, platform priority e stable sequence. | Proposto para revisão |
| [ADR-0013 — snapshot/restore determinístico](0013-deterministic-scheduler-snapshot-restore.md) | DTOs versionados, fila canônica, restauração atômica e equivalência de replay. | Implementação E1–E7 concluída na stack; revisão e integração pendentes |
| [ADR-0014 — execução e sincronização temporal de dispositivos](0014-temporal-device-execution-and-synchronization.md) | Contratos extensíveis para múltiplas CPUs, chips temporizados, safe points, domínios independentes e determinismo. | Proposto para revisão |
| [ADR-0015 — contrato temporal SMS/Z80/barramento](0015-sms-z80-bus-temporal-contract.md) | Safe points retomáveis, transações Z80/SMS e decisão de granularidade condicionada a evidências. | Proposta para revisão; runtime não autorizado |

## Documentação normativa do Scheduler\n\n- [Master Clock Scheduler — índice](../scheduler/README.md)\n- [Manual de arquitetura e integração](../scheduler/scheduler-integration-manual.md)\n- [Contrato de integração de componentes](../scheduler/component-integration-contract.md)\n- [Prompt permanente de integração](../../prompts/scheduler-component-integration.md)\n\n## Documentos de arquitetura temporal

- [Modelo temporal cycle-accurate do sistema](../cycle-accurate-system-model.md)
- [Arquitetura temporal do VDP SMS](../sms-vdp-cycle-accurate-design.md)
- [Registro de evidências de precisão temporal](../cycle-accuracy-evidence-register.md)
- [Auditoria de integração VRAM/CRAM](../../audits/sms-vram-access-rust-integration-audit-2026-11-01.md)
- [Caracterização T0 do ASIC SMS1 315-5124](../../audits/sms1-315-5124-vdp-t0-characterization-2026-11-02.md)
- [Matriz de validação T0.1 do SMS1 VDP](../../audits/sms1-315-5124-vdp-t01-validation-matrix-2026-11-02.md)
- [Modelo SMS1 315-5124 e processo de três frentes](../sms1-315-5124-vdp-model-and-three-track-process.md)
- [Registro de evidências temporais do SMS1](../sms1-vdp-temporal-evidence-register.md)

## Gate de implementação

As ADRs consolidam contratos e decisões; não significam que os componentes já estejam implementados ou validados. Código de runtime deve ser desenvolvido em branches próprias, com testes e workflows no SHA candidato exato.

Baseline de evidência: [F0 — inventário remoto e reconciliação das PRs](../migrations/f0-remote-inventory-2026-10-31.md).

A ADR-0013 registra o progresso temporal das PRs #343–#346, a auditoria independente da Etapa E e a sequência de implementação aprovada. A stack consolidada de Scheduler (#358) e a cobertura multi-domain (#359) foram integradas à `main` em 2026-11-02. A documentação normativa atual está em `docs/architecture/scheduler/`; o ledger `docs/evidence/adr-0013-snapshot-restore-validation.md` mantém os heads e workflows exatos. A integração com CPU, barramento e dispositivos continua separada e pendente.

A ADR-0014 define o contrato futuro de execução e sincronização de dispositivos. A cobertura multi-domain da PR #359 valida avanço/replay de domínios racionais em timestamps inteiros, mas não substitui a implementação E1–E7 da ADR-0013 nem autoriza integração temporal prematura de CPU, barramento, VDP, PSG ou FM.

A ADR-0011 estabelece a direção arquitetural de precisão temporal integrada. Os documentos SMS1 T0/T0.1 e o registro de evidências delimitam explicitamente os comportamentos ainda desconhecidos; eles não autorizam inferir offsets, fetch slots, WAIT, fases de IRQ ou prioridades simultâneas sem evidência específica.
