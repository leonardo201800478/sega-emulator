# Auditoria DD/FD — Frente 3: temporização, barramento e integração

**Data:** 2026-11-03  
**Baseline:** `main` `e4604f3152c09e8d16c37f94f94a4b344db33b62`  
**Schema:** 25  
**Escopo:** ordem M1/non-M1, M-cycles/T-states, WAIT, requests e retomada.  
**Natureza:** inspeção estática do código e conferência com ZiLOG UM008011-0816; nenhum trace físico/externo foi executado nesta auditoria.

## 1. Resumo executivo

O projeto já possui três blocos relevantes, ainda não integrados em uma execução DD/FD completa:

1. `Z80BusHandshake`: request estável, identidade monotônica, WAIT acumulado e validação de conclusão.
2. `Z80M1Cycle`: modelo separado de T1/T2/WAIT/T3/T4, refresh e amostragem de opcode.
3. `Z80MicroSequencer`: requests de opcode/memória e ciclos internos para um subconjunto BASE/CB/ED.

A integração desses três blocos não pode ser presumida. O microsequenciador atual soma `base_tstates` e WAIT quando o request termina; não expõe por si só a forma elétrica completa de cada ciclo. O modelo M1 detalhado é um módulo separado e não está demonstrado como driver de todos os fetches do sequenciador.

## 2. Contratos e responsabilidades atuais

### Z80BusHandshake — `native/z80_cpu/src/micro_ops.rs`

- Uma única transação pendente.
- Request contém id, operação, endereço e duração base.
- Leitura exige dado na conclusão; escrita rejeita dado de retorno.
- WAIT estende a transação sem alterar identidade do request.
- Snapshot preserva next_request_id, request pendente e WAIT.
- O Scheduler continua sendo o proprietário do tempo.

### Z80M1Cycle — `native/z80_cpu/src/m1.rs`

- M1 expõe T1, T2, WAIT, T3, T4 e Complete.
- WAIT é amostrado no fim de T2 e em cada Tw.
- Opcode é amostrado ao fim de T3.
- R incrementa ao iniciar M1, preservando o bit 7.
- Refresh address é I:R e fica exposto durante T4 com RFSH/MREQ ativos.
- Estado M1 pode ser exportado/restaurado com validação de invariantes.

### Z80MicroSequencer — `native/z80_cpu/src/sequencer.rs`

- `OpcodeFetch` solicita 4 T-states base.
- Leitura/escrita de memória e I/O usam requests separados.
- `complete_bus_cycle` consome a resposta e soma duração base + WAIT.
- Há estados explícitos para ciclos internos em famílias já implementadas.
- Prefixos DD/FD ainda terminam em `UnsupportedOpcode`, exceto continuação DD/FD repetida ou ED.

## 3. Evidência de temporização do manual ZiLOG

ZiLOG UM008011-0816 descreve tempos nominais e sequências de M-cycles para as instruções indexadas. Valores confirmados nas páginas impressas:

| Instrução | M-cycles | T-states | Sequência de duração indicada no manual |
|---|---:|---:|---|
| LD IX,nn | 4 | 14 | 4,4,3,3 |
| LD r,(IX+d) | 5 | 19 | 4,4,3,5,3 |
| LD (IX+d),r | 5 | 19 | 4,4,3,5,3 |
| LD (IX+d),n | 5 | 19 | 4,4,3,5,3 |
| INC (IX+d) | 6 | 23 | 4,4,3,5,4,3 |
| ADD IX,pp | 4 | 15 | 4,4,4,3 |
| EX (SP),IX | 6 | 23 | 4,4,3,4,3,5 |
| RLC (IX+d) | 6 | 23 | 4,4,3,5,4,3 |
| BIT b,(IX+d) | 5 | 20 | 4,4,3,5,4 |

Esses valores são evidência documental de duração nominal. Não especificam, sozinhos, a ordem de cada pino em cada subfase, nem comprovam o comportamento do Scheduler.

## 4. Ordem DDCB/FDCB

A forma indexed-CB deve ser representada por fases distintas:

1. DD ou FD: M1.
2. CB: M1.
3. displacement: leitura de memória de opcode/operandos não-M1, conforme sequência do manual.
4. subopcode: leitura não-M1.
5. acesso ao endereço index+signed displacement.
6. BIT: leitura sem write-back.
7. Rotate/shift, RES, SET: read-modify-write.
8. Cópia para registrador quando o campo z não é 6, para as famílias que a especificam.

A implementação atual do executor atômico consome DD/FD, CB, displacement e subopcode na ordem lógica, mas realiza acessos diretos a `CpuBus`; não gera uma sequência de requests retomáveis. O microsequenciador atual rejeita CB depois de DD/FD e não consome displacement/subopcode.

**R e M1:** o sequenciador não pode incrementar R no displacement nem no subopcode DDCB/FDCB se esses ciclos forem não-M1. O teste precisa observar R em cada fronteira, e não somente seu valor final.

**BIT:** não deve produzir write request. O tempo nominal de 20T é consistente com a tabela ZiLOG; RMW de 23T deve ser validado por família e trace.

## 5. WAIT, suspensão e requests

O handshake genérico permite WAIT em qualquer request pendente. O modelo M1 separado especifica amostragem em T2/Tw. Não há evidência suficiente para assumir que o handshake genérico, sem acoplamento ao modelo de ciclo, reproduz a amostragem física de WAIT em todas as operações indexadas.

Testes necessários:
- Request mantém o mesmo ID durante WAIT.
- Conclusão com ID incorreto não consome o request.
- Snapshot/restore mantém ID, endereço, operação e WAIT.
- WAIT aplicado a cada M-cycle elegível deve alongar o ciclo sem alterar a ordem dos requests.
- Request seguinte só é emitido após o ponto de conclusão correto.
- Restore deve gerar exatamente o mesmo trace e a mesma resposta final.

## 6. Integração compartilhada

O Z80 é compartilhado; a implementação DD/FD deve permanecer em `sega-cpus` e emitir operações compatíveis com os contratos de barramento transacional existentes. A integração ao SMS deve conectar o Z80 compartilhado ao barramento temporal por `BusTransaction`/`BusOperationResult`, sem criar um decoder ou scheduler específico do console.

Não foi confirmada nesta auditoria a ligação ponta a ponta entre `Z80M1Cycle`, `Z80BusHandshake`, `MachineScheduler` e os contratos de barramento da máquina SMS. Portanto, não atribuir `TEMPORAL_PASS` ao modelo atual.

## 7. Status da Frente 3

| Componente | Status |
|---|---|
| Handshake com request ID estável | IMPLEMENTED |
| WAIT acumulado no handshake | IMPLEMENTED |
| M1 T1/T2/Tw/T3/T4 e refresh | IMPLEMENTED como módulo isolado |
| Trace temporal completo DD/FD | NOT_IMPLEMENTED |
| Ordem M1/non-M1 DDCB/FDCB no sequenciador | NOT_IMPLEMENTED |
| WAIT por fase física em todos os ciclos DD/FD | INTEGRATION_PENDING |
| Snapshot de transação indexada pendente | NOT_IMPLEMENTED |
| Integração Scheduler/SMS via contratos compartilhados | INTEGRATION_PENDING |
| TEMPORAL_PASS | NÃO CONCEDIDO |

## 8. Próximas ações

1. Especificar cada M-cycle de DD/FD a partir dos diagramas ZiLOG, separando duração, sinais e request lógico.
2. Ligar o driver M1 ao fetch do sequenciador ou justificar/documentar a fronteira equivalente.
3. Adicionar fases não-M1 explícitas para displacement e subopcode indexed-CB.
4. Instrumentar trace determinístico por T-state, com address/data, M1, MREQ/RD/WR/RFSH/WAIT e identidade da transação.
5. Validar snapshot/replay durante request pendente e WAIT.
6. Integrar ao Scheduler/SMS somente pelo contrato transacional compartilhado e medir o comportamento no SHA candidato.
