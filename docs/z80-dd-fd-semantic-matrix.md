# Matriz semântica DD/FD do Z80 — reconciliação após PR #680

**Baseline documental reconciliada (2026-11-09):** main `1304bc0438e3ea333719d0ba3a495e2db9089da3` após PR #692. O histórico anterior permanece preservado; a nova evidência externa F-037 e a pendência Q/previous-Q estão registradas na seção 23.

**Data:** 2026-11-09
**Baseline remota auditada:** `main` — `4d8a862f254097e9dd1d0d1640edcd74f40ab12c` (2026-11-09, após PR #687 documental). A mudança #687 não alterou runtime Z80 nem a matriz de opcodes.
**E4C validada:** HEAD `8f04b44e6c6ba85920a7438b74c9876586d5736c`; CI [41747460761](https://github.com/leonardo201800478/sega-emulator/actions/runs/41747460761), 4/4 jobs PASS.
**Schema do microsequenciador:** 32 (baseline após PR #680)
**Escopo:** decoder DD/FD, IX/IY, IXH/IXL/IYH/IYL, (IX/IY+d), DDCB/FDCB.
**Natureza:** inventário do código ativo e das evidências disponíveis; E5 e E6 estão integradas com QUICK_PASS/SHORT_PASS nos escopos registrados, sem declarar conclusão funcional integral nem temporal do Z80.
**Meta global e scorecard:** consultar [Meta de conclusão do Z80 — cobertura, validação e critérios de aceite](z80-completion-scorecard.md) para critérios de cobertura, níveis de evidência e critérios de conclusão.

## 1. Legenda e regra de leitura

| Estado | Significado |
|---|---|
| `IMPLEMENTED_ATOMIC` | Há caminho de execução no executor atômico `Z80::step()`. |
| `IMPLEMENTED_MICROSEQUENCED` | Há fases retomáveis no microsequenciador, com testes direcionados e snapshot/WAIT nos safe points cobertos; não equivale a conformidade formal ou temporal. |
| `RECOGNIZED_ONLY` | O microsequenciador reconhece o byte de prefixo e agenda outro fetch M1, mas ainda não executa a família indexada. |
| `NOT_IMPLEMENTED` | A família não tem execução no microsequenciador retomável; o decode termina em `UnsupportedOpcode`. |
| `IMPLEMENTED_PENDING_VALIDATION` | Há implementação candidata no microsequenciador e testes direcionados na branch de PR, mas o HEAD final ainda não passou CI/merge na main. |
| `FUNCTIONAL_UNVERIFIED` | Existe handler, mas os testes localizados não demonstram a semântica integral da família. |
| `DIFFERENTIAL_PARTIAL` | Há comparação/fixtures selecionadas; não cobre todo o espaço da família nem constitui oráculo independente de silício. |
| `TEMPORAL_UNVERIFIED` | Não há trace de barramento por T-state validado independentemente para a família. |
| `NOT_RUN` | A suíte indicada não foi executada nesta auditoria. |

**Importante:** a presença de um handler, um retorno sem erro, ou igualdade entre o executor atômico e outro caminho do mesmo projeto não é prova suficiente de correção. O executor atômico é referência de comparação, não oráculo independente.

## 1.1 Interpretação normativa: semântica atômica versus precisão temporal

Esta matriz é um inventário de famílias e de caminhos de execução; não transforma o microsequenciador em requisito universal.

- O executor atômico deve executar corretamente a semântica de toda instrução suportada: registradores, flags, PC, SP, R, WZ/MEMPTR quando aplicável, memória e efeitos colaterais.
- Para instruções sem interação externa intermediária observável, `IMPLEMENTED_ATOMIC` é o nível de implementação suficiente. O tempo nominal documentado deve ser preservado quando conhecido, mas não se exigem fases internas artificiais.
- Priorizar precisão temporal nas instruções/processos que jogos usam para interagir com hardware em instantes sensíveis: I/O, VDP/PSG, raster, mapper, IRQ/NMI, HALT/acknowledge, WAIT ou contenção demonstrados.
- `NOT_IMPLEMENTED` na coluna do microsequenciador significa somente que não há sequência retomável naquele caminho; não significa que o opcode esteja ausente do executor atômico.
- `TEMPORAL_UNVERIFIED` não é falha funcional automática. É uma lacuna de evidência temporal que só bloqueia quando existir dependência observável concreta.
- Não é objetivo reproduzir todos os M-cycles/T-states, sinais M1/RFSH ou pinos do Z80 sem consumidor real. Não declarar precisão elétrica.

A matriz semântica e os resultados funcionais permanecem independentes da necessidade de um modelo temporal detalhado. A política superior é a [ADR-0016](architecture/adr/0016-observable-fidelity-and-proportional-timing.md).

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
| `DD op` | Decoder indexado; substitui HL/H/L conforme família | E3A1/E3A2/E4A/E4B/E4C/E5; fallback BASE para `Ignored` executa opcode já obtido sem refetch | Suítes E3A1/E3A2, E4A-E5 e `prefix_fallback_e6.rs` |
| `FD op` | Decoder indexado para IY | E3A1/E3A2/E4A/E4B/E4C/E5; fallback BASE para `Ignored` executa opcode já obtido sem refetch | Suítes E3A1/E3A2, E4A-E5 e `prefix_fallback_e6.rs` |
| `DD DD op`, `FD FD op` | Repetições contam 4T por prefixo extra | M1 repetidos, last-prefix-wins, fallback BASE e cadeia de 257 prefixos sem limite artificial | `prefix_fsm.rs`; `prefix_fallback_e6.rs` |
| `DD FD op`, `FD DD op` | Último prefixo seleciona IY/IX, respectivamente | Last-prefix-wins e fallback BASE preservam o opcode terminal e o custo de todos os prefixos | `prefix_fsm.rs`; `prefix_fallback_e6.rs` |
| `DD/FD ED op` | Prefixo de índice ignorado; ED segue o decoder ED | Index latch é cancelado para semântica; M-cycles/T-states do prefixo anterior permanecem nos estados suspensos e snapshots | `prefix_fallback_e6.rs`; ED absolute transfer; PR #486 |
| `DD/FD CB d op` | Consome displacement e subopcode e executa indexed-CB | Implementado na rota E5: CB M1, displacement MemoryRead/3T, opcode final MemoryRead non-M1/5T e operação indexed-CB | `indexed_cb_e5.rs`; teste legado de rejeição substituído por teste de request de displacement |
| `DD/FD op` não afetado | Fallback ao decoder BASE sem reler opcode; custo M1 preservado | Implementado para todas as instruções BASE já suportadas pelo executor; lacunas BASE continuam `UnsupportedOpcode` | `prefix_fallback_e6.rs`; CI #41757983537 |

Cadeias repetidas/alternadas não possuem limite artificial; teste com 257 prefixos confirma PC, R, M-cycles, T-states e dispatch do opcode BASE terminal.

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
| `24`, `25`, `26` | INC/DEC/LD IXH/IYH,n | 2 / 2 / 3 | Atualiza metade alta; INC/DEC altera flags de 8-bit, LD preserva flags; half-register não documentado como operando independente no UM0080 | 8 / 8 / 11 (prefixo + operação; trace independente pendente) | IMPLEMENTED_ATOMIC; testes dirigidos | IMPLEMENTED_MICROSEQUENCED; PR #680, WAIT/snapshot e 6 opcodes DD/FD |
| `2C`, `2D`, `2E` | INC/DEC/LD IXL/IYL,n | 2 / 2 / 3 | Atualiza metade baixa; INC/DEC altera flags de 8-bit, LD preserva flags; half-register não documentado como operando independente no UM0080 | 8 / 8 / 11 (prefixo + operação; trace independente pendente) | IMPLEMENTED_ATOMIC; testes dirigidos | IMPLEMENTED_MICROSEQUENCED; PR #680, WAIT/snapshot e 6 opcodes DD/FD |
| `40`–`7F`, exceto `76` | LD r,r' | 2 | H/L substituídos por IXH/IXL ou IYH/IYL quando aplicável; flags preservadas; HALT `76` não é indexado | 8 (dois M1; sem ciclo interno artificial) | IMPLEMENTED_ATOMIC; matriz dirigida | IMPLEMENTED_MICROSEQUENCED; PR #684, 48 formas, oracle, WAIT/snapshot e prefixos repetidos; CI PASS |
| `40`–`7F` com campo `r=6` ou `r'=6` | LD r,(IX/IY+d) / LD (IX/IY+d),r | 3 | EA=index+sign_extend(d); memória lida/escrita; H/L do outro operando são H/L normais; flags preservadas | 19 (5 M-cycles: 4,4,3,5,3) | IMPLEMENTED_ATOMIC; testes dirigidos | IMPLEMENTED_MICROSEQUENCED; E4A, 10 testes, IX/IY e matriz de registradores |
| `34`, `35` | INC/DEC (IX/IY+d) | 3 | EA=index+sign_extend(d); read-modify-write; flags S/Z/H/PV/N atualizadas; C preservado | 23 (6 M-cycles: 4,4,3,5,4,3) | IMPLEMENTED_ATOMIC; FUNCTIONAL_UNVERIFIED | IMPLEMENTED_MICROSEQUENCED; E4C, 5 testes, WAIT/snapshot/prefixos |
| `36` | LD (IX/IY+d),n | 4 | Lê displacement e imediato; escreve EA; flags preservadas | 19 (5 M-cycles: 4,4,3,5,3) | IMPLEMENTED_ATOMIC; testes dirigidos | IMPLEMENTED_MICROSEQUENCED; E4A, WAIT e snapshots por fase |
| `80`–`BF`, fonte diferente de 6 | ALU A,IXH/IXL/IYH/IYL ou r | 2 | A/F atualizados conforme operação ALU; X/Y conforme resultado, exceto CP conforme operando; half-register não documentado como operando independente no UM0080 | 8 (dois M1; sem ciclo interno artificial) | IMPLEMENTED_ATOMIC; matriz dirigida | IMPLEMENTED_PENDING_VALIDATION; PR #686, 16 formas, 10.816 casos, vetores fixos, WAIT/snapshot |
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

**E5 — integrada pela PR #485:** o microsequenciador possui fases próprias para displacement, opcode final non-M1, leitura efetiva e escrita opcional. O trace nominal implementado é `4,4,3,5,4` para BIT (20T) e `4,4,3,5,4,3` para RMW (23T). A leitura do opcode final é `MemoryRead` de 5T, não `OpcodeFetch`; portanto não incrementa R. O operando é lido em 4T. WAIT é contabilizado na transação pendente em cada fase.

**BIT — limite documental:** UM008011-0816, páginas impressas 247–250, declara S e P/V desconhecidos para BIT indexado; C não é afetado e H é set. O candidato conserva o modelo atual do executor atômico para S/PV e os bits X/Y derivados do endereço, sem promover essa escolha a fato de hardware. A validação independente desses bits permanece aberta.

## 6. Estado arquitetural e snapshot — estado após E5 integrada pela PR #485

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
| Testes de prefixo no sequenciador | Reconhecimento DD/FD/CB, M1, R, last-prefix-wins e execução E3A1/E3A2/E4A/E4B/E4C/E5 | Fallback geral E6 ainda pendente |
| `indexed_memory_e4a.rs` | Loads/stores IX/IY, displacement assinado, wrap, H/L, PC wrap, WAIT, snapshots e prefixos repetidos/alternados | Não substitui oráculo diferencial externo nem trace físico independente |
| `indexed_alu_e4b.rs` | 16 vetores de ALU executados com IX e IY; flags, CP/A, X/Y, WAIT em cada acesso, snapshots e last-prefix-wins | Não substitui oráculo diferencial externo ou trace físico independente |
| `indexed_incdec_e4c.rs` | 5 testes de INC/DEC indexado; IX/IY, flags/C, displacement, wrap, WAIT, snapshots, M-cycles e prefixos | Não substitui ZEXDOC/ZEXALL, oráculo diferencial externo ou trace físico independente |
| `indexed_cb_e5.rs` (PR #485; CI run #4245) | 8 testes PASS: 256 subopcodes por índice IX/IY, BIT sem escrita, RMW, destino z, flags conforme o modelo atual, endereços/ordem de transações, M-cycles/T-states, WAIT, prefixos, displacement 0/-128/+127 e snapshot/restore | Não é oráculo independente nem trace físico; não promove DIFFERENTIAL_PASS ou TEMPORAL_PASS |
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
| DDCB/FDCB BIT | IMPLEMENTED no executor atômico | QUICK_PASS / SHORT_PASS no microsequenciador; 64 BIT opcodes por índice e destinos z cobertos; BIT read-only | NOT_RUN nesta auditoria | IMPLEMENTED_MICROSEQUENCED; schema 28 e snapshots nos safe points | TEMPORAL_UNVERIFIED |
| DDCB/FDCB RMW | IMPLEMENTED no executor atômico | QUICK_PASS / SHORT_PASS no microsequenciador; rotações/shifts, RES/SET, cópia z e flags do modelo cobertos | NOT_RUN nesta auditoria | IMPLEMENTED_MICROSEQUENCED; schema 28 e snapshots em displacement/opcode/read/write | TEMPORAL_UNVERIFIED |
| Prefixos repetidos/alternados | IMPLEMENTED | QUICK_PASS / SHORT_PASS; seleção do último DD/FD e contagem M1 verificados também para DDCB/FDCB | NOT_RUN nesta auditoria | Latch, fases E5 e snapshots cobertos; schema 28 | TEMPORAL_UNVERIFIED |
| Prefixo ignorado / ED após DD/FD | IMPLEMENTED no executor atômico | E6 QUICK_PASS / SHORT_PASS nos grupos testados; ampliar auditoria semântica de todas as famílias | NOT_RUN nesta auditoria | Fallback geral integrado pela PR #486; BASE/ED não implementadas continuam explicitamente rejeitadas | TEMPORAL_UNVERIFIED |

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
- **E5 — DDCB/FDCB:** IMPLEMENTED / QUICK_PASS / SHORT_PASS, integrada pela PR #485 no merge SHA `f4130104fa971289507c0b15ab2d8eabd1dd5b00`; CI #4245 3 jobs PASS, Scheduler stress SKIPPED corretamente por não alterar o Scheduler. Diferencial externo e temporal não executados.
- **E6 — resolução geral DD/FD/ED/CB:** IMPLEMENTED / QUICK_PASS / SHORT_PASS pela PR #486; fallback BASE sem refetch, preservação de WAIT/snapshots/M-cycles e combinações repetidas/alternadas. CI #41757983537 Rust/Python/Windows PASS; Scheduler SKIPPED.
- **E7 — fechamento semântico:** em andamento. E7.1 adiciona HALT e M1 de retenção retomável; saída por NMI/INT, IM 0/1/2, restante das flags/estado e matriz integral permanecem pendentes. Ver [auditoria E7.1](audits/z80-e7-halt-m1-2026-11-04.md).
- **E8 — formal/diferencial:** ZEXDOC/ZEXALL e oráculos independentes.
- **E9 — temporal/integração:** M/T-states, WAIT, interrupções, snapshots, Scheduler e SMS/SG-1000.

**Estado global:** E3A1/E3A2, E4A, E4B, E4C, E5 e E6 estão IMPLEMENTED no microsequenciador e QUICK_PASS/SHORT_PASS nos gates funcionais executados. E6 foi integrada pela PR #486; merge SHA do código `24a32e03e10c8e82022172faee69ec0c046f8cb0`. A documentação foi reconciliada pela PR #488, merge SHA `303a792b1caab1ba40dec9c9521208eb5aaa0b45`. CI #41759035927, #41759330209 e #41759833153 Rust/Python/Windows PASS; Scheduler stress SKIPPED. E7 — fechamento semântico global — está em andamento, iniciado pela E7.1 HALT. Diferencial externo completo, ZEXDOC/ZEXALL e traces independentes por T-state não foram executados; não declarar LONG_PASS, DIFFERENTIAL_PASS ou TEMPORAL_PASS. A integração temporal sistêmica do Z80 à máquina SMS permanece INTEGRATION_PENDING.


---

## Atualização E7 — baseline main 644b2c22879d63447f63b730ed9415db11e81fab (2026-11-04)

A matriz DD/FD continua sendo inventário semântico por família e não uma declaração de conclusão global. E5 e E6 estão integradas; E7.1 e E7.2A–E7.2D estão integradas nas PRs #490, #495, #496, #498 e #500. E7.2E foi integrada pela PR #506 (merge SHA `54f881872ec9f5a9923ebdb17643b37a19954ec5`); o estado IN_PROGRESS é histórico e está supersedido.

A PR #506 acrescentou matriz de estados inválidos e cenários WAIT=0/1/3 em M1, memória, I/O, ciclos internos e IM0/IM1/IM2. Ela foi integrada; no merge SHA, os checks aplicáveis Rust quality/unit regression, Windows x64 D3D12 compile e Python lint/regression concluíram SUCCESS. O registro QUEUED acima é uma fotografia histórica, não o estado atual.

As famílias BASE/CB/ED/DD/FD/DDCB/FDCB devem continuar classificadas individualmente por implementação, validação funcional, diferencial e temporal. Não extrapolar testes internos para oráculo independente. E7.3 HALT T4 e E7.6 IRQ real VDP estão integradas; E7.4 BUSREQ e notificação externa RETI/daisy-chain permanecem diferidas até existir consumidor concreto; E7.7 auditoria integral segue IN_PROGRESS. E8 bloqueada até fechar E7; E9 continua dependente de integração temporal sistêmica.


---

## Aditivo operacional — baseline após E7.3 (2026-11-04)

Baseline remota: `main` `fcd91302cb64a31d63b75f5e1d3b6eab336668d1`.

E6 continua integrada pela PR #486 (merge SHA `24a32e03e10c8e82022172faee69ec0c046f8cb0`). E7.3 (HALT T4) foi integrada pela PR #509 (merge SHA `869b402e4c8f2dbd9c5f5fa4a10d10ac26506461`). O estado de E7.3 não promove automaticamente as famílias DD/FD a validação diferencial ou temporal completa. E7.4.1A (#512) e E7.4.1B (#514) continuam não integradas; C1 — instante racional comum do Temporal Core/Scheduler — é a próxima etapa. Diferencial externo completo, ZEXDOC/ZEXALL, LONG_PASS e TEMPORAL_PASS global permanecem pendentes.


---

## Aditivo normativo — granularidade temporal (2026-11-05)

A política vigente da [ADR-0016](architecture/adr/0016-observable-fidelity-and-proportional-timing.md) estabelece prioridade temporal para instruções/processos de gameplay cuja ordem ou duração altere uma interação com o console. Para instruções internas sem estado intermediário observável, execução atômica semanticamente exata e duração nominal conhecida são suficientes.

As classificações históricas `TEMPORAL_UNVERIFIED`, `NOT_IMPLEMENTED` no microsequenciador e referências a traces por T-state devem ser interpretadas dentro desse escopo. Elas não criam obrigação de implementar microsequenciamento universal nem bloqueio funcional automático. O E7 deve fechar a semântica de todas as famílias suportadas; a validação temporal deve cobrir somente as interações que jogos/periféricos realmente observam. E8 continua subordinada ao fechamento semântico de E7, não a uma exigência de precisão elétrica.



---

## Aditivo operacional — baseline após IRQ VDP SMS e RESET Z80 (2026-11-05)

Este aditivo supersede os estados históricos anteriores que ainda listavam E7.6 como pendente. A baseline é a `main` em `57d9405218de46795dd45876a52e3dbcec186ffd`; a cobertura de RESET é adicionada pela PR #536.

- **E7.6 — IRQ real do VDP SMS:** integrada pela PR #531. Testes executam `SmsMachine::step()` e verificam VBlank na linha 192 e Line IRQ na linha 2 com R10=1, entrega da linha INT ao Z80, aceitação IM 1 e reconhecimento via leitura do status do VDP. Isso comprova o caminho SMS VDP→Z80 coberto, não todos os modos/variantes de interrupção.
- **RESET Z80:** a PR #536 cobre PC/I/R, IFF1/IFF2, IM 0, saída de HALT, limpeza de NMI/EI delay, preservação dos registradores gerais/alternativos, IX/IY, SP, linha INT externa e monotonicidade do tempo da máquina/PSG/VDP.
- **Escopo temporal do RESET:** a operação de host é uma transição atômica para o estado definido pelo pino RESET; a largura elétrica mínima do pulso não é simulada sem consumidor de gameplay demonstrado, conforme ADR-0016.
- **E7.4 — BUSREQ/BUSACK:** não integrar microsequenciamento genérico agora. A arbitragem será retomada quando a integração Mega Drive (P4) apresentar o consumidor concreto do Z80 compartilhado.
- **E7.5 — RETI/daisy-chain:** a semântica funcional RETI/RETN permanece coberta; notificação externa de daisy-chain fica diferida até existir periférico suportado que a consuma. Não criar eventos por instrução sem esse consumidor.
- **E7.7 — auditoria semântica integral:** permanece IN_PROGRESS. A divergência de flags de I/O em bloco continua sem oráculo independente: UM008011-0816 declara C não afetado em INI/IND, enquanto a referência secundária descreve C/H derivados de soma. A implementação atual segue a hipótese secundária de compatibilidade NMOS; vetores SingleStepTests com ancestralidade em Ares não provam silício independente. `OUT (C),0` também permanece variante-dependente.
- **E8:** BLOCKED até o fechamento formal de E7. Não iniciar ZEXDOC/ZEXALL ou declarar DIFFERENTIAL_PASS antes de resolver a evidência semântica pendente.
- **E9:** permanece posterior à integração temporal observável e à comprovação de determinismo; não há TEMPORAL_PASS global.


---

## Aditivo — reconciliação E7.7 na main de 2026-11-06

**Baseline atual:** `08d9fac1d30a659bde8298e7da2f8ae1d75510d1` (PR #584 integrada). Este aditivo prevalece sobre classificações históricas anteriores quando houver conflito, sem apagar a trilha de auditoria.

### Evidências integradas desde a baseline antiga

- PR #562 integrou a correção do erratum P/V de `LD A,I` e `LD A,R` no microsequenciador, distinguindo INT de NMI e consumindo o latch após uma instrução intermediária. Os testes incluem snapshot/restore.
- PR #563 integrou testes para as flags das oito instruções de I/O em bloco, para DD/FD `LD r,r'` e para DD/FD half-register ALU e INC/DEC. As matrizes são regressões matemáticas/funcionais independentes da implementação do decoder em alguns aspectos, mas não são oráculos de silício.
- `docs/audits/z80-e7-block-io-flags-2026-11-05.md` conclui que o runtime atômico e o microsequenciador correspondem às fórmulas pesquisadas para Zilog NMOS; nenhuma alteração de runtime foi justificada para essa subquestão.

### Classificação corrigida de I/O em bloco

As flags de INI/IND/INIR/INDR/OUTI/OUTD/OTIR/OTDR ficam **caracterizadas para o perfil Zilog NMOS**, não universalmente comprovadas para CMOS e clones. A evidência experimental citada é de hardware Z80 NMOS em plataforma ZX, não de cada revisão do Master System. A diferença NMOS/CMOS permanece NOT VERIFIED e a validação direta no SMS físico permanece NOT RUN.

### Pendências que permanecem

- E7.7 global continua IN_PROGRESS: a auditoria integral de todas as famílias/efeitos semânticos não está concluída.
- `OUT (C),0` continua variante-dependente; não escolher um comportamento universal sem evidência do perfil-alvo.
- Não promover as matrizes de half-register e I/O em bloco a DIFFERENTIAL_PASS ou TEMPORAL_PASS; os modelos e vetores não são um oráculo independente de silício.
- E8 permanece BLOCKED até o fechamento documentado de E7. Não iniciar ZEXDOC/ZEXALL nesta etapa.
- E7/E9 não exigem traces elétricos universais nem microsequenciamento de instruções puramente internas; aplicar ADR-0016 e priorizar efeitos observáveis pelo SMS/SG-1000.

**Status:** evidência atualizada; E7.7 ainda não encerrada.


---

## Registro historico apos PR #595 — fotografia de 2026-11-06

Na fotografia imediatamente apos a PR #595, a `main` estava em `733152f61b706b2243914ae017643cef949d4ae1`. A baseline posterior, apos a PR #597, esta registrada no aditivo seguinte. A PR #595 integrou a conexao da IRQ do VDP SMS temporal a entrada INT do Z80 e preserva a fonte INT externa em snapshot/restore. A alteracao ocorreu em `native/sms_core/src/temporal_machine.rs`; nao modificou o decoder DD/FD, o microsequenciador Z80 ou os resultados registrados para E4C, E5 e E6.

O CI #5178 passou no HEAD de codigo `69ca40ab29c486f26852adb7536a04f95f9dd7ae`, incluindo testes SMS de IRQ/INT e regressao do workspace. Essa evidencia e especifica a integracao de IRQ SMS e nao altera a classificacao das familias semanticas DD/FD.

- E7.7 continua IN_PROGRESS.
- E8 continua BLOCKED ate o fechamento documentado de E7; nao iniciar ZEXDOC/ZEXALL.
- Nao reabrir E4C/E5/E6 sem divergencia semantica reproduzivel.
- Nao promover testes de IRQ SMS a DIFFERENTIAL_PASS, TEMPORAL_PASS ou fechamento global do Z80.


---

## Aditivo de baseline apos PR #597 — 2026-11-06

A main atual e `0fb09e2cad46a92aefe6a98ca07549a5ba91d78c`. A PR #597 adicionou teste da fronteira SMS worker → `NativeAudioSession` → `PcmSink` com sink em memória. Nenhuma instrução, prefixo DD/FD, flag, WZ/MEMPTR, fase M-cycle/T-state ou lógica do microsequenciador foi alterada.

O CI #5185 passou no HEAD `30554cc6642dd49d06adf495dca3e80f4822b081`. O teste de áudio SMS/SG-1000 foi executado; esta evidência não é diferencial de CPU nem temporal Z80.

- E7.7 permanece IN_PROGRESS.
- E8 permanece BLOCKED até o fechamento documentado de E7; não iniciar ZEXDOC/ZEXALL.
- Não promover a integração de áudio SMS a DIFFERENTIAL_PASS, TEMPORAL_PASS ou fechamento global do Z80.


---

## Reconciliação da baseline — 2026-11-07

**`main` auditada:** `e77eff26d80fd77a64546373c3e92326a0cc4e7a` (após merge da PR #628).

Esta seção supersede as baselines históricas acima como referência da `main` corrente, sem apagar as evidências históricas vinculadas aos respectivos SHAs.

- PR #613 integrou o runtime SMS temporal/vídeo na aplicação e modificou `rust/runtime/sms-runtime/src/worker.rs`, mas não alterou `native/z80_cpu`, o decoder DD/FD ou a implementação das famílias indexadas.
- PR #614 integrou áudio e controles na aplicação; não alterou o núcleo Z80.
- PR #628 corrigiu classificação de impacto e gate de CI; não alterou o código Z80 ou os resultados das famílias DD/FD.
- Portanto, os estados semânticos documentados para E4C, E5 e E6 não são reabertos nem promovidos por essas integrações.
- A matriz continua sendo inventário de implementação e evidências; a baseline atual não equivale a uma nova execução das suítes.
- E7.7 permanece IN_PROGRESS; E8 permanece BLOCKED até fechamento formal de E7.
- `TEMPORAL_UNVERIFIED` continua sendo uma classificação de lacuna de evidência, não uma falha funcional automática. Aplicar ADR-0016 e exigir precisão temporal somente onde houver interação observável demonstrada.


---

## Baseline e evidência E7.7 — 2026-11-07

**Main atual:** `43323df653497a98c7afae52a6f3ea198f46cdb0`. PRs #632 e #634 alteraram somente documentação; não reabriram E4C/E5/E6 nem produziram nova validação de CPU.

A subquestão `ED 71 / OUT (C),0` está resolvida para o perfil NMOS das placas Master System documentadas no levantamento: SMS Power lista Zilog Z0840004PSC em várias revisões e Zilog Z0840004PSC ou NEC D780C-1 em uma revisão inicial PAL. O produto Zilog distingue Z08400 NMOS de Z84C00 CMOS. As referências comparativas descrevem ED 71 como saída 00h no Zilog NMOS e FFh no Zilog CMOS, com exceções em clones; o teste de runtime integrado verifica a saída 00h. Consulte [auditoria E7.7 ED 71](audits/z80-e7-77-out-c-zero-variant-2026-11-07.md).

**Classificação:** `OUT (C),0` = `RESOLVED_FOR_DOCUMENTED_SMS_NMOS_PROFILE`; outros clones/variantes = `NOT_VERIFIED`. Não adicionar uma configuração de variante CMOS ao runtime sem um console/placa suportado que a exija. E7.7 global permanece `IN_PROGRESS`; E8 permanece `BLOCKED`.


---

## Reconciliação de baseline — main 2026-11-09

**Baseline atual:** `main` = `ca1b26089bd649a9248fab4ecbe47b78bc2eb49d`.

PR #672 alterou apenas GUI/D3D12. PR #673 adicionou teste/documentação de transferências absolutas ED no limite `0xFFFF`; nenhuma das duas alterou as famílias DD/FD, o decoder de prefixos ou os resultados E4C/E5/E6.

- E4C, E5 e E6 permanecem integradas conforme as evidências registradas nas seções históricas; não houve reexecução ou promoção de status nesta reconciliação.
- O scorecard Z80 e a auditoria ED foram reconciliados com a mesma baseline.
- E7.7 permanece IN_PROGRESS; E8 permanece BLOCKED até o fechamento formal de E7.
- As classificações `TEMPORAL_UNVERIFIED` e `FUNCTIONAL_UNVERIFIED` da matriz continuam com seu significado original; nenhuma evidência de ED altera a matriz DD/FD.


---

## Reconciliação atual — main 2026-11-09

**Baseline:** `main` = `1e14d32d5871326cc7f904cef7c5ad4feb1cd86c`.

- E7.2E está integrada pela PR #506; a antiga anotação QUEUED foi supersedida.
- E7.3/HALT T4 e E7.6/IRQ VDP SMS estão integradas.
- E7.4/BUSREQ permanece diferida até a integração Mega Drive apresentar consumidor concreto; E7.5/RETI mantém a semântica interna e difere apenas a notificação externa de daisy-chain até existir periférico suportado.
- A PR #675 adicionou somente teste de flags de repetição ED para I/O em bloco; não alterou famílias DD/FD, decoder de prefixos ou runtime.
- A evidência de hardware para PF/HF/XY no ciclo de repetição de I/O em bloco é parcial e restrita ao perfil NMOS/testes publicados. A execução do ROM no core e no SMS físico não foi realizada.
- `OUT (C),0` está resolvido para o perfil NMOS das placas SMS documentadas; não extrapolar para todos os clones/CMOS.
- E7.7 permanece IN_PROGRESS; E8 permanece BLOCKED; E9 não possui TEMPORAL_PASS global.


## Atualização IM2/RETI — main 2026-11-09

Baseline atual: `043ecb61d8d1cc77245fa72639d7f210d88cab26`.

As PRs #677 e #678 adicionaram somente testes ao módulo Z80: observação das flags de I/O em bloco por INT IM2, execução de PUSH AF/POP AF/EI/RETI e retomada da instrução repetitiva. Nenhuma semântica DD/FD, decoder de prefixos ou runtime foi alterada.

A semântica interna RETI/RETN e o retorno após IM2 são cobertos no core. A notificação externa de daisy-chain continua diferida até existir um periférico suportado; isso não bloqueia a semântica de retorno já implementada. E7.7 permanece IN_PROGRESS; E8 permanece BLOCKED.


## Reconciliação atual — main 2026-11-09 e candidato F-036

- **Main integrada:** `43772d5cd5395de273bd3898895cf1deed81aa53`. PR #680 integrou os seis opcodes INC/DEC/LD imediato das metades IX/IY; CI #42609912469 passou no HEAD `e20ec4b6c858bcaca265114ba543c19301dbb2e0`.
- PR #683 integrou a política de CI seletivo, com jobs focados PSG/VDP/input e agregador de qualidade; CI #42613123290 passou no HEAD `eaa88a249989b2ca6aee931e2a963df75a396008`.
- **F-036 integrado:** PR #684, HEAD validado `03dbd1552c8eb58ff8f9ef28d9ab0e97b615b4f3`, CI #42614755935 PASS, merge SHA `0b9b298f0f9791a6fa6db17580ad741db5c0515d`. A suíte `indexed_half_register_ld_e7.rs` cobre 24 formas register-only por prefixo, HALT, prefixos repetidos, WAIT e snapshot/restore. Estado: `IMPLEMENTED_MICROSEQUENCED`.
- As exceções DD/FD 66, 6E, 74 e 75 continuam no caminho E4A de displacement e usam H/L reais. Não alterar essa separação.
- **F-037** (ALU A,IXH/IXL/IYH/IYL) permanece `NOT_IMPLEMENTED` no microsequenciador e será uma PR separada após F-036.
- E7 permanece IN_PROGRESS; E8/ZEXDOC/ZEXALL permanece BLOCKED. Nenhum QUICK_PASS/SHORT_PASS adicional é atribuído até a execução da suíte no HEAD final da PR #684.


### Evidência de integração F-036

- PR #684 squash-merged em `0b9b298f0f9791a6fa6db17580ad741db5c0515d`.
- HEAD exato validado: `03dbd1552c8eb58ff8f9ef28d9ab0e97b615b4f3`; run #42614755935 PASS.
- A execução exclusiva Z80 deixou D3D12, áudio, SDL3, VDP, PSG, Scheduler stress e workspace como SKIPPED; o agregador passou e o merge foi aceito.
- Tentativas anteriores: #42614347670 FAIL em rustfmt; #42614617507 FAIL em Clippy (`field_reassign_with_default`). As correções foram aplicadas antes do HEAD final; nenhuma falha intermediária foi tratada como PASS.
- F-036 CLOSED; F-037 GAP_CONFIRMED; E7 IN_PROGRESS; E8 BLOCKED.


### Candidato F-037 — PR #686

- Main de entrada: `5aaa74af3fbf6fa7f70ecb1e6b71f3a966bb91e4`.
- Commit de implementação: `5f9ca825feb45633c2baf200e502f1a05bf99c7d`; último commit de teste/otimização antes desta atualização documental: `2ef8d4c0c73cf5a339f8d258ba9e314bfeebcdd2`. O HEAD final da PR será consultado no GitHub antes do merge.
- O microsequenciador aceita as 16 combinações DD/FD de ADD, ADC, SUB, SBC, AND, XOR, OR e CP com fonte IXH/IXL/IYH/IYL. A execução reutiliza `read_indexed_register()` e `execute_alu_operation()`.
- A suíte contém matriz diferencial de 10.816 casos, vetores fixos para flags e CP X/Y, preservação de Q/previous-Q, WZ, PC/R, IFF, I, IX/IY/H/L, WAIT nos dois M1, snapshot/restore e prefixos repetidos.
- O operando código 6 (`(IX+d)/(IY+d)`) permanece no caminho de memória indexada E4B; não é substituído por IXH/IXL/IYH/IYL.
- Estado F-037: `CLOSED` após PR #686, HEAD `a0937e8fa1bb4547d1ee53a9e3431392a2fa443a`, run #42619522899 PASS e merge SHA `e7e37290e1baa9119326e9e7213f77cadcc37f5e`. E7: IN_PROGRESS. E8: BLOCKED.


### Fechamento F-037 — PR #686

- Main integrada: `e7e37290e1baa9119326e9e7213f77cadcc37f5e`.
- HEAD validado: `a0937e8fa1bb4547d1ee53a9e3431392a2fa443a`; run #42619522899 PASS.
- Implementadas as 16 formas DD/FD ALU com IXH/IXL/IYH/IYL, reutilizando a ALU compartilhada e sem ciclos internos artificiais.
- 10.816 casos diferenciais, vetores fixos para A/F e X/Y de CP, carry-in, Q/previous-Q, WAIT, snapshot/restore e prefixos repetidos.
- F-037 CLOSED. F-028 AUDIT_PARTIAL; E7 IN_PROGRESS; E8 BLOCKED.
- Evidência complementar pendente: importar vetores externos SingleStepTests Z80 NMOS com atribuição/licença MIT e testar a execução retomável contra esses estados finais.


## 23. Reconciliação após PR #692 — F-037 externo e F-028 residual

### F-037 — ALU com metades IX/IY

- PR #692 integrada na main `1304bc0438e3ea333719d0ba3a495e2db9089da3`.
- HEAD validado `1606556814a8746a445d3ad32fc948ab97f0fde5`; CI #42622828177 PASS.
- SingleStepTests/Z80 fixado em `ebe1875d48f374bcfd4b505d8eb8ee751568b5f7` (MIT).
- 32 arquivos × 1.000 casos = **32.000/32.000 PASS** no microsequenciador retomável.
- Abrange DD/FD 84/85, 8C/8D, 94/95, 9C/9D, A4/A5, AC/AD, B4/B5 e BC/BD.
- Comparação arquitetural inclui F, Q/P, PC, R, WZ, registradores alternativos, IFF/IM/EI, IX/IY e RAM. A comparação temporal limita-se às duas requisições lógicas M1 e 8 T-states agregados; não declara equivalência elétrica.

### F-028 — exceções e Q/previous-Q

**Exceções H/L com memória:** a inspeção dos dois executores confirma que DD/FD 66/6E escrevem H/L reais e 74/75 leem H/L reais. No executor atômico, os helpers de index register são chamados com substituição de metades desabilitada; no microsequenciador, os estados MemoryReadRegister/MemoryWrite usam read_register/write_register comuns. Não alterar E4A/E4B sem divergência reproduzida.

**Prefixos:** prefix_fsm.rs cobre repetição/alternância e last-prefix-wins; prefix_fallback_e6.rs cobre prefixos ignorados e retomada. A classificação de 76 como prefixo ignorado preserva HALT como opcode terminal.

**Pendência identificada por inspeção semântica:** para Zilog NMOS, DD/FD imediatamente antes de SCF/CCF devem ser tratados como a instrução precedente não modificadora de flags. A referência de hardware Z80Decoder informa que o prefixo faz perder o estado de modificação de flags anterior. O código atual mantém q_previous ao atravessar DD/FD.

Vetores externos discriminantes fixados na mesma revisão SingleStepTests:

| Fixture | Git blob SHA-1 | Estado inicial | Estado final esperado |
|---|---|---|---|
| DD 37 0001 | `c68369350b5a47ebb481d35a7b9e4dd86e292716` | A=50, F=75, Q=75 | F=65, Q=65 |
| FD 3F 0001 | `3c8a105d7f3e9d6cae78e2e47b03d7d642f2b345` | A=06, F=ED, Q=ED | F=FC, Q=FC |

Reprodução obtida na PR #695, workflow run #42624833386: DD 37 0001 falhou com F obtido=45 e esperado=65. O teste interrompeu no primeiro mismatch; FD 3F ainda não foi alcançado. A fórmula atual, preservando q_previous=F, prevê F=45 no primeiro caso e F=D4 no segundo; com q_previous limpo, os resultados esperados são F=65 e F=FC. A correção dos dois executores e a validação de snapshot/restore estão na PR de correção Q/previous-Q, ainda pendente de CI.

**Próxima tarefa objetiva:** adicionar DD 37 e FD 3F ao harness externo, reproduzir no microsequenciador e executor atômico, corrigir a fronteira de prefixo sem alterar E4A/E4B e validar snapshot/restore após o primeiro M1.