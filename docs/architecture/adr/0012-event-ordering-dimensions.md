# ADR-0012 — separação das dimensões de ordenação temporal

- **Estado:** Proposto para revisão.
- **Data:** 2026-11-01.
- **Base:** PR #345, sem alterar `main`.
- **Dependências:** ADR-0010, ADR-0011 e scheduler temporal em evolução.

## Contexto

A fila anterior ordenava eventos por `(MachineTime, EventPriority, sequence)`. O campo único misturava duas perguntas diferentes: em qual estrato temporal do perfil o evento deve ser observado e qual política de despacho de software deve desempatar eventos nesse estrato.

O projeto também possui `ClockEdgeTime`, que representa matematicamente posições racionais de bordas locais, mas a fila continua aceitando somente `MachineTime` inteiro. A nova dimensão de hardware phase não deve mascarar essa lacuna.

## Decisão proposta

A chave passa a ser:

`(absolute_timestamp, hardware_phase, platform_priority, stable_sequence)`

1. **Absolute timestamp:** instante absoluto monotônico em `MachineTime`. É a primeira dimensão e não pode regredir.
2. **Hardware phase:** rótulo ordinal fornecido pelo perfil da máquina para distinguir estratos de observabilidade no mesmo timestamp. Não é duração, offset elétrico, número de T-state, fração de tick ou medição física. Valores menores são despachados primeiro, mas nenhum valor SMS é atribuído nesta ADR.
3. **Platform priority:** política de despacho do runtime dentro do mesmo timestamp e hardware phase. Não representa precedência elétrica nem substitui uma fase de hardware.
4. **Stable sequence:** contador global de inserção da fila, atribuído pelo próprio scheduler/queue. É o último desempate determinístico e não é configurável pelo dispositivo.

A fila rejeita inserção causal no mesmo timestamp quando o par `(hardware_phase, platform_priority)` retrocede em relação ao último evento despachado. A sequência não pode ser fornecida pelo produtor.

## Compatibilidade

- `EventPriority` permanece como alias de compatibilidade para `PlatformPriority`.
- `EventQueue::schedule` e `MachineScheduler::schedule_at` continuam disponíveis e usam `HardwarePhase(0)`.
- Novos produtores podem usar `schedule_with_phase` e `schedule_at_with_phase`.
- A compatibilidade de API não significa que fase zero seja a fase física correta para um dispositivo.

## Limites explícitos

- `HardwarePhase` nesta fatia é apenas um estrato ordinal. A representação de offsets sub-tick exige que `ClockEdgeTime` ou outra representação exata seja integrada à chave da fila em uma etapa própria.
- `ClockEdgeTime` ainda é uma consulta matemática não mutável; não avança clock domains e não agenda dispatch sub-tick.
- Nenhum valor, ordem, clock edge, fase de reset ou precedência CPU/VDP/PSG é definido para SMS1, SMS2, 315-5124, 315-5246 ou SG-1000.
- A prioridade do software deve ser definida separadamente por política do runtime e nunca usada para esconder uma ambiguidade física.
- O stable sequence resolve apenas eventos idênticos nas dimensões anteriores; não prova ordem elétrica.

## Evidência e entrada de perfil

Antes de um perfil preencher valores de hardware phase ou regras de prioridade:
- registrar fonte, revisão, página/seção/figura ou setup de medição;
- distinguir PRIMARY_DOC, MEASURED_HARDWARE, INDEPENDENT_TEST, EMULATOR_SOURCE, COMMUNITY_DOC, INFERENCE e UNKNOWN;
- preservar ancestralidade de oráculos e conflitos;
- propor teste reproduzível que confirme/falsifique a regra.

Enquanto não houver evidência, a regra permanece UNKNOWN e não deve ser codificada como fato. O manual Z80 UM008011 descreve sinais e ciclos do Z80, mas não determina sozinho a integração elétrica Z80/VDP/PSG do SMS.

## Validação exigida

- Ordenação lexicográfica independente das quatro dimensões.
- Estabilidade por sequência de inserção.
- Rejeição causal de regressão de fase e de prioridade separadamente.
- Compatibilidade das APIs antigas com fase zero.
- Determinismo após replay/snapshot (ainda pendente de snapshot serializável completo).
- CI Rust/Python no SHA candidato exato.

Esta ADR não declara `TEMPORAL_PASS`.
