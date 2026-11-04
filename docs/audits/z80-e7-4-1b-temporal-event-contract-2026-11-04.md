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
