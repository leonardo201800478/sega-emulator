# ADRs — decisões arquiteturais

Este índice lista decisões vigentes e registros históricos. ADR aceita não significa que a implementação esteja concluída.

| ADR | Decisão | Estado |
|---|---|---|
| [ADR-0001 — motor Z80 compartilhado](0001-shared-z80-engine.md) | Um único núcleo Z80 reutilizável, com adaptação por plataforma. | Referência arquitetural; validar no código |
| [ADR-0002 — scheduler temporal](0002-machine-temporal-scheduler.md) | Coordenação de eventos e clocks por máquina composta. | Aplicar na granularidade necessária |
| [ADR-0003 — fronteiras host/Rust](0003-host-runtime-boundaries.md) | Runtime de hardware em Rust; Python/Qt como host de aplicação. | Diretriz vigente |
| [ADR-0011 — modelo cycle-accurate](0011-cycle-accurate-temporal-model.md) | Decisão histórica de precisão cycle-accurate universal. | Supersedida pela ADR-0016 |
| [ADR-0012 — dimensões de ordenação temporal](0012-event-ordering-dimensions.md) | Ordenação determinística de eventos. | Usar quando necessária ao Scheduler |
| [ADR-0013 — snapshot/restore determinístico](0013-deterministic-scheduler-snapshot-restore.md) | Snapshot, restauração e replay determinísticos. | Aplicar ao estado ativo da máquina |
| [ADR-0014 — execução e sincronização temporal](0014-temporal-device-execution-and-synchronization.md) | Proposta geral para dispositivos temporizados. | Não impõe granularidade universal |
| [ADR-0015 — contrato temporal SMS/Z80/barramento](0015-sms-z80-bus-temporal-contract.md) | Proposta e histórico da integração temporal SMS. | Referência histórica; aplicar somente o necessário |
| [ADR-0016 — fidelidade observável](0016-observable-fidelity-and-proportional-timing.md) | Fidelidade comportamental e temporal proporcional ao console, sem simulação elétrica integral. | **Vigente** |

## Documentação de referência

- [Escopo do projeto](../../project-scope.md)
- [Roadmap mestre](../../project-roadmap.md)
- [Estado dos componentes](../../component-status.md)
- [Política de fidelidade](0016-observable-fidelity-and-proportional-timing.md)
- [Scheduler — índice](../scheduler/README.md)
- [SMS VDP — estado funcional e limites conhecidos](../sms-vdp.md)
- [Perfis de hardware Z80](../../hardware/z80-silicon-platform-profiles.md)

ADRs e auditorias antigas preservam decisões e evidências históricas. Quando um documento antigo exigir precisão maior do que a necessária ao comportamento observável de um console, prevalece a ADR-0016. Uma proposta ou PR aberta não altera a política nem o estado integrado da main.
