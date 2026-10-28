# Auditoria completa do espaço de opcodes ED do Z80

**Escopo:** auditoria estática do despachante Rust ativo em `native/sms_core/src/cpu.rs`, método `execute_ed()`, e dos testes presentes no mesmo arquivo.  
**Alvo de referência:** Zilog Z80 clássico (NMOS como hipótese de compatibilidade inicial do projeto); não generalizar para Z180, R800 ou clones.  
**Resultado:** os 256 segundos bytes ED têm caminho de execução, mas isso não significa que 256 instruções semânticas estejam implementadas ou validadas.

## 1. Resumo executivo

| Medida | Resultado da auditoria estática |
|---|---:|
| Segundo bytes possíveis após ED | 256 |
| Codificações tratadas por famílias explícitas no despachante | 78 |
| Codificações que caem no fallback ED-NOP | 178 |
| Famílias/codificações com implementação explícita | agrupadas na tabela abaixo |
| Codificações explícitas com ao menos um vetor semântico dirigido | 78/78 bytes cobertos por famílias, com profundidade desigual por condição |\n| Validação diferencial automatizada contra hardware/oráculo independente | **Não executada** |
| Precisão ciclo a ciclo / traço ordenado de barramento | **Não implementada por esta interface** |

A contagem de 78 considera as codificações distintas explicitamente reconhecidas pelos padrões e casos do `match`; inclui aliases de NEG, RETN/RETI e IM. As outras 178 codificações retornam 8 T-states pelo `_ => 8` final. Isso caracteriza um fallback funcional no código, não prova de que cada byte reservado seja um NOP em toda variante de CPU.

O teste `every_primary_and_prefixed_encoding_is_classified_without_fallback_errors` continua sendo apenas um smoke test geral. A nova cobertura adiciona `all_256_ed_encodings_match_nominal_cycle_vectors`, que percorre cada segundo byte ED com uma expectativa de T-states, PC e total acumulado, e `every_ed_fallback_encoding_is_an_eight_tstate_no_op`, que verifica os 178 bytes do fallback. Esses testes comprovam execução e o contrato testado; não demonstram, sozinhos, que todos os resultados semânticos correspondem a silício.

## 2. Inventário das codificações explícitas

Os tempos listados são os retornados atualmente pelo núcleo (T-states nominais agregados). Não equivalem a uma sequência de M-cycles/T-states de barramento.

| Segundo byte ED | Instrução / família | Efeito implementado observado no código | T-states nominais | Evidência e lacunas |
|---|---|---|---:|---|
| `47` | LD I,A | Copia A para I | 9 | Vetor dirigido para LD I,A e LD R,A; faltam valores de fronteira adicionais |
| `4F` | LD R,A | Copia A para R | 9 | Vetor dirigido confirma cópia dos oito bits de A; faltam casos adicionais |
| `57`, `5F` | LD A,I / LD A,R | Copia I/R para A; mantém C, atualiza S/Z/PV/X/Y e usa IFF2 para PV; arma estado interno `p` para a borda de interrupção | 9 | Vetores dirigidos para I=0, I=0xA8, R com incremento de fetch, IFF2 e C; faltam mais combinações de interrupção |
| `43, 53, 63, 73` | LD (nn),BC/DE/HL/SP | Escreve palavra em little-endian; WZ recebe nn+1 | 20 | Vetores dirigidos para BC/DE/HL/SP e ordem little-endian; wrap de endereço ainda pendente |
| `4B, 5B, 6B, 7B` | LD BC/DE/HL/SP,(nn) | Lê palavra little-endian; WZ recebe nn+1 | 20 | Vetores dirigidos para BC/DE/HL/SP e WZ; wrap de endereço ainda pendente |
| `4A, 5A, 6A, 7A` | ADC HL,BC/DE/HL/SP | Usa carry de entrada e atualiza HL e flags aritméticas de 16 bits | 15 | Todos os pares cobertos por vetores dirigidos; overflow, half-carry e carry de entrada têm casos específicos, mas não é varredura combinatória |
| `42, 52, 62, 72` | SBC HL,BC/DE/HL/SP | Usa borrow/carry de entrada e atualiza HL e flags aritméticas de 16 bits | 15 | Todos os pares cobertos por vetores dirigidos; borrow e fronteiras aritméticas têm casos específicos, mas não é varredura combinatória |
| `40,48,50,58,60,68,70,78` | IN r,(C) | Lê porta BC; destino código 6 descarta o valor; atualiza S/Z/PV/X/Y e preserva C; WZ=BC+1 | 12 | Teste com bus de I/O verifica os oito destinos, endereço BC e flags para valor 0xA5; falta varredura de valores/estados |
| `41,49,51,59,61,69,71,79` | OUT (C),r / OUT (C),0 | Escreve registrador para porta BC; código 6 escreve zero | 12 | Teste com bus de I/O verifica os oito valores/endereços e preservação de flags; `OUT (C),0` permanece dependente da variante-alvo |
| `44,4C,54,5C,64,6C,74,7C` | NEG e aliases | Calcula 0−A usando helper de subtração e atualiza A/F | 8 | Os oito aliases são comparados com A=0x80; faltam os demais valores de fronteira |
| `45,4D,55,5D,65,6D,75,7D` | RETN / RETI / aliases | Todos executam pop de PC, copiam IFF2 para IFF1 e atualizam WZ | 14 | Vetores dirigidos percorrem os oito aliases, PC/SP e cópia IFF2→IFF1. O modelo não emite evento externo distinto para RETI/daisy chain |
| `46,4E,66,6E` | IM 0 / aliases | Define modo de interrupção 0 | 8 | Todos os aliases IM 0 têm vetor dirigido |
| `56,76` | IM 1 | Define modo de interrupção 1 | 8 | Ambos os aliases têm vetor dirigido; integração com INT também tem teste existente |
| `5E,7E` | IM 2 | Define modo de interrupção 2 | 8 | Ambos os aliases têm vetor dirigido; integração com INT também tem teste existente |
| `67` | RRD | Troca nibbles entre A e (HL) segundo a operação RRD | 18 | Vetor dirigido verifica A, memória, flags e WZ |
| `6F` | RLD | Troca nibbles entre A e (HL) segundo a operação RLD | 18 | Vetor dirigido verifica A, memória, flags e WZ |
| `A0,A8,B0,B8` | LDI/LDD/LDIR/LDDR | Transfere byte (HL)→(DE), ajusta HL/DE/BC e flags; formas repetitivas recuam PC e retornam 21 T quando continuam | 16 / 21 | Vetores dirigidos cobrem as quatro formas, direção, contador, flags selecionadas, repetição e término; limites e todas as flags ainda pendentes |
| `A1,A9,B1,B9` | CPI/CPD/CPIR/CPDR | Compara A com (HL), ajusta HL/BC e flags; formas repetitivas continuam enquanto BC≠0 e A≠valor | 16 / 21 | Vetores dirigidos cobrem as quatro formas, decremento, match e continuação; flags X/Y/WZ e fronteiras de BC ainda pendentes |
| `A2,AA,B2,BA` | INI/IND/INIR/INDR | Lê porta BC, escreve em (HL), ajusta HL/B, calcula flags e repete conforme B | 16 / 21 | Vetores dirigidos cobrem as quatro formas, endereço de porta, memória, decremento/incremento e repetição; flags não documentadas exigem validação de hardware |
| `A3,AB,B3,BB` | OUTI/OUTD/OTIR/OTDR | Lê (HL), escreve em porta baseada no B decrementado e C, ajusta HL/B, calcula flags e repete conforme B | 16 / 21 | Vetores dirigidos cobrem as quatro formas, endereço/valor de saída, direção e repetição; flags não documentadas exigem validação de hardware |

**Nota sobre aliases:** os padrões de NEG e RETN/RETI reconhecem oito bytes cada e executam um mesmo corpo. Para RETI, o núcleo não distingue a semântica externa de sinalização de fim de interrupção usada por controladores com daisy chain. A equivalência interna de retorno não prova equivalência do comportamento externo.

## 3. Espaço ED completo: classificação dos 256 segundos bytes

Esta tabela classifica todo o espaço por intervalos e exceções. “Fallback ED-NOP” significa somente que o despachante atual retorna 8 T-states sem efeitos explícitos adicionais. A classificação não certifica a semântica em silício de variantes ou clones.

| Intervalo / conjunto | Classificação atual | Detalhe |
|---|---|---|
| `00–3F` | Fallback ED-NOP | Nenhuma codificação neste intervalo tem handler explícito |
| `40–7F` | Mistura de handlers e fallback | Contém IN/OUT, ADC/SBC HL, transferências de palavra, NEG, RETN/RETI, IM, LD I/R e RRD/RLD; os demais bytes retornam pelo fallback |
| `80–9F` | Fallback ED-NOP | Nenhuma codificação neste intervalo tem handler explícito |
| `A0–A3` | Operações de bloco explícitas | LDI, CPI, INI, OUTI |
| `A4–A7` | Fallback ED-NOP | Sem handler explícito |
| `A8–AB` | Operações de bloco explícitas | LDD, CPD, IND, OUTD |
| `AC–AF` | Fallback ED-NOP | Sem handler explícito |
| `B0–B3` | Operações de bloco repetitivas explícitas | LDIR, CPIR, INIR, OTIR |
| `B4–B7` | Fallback ED-NOP | Sem handler explícito |
| `B8–BB` | Operações de bloco repetitivas explícitas | LDDR, CPDR, INDR, OTDR |
| `BC–FF` | Mistura de aliases explícitos e fallback | Os handlers já listados reaparecem nos aliases de NEG/RETN/RETI/IM e registradores especiais; todos os demais bytes caem no fallback |

### Codificações explícitas, agrupadas para auditoria automática

- Registradores especiais: `47, 4F, 57, 5F`.
- Transferência absoluta de pares: `43, 53, 63, 73, 4B, 5B, 6B, 7B`.
- Aritmética de 16 bits: `42, 52, 62, 72, 4A, 5A, 6A, 7A`.
- I/O por BC: `40,48,50,58,60,68,70,78,41,49,51,59,61,69,71,79`.
- NEG: `44,4C,54,5C,64,6C,74,7C`.
- RETN/RETI: `45,4D,55,5D,65,6D,75,7D`.
- IM: `46,4E,66,6E,56,76,5E,7E`.
- Nibble: `67,6F`.
- Bloco: `A0,A8,B0,B8,A1,A9,B1,B9,A2,AA,B2,BA,A3,AB,B3,BB`.

A soma desses conjuntos é 78 codificações únicas. As outras 178 são aceitas pelo fallback. O fallback também faz com que um smoke test de todos os bytes passe sem comprovar a correção semântica das codificações reservadas.

## 4. Auditoria de semântica e riscos encontrados

### 4.1 Operações de bloco: prioridade alta

Os handlers executam toda a operação dentro de uma chamada de instrução e retornam apenas a duração nominal agregada. Isso impede validar a sequência temporal de cada leitura/escrita. Antes de afirmar conformidade, criar vetores independentes para:

- LDI/LDD: HL/DE incrementam/decrementam, BC decrementa, memória e flags preservadas/alteradas conforme especificação.
- LDIR/LDDR: continuação quando BC≠0; PC volta ao prefixo ED; término em BC=0; 16 T-states na última iteração e 21 nas iterações que repetem.
- CPI/CPD: resultado da comparação, H, N, S/Z, P/V, X/Y e WZ.
- CPIR/CPDR: saída antecipada quando A=(HL), além do término por BC=0.
- INI/IND/INIR/INDR e OUTI/OUTD/OTIR/OTDR: sequência exata de acesso à porta/memória, valor do endereço de I/O, decremento de B, flags e condições de repetição.

A lógica atual de flags de I/O de bloco contém fórmulas próprias e não deve ser considerada comprovada por mera inspeção. Usar casos discriminantes com carry/half-carry, valor transferido com bit 7 ligado/desligado, B inicial 1/2/0 e HL em limites.

### 4.2 RETN versus RETI

O mesmo handler cobre os oito bytes e sempre restaura IFF1 a partir de IFF2. Isso cobre a operação interna de retorno testada, mas não representa uma notificação externa específica de RETI. Se o sistema de interrupções do SMS/SG-1000 precisar de daisy chain, será necessária uma saída de evento separada; não simular esse sinal alterando PC/IFFs.

### 4.3 I/O por (C)

IN/OUT por BC tem caminho explícito, mas a interface `CpuBus` não representa fases de barramento nem o wait state de I/O. `IN (C)` (código destino 6) descarta o valor e atualiza flags; `OUT (C),0` escreve zero conforme a implementação. O comportamento de saída zero deve ser documentado para a variante escolhida e testado em relação à fonte independente apropriada.

### 4.4 Flags, WZ e estado de interrupção

- `LD A,I`/`LD A,R`: PV usa IFF2 e o núcleo usa um latch interno para tratar uma borda de interrupção; testar com INT pendente e todos os estados de IFF.
- `ADC/SBC HL,rr`: conferir exaustivamente carry/borrow, H, overflow, S/Z e bits X/Y do byte alto.
- `RRD/RLD`: validar os dois nibbles, memória, A, S/Z/PV, preservação de C, WZ e ciclos.
- `NEG`: testar A=00, 01, 7F, 80, FF e todos os aliases.
- `IM`: verificar todos os aliases e o efeito real em subsequente INT; codificações redundantes não podem produzir modos diferentes.
- `WZ/MEMPTR`: o código atual atualiza WZ em vários handlers ED, mas a matriz de atualizações precisa de comparação por instrução. Um valor WZ plausível não é evidência suficiente.
- `Q`: o efeito da escrita de F em sequências posteriores deve ser testado separadamente; não inferir a correção de X/Y de um único caso.

### 4.5 T-states nominais não são temporização de barramento

Os valores devolvidos pelo handler não codificam uma sequência de M1, leituras/escritas de memória, I/O, refresh ou wait states. A auditoria não encontrou um modelo de eventos ordenados dentro de `execute_ed()`. Portanto, mesmo valores nominais que coincidam com o manual não comprovam a temporização física.

## 5. Matriz de testes e evidência no SHA candidato

**SHA de código validado:** `e9776e705d0ff55bfc701a933ff7bea2d70c1974` (branch `test/exhaustive-z80-ed-opcodes`, PR #183).  
**Escopo do resultado:** regressão determinística em CI; sem hardware físico, ROM de teste externa executada, nem comparação automatizada diferencial.

| Área | Teste/evidência | Resultado |
|---|---|---|
| Todos os 256 segundos bytes ED | `all_256_ed_encodings_match_nominal_cycle_vectors`: resultado, T-states retornados/acumulados e PC esperado por byte | **PASS** |
| Os 178 bytes que usam fallback | `every_ed_fallback_encoding_is_an_eight_tstate_no_op`: 8 T-states, PC=2 e ausência de alterações nos registradores principais testados | **PASS** |
| Transferência absoluta de palavras | `ed_absolute_word_loads_and_stores_cover_all_register_pairs`: BC/DE/HL/SP, little-endian e WZ | **PASS** |
| ADC/SBC HL | `ed_adc_sbc_hl_vectors_cover_signed_overflow_half_carry_and_borrow` e `ed_adc_sbc_hl_decode_all_four_register_pairs`: todos os pares e casos aritméticos discriminantes | **PASS** |
| Registradores especiais e flags | `ed_ld_i_a_and_ld_r_a_copy_accumulator_without_touching_flags`, `ed_ld_a_i_and_ld_a_r_update_flags_and_preserve_carry` | **PASS** |
| NEG, IM e RETN/RETI | `ed_neg_aliases_produce_identical_results_and_flags`, `ed_interrupt_mode_aliases_select_the_documented_mode`, `ed_retn_reti_aliases_restore_iff1_and_pop_return_address` | **PASS** |
| IN/OUT por BC | `ed_in_out_register_forms_use_bc_port_and_expected_data`: todos os destinos/fontes, endereço BC, flags de IN e saída zero do modelo | **PASS** |
| RRD/RLD | `ed_rrd_rld_match_nibble_transfer_and_documented_flags`: nibbles, A, memória, flags e WZ | **PASS** |
| LDI/LDD/LDIR/LDDR | `ed_block_transfer_vectors_check_copy_direction_counter_flags_and_repeat` | **PASS** |
| CPI/CPD/CPIR/CPDR | `ed_block_compare_vectors_check_match_stop_repeat_and_flags`, `ed_cpd_decrements_hl_and_cpdr_stops_on_match` | **PASS** |
| INI/IND/INIR/INDR e OUTI/OUTD/OTIR/OTDR | `ed_block_io_vectors_check_ports_memory_counter_and_repeat`, `ed_decrement_block_io_variants_update_hl_and_ports`: todos os oito opcodes, direção e endereços/valores de I/O | **PASS** |
| Rust workspace | `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --locked` via workflows | **PASS** |
| Núcleo SMS focado | formatação, lint e testes focados no SMS Core via workflow | **PASS** |
| CI Python | formatação, lint e pytest do workflow geral | **PASS** |

### Workflows no SHA `e9776e705d0ff55bfc701a933ff7bea2d70c1974`

- [SMS Z80 Platform Validation](https://github.com/leonardo201800478/sega-emulator/actions/runs/40636388858) — **PASS**.
- [SMS Core Fast Validation](https://github.com/leonardo201800478/sega-emulator/actions/runs/40636388894) — **PASS**.
- [CI](https://github.com/leonardo201800478/sega-emulator/actions/runs/40636388836) — **PASS**.

### Limites e conflitos de referência

- Os 256 bytes agora têm um vetor de execução/ciclos/PC; isso não é o mesmo que 256 validações semânticas exaustivas para todos os valores possíveis de registradores, flags, memória e estado de interrupção.
- Cada uma das 78 codificações explícitas está coberta por ao menos um vetor dirigido em sua família, mas vários vetores cobrem apenas um ou poucos estados iniciais. Falta uma comparação diferencial automatizada contra um oráculo independente com procedência/licença verificadas.
- Os 178 bytes de fallback são testados conforme a política atual de ED-NOP do Z80 clássico. A validação não demonstra que todos os clones/variantes tenham comportamento idêntico.
- Há uma divergência relevante sobre flags de I/O de bloco: o manual ZiLOG UM008011-0816 descreve C como não afetado em INI/OUTI, enquanto *The Undocumented Z80 Documented* descreve C/H derivados do carry da soma de dados/endereço. O vetor INI discrimina o comportamento C/H implementado com base na referência secundária. Isso deve permanecer identificado como hipótese de compatibilidade NMOS até comparação com ROM independente/hardware; não é prova universal para clones.
- `OUT (C),0` é testado como saída de zero no modelo atual, mas fontes secundárias registram variação entre lotes/implementações de Z80. Essa escolha precisa permanecer vinculada à variante-alvo.
- Não há validação de traços ordenados de barramento, wait states ou precisão ciclo a ciclo. Os números verificados são T-states nominais agregados.

## 6. Critérios para fechar a auditoria ED

1. Expandir vetores fixos por codificação explícita para cobrir registradores, memória/I/O, PC, SP, F, IFFs, R, WZ e duração nominal aplicável em múltiplos estados iniciais.
2. Para instruções paramétricas, adicionar fronteiras e estados iniciais discriminantes; para blocos repetitivos, testar iteração que continua e a última iteração.
3. Expandir casos de fronteira para cada alias e documentar a variante-alvo; os aliases básicos já têm cobertura dirigida.
4. Comparar com ao menos um oráculo independente de procedência/licença verificadas e, quando possível, com uma segunda referência sem ancestralidade comum conhecida.
5. Para bytes fallback, manter teste estrutural separado da decisão de compatibilidade. Confirmar a política de ED-NOP contra documentação e variante; não tratar fallback como instrução documentada sem fonte.
6. Executar e registrar checks no SHA candidato exato: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --locked`, workflows pertinentes e seus logs.
7. Criar futuramente traços de barramento para ED, incluindo I/O, blocos e interrupções, antes de declarar precisão temporal.

## 7. Fontes

1. ZiLOG, **Z80 CPU User Manual**, UM008011-0816 (2016), especialmente tabela de opcodes e descrições individuais de instruções ED, flags e temporização. [PDF oficial](https://www.zilog.com/docs/z80/um0080.pdf).
2. ZiLOG, **Z80 Family CPU User Manual** (2001), usado no projeto como comparação histórica; conferir visualmente o PDF quando a extração textual estiver corrompida.
3. [Z80.info — The Undocumented Z80 Documented](https://z80.info/zip/z80-documented.pdf), referência secundária para comportamentos não documentados; não substitui a fonte primária nem a identificação de variante. Há divergência explícita com a tabela simplificada do manual ZiLOG para flags de I/O de bloco; a documentação acima registra essa incerteza.
4. Código auditado: [`native/sms_core/src/cpu.rs`](https://github.com/leonardo201800478/sega-emulator/blob/main/native/sms_core/src/cpu.rs), função `execute_ed()` e testes no mesmo arquivo.

**Conclusão atualizada:** os 256 segundos bytes ED têm testes de execução/ciclos/PC; os 178 bytes de fallback têm testes do contrato ED-NOP; e todas as 78 codificações explícitas têm pelo menos um vetor semântico dirigido por família. Os workflows passaram no SHA indicado. Ainda não há evidência suficiente para declarar validação semântica exaustiva ou diferencial do espaço ED: faltam matrizes combinatórias mais profundas, resolução dos conflitos de referência, oráculo independente automatizado e validação em hardware/ROM externa. A precisão ciclo a ciclo continua pendente.
