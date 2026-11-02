# Master Clock Scheduler — documentação normativa

**Status da documentação:** baseline arquitetural em consolidação.  
**Implementação auditada:** `native/temporal_core` e `native/machine_scheduler` na `main` em `eb0b951e8f78394b7f83d8750506a3678dab75bb`.  
**Regra:** esta documentação distingue contratos já implementados de requisitos de integração ainda não implementados.

O Scheduler é a autoridade de tempo virtual de uma instância de máquina. Ele coordena domínios de clock racionais e eventos determinísticos; não executa instruções, não sintetiza áudio, não renderiza pixels, não arbitra sinais elétricos e não chama callbacks de dispositivos.

## Documentos

1. [Manual de arquitetura e integração](scheduler-integration-manual.md) — visão normativa, responsabilidades e fluxo de integração.
2. [Contrato de componente temporal](component-integration-contract.md) — responsabilidades e interface conceitual.
3. [Regras de domínio de clock](clock-domain-rules.md) — unidade temporal, razões, fase e avanço.
4. [Ordenação e causalidade de eventos](event-ordering-rules.md) — chave canônica, causalidade e delta events.
5. [Execução determinística](deterministic-execution.md) — invariantes, chunk invariance, limites e não determinismo proibido.
6. [Snapshot, restore e codec](snapshot-restore.md) — persistência e replay.
7. [Requisitos de validação](validation-requirements.md) — testes e critérios de evidência.
8. Exemplos de integração:
   - [Z80 e barramento](integration-examples/z80-bus.md)
   - [VDP SMS](integration-examples/sms-vdp.md)
   - [SN76489](integration-examples/sn76489.md)
9. [Prompt operacional para agentes](../../prompts/scheduler-component-integration.md).

## Código-fonte de referência

| Responsabilidade | Local |
|---|---|
| Tempo, clocks racionais, borda racional e fila de eventos | `native/temporal_core/src/clock.rs`, `event.rs`, `state.rs` |
| Orquestração por máquina | `native/machine_scheduler/src/lib.rs` |
| DTOs versionados | `native/machine_scheduler/src/state.rs` |
| Codec binário | `native/machine_scheduler/src/codec.rs` |
| Replay e snapshot | `native/machine_scheduler/tests/snapshot_replay.rs`, `multi_domain_replay.rs`, `stress_replay.rs` |

## Estado real conhecido

- **Implementado:** `MachineTime` inteiro `u128`; `ClockRatio` racional; fase residual; `ClockEdgeTime` para cálculo exato de bordas; `EventQueue` com timestamp inteiro, fase ordinal, prioridade e sequência estável; registro ordenado de domínios; despacho sem callback; avanço ocioso limitado; orçamento delta; DTOs, restore validado, codec versionado e testes de replay.
- **Não implementado pelo Scheduler:** execução de CPUs; execução interna de VDP/PSG/FM; transações de barramento; WAIT e arbitragem; interrupções físicas; despacho de eventos em timestamp sub-tick; integração operacional do Z80/SMS/SG-1000.
- **Não alegar:** cycle-accurate integrado, sub-tick dispatch, bus arbitration ou `TEMPORAL_PASS` com base apenas nos testes do Scheduler.

A stack das PRs #358 e #359 foi integrada à `main`; a situação atual deve sempre ser reconfirmada no GitHub antes de qualquer mudança. A PR #396 contém contratos temporais adicionais em revisão e não deve ser presumida integrada.

## Hierarquia documental

Este conjunto define regras gerais. ADRs definem decisões e propostas específicas; código define o comportamento efetivamente executado; testes definem os cenários cobertos. Em conflito:
1. Não inferir comportamento físico a partir de prioridade de software.
2. Não declarar implementado o que existe apenas como contrato/proposta.
3. Registrar divergência entre documento, código e teste antes de alterar a implementação.
4. Uma mudança normativa deve atualizar este conjunto e, quando aplicável, a ADR correspondente.

## Vocabulário de status

- **IMPLEMENTED:** existe no código de runtime.
- **TESTED:** existe teste reproduzível para o comportamento indicado.
- **PROPOSED:** contrato/desenho documentado, sem runtime correspondente.
- **INTEGRATION_PENDING:** componente existe, mas não é controlado pelo Scheduler na máquina.
- **UNKNOWN:** falta evidência suficiente; não preencher com suposição.
