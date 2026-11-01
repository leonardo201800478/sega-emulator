# ADR-0011 — precisão temporal cycle-accurate como requisito arquitetural

- **Estado:** Aceito para orientar novas implementações.
- **Data:** 2026-11-01.
- **Escopo:** SG-1000 e Master System; extensão multi-sistema preservada.
- **Dependências:** ADR-0001 (Z80), ADR-0002 (scheduler), ADR-0003 (fronteiras host/Rust), ADR-0010 (runtime Rust e perfis SMS).
- **Implementação:** parcial/pendente. Esta decisão não declara nenhum `TEMPORAL_PASS`.

## Contexto

O SMS VDP atual usa avanço acumulado em T-states e renderização por scanline. A auditoria de 2026-10-29/30 documenta que writes durante a linha não carregam fase horizontal até o renderer e podem ser observados somente no fechamento da scanline. O contrato temporal Z80 atual possui traces por T-state para um subconjunto de instruções ED, mas não integra scheduler, WAIT e todos os sinais/ciclos.

O Temporal Core atual oferece tempo monotônico, razões racionais, fase e fila determinística. A fundação do Machine Scheduler em PR #341 ainda avança domínios em blocos agregados e não oferece, por si só, bordas individuais e micro-operação de CPU. Portanto, não é base suficiente para alegar precisão ciclo a ciclo ou integrar diretamente o Z80 e o VDP sem uma revisão de contrato.

## Decisão

1. Cycle-accurate será requisito arquitetural de destino desde a fundação de novos componentes digitais.
2. A implementação será orientada a eventos, com bordas de clock representáveis individualmente e processamento agrupado somente quando a equivalência temporal puder ser demonstrada.
3. Um scheduler por instância de máquina será a única autoridade temporal. Não haverá scheduler autônomo por chip nem threads de execução de hardware por componente.
4. CPU Z80 será preparada para sequenciamento de micro-operações, T-states, M-cycles, transações de barramento, refresh, WAIT e interrupções.
5. Barramento será temporal: operações terão fase, instante, duração, conclusão e efeitos colaterais definidos pelo perfil.
6. O VDP SMS será construído/refatorado com estado temporal explícito (raster, fetch, sprites, pixel pipeline, latches, VRAM/CRAM e IRQ), preparado para sincronização intralinha e sem depender de renderização scanline como autoridade de hardware.
7. PSG/FM evoluirão em tempo virtual; a produção de áudio poderá ser agrupada sem perder a cronologia digital.
8. SG-1000 permanece baseline obrigatório de preservação. Seu perfil temporal e VDP TMS9918A serão validados separadamente; não presumir que o contrato SMS se transfere integralmente.
9. Runtime, CPU, barramento e chips permanecem em Rust. Python/Qt é controle/telemetria; host de áudio/vídeo/input não determina o tempo emulado.
10. Não serão adotados JIT, frame skip, paralelismo especulativo ou múltiplos schedulers para atingir o objetivo.

## Decisão de granularidade

Não se exige despachar um evento global para cada tick mestre ou pixel. O scheduler deve escolher o próximo evento/borda relevante anunciado pelos componentes. Cada dispositivo pode consumir intervalos internamente até o próximo ponto de observabilidade, desde que sincronize antes de qualquer acesso externo e não atravesse eventos de outros dispositivos.

A chave de ordenação deve conter timestamp, fase de hardware/plataforma, prioridade específica e sequência estável. Valores/ordens de fase não serão fixados sem evidência do sistema e da revisão. Eventos simultâneos CPU/VDP, fetch/write, IRQ/acknowledge e PSG/write precisam de regras explícitas e testes.

## Consequências

### Positivas
- Evita reescrita futura do VDP e da CPU para adicionar efeitos raster, contenção e IRQs subinstrução.
- Permite reproduzir a primeira divergência temporal.
- Mantém determinismo e arquitetura multi-sistema.
- Permite otimização por processamento em spans sem comprometer precisão, quando comprovada.

### Custos e riscos
- A modelagem, pesquisa de hardware e validação tornam-se mais caras.
- Dados temporais incompletos podem bloquear decisões de implementação.
- O scheduler exige uma semântica de borda/evento mais rigorosa que a API atual de avanço agregado.
- Save states precisam incluir eventos e pipelines em andamento.
- Código de emuladores de referência não substitui documentação ou medições físicas.

## Estratégia

A implementação seguirá os marcos em [modelo temporal cycle-accurate](../cycle-accurate-system-model.md):
T0 evidência/contratos; T1 primitivas temporais; T2 bus transactions; T3 Z80 micro-ops; T4 SMS VDP temporal; T5 integração CPU/VDP/bus; T6 PSG/FM; T7 máquina/replay/save states; T8 validação.

A arquitetura específica do VDP está em [SMS VDP — arquitetura temporal](../sms-vdp-cycle-accurate-design.md). As perguntas abertas e as fontes estão no [registro de evidências](../cycle-accuracy-evidence-register.md).

## Critérios para alegar precisão

Nenhum componente ou sistema será rotulado cycle-accurate apenas por:
- total agregado de T-states;
- execução de um evento por scanline;
- comparação visual de um frame;
- compatibilidade com um único emulador;
- ausência de regressões funcionais.

O gate TEMPORAL_PASS exige traces temporais integrados, comparação com fontes/oráculos de ancestralidade conhecida, primeira divergência reportada, determinismo, save/restore temporal e CI verde no SHA candidato exato.

## Revisão

Esta ADR deve ser revisada se medições de hardware demonstrarem que a unidade temporal escolhida, a ordenação de eventos ou a granularidade de integração não representam corretamente o hardware. Correções devem ser rastreáveis a evidência e não podem ser justificadas apenas por conveniência de implementação.
