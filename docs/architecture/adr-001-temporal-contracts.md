# ADR-001: Contratos temporais compartilhados para CPU, barramento e Scheduler

- **Status:** PROPOSTO PARA REVISÃO — contratos absorvidos pela PR #418 (T5a); aprovação arquitetural independente pendente.
- **Data:** 2026-11-02
- **Baseline:** `62ab404e67fffbe8d0da7bf956e6b801a2e2bbd7`
- **Escopo:** contratos neutros para preparar a execução interrompível do Z80 e a integração temporal SMS1.
- **Não objetivo:** implementar microsequenciador, WAIT físico, arbitragem, perfil de clock SMS1 ou alterar semântica do hardware.

## Contexto

O Scheduler mantém `MachineTime` inteiro, domínios de clock racionais com fase residual e fila de eventos ordenada por timestamp, `HardwarePhase`, `PlatformPriority` e sequência estável. A fila rejeita eventos retroativos e regressões da chave causal. O runtime também passou a conter um microsequenciador Z80 retomável para um subconjunto de instruções; T5a (#418) mapeia suas requisições pendentes para os contratos definidos aqui, sem transferir a propriedade da continuação para o barramento ou Scheduler.

Esta ADR fixa o vocabulário e as invariantes compartilhadas. A integração física e a ampliação semântica permanecem em etapas posteriores.

## Decisões

### 1. Representação temporal

1. O tempo absoluto é `MachineTime`, um contador inteiro de ticks na unidade-mestre definida pelo perfil da máquina.
2. Nenhum `f32/f64`, nanossegundo de host ou relógio de parede participa do estado determinístico.
3. Conversões de domínios usam `ClockRatio` racional e preservam o resto/fase; não arredondar cada avanço isoladamente.
4. `ClockEdgeTime` representa uma borda local em posição racional exata quando ela não coincide com um tick inteiro. A fila de eventos atual aceita apenas `MachineTime`; `HardwarePhase` é apenas desempate ordinal, nunca substituto de timestamp sub-tick. Se o perfil SMS ou outro sistema exigir eventos sub-tick, a ampliação da fila será uma decisão explícita posterior, com testes de ordenação e snapshot.
5. O timestamp de uma transação é o instante lógico em que a transação começa. `duration_tstates` descreve sua extensão local; o instante de conclusão deve ser calculado pelo domínio de clock do dispositivo, não por soma em ponto flutuante.

### 2. Contrato de transação

A estrutura `BusTransaction` contém:
- timestamp absoluto;
- índice tipado e estável do M-cycle na continuação da CPU (\`MCycleIndex\`);
- índice tipado de T-state dentro do M-cycle (\`TStateIndex\`);
- fase lógica (`Setup`, `Sample`, `Wait`, `Hold`, `Complete`, `Internal`);
- tipo (`OpcodeFetch`, `MemoryRead/Write`, `IoRead/Write`, `InterruptAcknowledge`, `Refresh`, `Internal`);
- direção, endereço opcional, dado opcional e duração em T-states.

As fases são rótulos de protocolo e não descrevem bordas elétricas do Z80. O mapeamento de T-states para bordas/pinos depende da evidência e do perfil de hardware. Uma leitura não carrega dado antes da resposta; uma escrita exige dado; ciclo interno não carrega endereço/dado nem direção de leitura/escrita.

### 3. Resultado e retomada

`BusOperationResult<T, E>` separa:
- `Completed`: operação concluída, valor e timestamp de conclusão;
- `Wait`: operação não concluída, com timestamp futuro para retomada;
- `Suspended`: continuação preservada e identificada pelo proprietário;
- `Fault`: erro determinístico de protocolo/dispositivo.

WAIT deve apontar para timestamp estritamente posterior ao instante solicitado; uma conclusão não pode preceder a solicitação. Não se permite busy-wait. A implementação futura deve garantir que efeitos colaterais de leitura/escrita ocorram uma única vez e somente no ponto de conclusão definido pelo perfil.

### 4. Progresso de dispositivos

`DeviceProgress` informa até qual timestamp o dispositivo avançou e, opcionalmente, o próximo evento conhecido. O próximo evento não pode estar no passado em relação ao limite alcançado. A máquina consulta os dispositivos estaticamente/genericamente no hot path; não introduzir dispatch dinâmico por transação.

### 5. Causalidade e ordenação

A chave determinística do Scheduler é:

`(MachineTime, HardwarePhase, PlatformPriority, insertion_sequence)`

- Timestamp menor sempre precede timestamp maior.
- Fase de hardware é uma estratificação definida pelo perfil, não uma afirmação elétrica universal.
- Prioridade de plataforma é política determinística do software e não precedência física.
- Sequência estável desempata eventos equivalentes.
- Após dispatch, não se pode agendar evento anterior ao watermark causal; no timestamp corrente, não se pode regredir fase/prioridade.
- Um evento simultâneo cuja precedência física não esteja documentada permanece UNKNOWN; não atribuir ordem arbitrária e descrevê-la como hardware-accurate.

### 6. Avanço e chunk invariance

A máquina deve avançar somente até o menor limite entre alvo solicitado e próximo evento conhecido. Cada domínio preserva fase racional. Para qualquer intervalo representável, avançar em um bloco ou em blocos sucessivos deve produzir o mesmo contador de bordas, fase residual, estado de dispositivo e trace.

A Etapa 1 especifica esse contrato; os testes integrados CPU/Bus/VDP pertencem às etapas posteriores.

### 7. Reset e estado parcial

- Reset define timestamp inicial, contadores, fase e política de eventos conforme perfil explícito; não presumir que todos os domínios reiniciem na mesma fase sem evidência.
- Snapshot de CPU deve incluir continuação/micro-operação, índices M/T, transação pendente, dados latched e estado WAIT/Suspended.
- Snapshot de barramento deve incluir transação em voo e qualquer efeito pendente que ainda não foi confirmado.
- Snapshot do Scheduler inclui timestamp, domínios, fases, fila, sequência e watermark causal.
- Restore deve validar referências cruzadas e ser atômico: falha não pode deixar componentes parcialmente restaurados.
- Os tipos desta etapa não afirmam que os componentes já serializam esses estados; isso será implementado e validado nas etapas 2, 3 e 8.

## Interfaces de referência

- `MachineTime`, `ClockRatio`, `ClockDomain`, `ClockEdgeTime`: `sega-temporal-core`.
- `BusTransaction`, `BusOperationResult<T,E>`, `DeviceProgress`: módulo `contract` de `sega-temporal-core`.
- `HardwarePhase`, `PlatformPriority`, `ScheduledEvent<T>`: `sega-temporal-core`.
- `MachineScheduler<T>`: `sega-machine-scheduler`.

Os contratos desta ADR são implementados em `native/temporal_core` e consumidos pela PR #418, que é a origem canônica da implementação e do mapeamento T5a. A PR #396 é supersedida e não deve permanecer como uma segunda proposta de integração dos mesmos arquivos. O executor legado continua preservado; o caminho retomável atual é limitado ao subconjunto explicitamente testado.

## Critérios de aceitação da Etapa 1

- [x] Unidade temporal inteira/racional documentada; fase preservada.
- [x] Tipos de M-cycle/T-state, fase lógica, direção, endereço, dados, tipo e duração definidos.
- [x] Resultados Completed/Wait/Suspended/Fault definidos.
- [x] WAIT exige retomada futura; evento retroativo rejeitado.
- [x] Ordenação causal documentada e alinhada à fila existente.
- [x] Progresso/next-event de dispositivo definido.
- [x] Reset, snapshot e restore de estado parcial especificados.
- [ ] Revisão arquitetural e aprovação do contrato.
- [x] Código T5a, incluindo validação de endereço `(HL)`, codec com CRC32 válido e reconciliação `instruction_tstates`/Scheduler: SHA `0c178ed6c8da6aeddf8e25682a2ebc620eec786f`, run `41536695747`, quatro jobs PASS.
- [x] HEAD documental predecessor: SHA `646bd556572a1f077dc3c003407c6e64439f66f3`, run `41534269758`, quatro jobs PASS. Atualizações documentais posteriores são validadas no SHA final da PR, registrado no body. Execução local: NOT RUN.
- [x] PR #418 integrada em `main` pelo merge SHA `6c8e18b1c0a134ee56e8282d1cbc7660373100f8`; aprovação arquitetural independente permanece pendente (critério anterior).

**Estado:** IMPLEMENTED / QUICK_PASS para o código integrado pela PR #418 (HEAD `6e73c0655521941c9e4ab7384f4508954306561a`, CI da PR `41538464875`, 4/4 jobs PASS) / INTEGRATED em `main` no merge SHA `6c8e18b1c0a134ee56e8282d1cbc7660373100f8` / AUDITORIA TÉCNICA T5a APROVADA (comentário 6590621514; escopo limitado) / APPROVAL_PENDING para revisão arquitetural independente da ADR-001. O CI pós-merge `41540515497` teve Scheduler deterministic stress SKIPPED; esse job não é PASS pós-merge. Não declara `TEMPORAL_PASS`.
