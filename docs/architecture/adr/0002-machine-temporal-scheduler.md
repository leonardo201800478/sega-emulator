# ADR-0002 — scheduler temporal único por máquina

- **Status:** Proposto para implementação
- **Data:** 2026-10-31
- **Escopo:** timeline determinística, domínios de clock, eventos e stepping temporal

## Contexto

A PR #315 introduz `sega-temporal-core` com `MachineTime`, conversão racional `ClockDomain` e `EventQueue<T>`. Os testes cobrem chunk invariance, fase fracionária, monotonicidade, overflow, ordenação e causalidade. O código não implementa ainda scheduler de máquina, micro-operações, WAIT, contenção, raster events ou integração PSG.

Os runtimes atuais ainda avançam dispositivos em fronteiras agregadas e possuem conversões/filas locais. A arquitetura precisa estabelecer ownership temporal antes de integrar VDP, PSG e CPUs.

## Decisão

1. Cada instância de máquina terá exatamente um `MachineScheduler` Rust, dono do relógio virtual e da ordem causal dos eventos.
2. O scheduler coordenará múltiplos domínios de clock de CPU, VDP, PSG/FM, DMA, barramento e demais dispositivos. Não haverá scheduler independente por dispositivo.
3. `MachineTime` será inteiro e monotônico, com unidade definida/documentada pelo perfil da máquina. Não existe uma unidade universal de frequência presumida.
4. `ClockRatio` e `ClockDomain` preservarão fase fracionária e conversão determinística sem ponto flutuante. Clock gating/pausa de domínio será representado por estado explícito de habilitação, sem fingir uma razão de clock física igual a zero.
5. `EventQueue` ordenará eventos por timestamp, prioridade definida pelo perfil da máquina e sequência estável de inserção. As prioridades e a política delta-cycle precisam ser documentadas por plataforma.
6. Eventos causais no mesmo timestamp não poderão retroagir para prioridade anterior já despachada. O scheduler deve definir quando agrupar eventos simultâneos e quando avançar o delta-cycle.
7. CPU e dispositivos deverão oferecer fronteiras de execução retomáveis. O scheduler nunca deve precisar chamar Python por instrução, T-state, pixel ou amostra.
8. O caminho de stepping por instrução permanecerá disponível como fachada de compatibilidade, drenando a mesma semântica/micro-operações usadas pelo modo temporal.
9. O relógio do host, frame pacing, VSync/VRR, timer Qt, worker de áudio e chegada de input físico não avançam diretamente o tempo emulado.

## T1 — escopo mínimo do scheduler headless

T1 deve ser independente de Qt, SDL, áudio e vídeo e demonstrar:

- relógio virtual monotônico e avanço até deadline;
- registro de domínios e preservação de fase;
- fila determinística de eventos com prioridades de plataforma;
- safe points de execução e política delta-cycle;
- tratamento explícito de overflow, eventos no passado e causalidade;
- replay determinístico de sequência fixa de eventos;
- snapshots suficientes para repetir o mesmo avanço;
- testes de chunk invariance e invariância em relação ao tamanho dos passos.

Não incluir ainda temporização analógica, waveforms elétricos completos, raster do VDP ou timing de chips sem evidência técnica.

## Integração progressiva

1. Validar o crate T0 da #315 e integrar sem alterar os runtimes atuais.
2. Implementar T1 headless em branch própria.
3. Conectar primeiro o stepping agregado da CPU e os domínios existentes, mantendo resultados anteriores.
4. Evoluir o Z80 para micro-operações e eventos de barramento por famílias, preservando um único decoder.
5. Integrar VDP/PSG e interrupções com prioridades e fases sustentadas por documentação/testes.
6. Medir divergências e localizar o primeiro evento temporal divergente; não aceitar apenas igualdade de total de ciclos.

## Critérios de aceitação

- [ ] Um scheduler por instância de máquina.
- [ ] Domínios de clock com fase preservada e snapshots reproduzíveis.
- [ ] Ordem estável para eventos no mesmo timestamp.
- [ ] Sem avanço retroativo ou overflow silencioso.
- [ ] Host pacing e apresentação não controlam o relógio lógico.
- [ ] Testes headless reproduzem traces idênticos em execuções repetidas.
- [ ] Testes demonstram chunk invariance em diferentes tamanhos de avanço.
- [ ] A integração não altera resultados SG-1000/SMS sem regressões e oráculos.
- [ ] `TEMPORAL_PASS` só é atribuído após validar a sequência de eventos/acessos no escopo declarado.

## Consequências

A arquitetura temporal pode avançar antes da integração de vídeo/input host porque T1 é headless. A precisão ciclo a ciclo completa continua sendo incremental e não é requisito para declarar a fundação T0/T1 implementada. Entretanto, o ownership temporal, T-states, acessos de barramento, waits, interrupções e ordem de eventos são parte da arquitetura desde o início.

## Referências internas

- [F0 — inventário remoto e reconciliação das PRs](../migrations/f0-remote-inventory-2026-10-31.md)
- [PR #315 — Temporal Core](https://github.com/leonardo201800478/sega-emulator/pull/315)
- [Plano do Master Scheduler](../native-master-scheduler.md)
- [Plano de reorganização Z80/runtime](../migrations/shared-z80-runtime-reorganization.md)
