# Matriz semântica executável do Z80 — SMS

**Base:** branch `audit/z80-prefix-classification-matrix`, criada a partir de `280aabdb4a0567e5238da9bf1eb971ad5d594c85`.
**Propósito:** ligar cada família semântica aos testes existentes e impedir que a cobertura de dispatch seja confundida com correção semântica.
**Limite:** esta matriz é um gate incremental. Não declara completude semântica do Z80, compatibilidade universal com clones nem precisão temporal.

## 1. Gate executável de classificação

O teste `native/sms_core/tests/z80_semantic_matrix.rs` enumera 7 espaços × 256 bytes = **1.792 codificações**:

| Espaço | Classificação explícita |
|---|---|
| Base | Instrução base ou prefixo CB/DD/ED/FD |
| CB | Família CB atribuída |
| ED | Instrução ED atribuída ou fallback para byte não atribuído |
| DD / FD | 81 formas index-sensitive, 4 exceções de memória que usam H/L reais, 167 formas que ignoram o prefixo e 4 bytes de prefixo |
| DD-CB / FD-CB | Família indexed-CB |

A matriz contabiliza **78 bytes ED atribuídos** e **178 bytes ED não atribuídos**, para os quais o runtime atual segue o fallback ED-NOP. O teste também executa um passo para cada codificação em cada espaço e exige que a CPU retorne T-states agregados positivos sem `UnsupportedOpcode`.

Para DD/FD, a classificação diferencia instruções que substituem HL por IX/IY, operações com H/L ou (HL), as quatro formas especiais `LD H/L,(IX/IY+d)` e `LD (IX/IY+d),H/L` que usam os registradores H/L reais, prefixos aninhados e opcodes que ignoram o prefixo. A lista é uma classificação de opcodes para direcionar a validação; não é, isoladamente, prova da semântica executada pelo decoder.

**Interpretação obrigatória:** esse gate prova enumeração, classificação conforme o inventário e ausência de falha de dispatch nos estados de teste escolhidos. Não prova que a instrução produziu os registradores, flags, efeitos de memória/I/O ou T-states corretos.

## 2. Matriz de evidência semântica por família

Os estados abaixo descrevem a evidência versionada no repositório, não uma certificação de hardware.

| Família | Evidência principal no repositório | Estado | Lacuna que permanece |
|---|---|---|---|
| Loads de 8/16 bits, indiretos e imediatos | `z80_expanded_opcode_matrix.rs`, `z80_memory_integration.rs`, `z80_singlestep_reference_vectors.rs` | PARCIAL — vetores dirigidos | Cobertura combinatória de origem/destino, wraparound e efeitos WZ por forma |
| INC/DEC e ALU de 8 bits | `z80_alu_reference_vectors.rs`, `alu.rs` | PARCIAL — há testes de ALU e vetores | Integrar casos-limite para todas as formas de opcode, incluindo preservação de flags |
| DAA | `z80_alu_reference_vectors.rs` e testes unitários da ALU | DIRECIONADA | Provar toda a tabela A/F de entrada e diferenças de variante sem usar o mesmo código como oráculo |
| Rotação, shift e flags CB | `z80_expanded_opcode_matrix.rs`, `z80_alu_reference_vectors.rs` | PARCIAL | Todas as operações RLC/RRC/RL/RR/SLA/SRA/SLL/SRL, todos os destinos e limites de carry |
| BIT/RES/SET em registradores e (HL) | `z80_expanded_opcode_matrix.rs` e vetores de referência | PARCIAL | Cobrir todas as combinações bit/registro e separar flags documentadas de X/Y não documentados |
| Saltos, JR/DJNZ, CALL/RET/RST | `z80_expanded_opcode_matrix.rs`, `z80_singlestep_reference_vectors.rs` | PARCIAL | Todas as condições tomadas/não tomadas, deslocamentos -128/+127, wraparound e efeitos WZ |
| Pilha, PUSH/POP e EX (SP),HL | `z80_expanded_opcode_matrix.rs`, `z80_memory_integration.rs` | PARCIAL | SP em 0000/FFFF, ordem dos acessos e todas as combinações de registradores |
| EX AF,AF' / EXX e bancos alternativos | `z80_singlestep_reference_vectors.rs`, `z80_expanded_opcode_matrix.rs` | DIRECIONADA | Sequências combinadas com flags Q/P e alternância repetida |
| IX/IY, deslocamentos e H/L | `z80_indexed_semantics.rs`, `z80_indexed_displacement_boundaries.rs`, `z80_index_pair_semantics.rs`, `z80_indexed_prefix_wraparound.rs`, `z80_expanded_opcode_matrix.rs` | PARCIAL — vetores dirigidos de limites, par indexado, prefixos repetidos e wraparound | Matriz completa de formas indexadas, todas as exceções H/L e todas as combinações de prefixos |
| DD-CB / FD-CB | `z80_indexed_cb_semantics.rs`, `z80_indexed_cb_bit_res_set_wz.rs`, `z80_expanded_opcode_matrix.rs` | DIRECIONADA — todos os índices de bit e campos de destino para BIT/RES/SET sob DD e FD, com estado, memória, flags, PC, R, WZ e T-states agregados | Semântica completa das famílias de rotação/shift, validação diferencial independente de WZ/MEMPTR e traces temporais; vetores dirigidos não provam conformance total |
| ED: aritmética de 16 bits e transferências absolutas | `z80_singlestep_reference_vectors.rs`, `z80_expanded_opcode_matrix.rs` | PARCIAL | Cada forma e par de registradores, carry/half-carry/overflow e wraparound |
| ED: I/R, LD A,I/R, IM e RETN/RETI | `z80_interrupt_semantics.rs`, `z80_singlestep_reference_vectors.rs` | DIRECIONADA | Interações de interrupção e estado interno em todas as fronteiras |
| LDI/LDD/LDIR/LDDR e CPI/CPD/CPIR/CPDR | `z80_ed_block_transfer_compare_boundaries.rs`, `z80_singlestep_reference_vectors.rs` | SHORT_PASS no escopo exercitado | Estados iniciais exaustivos e confirmação independente de flags não documentadas |
| INI/IND/OUTI/OUTD e formas repetitivas | `z80_ed_block_io_reference_vectors.rs` | SHORT_PASS no escopo exercitado | Independência do oráculo e sequência temporal real de portas |
| INT IM 0/1/2 | `z80_interrupt_semantics.rs` | DIRECIONADA | Todos os bytes fornecidos pelo barramento, fronteiras EI e sequência ordenada de acknowledge |
| NMI, HALT, EI/DI | `z80_nmi_halt_semantics.rs`, `z80_interrupt_semantics.rs` | DIRECIONADA | Interleavings de requests, HALT com R, aceitação imediatamente antes/depois de EI |
| Flags X/Y, Q/P, WZ/MEMPTR e R | `z80_indexed_cb_bit_res_set_wz.rs` verifica WZ=IX/IY+d em BIT/RES/SET e flags X/Y de BIT indexado a partir do byte alto do endereço efetivo; demais famílias têm vetores próprios | PARCIAL / dependente da família | Obter confirmação diferencial independente para WZ e flags não documentadas, por família e variante de CPU; não extrapolar estes vetores para todas as instruções |
| Prefixos repetidos e aliases não documentados | `z80_indexed_prefix_wraparound.rs` + gate de classificação | DIRECIONADA em prefixos DD/DD e DD/FD | Fixar comportamento de cada combinação com documentação ou oráculo identificado |
| T-states nominais | Vetores de referência e retornos de `step()` | PARCIAL | Ampliar por forma e condição tomada/não tomada |
| M-cycles, traces de barramento e T-states ordenados | Nenhum trace completo validado no contrato atual | PENDENTE | Definir trace, eventos e waits; não alegar ciclo a ciclo |

## 3. Regras para alterar o estado da matriz

Uma linha só pode avançar de **PARCIAL** para **DIRECIONADA** quando houver ao menos um teste que compara explicitamente estado inicial, estado final, flags, PC/SP, efeitos de memória/I/O pertinentes e T-states nominais. Um teste que apenas chama `step()` não atende esse critério.

Uma linha só pode receber **SHORT_PASS** quando o conjunto de testes direcionados passar no SHA candidato exato e os limites declarados estiverem registrados. **LONG_PASS** exige uma campanha prolongada realmente executada. **TEMPORAL_PASS** exige comparação de traces temporais ordenados contra evidência adequada; não pode ser inferido de T-states agregados.

Cada novo vetor externo deve registrar URL, commit, licença e proveniência. O corpus SingleStepTests/Z80 está fixado em `ebe1875d48f374bcfd4b505d8eb8ee751568b5f7`, mas seu README informa ancestralidade no núcleo Z80 do Ares. Ele é útil para regressão diferencial, não um oráculo independente do Ares.

## 4. Aceitação desta atualização

- [x] Os sete espaços de opcode continuam enumerados integralmente.
- [x] DD/FD distinguem formas index-sensitive, exceções H/L, prefixos ignorados e prefixos aninhados.
- [x] Vetores dirigidos verificam deslocamentos assinados extremos, par indexado e registradores de índice altos/baixos.
- [x] Vetores DD-CB/FD-CB verificam efeito de memória, destino em registrador, flags e contadores arquiteturais em casos direcionados.
- [x] `z80_indexed_cb_bit_res_set_wz.rs` cobre BIT/RES/SET para os 8 índices de bit, os 8 campos de destino e DD/FD, com dados 00/FF; verifica destino de registrador, memória, flags, WZ, PC, R e T-states agregados.
- [x] O runtime não foi alterado nesta atualização; qualquer correção futura depende de divergência reproduzível contra evidência independente.
- [x] Prefixos DD repetidos e sequência DD/FD verificam precedência do último prefixo, PC, R e T-states agregados.
- [x] Um vetor de deslocamento negativo verifica wraparound do endereço efetivo para FFFF.
- [ ] CI, SMS Core Fast Validation e SMS Z80 Platform Validation no SHA candidato.
- [ ] Revisão independente das semânticas e contagens.
- [ ] Fechamento das lacunas remanescentes — não declarado concluído por esta atualização.
