# E7.4.1B — contrato de eventos T-state para o barramento temporal SMS

**Data:** 2026-11-04  
**Baseline:** `main` = `869b402e4c8f2dbd9c5f5fa4a10d10ac26506461`  
**Contrato-base:** PR #512, HEAD `19866f365b4873603a7dd230121a1127aeca9c2d`  
**Status:** decisão de arquitetura para orientar a implementação; NÃO é integração funcional.

## 1. Auditoria do comportamento existente

A implementação candidata da PR #512 já possui `Z80MCycle`, `Z80MCycleState` e `advance_tstate(wait_asserted)`. O contrato distingue ciclo nominal, WAIT mandatório, WAIT externo e borda final do M-cycle.

O SMS temporal ainda possui três mecanismos diferentes:

| Caminho | Mecanismo existente | Consequência |
|---|---|---|
| Memória/I/O | `SmsTemporalCoordinator::schedule_bus_completion` agenda um único `BusCompletion` após `base_tstates + waits` | Nenhuma borda intermediária é despachada |
| Ciclo interno | `schedule_internal_cycle` agenda um único `InternalCycleComplete` | A máquina hoje restringe a chamada a 1T; não representa intervalos internos multi-T como um fluxo de bordas |
| M1 | `schedule_m1_tstate` agenda um evento por T-state | Já existe progressão temporal explícita, mas em modelo próprio `SmsTemporalM1` |

O barramento aplica leitura, escrita e efeitos de I/O no despacho de `BusCompletion`. Esse commit final deve ser preservado. O estado do barramento e a restauração da máquina validam hoje exatamente um evento pendente para uma transação em andamento; essa suposição precisa mudar junto com o novo fluxo.

## 2. Decisão: uma borda agendada por vez

A integração deve manter o `MachineScheduler` como única autoridade de tempo e agendar **um único próximo T-state por vez** para a transação ativa. Não criar um segundo relógio, contador paralelo de tempo absoluto ou loop que avance ciclos fora do Scheduler.

O evento proposto para a transação de barramento é:

- `BusTStateComplete { request_id, elapsed_tstates }`

O campo `elapsed_tstates` identifica a ordinalidade da borda já despachada, iniciando em 1. A timestamp do evento deve ser exatamente a timestamp anterior acrescida de `master_ticks_per_tstate`. O evento não contém dados lidos nem efetua commit.

Após despachar a borda:
1. validar request ID, ordinalidade e timestamp contra o estado persistido;
2. avançar exatamente um passo de `Z80MCycle`;
3. persistir o novo `Z80MCycleState`;
4. se o ciclo ainda não terminou, agendar a próxima borda;
5. se a borda final foi atingida, aplicar exatamente uma vez a leitura/escrita/I/O e expor `SmsBusCompletion` latched para a CPU.

A resposta de barramento continua latched até `acknowledge_completion`. Falha ao completar a continuação da CPU não pode repetir o efeito de escrita nem reler dados depois do commit.

## 3. Mapeamento de operação

| `Z80BusOperation` | `Z80MCycleKind` |
|---|---|
| `OpcodeFetch` | `OpcodeFetch` — permanece no caminho M1 existente durante esta fatia |
| `MemoryRead` | `MemoryRead` |
| `MemoryWrite` | `MemoryWrite` |
| `PortRead` | `IoRead` |
| `PortWrite` | `IoWrite` |
| `InterruptAcknowledge` | `InterruptAcknowledge` |

O mapeamento não autoriza substituir M1 nesta fatia. M1 tem pins, refresh, T4 e regras de interrupção próprias; sua unificação só pode ocorrer após cobertura de equivalência e semântica dos pins.

## 4. WAIT: separar fixture, wait mandatório e sinal externo

A API atual recebe `fixture_wait_tstates: u32`, isto é, um total de waits determinísticos para testes. Isso **não** é uma linha elétrica WAIT e não comprova amostragem física.

A implementação de integração deve manter separados:
- waits obrigatórios da operação (por exemplo, o wait automático do acknowledge já modelado);
- waits fornecidos pela fixture determinística;
- eventual nível WAIT externo, que só deve ser conectado quando houver uma origem temporal/elétrica explícita.

Para preservar os testes atuais, a fixture pode dirigir uma sequência determinística de amostras WAIT, sem ser rotulada como sinal físico. Os waits obrigatórios não podem ser somados ao contador externo nem consumidos por ele. A sequência deve ser documentada e testada nos limites T2/TW, inclusive fixture zero, um e múltiplos waits.

**Ponto de revisão obrigatório:** antes de declarar a janela de amostragem eletricamente fiel, confrontar a semântica de `advance_tstate` com UM008011-0816 e os traces existentes. Não deslocar a amostragem por conveniência de implementação.

## 5. Estado serializável e invariantes

O `SmsTemporalBusState` deverá persistir, no mínimo:
- request completo e timestamp de início;
- `Z80MCycleState` ativo;
- número de waits fixture ainda não consumidos e sua política de amostragem;
- ordinal da próxima borda/evento esperado;
- conclusão latched, se a borda final já foi despachada.

O evento pendente deve concordar exatamente com o request ID, ordinal esperado e próxima timestamp. No estado entre bordas deve existir exatamente um evento de T-state. Depois da borda final, não deve restar evento da transação; a resposta deve estar latched. Não aceitar snapshots com progresso temporal maior que o Scheduler, ordinais repetidos, ciclo já completo com evento pendente, ciclo incompleto sem evento pendente, ou mismatch entre elapsed T-states e timestamps.

A mudança exige incremento explícito das versões de estado afetadas e testes de migração/rejeição conforme a política de compatibilidade do projeto. Não relaxar validação antiga para aceitar estados ambíguos.

## 6. Compatibilidade da API de execução

A API atual `dispatch_next()` é usada por `SmsTemporalMachine` para completar diretamente a continuação CPU. Uma borda intermediária não pode ser reportada como `BusCompletion` nem chamar `complete_bus_cycle`.

A integração deve separar:
- despacho de fronteira temporal (pode ser T-state intermediário);
- conclusão de transação de barramento (somente borda final);
- aceitação da conclusão pelo sequenciador.

O runner de instrução pode consumir internamente as bordas intermediárias, mas deve existir uma API de fronteira segura para testes e snapshots. Não introduzir callbacks por instrução na GUI/Python.

## 7. Ciclos internos e M1

- Ciclos internos expostos atualmente pelo sequenciador são intervalos de 1T. Devem continuar sendo eventos individuais do Scheduler nesta etapa. Não inventar acesso ao barramento.
- Intervalos internos futuros com duração maior precisam ser decompostos em T-states explicitamente, preservando a semântica da fase que os emitiu.
- M1 continua no modelo `SmsTemporalM1` nesta fatia. A equivalência com `Z80MCycle` deve ser um trabalho separado com testes de refresh, pins, WAIT, HALT T4 e interrupções.

## 8. Matriz mínima de aceitação da implementação

| Cenário | Critério |
|---|---|
| MemoryRead/MemoryWrite | Uma borda Scheduler por T-state; sem efeito antes da última borda |
| PortRead/PortWrite | Efeito de I/O somente no commit final |
| WAIT fixture = 0 | Exatamente duração nominal e nenhuma TW |
| WAIT fixture = 1 | Uma TW na janela definida e uma borda final posterior |
| WAIT fixture > 1 | TWs consecutivos, contador e timestamps monotônicos |
| Wait mandatório + fixture | Contadores independentes; duração total exata |
| Snapshot antes de T2 | Restore produz exatamente as mesmas bordas e resultado |
| Snapshot durante TW | Restore mantém ordinal, waits restantes e próxima timestamp |
| Snapshot após borda final, antes do ack CPU | Efeito aplicado uma vez; resposta latched e sem evento pendente |
| Execução contínua vs. blocos | Estado final, memória, I/O, CPU e Scheduler idênticos |
| Overflow/estado adulterado | Restore ou avanço rejeitado sem efeitos parciais |
| M1 e ciclos internos | Regressão preserva os caminhos existentes |

Todas as suítes devem ser classificadas individualmente PASS/FAIL/BLOCKED/NOT RUN. Testes locais não executados permanecem NOT RUN. CI precisa validar o HEAD exato.

## 9. Sequência de implementação

1. Introduzir evento tipado de borda de barramento e API de agendamento de exatamente um T-state.
2. Persistir `Z80MCycleState` e o ordinal do evento no `SmsTemporalBusState`; atualizar versão e validação de restore.
3. Fazer `SmsTemporalBus` despachar bordas intermediárias sem side effect e commit somente na borda final.
4. Adaptar `SmsTemporalMachine` para consumir bordas intermediárias sem chamar a CPU até a conclusão; expor safe point de fronteira.
5. Adaptar `instruction_timeline_is_valid` para cruzar o tempo decorrido com o progresso do M-cycle, sem inferir fronteiras por `mcycle_index`/`tstate_index`.
6. Adicionar testes de snapshot/restore, execução em blocos e equivalência; rodar gates completos.
7. Só então considerar unificação M1/ciclos internos e avançar para BUSREQ/BUSACK.

## 10. Não objetivos desta fatia

Não implementar BUSREQ/BUSACK, DMA, ownership externo, refresh DRAM, pins elétricos externos, novo scheduler ou alteração de clocks. Não marcar E7.4.1 como concluída apenas porque o evento intermediário existe. Não promover testes internos a diferencial independente ou validação temporal sistêmica.

## 11. Referências

- ZiLOG, Z80 CPU User Manual, UM008011-0816 (2016), referência técnica primária fornecida no projeto.
- PR #511 — auditoria E7.4/BUSREQ/BUSACK.
- PR #512 — contrato isolado `Z80MCycle`.
- SMS Power, Cartridge Slot: https://www.smspower.org/Development/CartridgeSlot
- SMS Power, SMS Cartridge: https://www.smspower.org/Development/SMSCartridge


## 12. Retificação obrigatória — fase elétrica de WAIT e TW automático (2026-11-04)

A revisão direta do ZiLOG UM008011-0816 (edição 0816) encontrou dois requisitos que precisam ser tratados como parte do contrato, não como detalhes de implementação.

### 12.1 Fase de amostragem

O manual especifica que a CPU amostra WAIT na **borda de descida do clock** durante T2 e em cada TW subsequente. A decisão de inserir o próximo TW é, portanto, tomada na fase de descida; a borda de subida seguinte completa a transição temporal correspondente.

O contrato atual de E7.4.1B expõe somente eventos de conclusão por T-state e Z80MCycle::advance_tstate(wait_asserted) recebe o nível WAIT no mesmo avanço que retorna uma borda descrita como subida. Isso não identifica nem valida a fase de descida. A fixture atual pode testar contagem determinística, mas não pode ser chamada de amostragem elétrica fiel.

**Decisão de modelagem:** separar explicitamente:
- fase de clock/borda de descida em que WAIT é amostrado;
- nível WAIT amostrado e decisão de manter/encerrar a janela de espera;
- borda de subida que conclui o T-state e avança o M-cycle;
- TW efetivamente inseridos, com origem obrigatória ou externa.

O Scheduler continua sendo a única autoridade temporal. Antes da implementação, deve-se provar que a resolução temporal de MachineTime representa a meia-fase do clock sem arredondamento. Se a escala atual não representar meia unidade de T-state (por exemplo, quando há número ímpar de master ticks por T-state), definir uma representação racional/quantum temporal apropriada no domínio compartilhado; não arredondar a amostragem para uma borda inteira nem criar um relógio paralelo no SMS.

### 12.2 I/O possui um TW automático

O UM008011-0816, seção Input or Output Cycles e Figura 7 (página impressa 10), especifica que cada ciclo de I/O contém um TW automaticamente inserido entre T2 e T3. Esse TW existe para permitir a decodificação do endereço e a ativação de WAIT pelo periférico. WAIT é amostrado durante esse estado e pode prolongar o ciclo.

No caminho atual, os requests PortRead/PortWrite são descritos como 4T, mas mandatory_wait_tstates() só reconhece InterruptAcknowledge. Assim, a implementação candidata representa I/O como quatro T-states nominais e não identifica o TW automático. A duração agregada de 4T não basta para demonstrar a posição correta do TW nem a janela de amostragem.

**Decisão de modelagem:** a representação do ciclo I/O deve conter três T-states nominais e um TW automático posicionado entre T2 e T3, além de TWs externos adicionais quando WAIT permanecer ativo nas bordas de amostragem aplicáveis. O contrato de request deve explicitar se base_tstates significa duração total mínima ou apenas T-states nominais; não inferir essa semântica pelo valor numérico 4. A migração deve preservar os tempos totais esperados e atualizar validadores/testes de fase.

### 12.3 Acknowledge de INT e waits automáticos

O UM008011-0816, seção Interrupt Request/Acknowledge Cycle e Figura 9 (página impressa 12), determina dois waits automáticos no ciclo especial de acknowledge. O estado de ciclo deve identificar esses TWs como fases temporais reais, sem somá-los ao contador de WAIT externo. A janela de amostragem WAIT deve seguir a regra geral de T2/TW do manual; não ignorar amostras durante TW obrigatório.

### 12.4 Consequências para o contrato e snapshots

O estado serializável deve preservar, no mínimo:
- fase atual do clock (incluindo se a próxima transição é amostragem em falling edge ou conclusão em rising edge);
- fase T/M atual e ordinal monotônico de transição;
- TW automático pendente/consumido e sua posição no ciclo;
- nível WAIT amostrado na borda correta, origem da amostra e decisão resultante;
- contagem separada de TW automáticos e TW externos;
- próxima timestamp/quantum do Scheduler e identidade do request.

Snapshot/restore deve ser exercitado imediatamente antes e depois da borda de amostragem, durante TW automático de I/O, durante cada TW do acknowledge INT, durante TW externo e entre falling/rising edges. Execução contínua e retomada devem produzir a mesma sequência ordenada de amostras, bordas, efeitos de barramento e timestamp final.

### 12.5 Critérios de aceite adicionais

| Caso | Critério |
|---|---|
| Memory read/write | WAIT amostrado no falling edge de T2; TW adicional decidido nessa amostra |
| M1 | Não alterar o caminho M1 nesta subetapa; abrir integração específica que valide o mesmo ponto de amostragem e refresh |
| I/O read/write | 3T nominais + 1 TW automático na posição T2–T3; WAIT amostrado durante o TW e extensão quando aplicável |
| INT acknowledge | Dois TW automáticos explicitamente posicionados; WAIT não ignorado nas amostras T2/TW |
| WAIT pulse fora da janela | Não inserir TW se o nível não estiver ativo na borda de amostragem definida |
| WAIT ativo na amostra | Inserir TW seguinte e repetir a amostragem em cada TW |
| Snapshot em falling/rising boundary | Restore preserva a fase exata e não duplica nem omite uma amostra |
| Resolução temporal ímpar | Nenhuma aproximação/arredondamento; representação temporal exata ou estado BLOCKED |

### 12.6 Fontes e classificação da evidência

- **Fonte primária:** ZiLOG, Z80 CPU User Manual, UM008011-0816, pp. 7–8 (WAIT amostrado na borda de descida durante T2 e cada TW), p. 10/Figura 7 (um TW automático por ciclo I/O), p. 12/Figura 9 (dois TW automáticos em INT acknowledge). PDF oficial: https://www.zilog.com/docs/z80/um0080.pdf
- **Engenharia reversa independente:** Floooh, Getting into way too much detail with the Z80 netlist simulation, que descreve a amostragem WAIT em meia-fase e a dependência da fase conforme o tipo de M-cycle: https://floooh.github.io/2021/12/06/z80-instruction-timing.html
- **Referência comparativa de emulador:** MAME Z80 (src/devices/cpu/z80/z80.cpp e z80.hxx), a ser inspecionado para comparação de estados/serviços, sem tratá-lo como autoridade superior ao manual: https://github.com/mamedev/mame/tree/master/src/devices/cpu/z80

**Status revisado:** o evento de uma borda por T-state continua sendo um avanço útil, mas a modelagem de WAIT/TW está **INCOMPLETE** até representar a fase de amostragem e os TWs automáticos de I/O/INT. E7.4.1B não pode ser concluída nem servir de base para BUSREQ/BUSACK antes de resolver e validar estes pontos.


---

## Aditivo de publicação documental — baseline `fcd91302cb64a31d63b75f5e1d3b6eab336668d1` (2026-11-04)

Este relatório é uma especificação/auditoria documental independente. Sua publicação não integra nem aprova o contrato executável da PR #512 ou os eventos T-state da PR #514.

- PR #512 permanece aberta e não integrada; seu contrato M-cycle isolado não satisfaz todos os requisitos WAIT/TW descritos neste documento.
- PR #514 permanece aberta e não integrada; os eventos de conclusão T-state não representam ainda a fase falling edge nem os TWs automáticos exigidos.
- WAIT/TW permanece **INCOMPLETE/BLOCKED** até implementar e testar amostragem na falling edge durante T2/TW, TW automático de I/O entre T2/T3, dois TWs automáticos no INT acknowledge e amostragem WAIT externo também durante TW obrigatório.
- A resolução temporal de meia-fase depende de C1: instante racional compartilhado no Temporal Core/Scheduler, com snapshots e codec exatos. Não arredondar nem usar `HardwarePhase` como sub-tick.
