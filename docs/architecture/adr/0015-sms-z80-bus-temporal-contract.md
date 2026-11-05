# ADR-0015 — proposta histórica de integração temporal SMS/Z80/barramento

**Estado:** Referência histórica, não normativa. A proposta é limitada pela [ADR-0016](0016-observable-fidelity-and-proportional-timing.md).

- **Data original:** 2026-11-02.
- **Objetivo original:** documentar uma integração incremental entre Z80, barramento SMS e Scheduler.
- **Situação:** parte da infraestrutura e das transações foi desenvolvida em PRs posteriores; a proposta completa de micro-operações, fases e integração com um VDP inteiramente temporal não é requisito global.

## Decisão vigente

A ADR-0016 substitui qualquer interpretação desta ADR que exija:
- microsequenciamento por T-state de todas as instruções;
- fases sub-T-state universais;
- traces de todos os sinais/pinos;
- um novo pipeline VDP por dot/pixel;
- arbitragem, WAIT ou ownership externo sem consumidor real;
- TEMPORAL_PASS como gate universal de plataforma.

O barramento e o Scheduler devem representar apenas as transações, esperas e eventos necessários para reproduzir o comportamento observável do Master System.

## Evidência histórica preservada

Esta ADR foi escrita durante a proposta de integração Z80/SMS e registra:
- a distinção entre execução por instrução e transações de barramento;
- o uso de requests e respostas latched para efeitos exactly-once;
- a necessidade de preservar o estado da máquina quando uma transação real for suspensa;
- a integração T4 documentada na PR #416, com escopo restrito a fetch, memória, ciclos internos e WAIT de fixture.

Esses registros descrevem decisões e código de uma etapa anterior. Não provam que todos os ciclos ou sinais sejam necessários para os consoles atuais.

## Critério para retomar qualquer detalhe

Antes de reabrir uma etapa temporal, identificar o jogo, periférico, revisão ou interação que falha sem ela; demonstrar a dependência com documentação ou teste; escolher a menor granularidade suficiente; medir o custo de CPU, memória e manutenção; e validar a correção sem regressões.

Não retomar timeline racional, sub-fases, arbitragem ou traces completos apenas para satisfazer uma proposta antiga.

## Referências vigentes

- [ADR-0016 — fidelidade observável](0016-observable-fidelity-and-proportional-timing.md)
- [Roadmap mestre](../../project-roadmap.md)
- [Escopo do projeto](../../project-scope.md)
- [Estado dos componentes](../../component-status.md)
