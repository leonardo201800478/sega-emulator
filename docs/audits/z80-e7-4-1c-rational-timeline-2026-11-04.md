# Auditoria E7.4.1C — linha temporal racional compartilhada e fases WAIT/TW

**Data:** 2026-11-04\
**Baseline documental atualizada:** `main` = `fcd91302cb64a31d63b75f5e1d3b6eab336668d1` (após PR #517)\
**Branch:** `docs/z80-e7-4-1c-rational-timeline-audit-20261104`\
**Escopo:** auditoria técnica e decisão arquitetural preparatória. Não altera runtime, não integra PRs candidatas e não declara E7.4.1 concluída.

## 1. Resultado executivo

A baseline possui uma primitiva de borda fracionária exata (`ClockEdgeTime`), mas ela ainda não é a unidade temporal do `MachineScheduler`. O Scheduler, a fila de eventos, os watermarks, os domínios de clock e seus DTOs/codec continuam usando `MachineTime` inteiro.

Isso impede representar, no mesmo eixo causal, eventos que ocorram em uma meia-fase de clock quando a unidade master não é divisível por dois. `HardwarePhase` não resolve essa lacuna: é apenas uma chave ordinal de desempate no mesmo timestamp.

**Decisão:** antes de corrigir WAIT/TW no Z80 ou integrar BUSREQ/BUSACK, E7.4.1C deve especificar e implementar uma linha temporal racional comum, usada pelo Scheduler e por todos os dispositivos. Não arredondar a falling edge, não codificar meia-fase como prioridade e não criar um segundo scheduler/relógio específico do SMS.

A implementação de código fica bloqueada nesta sessão porque o clone local falhou por indisponibilidade de resolução DNS de `github.com`. Não foi possível executar os gates locais. Esta PR é, portanto, documental e de modelagem; não reivindica validação de runtime.

## 2. Frente 1 — engenharia reversa e referências independentes

### 2.1 ZiLOG UM008011-0816

Fonte primária: [Z80 CPU User Manual, UM008011-0816](https://www.zilog.com/docs/z80/um0080.pdf).

- A seção Timing, página impressa 7, estabelece que WAIT é amostrado na borda descendente do clock durante T2 e em cada TW subsequente.
- A seção Input or Output Cycles, página impressa 10, determina um TW automático em cada ciclo I/O, entre T2 e T3; o pedido WAIT é amostrado durante esse TW.
- A Figura 9, página impressa 13, mostra dois TWs automáticos no interrupt acknowledge.
- A mesma seção explica que o acknowledge é um M1 especial com IORQ, e não um ciclo I/O comum.

Consequência: WAIT externo não pode ser ignorado enquanto um TW obrigatório estiver em andamento. As origens dos waits precisam permanecer distinguíveis para que a política de TW automático e a amostragem externa sejam reproduzíveis.

### 2.2 Simulação de netlist Z80 — Floooh

Referência: [Getting into way too much detail with the Z80 netlist simulation](https://floooh.github.io/2021/12/06/z80-instruction-timing.html), seção “Wait states”.

- A simulação registra sinais em meia-fase e identifica o ponto de amostragem de WAIT separadamente por classe de M-cycle.
- Para fetch e memória, apresenta a amostragem na segunda metade de T2.
- Para I/O, apresenta a amostragem na segunda metade do estado que nomeia T3, após o TW automático mostrado pelo manual.

A diferença de rótulo não deve ser tratada como conflito físico sem alinhar os diagramas: o manual nomeia explicitamente o TW automático entre T2 e T3; a simulação descreve o ponto pela sequência de half-cycles e usa nomenclatura de estado própria. O contrato do emulador deve registrar borda/fase e sinais, não depender apenas do rótulo textual T3/TW.

Limite da referência: o autor alerta que a netlist pode refletir um derivado Z8400 e ter diferenças em comportamentos não documentados. É uma fonte independente útil para traces, mas não substitui o manual nem uma captura do Z80 clássico.

### 2.3 ares e MAME

- ares, revisão fixada no histórico do projeto: [d400eee9764f](https://github.com/ares-emulator/ares/commit/d400eee9764f). Os caminhos Z80 consultados são úteis para decomposição nominal de operações e comparação de implementação. A interface de execução por instrução não constitui prova independente de amostragem WAIT em meia-fase.
- MAME, revisão fixada no histórico do projeto: [629c58163f0cf6be71bd42c37c92f04c8426aa60](https://github.com/mamedev/mame/commit/629c58163f0cf6be71bd42c37c92f04c8426aa60). A descrição de operações e os estados BUSREQ/BUSACK são referência comparativa; a análise disponível não comprova por si só a forma de onda WAIT em meia-fase.

Nenhum dos dois deve prevalecer sobre ZiLOG para os requisitos documentados de WAIT/TW.

## 3. Frente 2 — requisitos documentais consolidados

O contrato temporal deve separar pelo menos:

1. **Fase elétrica:** falling edge e rising edge como eventos distintos.
2. **Fase nominal:** M-cycle, T-state e subfase/ordinal de TW.
3. **Origem do wait:** `AutomaticIo`, `AutomaticIntAck` e `ExternalWait`, sem somar tudo em um contador indistinto.
4. **Amostra:** timestamp exato, fase elétrica, classe do M-cycle, ordinal T/TW, nível observado e resultado da amostragem.
5. **Commit de barramento:** leitura/escrita só no ponto elétrico definido para a operação; nenhuma borda intermediária pode duplicar efeitos.
6. **Continuação:** fase, próxima transição, request ID e todos os latches devem sobreviver a snapshot/restore.

Regras mínimas:
- Fetch M1 e memória: amostrar WAIT na falling edge de T2 e em cada TW subsequente.
- I/O: inserir o TW automático entre T2 e T3 e amostrar WAIT na falling edge desse TW; se WAIT continuar ativo, repetir TW completo.
- INT acknowledge: representar os dois TWs obrigatórios explicitamente e continuar amostrando WAIT nas fases aplicáveis, sem ignorar WAIT externo por haver wait mandatório.
- Os estados devem ser representados pela mesma timeline global do Scheduler, não por contadores locais que só se reconciliam no fim do M-cycle.

## 4. Frente 3 — auditoria arquitetural da baseline

### 4.1 `native/temporal_core/src/clock.rs`

- `MachineTime` é um inteiro `u128` de ticks master.
- `ClockRatio` e `ClockDomain` preservam resto racional para conversão de ciclos e garantem chunk invariance em avanços para timestamps inteiros.
- `ClockEdgeTime` já contém `whole_ticks`, numerador e denominador fracionários, com comparação exata por cross multiplication de u64 em u128.
- `ClockDomain::edge_time_after()` e `next_edge_time()` calculam bordas futuras sem mutar o domínio.
- `ClockEdgeTime` não tem integração com a fila de eventos; a primitiva é de preview, não a unidade de despacho.

### 4.2 `native/temporal_core/src/event.rs` e `state.rs`

- `ScheduledEvent.timestamp`, `EventQueueWatermark.timestamp` e `EventQueue::last_dispatched` são `MachineTime`.
- A ordenação causal é (timestamp inteiro, HardwarePhase, PlatformPriority, sequence).
- O comentário do código declara expressamente que a fila não aceita timestamps racionais e que HardwarePhase não os substitui.
- Export/restore valida ordenação e watermark apenas na granularidade inteira.

### 4.3 `native/machine_scheduler/src/lib.rs`, `state.rs` e `codec.rs`

- `MachineScheduler.now`, `ClockDomainState.current_time/registration_start`, `SchedulerState.now/last_dispatch_timestamp` usam `MachineTime`.
- O codec binário codifica cada timestamp como um `u128` inteiro de 16 bytes.
- Portanto, a migração afeta o relógio do Scheduler, preview/advance dos domínios, fila de eventos, watermarks, DTOs, codec e validação de snapshots. Não é uma alteração isolada em `SmsTemporalBus`.

### 4.4 SMS temporal

`SmsTemporalBus` ainda valida deadlines como diferenças inteiras múltiplas de `master_ticks_per_tstate`. Esse caminho não representa uma meia-fase quando o período do T-state não é divisível pela unidade master. A correção deve vir da timeline compartilhada; não deve ser implementada como exceção local SMS.

## 5. Decisão de modelagem C1 — instante comum

A próxima implementação deve avaliar explicitamente duas opções antes de congelar a API:

- **Opção A — promover `ClockEdgeTime`:** transformá-lo no instante canônico usado por eventos, Scheduler, domínios e snapshots, renomeando-o se necessário para `MachineInstant`.
- **Opção B — introduzir `MachineInstant` separado:** manter `MachineTime` como compatibilidade/valor integral e migrar todos os campos que representam instantes absolutos para `MachineInstant`.

A escolha deve evitar dois eixos temporais concorrentes. A API não pode oferecer uma conversão implícita que descarte a fração.

Requisitos de representação:
- Forma canônica: parte inteira + fração própria, denominador positivo e fração reduzida; timestamp integral com fração zero em forma única.
- Comparação total e exata, sem ponto flutuante.
- Soma/subtração e avanço de clock com overflow verificado; erro não pode deixar estado parcialmente mutado.
- Não assumir que multiplicar denominadores cabe em u64/u128. A implementação precisa definir limites e algoritmo de comparação/aritmética que não introduza overflow silencioso (por exemplo, redução por gcd antes de multiplicar e comparação racional sem produto intermediário não representável).
- Preservar ordenação estável por timestamp, HardwarePhase, PlatformPriority e sequence.
- Eventos com a mesma parte inteira, mas frações distintas, precisam ser despachados na ordem racional correta.
- Watermark causal e validação de eventos passados devem operar na mesma precisão do timestamp.
- Snapshot/restore e codec binário devem preservar exatamente a fração e rejeitar estados não canônicos.
- Migração de formato exige version bump explícito e testes de compatibilidade/rejeição; não reinterpretar bytes antigos como timestamp racional.

**Não congelar ainda** a largura final dos campos nem uma API pública antes de provar limites de denominador para todos os perfis de clock previstos. O fato de `ClockEdgeTime` comparar duas frações u64 com u128 não prova que soma, subtração ou composição de frações arbitrárias seja segura.

## 6. Critérios de aceitação C1

- [ ] Testes de normalização: frações equivalentes resultam em uma forma canônica.
- [ ] Testes de ordenação: inteiros, frações menores/maiores, igualdade racional e valores próximos ao limite.
- [ ] Testes de soma/subtração: carry/borrow, denominadores coprimos, overflow e atomicidade em erro.
- [ ] Testes com clocks 7/3, 3/2 e razões que gerem múltiplas bordas dentro de um tick master.
- [ ] Eventos distintos dentro do mesmo tick inteiro são ordenados corretamente.
- [ ] Watermark impede regressão racional mesmo quando a parte inteira é igual.
- [ ] Clock-domain preview/advance conserva chunk invariance em instantes racionais.
- [ ] Snapshot/restore do Scheduler preserva instante atual, domínios, fila, watermark e sequência.
- [ ] Codec binário round-trip e rejeita versão antiga/incompatível e frações não canônicas.
- [ ] Teste diferencial de execução contínua versus blocos produz o mesmo trace e estado final.
- [ ] Testes de regressão para todos os consumidores atuais de `MachineTime`; nenhuma conversão truncada.
- [ ] Rust fmt, Clippy com `-D warnings` e workspace tests PASS; Scheduler deterministic stress PASS.

C1 é pré-requisito para C2 (fases elétricas) e C3 (política TW) no runtime integrado. BUSREQ/BUSACK permanece bloqueado até C1–C6 e E7.4.1 serem validadas.

## 7. Estado das PRs candidatas existentes

As três PRs continuam abertas e não integradas. Suas branches têm uma divergência de um commit em relação à main atual, pois foram criadas sobre `869b402e4c8f2dbd9c5f5fa4a10d10ac26506461`; não rebasear automaticamente nem mesclar bases sem revisar dependências e commits.

| PR | CI no HEAD candidato | Classificação adicional |
|---|---|---|
| [#512](https://github.com/leonardo201800478/sega-emulator/pull/512) | Workflow 41891089747: Rust PASS, Python PASS, Windows PASS, Scheduler stress PASS (4/4) | Implementa contrato M-cycle isolado, mas não corrige os requisitos WAIT/TW/falling-edge; não merge |
| [#513](https://github.com/leonardo201800478/sega-emulator/pull/513) | Workflow 41896880779: Rust PASS, Python PASS, Windows PASS, Scheduler stress SKIPPED | Especificação documental retificada; não implementa runtime |
| [#514](https://github.com/leonardo201800478/sega-emulator/pull/514) | Workflow 41896166347: Rust PASS, Python PASS, Windows PASS, Scheduler stress SKIPPED | Eventos T-state candidatos, mas sem modelo elétrico WAIT/TW completo; não merge |

Os workflows acima são evidência de CI convencional nos HEADs citados; não provam fidelidade temporal. Testes locais nesta sessão: NOT RUN. A main 5452ec9 não apresentou workflows de push associados na consulta feita; CI pós-merge da #511 fica NOT VERIFIED, não PASS.

## 8. Plano incremental após esta auditoria

1. C1: escolher e implementar o instante racional compartilhado no temporal-core/Scheduler, com codec e snapshots.
2. C2: modelar falling/rising edges e transições suspensíveis no contrato comum.
3. C3: representar TWs automáticos e WAIT externo por origem e amostra.
4. C4: integrar o contrato ao Z80 compartilhado e ao SMS temporal bus, sem duplicar CPU nem scheduler.
5. C5: snapshots completos entre subfases.
6. C6: regressão direcionada, execução contínua/blocos e traces independentes.
7. Somente após C1–C6: retomar E7.4.2 BUSREQ/BUSACK.
8. E7.5–E7.7 continuam obrigatórias; E8 permanece bloqueada até fechar E7.

**Conclusão:** a próxima tarefa de implementação é C1 no `sega-temporal-core` e `machine-scheduler`, não uma correção isolada de M-cycle nem BUSREQ. O código ainda não foi alterado nesta etapa porque os testes locais não puderam ser executados; a especificação agora torna explícito o escopo e os critérios para retomar a implementação sem comprometer a precisão temporal.


---

## Aditivo de reconciliação — publicação na baseline atual (2026-11-04)

- A main avançou da baseline histórica `5452ec9eb4307b997badff1858bfc60e7078a937` para `fcd91302cb64a31d63b75f5e1d3b6eab336668d1`; a diferença inclui a PR #516 (toolchain/workspaces) e a PR #517 (lockfiles independentes e correção Clippy).
- As PRs #512 e #514 continuam abertas e não integradas. Os resultados de CI citados acima são específicos aos respectivos HEADs e não provam correção temporal.
- A especificação WAIT/TW foi separada de sua branch empilhada para publicação documental independente; a implementação M-cycle da #512 não é incorporada por este documento.
- C1 continua sendo a próxima tarefa de código: instante racional canônico integrado ao Temporal Core e Scheduler, com comparação/aritmética exatas, watermarks, snapshots e codec versionado. C2–C6 permanecem dependentes de C1; BUSREQ/BUSACK não deve avançar antes disso.
- O estado operacional consolidado está no [scorecard Z80](../z80-completion-scorecard.md). E7 global permanece `INTEGRATION_PENDING`; E8 está bloqueada.
