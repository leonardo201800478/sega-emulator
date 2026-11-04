# Matriz semântica DD/FD do Z80 — estado após E5 candidata PR #482

**Data:** 2026-11-04  
**Baseline auditada:** `main` — `a0e58fd05c7f62c42f82b86d15d8532e06a9c59b`, após PR #476; implementação E5 candidata em PR #482.  
**E4C validada:** HEAD `8f04b44e6c6ba85920a7438b74c9876586d5736c`; CI [41747460761](https://github.com/leonardo201800478/sega-emulator/actions/runs/41747460761), 4/4 jobs PASS.  
**Schema do microsequenciador:** 28 (candidato E5)  
**Escopo:** decoder DD/FD, IX/IY, IXH/IXL/IYH/IYL, (IX/IY+d), DDCB/FDCB.  
**Natureza:** inventário do código ativo e das evidências disponíveis; não declara conclusão funcional nem temporal do Z80.  
**Meta global e scorecard:** consultar [Meta de conclusão do Z80 — cobertura, validação e critérios de aceite](z80-completion-scorecard.md) para percentuais de referência, fórmulas, níveis de evidência e critérios de conclusão.

## 1. Legenda e regra de leitura

| Estado | Significado |
|---|---|
| `IMPLEMENTED_ATOMIC` | Há caminho de execução no executor atômico `Z80::step()`. |
| `IMPLEMENTED_MICROSEQUENCED` | Há fases retomáveis no microsequenciador, com testes direcionados e snapshot/WAIT nos safe points cobertos; não equivale a conformidade formal ou temporal. |
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
| `main` na baseline E0 | `e4604f3152c09e8d16c37f94f94a4b344db33b62` |
| Main após PR #471 / E4A | `e9a3eff6c3733b4f11443954a647471c9d3908ff` |
| Candidato E4B / PR #472 | `245d42eb0337d41295329155de8e151feceb9022` |
| Schema do snapshot na baseline E0 | `Z80_MICRO_SEQUENCER_STATE_VERSION = 25` |
| Schema atual do candidato E4A | `Z80_MICRO_SEQUENCER_STATE_VERSION = 27` em `native/z80_cpu/src/lib.rs` |
| Decoder atômico | `native/z80_cpu/src/cpu.rs`, `execute_indexed()` e `execute_indexed_cb()` |
| Microsequenciador | `native/z80_cpu/src/sequencer.rs` |
| Teste de inventário BASE | `native/z80_cpu/tests/base_sequencer_coverage.rs` — não cobre semântica DD/FD |
| Classificação ampla do executor atômico | Testes unitários em `native/z80_cpu/src/cpu.rs`: `every_primary_and_prefixed_encoding_is_classified_without_fallback_errors` e `every_indexed_cb_encoding_is_classified_for_ix_and_iy` |
| Teste dirigido de IX/IY | `indexed_prefix_supports_ix_iy_displacements_and_indexed_cb` no módulo de testes de `cpu.rs` |
| Teste indexed-CB exaustivo existente | `native/sms_core/tests/z80_indexed_cb_exhaustive_rotate_shift.rs` |
| Novos vetores sintéticos E1 | `native/z80_cpu/tests/indexed_semantic_baseline.rs`: BIT sem write-back, RMW + cópia z, exceção H/L, half-register, displacement com wrap e prefixos alternados |

A matriz antiga `docs/z80-opcode-matrix.md` descreve principalmente a implementação Python arquivada. Ela não deve ser tratada como inventário normativo do runtime Rust atual.

## 3. Prefixos e seleção do índice

| Sequência | Comportamento no executor atômico | Microsequenciador | Evidência / lacuna |
|---|---|---|---|
| `DD op` | Decoder indexado; substitui HL/H/L conforme família | Executa E3A1/E3A2, E4A, E4B e E4C; DDCB/FDCB ainda pendentes | `indexed_16bit_e3a1.rs`, `indexed_absolute_transfer_e3a2a.rs`, `indexed_stack_e3a2b.rs`, `indexed_exchange_stack_e3a2c.rs` |
| `FD op` | Decoder indexado para IY | Executa o subconjunto E3A1/E3A2; rejeita displacement e famílias ainda não implementadas | Mesmas suítes E3A1/E3A2, cobrindo IX e IY |
| `DD DD op`, `FD FD op` | Repetições contam 4T por prefixo extra | M1 repetidos, last-prefix-wins e execução E3A1/E3A2/E4A/E4B/E4C; outras famílias ainda rejeitadas | Testes E3A1/E3A2 com prefixos repetidos/alternados e M-cycle acumulado |
| `DD FD op`, `FD DD op` | Último prefixo seleciona IY/IX, respectivamente | Last-prefix-wins executa E3A1/E3A2/E4A/E4B/E4C; demais famílias ainda pendentes | Testes E3A1/E3A2 de seleção IX/IY e prefixos alternados |
| `DD/FD ED op` | Prefixo de índice ignorado; ED segue o decoder base | O estado de prefixo ED é reconhecido; execução ED pode seguir seu subset próprio | Exige teste de estado e custo total por instrução |
| `DD/FD CB d op` | Consome displacement e subopcode e executa indexed-CB | Rejeita CB após DD/FD; não consome displacement/subopcode | `dd_cb_stops_before_treating_displacement_as_an_m1_opcode` |
| `DD/FD op` não afetado | Fallback ao decoder base sem reler o opcode; custo adicional do prefixo preservado | Fallback ainda não está implementado de forma geral para DD/FD | Testar bytes consumidos, R, PC, custo e ausência de leitura duplicada |

Não existe limite artificial de prefixos repetidos no contrato arquitetural. Testes de stress para cadeias longas ainda precisam ser associados ao caminho retomável.

## 4. Matriz de famílias DD/FD

Os opcodes abaixo são o byte posterior ao prefixo DD ou FD. A coluna de T-states representa o valor nominal documentado para uma instrução com um prefixo, não uma prova de distribuição temporal no sequenciador.

| Opcode(s) após DD/FD | Família | Bytes totais (1 prefixo) | Substituição / estado / flags | T-states ZiLOG / observação | Executor atômico | Microsequenciador |
|---|---|---:|---|---:|---|---|
| `21 nn nn` | LD IX/IY,nn | 4 | IX/IY recebe nn; flags preservadas; PC consome 2 operandos | 14 (4 M-cycles) | IMPLEMENTED_ATOMIC; teste dirigido | IMPLEMENTED_MICROSEQUENCED; 10 testes E3A1 |
| `22 nn nn` | LD (nn),IX/IY | 4 | Escreve low/high em nn e nn+1; WZ=nn+1; flags preservadas | 20 (6 M-cycles) | IMPLEMENTED_ATOMIC; testes dirigidos | IMPLEMENTED_MICROSEQUENCED; 5 testes E3A2a |
| `2A nn nn` | LD IX/IY,(nn) | 4 | Lê nn e nn+1; IX/IY recebe word; WZ=nn+1; flags preservadas | 20 (6 M-cycles) | IMPLEMENTED_ATOMIC; testes dirigidos | IMPLEMENTED_MICROSEQUENCED; 5 testes E3A2a |
| `23`, `2B` | INC/DEC IX/IY | 2 | Atualiza word; flags preservadas; wrap 16-bit | 10 (2 M-cycles) | IMPLEMENTED_ATOMIC; teste dirigido | IMPLEMENTED_MICROSEQUENCED; 10 testes E3A1 |
| `09`, `19`, `29`, `39` | ADD IX/IY,BC/DE/IX|IY/SP | 2 | IX/IY recebe soma; S/Z/PV preservadas; H/C atualizados; N reset; XY do high byte; WZ=index+1 | 15 (4 M-cycles) | IMPLEMENTED_ATOMIC; testes dirigidos | IMPLEMENTED_MICROSEQUENCED; 10 testes E3A1 |
| `E1`, `E5` | POP/PUSH IX/IY | 2 | POP lê low/high e incrementa SP após cada leitura; PUSH escreve high/low e decrementa SP antes de cada escrita; flags preservadas | 14 / 15 (4 M-cycles) | IMPLEMENTED_ATOMIC; testes dirigidos | IMPLEMENTED_MICROSEQUENCED; 4 testes E3A2b |
| `E3` | EX (SP),IX/IY | 2 | Lê low/high, escreve o par antigo low/high; SP preservado; WZ recebe word lida; flags preservadas | 23 (6 M-cycles) | IMPLEMENTED_ATOMIC; testes dirigidos | IMPLEMENTED_MICROSEQUENCED; 2 testes E3A2c |
| `E9` | JP (IX/IY) | 2 | PC recebe IX/IY; sem operandos adicionais; flags preservadas | 8 (2 M-cycles) | IMPLEMENTED_ATOMIC; testes dirigidos | IMPLEMENTED_MICROSEQUENCED; 10 testes E3A1 |
| `F9` | LD SP,IX/IY | 2 | SP recebe IX/IY; flags preservadas | 10 (2 M-cycles) | IMPLEMENTED_ATOMIC; testes dirigidos | IMPLEMENTED_MICROSEQUENCED; 10 testes E3A1 |
| `24`, `25`, `26` | INC/DEC/LD IXH/IYH,n | 2 / 2 / 3 | Atualiza metade alta; INC/DEC altera flags de 8-bit, LD preserva flags; half-register não documentado como operando independente no UM008011 | 8 / 8 / 11 (derivado do prefixo + operação BASE; trace pendente) | IMPLEMENTED_ATOMIC; FUNCTIONAL_UNVERIFIED | NOT_IMPLEMENTED |
| `2C`, `2D`, `2E` | INC/DEC/LD IXL/IYL,n | 2 / 2 / 3 | Atualiza metade baixa; INC/DEC altera flags de 8-bit, LD preserva flags; half-register não documentado como operando independente no UM008011 | 8 / 8 / 11 (derivado do prefixo + operação BASE; trace pendente) | IMPLEMENTED_ATOMIC; FUNCTIONAL_UNVERIFIED | NOT_IMPLEMENTED |
| `40`–`7F`, exceto `76` | LD r,r' | 2 | H/L substituídos por IXH/IXL ou IYH/IYL quando aplicável; flags preservadas; HALT `76` não é indexado | 8 (prefixo + M1 BASE; trace pendente) | IMPLEMENTED_ATOMIC; teste dirigido parcial | NOT_IMPLEMENTED |
| `40`–`7F` com campo `r=6` ou `r'=6` | LD r,(IX/IY+d) / LD (IX/IY+d),r | 3 | EA=index+sign_extend(d); memória lida/escrita; H/L do outro operando são H/L normais; flags preservadas | 19 (5 M-cycles: 4,4,3,5,3) | IMPLEMENTED_ATOMIC; testes dirigidos | IMPLEMENTED_MICROSEQUENCED; E4A, 10 testes, IX/IY e matriz de registradores |
| `34`, `35` | INC/DEC (IX/IY+d) | 3 | EA=index+sign_extend(d); read-modify-write; flags S/Z/H/PV/N atualizadas; C preservado | 23 (6 M-cycles: 4,4,3,5,4,3) | IMPLEMENTED_ATOMIC; FUNCTIONAL_UNVERIFIED | IMPLEMENTED_MICROSEQUENCED; E4C, 5 testes, WAIT/snapshot/prefixos |
| `36` | LD (IX/IY+d),n | 4 | Lê displacement e imediato; escreve EA; flags preservadas | 19 (5 M-cycles: 4,4,3,5,3) | IMPLEMENTED_ATOMIC; testes dirigidos | IMPLEMENTED_MICROSEQUENCED; E4A, WAIT e snapshots por fase |
| `80`–`BF`, fonte diferente de 6 | ALU A,IXH/IXL/IYH/IYL ou r | 2 | A/F atualizados conforme operação ALU; half-register não documentado como operando independente no UM008011 | 8 (prefixo + M1 BASE; trace pendente) | IMPLEMENTED_ATOMIC; FUNCTIONAL_UNVERIFIED | NOT_IMPLEMENTED |
| `80`–`BF`, fonte 6 | ALU A,(IX/IY+d) | 3 | EA=index+sign_extend(d); memória lida; A/F atualizados; CP preserva A e copia X/Y do operando | 19 (5 M-cycles: 4,4,3,5,3) | IMPLEMENTED_ATOMIC; FUNCTIONAL_UNVERIFIED | IMPLEMENTED_MICROSEQUENCED; E4B, 16 vetores x IX/IY, WAIT/snapshot/prefixos |
| Demais opcodes | Prefixo ignorado, prefixo repetido, ED ou fallback da instrução base | Prefixos extras + comprimento BASE | Sem substituição IX/IY; PC/R e custo do prefixo precisam ser mantidos; ED cancela indexação | BASE + 4T por prefixo ignorado | Fallback implementado no executor atômico | NOT_IMPLEMENTED / gap de fallback |

**Exceção essencial H/L:** nas formas `LD H,(IX/IY+d)`, `LD L,(IX/IY+d)`, `LD (IX/IY+d),H` e `LD (IX/IY+d),L`, o registrador H/L é o par HL normal, não IXH/IXL ou IYH/IYL. O decoder atômico e o microsequenciador possuem caminho de operandos de memória que usa H/L convencionais; a matriz E4A cobre load e store de H e L. A validação diferencial externa continua NOT_RUN.

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

**E5 — candidato PR #482:** o microsequenciador agora possui fases próprias para displacement, opcode final non-M1, leitura efetiva e escrita opcional. O trace nominal implementado é `4,4,3,5,4` para BIT (20T) e `4,4,3,5,4,3` para RMW (23T). A leitura do opcode final é uma transação `MemoryRead` de 5T, não `OpcodeFetch`; portanto não incrementa R. O operando é lido em 4T. O WAIT é contabilizado pela transação pendente em cada fase.

**BIT — limite documental:** UM008011-0816, páginas impressas 247–250, declara S e P/V desconhecidos para BIT indexado; C não é afetado e H é set. O candidato conserva o modelo atual do executor atômico para S/PV e os bits X/Y derivados do endereço, sem promover essa escolha a fato de hardware. A validação independente desses bits permanece aberta.

## 6. Estado arquitetural e snapshot — estado após E5 candidato PR #482

| Estado / recurso | Executor atômico | Microsequenciador DD/FD |
|---|---|---|
| IX/IY | Campos arquiteturais existentes | E3A1/E3A2, E4A, E4B, E4C e E5 candidato executam IX/IY, loads/stores, ALU, RMW e DDCB/FDCB via fases serializáveis; E5 usa as fases IndexedCbDisplacementRead, IndexedCbOpcodeRead, IndexedCbMemoryRead e IndexedCbMemoryWrite |
| IXH/IXL/IYH/IYL | Acesso por helper de registrador indexado | Sem decode funcional no microsequenciador; permanece fora de E3A1/E3A2 |
| PC / SP / wraparound | Handlers existentes; cobertura por família precisa ser ampliada | PC consome prefixos/opcodes; PUSH/POP/EX atualizam ou preservam SP nos pontos de barramento corretos; wrap testado |
| Flags S/Z/H/PV/N/C e X/Y | Helpers e handlers existentes; não equivalem a validação independente | E3A1/E3A2 preservam flags; E4B atualiza flags ALU; E4C atualiza S/Z/H/PV/N e preserva C via helper INC/DEC compartilhado |
| WZ/MEMPTR | Atualizado em handlers selecionados; falta matriz por família | E3A2: absolute transfers usam nn+1; E4A/E4B usam WZ=EA conforme o modelo MEMPTR do core; efeitos não documentados não são alegados como captura de silício |
| Q / previous-Q | Não comprovado por esta auditoria para todas as famílias | Não comprovado |
| R / refresh | Fetch de opcode no executor atômico; cobertura específica DD/FD incompleta | M1 de DD/FD e CB; o opcode final DDCB/FDCB é non-M1. A suíte E5 verifica que R conta somente os prefixos M1, incluindo cadeias repetidas/alternadas |
| INT/NMI/EI delay/HALT | Há lógica geral no executor atômico; interação com instruções indexadas ainda precisa de vetores | Sem interrupção integrada em todas as fases DD/FD |
| Snapshot schema | Não é save state integral do sistema | Schema 28 no candidato; FSM DD/FD e fases E3A1/E3A2/E4A/E4B/E4C/E5 validam/restauram estados intermediários com requests pendentes |

O schema 26 foi introduzido por E2; E4A elevou para 27 para serializar displacement e store imediato. E4B e E4C reutilizaram fases existentes. E5 adiciona fases DDCB/FDCB e eleva o schema para 28. A suíte candidata faz round-trip antes de cada request pendente; o codec não equivale a save state integral da máquina.

## 7. Evidências e lacunas

| Evidência | O que demonstra | O que não demonstra |
|---|---|---|
| Testes de classificação de 256 opcodes com DD/FD | O executor atômico retorna sem erro para os casos gerados | Semântica correta, flags, bytes consumidos ou ciclos corretos |
| Teste de classificação dos 256 indexed-CB para IX/IY | Todos os subopcodes alcançam caminho sem erro | Ausência de escrita em BIT, resultado correto, cópia z, trace ou timing |
| Teste dirigido `indexed_prefix_supports_ix_iy_displacements_and_indexed_cb` | Uma sequência representativa de IX/IY, half-register, displacement, BIT e SET | Todas as variantes e condições de borda |
| Teste `z80_indexed_cb_exhaustive_rotate_shift.rs` | Rotate/shift indexed-CB memória-only exaustivo em valores/carry | BIT/RES/SET, cópia z e microsequenciador |
| Testes de prefixo no sequenciador | Reconhecimento DD/FD, M1, R e latch do último prefixo; execução E3A1/E3A2/E4A/E4B/E4C | DDCB/FDCB e fallback geral ainda pendentes |
| `indexed_memory_e4a.rs` | Loads/stores IX/IY, displacement assinado, wrap, H/L, PC wrap, WAIT, snapshots e prefixos repetidos/alternados | Não substitui oráculo diferencial externo nem trace físico independente |
| `indexed_alu_e4b.rs` | 16 vetores de ALU executados com IX e IY; flags, CP/A, X/Y, WAIT em cada acesso, snapshots e last-prefix-wins | Não substitui oráculo diferencial externo ou trace físico independente |
| `indexed_incdec_e4c.rs` | 5 testes de INC/DEC indexado; IX/IY, flags/C, displacement, wrap, WAIT, snapshots, M-cycles e prefixos | Não substitui ZEXDOC/ZEXALL, oráculo diferencial externo ou trace físico independente |
| `indexed_cb_e5.rs` (PR #482; CI pendente) | Testa 256 subopcodes em IX e IY, BIT sem escrita, RMW, destino z, flags conforme o modelo atual, sequência de transações, M-cycles/T-states, WAIT, prefixos, displacement 0/-128/+127 e snapshot/restore | Ainda não executado em CI nesta fotografia; não é oráculo independente, trace físico ou TEMPORAL_PASS |
| CI E4C — run `41747460761` | Rust, Python, Scheduler deterministic stress e Windows x64 D3D12/WASAPI/XAudio2: 4 jobs PASS no HEAD `8f04b44e6c6ba85920a7438b74c9876586d5736c` | Não prova LONG_PASS, ZEXDOC/ZEXALL ou TEMPORAL_PASS |
| CI E4A — run `41734982519` | Rust, Python, Scheduler stress short e Windows x64 vídeo/áudio: 4 jobs PASS | Não prova LONG_PASS, ZEXDOC/ZEXALL ou TEMPORAL_PASS |
| CI E4B — run `41738004725` | Rust, Python, Scheduler stress short e Windows x64 vídeo/áudio: 4 jobs PASS | Não prova LONG_PASS, ZEXDOC/ZEXALL ou TEMPORAL_PASS |
| Igualdade com `Z80::step()` | Consistência entre caminhos do projeto | Independência do oráculo |

### Matriz de estado atual

| Família | Reconhecimento atômico | Funcional atômico | Diferencial externo | Snapshot indexado | Temporal |
|---|---|---|---|---|---|
| Operações 16-bit IX/IY (E3A1/E3A2) | IMPLEMENTED | Testes dirigidos E3A1/E3A2; 21 testes no total | NOT_RUN nesta auditoria | IMPLEMENTED_MICROSEQUENCED; snapshot/WAIT por fase coberta | TEMPORAL_UNVERIFIED |
| IXH/IXL/IYH/IYL | IMPLEMENTED no executor atômico | FUNCTIONAL_UNVERIFIED | NOT_RUN nesta auditoria | NOT_IMPLEMENTED no sequenciador | TEMPORAL_UNVERIFIED |
| LD indexado / displacement (E4A) | IMPLEMENTED no executor atômico | SHORT_PASS no microsequenciador; testes dirigidos de IX/IY, H/L, WAIT, PC/address wrap e prefixos | NOT_RUN nesta auditoria | IMPLEMENTED_MICROSEQUENCED; schema 27 e snapshots por fases cobertas | TEMPORAL_UNVERIFIED |
| ALU indexada (E4B) | IMPLEMENTED no executor atômico | SHORT_PASS no microsequenciador; 16 vetores x IX/IY, flags, WAIT, snapshot e prefixos | NOT_RUN nesta auditoria | IMPLEMENTED_MICROSEQUENCED; reutiliza MemoryReadAlu e schema 27 | TEMPORAL_UNVERIFIED |
| INC/DEC (IX/IY+d) (E4C) | IMPLEMENTED no executor atômico | QUICK_PASS / SHORT_PASS no microsequenciador; 5 testes dirigidos, flags/C, IX/IY, WAIT e prefixos | NOT_RUN nesta auditoria | IMPLEMENTED_MICROSEQUENCED; schema 27 e snapshots nos safe points cobertos | TEMPORAL_UNVERIFIED |
| DDCB/FDCB BIT | IMPLEMENTED no executor atômico | FUNCTIONAL_UNVERIFIED; teste de classificação | NOT_RUN nesta auditoria | NOT_IMPLEMENTED no sequenciador; próximo E5 | TEMPORAL_UNVERIFIED |
| DDCB/FDCB RMW | IMPLEMENTED no executor atômico | DIFFERENTIAL_PARTIAL para rotate/shift memória-only; RES/SET com cobertura menor | NOT_RUN nesta auditoria | NOT_IMPLEMENTED no sequenciador; próximo E5 | TEMPORAL_UNVERIFIED |
| Prefixos repetidos/alternados | IMPLEMENTED | Testes dirigidos de seleção do último prefixo | NOT_RUN nesta auditoria | Latch e contagem M1 preservados; execução E3A1/E3A2 coberta | TEMPORAL_UNVERIFIED |
| Prefixo ignorado / ED após DD/FD | IMPLEMENTED no executor atômico | Testes dirigidos; ampliar para todos os grupos | NOT_RUN nesta auditoria | Fallback geral pendente; próximo E6 | TEMPORAL_UNVERIFIED |

## 8. Referências técnicas primárias

1. ZiLOG, **Z80 CPU User Manual**, UM008011-0816 (2016), arquivo do projeto `um0080.pdf`.
   - Indexed addressing e displacement assinado: seção Indexed Addressing, páginas impressas 36–37.
   - `LD r,(IX+d)`: página impressa 75, 5 M-cycles / 19 T-states, sequência `(4,4,3,5,3)`.
   - `LD r,(IY+d)`: página impressa 77, 5 M-cycles / 19 T-states.
   - `LD (IX+d),r`: página impressa 81; `LD (IY+d),r`: página impressa 83; ambas 5 M-cycles / 19 T-states.
   - `LD (IX+d),n`: página impressa 86; `LD (IY+d),n`: página impressa 87; ambas 5 M-cycles / 19 T-states.
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
- **E3 — operações IX/IY de 16 bits:** E3A1/E3A2 implementadas no microsequenciador retomável; PR #470 candidata, CI 4/4 PASS.
- **E4A — LD indexado/displacement:** IMPLEMENTED e SHORT_PASS no candidato PR #471; 19T/5M, WAIT e snapshots nas fases cobertas.
- **E4B — ALU A,(IX/IY+d):** IMPLEMENTED / QUICK_PASS / SHORT_PASS no escopo registrado; diferencial externo NOT_RUN.
- **E4C — INC/DEC (IX/IY+d):** IMPLEMENTED / QUICK_PASS / SHORT_PASS pela PR #475; CI 41747460761 4/4 PASS; 5 testes dirigidos; LONG_PASS/diferencial/temporal NOT_RUN.
- **E5 — DDCB/FDCB:** separar fetches M1, displacement, opcode final, leitura e RMW.
- **E6 — prefixos ignorados:** implementar fallback BASE sem fetch duplicado e todas as combinações DD/FD/ED/CB.
- **E7 — fechamento semântico:** auditar BASE/CB/ED/DD/FD/DDCB/FDCB, flags, interrupções, HALT e snapshots.
- **E8 — formal/diferencial:** ZEXDOC/ZEXALL e oráculos independentes.
- **E9 — temporal/integração:** M/T-states, WAIT, interrupções, snapshots, Scheduler e SMS/SG-1000.

**Estado global:** E3A1/E3A2, E4A (LD indexado), E4B (ALU A,(IX/IY+d)) e E4C (INC/DEC indexado) estão IMPLEMENTED no microsequenciador e QUICK_PASS/SHORT_PASS nos gates executados. DDCB/FDCB e fallback geral permanecem pendentes. Diferencial externo, ZEXDOC/ZEXALL e traces independentes por T-state não foram executados; não declarar LONG_PASS ou TEMPORAL_PASS. A integração temporal sistêmica do Z80 à máquina SMS permanece INTEGRATION_PENDING.
