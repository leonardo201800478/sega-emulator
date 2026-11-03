# Matriz semântica DD/FD do Z80 — E1

**Data:** 2026-11-03  
**Baseline auditada:** `main` — `e4604f3152c09e8d16c37f94f94a4b344db33b62`  
**Schema do microsequenciador:** 25  
**Escopo:** decoder DD/FD, IX/IY, IXH/IXL/IYH/IYL, (IX/IY+d), DDCB/FDCB.  
**Natureza:** inventário do código ativo e das evidências disponíveis; não declara conclusão funcional nem temporal do Z80.

## 1. Legenda e regra de leitura

| Estado | Significado |
|---|---|
| `IMPLEMENTED_ATOMIC` | Há caminho de execução no executor atômico `Z80::step()`. |
| `RECOGNIZED_ONLY` | O microsequenciador reconhece o byte de prefixo e agenda outro fetch M1, mas ainda não executa a família indexada. |
| `NOT_IMPLEMENTED` | A família não tem execução no microsequenciador retomável; o decode termina em `UnsupportedOpcode`. |
| `FUNCTIONAL_UNVERIFIED` | Existe handler, mas os testes localizados não demonstram a semântica integral da família. |
| `DIFFERENTIAL_PARTIAL` | Há comparação/fixtures selecionadas; não cobre todo o espaço da família nem constitui oráculo independente de silício. |
| `TEMPORAL_UNVERIFIED` | Não há trace de barramento por T-state validado independentemente para a família. |
| `NOT_RUN` | A suíte indicada não foi executada nesta auditoria. |

**Importante:** a presença de um handler, um retorno sem erro, ou igualdade entre o executor atômico e outro caminho do mesmo projeto não é prova suficiente de correção. O executor atômico é referência de comparação, não oráculo independente.

## 2. Baseline E0 confirmada

| Item | Evidência |
|---|---|
| `main` | `e4604f3152c09e8d16c37f94f94a4b344db33b62` |
| Versão do snapshot | `Z80_MICRO_SEQUENCER_STATE_VERSION = 25` em `native/z80_cpu/src/lib.rs` |
| Decoder atômico | `native/z80_cpu/src/cpu.rs`, `execute_indexed()` e `execute_indexed_cb()` |
| Microsequenciador | `native/z80_cpu/src/sequencer.rs` |
| Teste de inventário BASE | `native/z80_cpu/tests/base_sequencer_coverage.rs` — não cobre semântica DD/FD |
| Classificação ampla do executor atômico | Testes unitários em `native/z80_cpu/src/cpu.rs`: `every_primary_and_prefixed_encoding_is_classified_without_fallback_errors` e `every_indexed_cb_encoding_is_classified_for_ix_and_iy` |
| Teste dirigido de IX/IY | `indexed_prefix_supports_ix_iy_displacements_and_indexed_cb` no módulo de testes de `cpu.rs` |
| Teste indexed-CB exaustivo existente | `native/sms_core/tests/z80_indexed_cb_exhaustive_rotate_shift.rs` |

A matriz antiga `docs/z80-opcode-matrix.md` descreve principalmente a implementação Python arquivada. Ela não deve ser tratada como inventário normativo do runtime Rust atual.

## 3. Prefixos e seleção do índice

| Sequência | Comportamento no executor atômico | Microsequenciador | Evidência / lacuna |
|---|---|---|---|
| `DD op` | Decoder indexado; substitui HL/H/L conforme família | Reconhece DD, mas rejeita opcode indexado comum | `execute_indexed`; `cb_ed_and_index_prefixes_issue_a_second_m1_fetch` |
| `FD op` | Decoder indexado para IY | Reconhece FD, mas rejeita opcode indexado comum | Mesmo caminho parametrizado pelo prefixo |
| `DD DD op`, `FD FD op` | Repetições contam 4T por prefixo extra | Fetches M1 repetidos; último prefixo fica no latch; opcode seguinte ainda rejeitado | `repeated_index_prefixes_are_counted_without_recursive_overflow` (atômico); `repeated_index_prefixes_count_each_m1_and_last_prefix_is_retained` (sequenciador) |
| `DD FD op`, `FD DD op` | Último prefixo seleciona IY/IX, respectivamente | Latch do último prefixo; execução indexada não implementada | O teste do sequenciador prova somente seleção/latch e rejeição subsequente |
| `DD/FD ED op` | Prefixo de índice ignorado; ED segue o decoder base | O estado de prefixo ED é reconhecido; execução ED pode seguir seu subset próprio | Exige teste de estado e custo total por instrução |
| `DD/FD CB d op` | Consome displacement e subopcode e executa indexed-CB | Rejeita CB após DD/FD; não consome displacement/subopcode | `dd_cb_stops_before_treating_displacement_as_an_m1_opcode` |
| `DD/FD op` não afetado | Fallback ao decoder base sem reler o opcode; custo adicional do prefixo preservado | Fallback ainda não está implementado de forma geral para DD/FD | Testar bytes consumidos, R, PC, custo e ausência de leitura duplicada |

Não existe limite artificial de prefixos repetidos no contrato arquitetural. Testes de stress para cadeias longas ainda precisam ser associados ao caminho retomável.

## 4. Matriz de famílias DD/FD

Os opcodes abaixo são o byte posterior ao prefixo DD ou FD. A coluna de T-states representa o valor nominal documentado para uma instrução com um prefixo, não uma prova de distribuição temporal no sequenciador.

| Opcode(s) após DD/FD | Família | Substituição / efeito | T-states ZiLOG | Executor atômico | Microsequenciador |
|---|---|---|---:|---|---|
| `21 nn nn` | LD IX/IY,nn | Carrega par de índice | 14 | IMPLEMENTED_ATOMIC; teste dirigido | NOT_IMPLEMENTED |
| `22 nn nn` | LD (nn),IX/IY | Escreve low/high em endereços consecutivos | 20 | IMPLEMENTED_ATOMIC; FUNCTIONAL_UNVERIFIED | NOT_IMPLEMENTED |
| `2A nn nn` | LD IX/IY,(nn) | Lê low/high em endereços consecutivos | 20 | IMPLEMENTED_ATOMIC; FUNCTIONAL_UNVERIFIED | NOT_IMPLEMENTED |
| `23`, `2B` | INC/DEC IX/IY | Incremento/decremento 16-bit; flags preservadas | 10 | IMPLEMENTED_ATOMIC; teste dirigido parcial | NOT_IMPLEMENTED |
| `09`, `19`, `29`, `39` | ADD IX/IY,BC/DE/IX|IY/SP | Soma 16-bit; flags S/Z/PV preservadas; H/C e X/Y conforme resultado | 15 | IMPLEMENTED_ATOMIC; FUNCTIONAL_UNVERIFIED | NOT_IMPLEMENTED |
| `E1`, `E5` | POP/PUSH IX/IY | Leitura/escrita da pilha | 14 / 15 | IMPLEMENTED_ATOMIC; FUNCTIONAL_UNVERIFIED | NOT_IMPLEMENTED |
| `E3` | EX (SP),IX/IY | Troca o par com duas posições da pilha | 23 | IMPLEMENTED_ATOMIC; FUNCTIONAL_UNVERIFIED | NOT_IMPLEMENTED |
| `E9` | JP (IX/IY) | PC recebe o par de índice | 8 | IMPLEMENTED_ATOMIC; FUNCTIONAL_UNVERIFIED | NOT_IMPLEMENTED |
| `F9` | LD SP,IX/IY | SP recebe o par de índice | 10 | IMPLEMENTED_ATOMIC; FUNCTIONAL_UNVERIFIED | NOT_IMPLEMENTED |
| `24`, `25`, `26` | INC/DEC/LD IXH/IYH,n | Operações na metade alta | 8 / 8 / 11 | IMPLEMENTED_ATOMIC; FUNCTIONAL_UNVERIFIED | NOT_IMPLEMENTED |
| `2C`, `2D`, `2E` | INC/DEC/LD IXL/IYL,n | Operações na metade baixa | 8 / 8 / 11 | IMPLEMENTED_ATOMIC; FUNCTIONAL_UNVERIFIED | NOT_IMPLEMENTED |
| `40`–`7F`, exceto `76` | LD r,r' | H/L são substituídos por IXH/IXL ou IYH/IYL quando aplicável | 8 | IMPLEMENTED_ATOMIC; teste dirigido parcial | NOT_IMPLEMENTED |
| `40`–`7F` com campo `r=6` ou `r'=6` | LD r,(IX/IY+d) / LD (IX/IY+d),r | Usa memória indexada; H/L do outro operando são H/L normais | 19 | IMPLEMENTED_ATOMIC; testes dirigidos parciais | NOT_IMPLEMENTED |
| `34`, `35` | INC/DEC (IX/IY+d) | Read-modify-write da memória indexada | 23 | IMPLEMENTED_ATOMIC; FUNCTIONAL_UNVERIFIED | NOT_IMPLEMENTED |
| `36` | LD (IX/IY+d),n | Lê displacement e imediato; escreve memória | 19 | IMPLEMENTED_ATOMIC; teste dirigido parcial | NOT_IMPLEMENTED |
| `80`–`BF`, fonte diferente de 6 | ALU A,IXH/IXL/IYH/IYL ou r | ALU com registrador/half-index | 8 | IMPLEMENTED_ATOMIC; FUNCTIONAL_UNVERIFIED | NOT_IMPLEMENTED |
| `80`–`BF`, fonte 6 | ALU A,(IX/IY+d) | Lê memória indexada e aplica ALU | 19 | IMPLEMENTED_ATOMIC; FUNCTIONAL_UNVERIFIED | NOT_IMPLEMENTED |
| Demais opcodes | Prefixo ignorado ou fallback da instrução base, conforme classificação Z80 | Sem substituição IX/IY; custo do prefixo deve ser mantido | Base + 4T por prefixo ignorado | Fallback implementado no executor atômico | NOT_IMPLEMENTED / gap de fallback |

**Exceção essencial H/L:** nas formas `LD H,(IX/IY+d)`, `LD L,(IX/IY+d)`, `LD (IX/IY+d),H` e `LD (IX/IY+d),L`, o registrador H/L é o par HL normal, não IXH/IXL ou IYH/IYL. O decoder atômico possui um caminho separado para os operandos de memória, mas a cobertura diferencial completa dessa exceção precisa ser demonstrada no microsequenciador.

## 5. Matriz DDCB/FDCB

Formato de bytes: `DD/FD CB d op`. O displacement é um inteiro de 8 bits em complemento de dois. O effective address é `IX/IY + sign_extend(d)`, com wrap de 16 bits.

| Campo x do subopcode | Família | Campo z | Efeito de memória | Efeito em registrador | Flags | Tempo nominal |
|---|---|---|---|---|---|---:|
| 0 | RLC, RRC, RL, RR, SLA, SRA, SLL, SRL | 6 | Read-modify-write | Nenhum | S/Z/X/Y/PV/C; H/N reset | 23T |
| 0 | Mesmas rotações/deslocamentos | 0–5, 7 | Read-modify-write | Copia resultado para B/C/D/E/H/L/A | Mesmo da linha anterior | 23T |
| 1 | BIT b,(IX/IY+d) | Todos | Somente leitura; nenhuma escrita de retorno | Nenhum | S/Z/H/PV/N/C e X/Y conforme variante/fonte | 20T |
| 2 | RES b,(IX/IY+d) | 6 | Read-modify-write | Nenhum | Flags preservadas | 23T |
| 2 | RES b,(IX/IY+d) | 0–5, 7 | Read-modify-write | Copia resultado para B/C/D/E/H/L/A | Flags preservadas | 23T |
| 3 | SET b,(IX/IY+d) | 6 | Read-modify-write | Nenhum | Flags preservadas | 23T |
| 3 | SET b,(IX/IY+d) | 0–5, 7 | Read-modify-write | Copia resultado para B/C/D/E/H/L/A | Flags preservadas | 23T |

O decoder atômico classifica os 256 subopcodes para IX e IY e tem teste exaustivo para as oito operações rotate/shift, todos os 256 valores de entrada e os dois estados de carry, com destino memória-only. Isso não equivale a teste semântico exaustivo dos 512 encodings completos, nem valida temporalidade.

**BIT é read-only:** o handler não deve emitir write-back. Rotação/deslocamento, RES e SET são operações read-modify-write. O campo z controla a cópia adicional para registrador nas operações x=0,2,3; z=6 não seleciona registrador adicional.

## 6. Estado arquitetural e snapshot — situação E1

| Estado / recurso | Executor atômico | Microsequenciador DD/FD |
|---|---|---|
| IX/IY | Campos arquiteturais existentes | Campos de estado existem, mas sem instruções indexadas |
| IXH/IXL/IYH/IYL | Acesso por helper de registrador indexado | Sem decode funcional |
| PC / SP / wraparound | Handlers existentes; cobertura por família precisa ser ampliada | Prefix fetch atualiza PC; instrução indexada não chega ao commit |
| Flags S/Z/H/PV/N/C e X/Y | Helpers e handlers existentes; não equivalem a validação independente | Sem efeitos DD/FD |
| WZ/MEMPTR | Atualizado em handlers selecionados; falta matriz por família | Sem atualização indexada |
| Q / previous-Q | Não comprovado por esta auditoria para todas as famílias | Não comprovado |
| R / refresh | Fetch de opcode no executor atômico; cobertura específica DD/FD incompleta | Testes existentes confirmam incremento por M1 de prefixos |
| INT/NMI/EI delay/HALT | Há lógica geral no executor atômico; interação com instruções indexadas ainda precisa de vetores | Sem interrupção integrada em todas as fases DD/FD |
| Snapshot schema | Não é save state integral do sistema | Schema 25; estado serializa prefix latch, mas não possui estado de execução indexada |

Não alterar o schema durante E1. O bump só será proposto quando E2–E7 exigirem latches adicionais e os safe points forem definidos.

## 7. Evidências e lacunas

| Evidência | O que demonstra | O que não demonstra |
|---|---|---|
| Testes de classificação de 256 opcodes com DD/FD | O executor atômico retorna sem erro para os casos gerados | Semântica correta, flags, bytes consumidos ou ciclos corretos |
| Teste de classificação dos 256 indexed-CB para IX/IY | Todos os subopcodes alcançam caminho sem erro | Ausência de escrita em BIT, resultado correto, cópia z, trace ou timing |
| Teste dirigido `indexed_prefix_supports_ix_iy_displacements_and_indexed_cb` | Uma sequência representativa de IX/IY, half-register, displacement, BIT e SET | Todas as variantes e condições de borda |
| Teste `z80_indexed_cb_exhaustive_rotate_shift.rs` | Rotate/shift indexed-CB memória-only exaustivo em valores/carry | BIT/RES/SET, cópia z e microsequenciador |
| Testes de prefixo no sequenciador | Reconhecimento DD/FD, M1, R e latch do último prefixo | Execução de instrução indexada e DDCB/FDCB |
| Igualdade com `Z80::step()` | Consistência entre caminhos do projeto | Independência do oráculo |

### Matriz de estado atual

| Família | Reconhecimento atômico | Funcional atômico | Diferencial externo | Snapshot indexado | Temporal |
|---|---|---|---|---|---|
| Operações 16-bit IX/IY | IMPLEMENTED | FUNCTIONAL_UNVERIFIED / testes selecionados | NOT_RUN nesta auditoria | NOT_IMPLEMENTED no sequenciador | TEMPORAL_UNVERIFIED |
| IXH/IXL/IYH/IYL | IMPLEMENTED | FUNCTIONAL_UNVERIFIED | NOT_RUN nesta auditoria | NOT_IMPLEMENTED no sequenciador | TEMPORAL_UNVERIFIED |
| LD indexado / displacement | IMPLEMENTED | FUNCTIONAL_UNVERIFIED / casos selecionados | NOT_RUN nesta auditoria | NOT_IMPLEMENTED no sequenciador | TEMPORAL_UNVERIFIED |
| ALU indexada | IMPLEMENTED | FUNCTIONAL_UNVERIFIED | NOT_RUN nesta auditoria | NOT_IMPLEMENTED no sequenciador | TEMPORAL_UNVERIFIED |
| DDCB/FDCB BIT | IMPLEMENTED | FUNCTIONAL_UNVERIFIED; teste de classificação | NOT_RUN nesta auditoria | NOT_IMPLEMENTED no sequenciador | TEMPORAL_UNVERIFIED |
| DDCB/FDCB RMW | IMPLEMENTED | DIFFERENTIAL_PARTIAL para rotate/shift memória-only; RES/SET com cobertura menor | NOT_RUN nesta auditoria | NOT_IMPLEMENTED no sequenciador | TEMPORAL_UNVERIFIED |
| Prefixos repetidos/alternados | IMPLEMENTED | Testes dirigidos de seleção do último prefixo | NOT_RUN nesta auditoria | Latch parcial no sequenciador | TEMPORAL_UNVERIFIED |
| Prefixo ignorado / ED após DD/FD | IMPLEMENTED | Testes dirigidos no executor legado/atômico; ampliar para todos os grupos | NOT_RUN nesta auditoria | Fallback geral pendente | TEMPORAL_UNVERIFIED |

## 8. Referências técnicas primárias

1. ZiLOG, **Z80 CPU User Manual**, UM008011-0816 (2016), arquivo do projeto `um0080.pdf`.
   - Indexed addressing e displacement assinado: seção Indexed Addressing, páginas impressas 36–37.
   - `LD r,(IX+d)`: página impressa 75, 5 M-cycles / 19 T-states.
   - `LD IX,nn`: página impressa 76, 4 M-cycles / 14 T-states.
   - `EX (SP),IX`: página impressa 128, 6 M-cycles / 23 T-states.
   - `INC (IX+d)`: página impressa 169, 6 M-cycles / 23 T-states.
   - `ADD IX,pp`: página impressa 195, 4 M-cycles / 15 T-states.
   - `RLC (IX+d)`: página impressa 217, 6 M-cycles / 23 T-states.
   - `BIT b,(IX+d)`: página impressa 218, 5 M-cycles / 20 T-states.
2. SingleStepTests/Z80, revisão fixada no projeto: `ebe1875d48f374bcfd4b505d8eb8ee751568b5f7`. Corpus derivado de implementação de emulador; usar como oráculo diferencial, não como captura de silício.
3. `raxoft/z80test`: candidato para validação adicional de flags/MEMPTR e instruções documentadas; integração e runner ainda precisam ser confirmados.
4. MAME, ares e Gearsystem: referências comparativas secundárias, não autoridade isolada.

Links:
- https://www.zilog.com/docs/z80/um0080.pdf
- https://github.com/SingleStepTests/z80
- https://github.com/raxoft/z80test
- https://www.smspower.org/

## 9. Decisões e próximos gates

- **E1 — documentação/inventário:** esta matriz classifica as famílias e separa evidência de dispatch, semântica, diferencial, snapshot e temporalidade.
- **E2 — FSM:** criar estado de prefixo efetivo e continuação indexada sem limite artificial de prefixos; preservar o caminho sem prefixo.
- **E3–E7 — execução:** implementar em micro-operações suspensíveis, usando `BusTransaction`/`BusOperationResult` compartilhados. Não chamar o executor atômico para executar a instrução indexada.
- **E8 — snapshot/interrupções:** serializar os novos latches e provar restore/replay em cada safe point.
- **E9 — validação/integracão:** diferencial externo, ZEXDOC/ZEXALL quando compatíveis, stress de Scheduler e integração SMS separada.

**Estado global:** DD/FD funcional existe no executor atômico, mas o microsequenciador retomável não executa instruções indexadas. Nenhuma família DD/FD recebe `TEMPORAL_PASS`. O Z80 e a integração SMS permanecem INTEGRATION_PENDING para este escopo.
