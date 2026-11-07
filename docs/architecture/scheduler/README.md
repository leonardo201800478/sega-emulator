# Master Clock Scheduler — documentação normativa

**Política de fidelidade:** [ADR-0016 — fidelidade observável](../../architecture/adr/0016-observable-fidelity-and-proportional-timing.md).  
**Implementação:** crates Rust em `native/temporal_core` e `native/machine_scheduler`.

O Scheduler coordena o tempo virtual e os eventos relevantes de uma instância de máquina. Não executa instruções, sintetiza áudio, renderiza pixels nem arbitra sinais elétricos por conta própria.

## Documentos mantidos

1. [Manual de arquitetura e integração](scheduler-integration-manual.md) — responsabilidades, execução e integração.
2. [Contrato de componente temporal](component-integration-contract.md) — ownership e interface conceitual.
3. [Regras de domínio de clock](clock-domain-rules.md) — relações de frequência e fase.
4. [Ordenação e causalidade de eventos](event-ordering-rules.md) — ordem determinística.
5. [Execução determinística](deterministic-execution.md) — monotonicidade e chunk invariance.
6. [Snapshot, restore e codec](snapshot-restore.md) — estado persistente e replay.
7. [Requisitos de validação](validation-requirements.md) — testes e evidências.
8. Exemplos de integração: [Z80/barramento](integration-examples/z80-bus.md), [VDP SMS](integration-examples/sms-vdp.md), [SN76489](integration-examples/sn76489.md).

## Evidência executada

- [Campanha longa de estresse — 2026-11-07](../../evidence/scheduler-stress-long-2026-11-07.md): 320.000 operações e 4.000.000 dispatches, 2 testes PASS. Os fontes relevantes foram comparados com a main.
- [Ledger geral de evidências](../../evidence/README.md).

A campanha comprova apenas os cenários do Scheduler executados; não comprova TEMPORAL_PASS da máquina SMS.

## Código-fonte

| Responsabilidade | Local |
|---|---|
| Tempo, clocks e fila de eventos | `native/temporal_core` |
| Scheduler da máquina | `native/machine_scheduler` |
| Adaptadores e estado de plataforma | crates de cada console |

**Regra de granularidade:** o Scheduler dá suporte à precisão temporal dos caminhos de gameplay que precisam de coordenação entre componentes. Ele não converte todo dispositivo em um componente temporal de granularidade fina. Operações sem interação intermediária observável podem permanecer atômicas e avançar em blocos, desde que preservem resultado, duração nominal e determinismo.

## Estado e limites

A infraestrutura do Scheduler é compartilhada e pode ser usada para coordenar múltiplos domínios. Sua existência não exige que cada chip seja atualizado a cada tick nem que todos os pinos sejam representados.

- A máquina deve sincronizar componentes antes de interações que possam observar ou alterar o estado de outro dispositivo.
- Um componente pode avançar em blocos até o próximo ponto de interação relevante, desde que preserve a ordem causal e o resultado funcional.
- Eventos devem representar ocorrências relevantes à máquina, não uma reprodução indiscriminada de cada pino, pixel ou fase elétrica.
- A unidade de tempo e a granularidade devem ser suficientes para a plataforma; não introduzir precisão sub-tick ou aritmética adicional sem dependência comprovada.
- O Scheduler não substitui a semântica dos dispositivos nem determina sozinho a precisão de CPU, VDP ou áudio.

A documentação histórica e propostas abertas não são critérios de integração. A ADR-0016 define quando uma camada temporal adicional é justificável.
