# Matriz semântica executável do Z80 — SMS

**Base:** branch `audit/z80-executable-semantic-matrix`, criada a partir de `ed1aee7c8f9c80feafbd4019699c5a9e52872886`.
**Propósito:** ligar cada família semântica aos testes existentes e impedir que a cobertura de dispatch seja confundida com correção semântica.
**Limite:** esta matriz é um gate incremental. Não declara completude semântica do Z80, compatibilidade universal com clones nem precisão temporal.

## 1. Gate executável de classificação

O teste `native/sms_core/tests/z80_semantic_matrix.rs` enumera 7 espaços × 256 bytes = **1.792 codificações**:

| Espaço | Classificação explícita |
|---|---|
| Base | Instrução base ou prefixo CB/DD/ED/FD |
| CB | Família CB atribuída |
| ED | Instrução ED atribuída ou fallback para byte não atribuído |
| DD / FD | Candidato à semântica IX/IY, prefixo aninhado, ou prefixo ignorável conforme o decoder |
| DD-CB / FD-CB | Família indexed-CB |

A matriz contabiliza **78 bytes ED atribuídos** e **178 bytes ED não atribuídos**, para os quais o runtime atual segue o fallback ED-NOP. O teste também executa um passo para cada codificação em cada espaço e exige que a CPU retorne T-states agregados positivos sem `UnsupportedOpcode`.

**Interpretação obrigatória:** esse gate prova enumeração, classificação conforme o inventário e ausência de falha de dispatch nos estados de teste escolhidos. Não prova que a instrução produziu os registradores, flags, efeitos de memória/I/O ou T-states corretos. Em particular, os espaços DD/FD incluem instruções que ignoram o prefixo; o candidato não é automaticamente uma nova operação indexada.

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
| IX/IY, deslocamentos e H/L | `z80_indexed_semantics.rs`, `z80_expanded_opcode_matrix.rs` | PARCIAL | Matriz completa de substituição, exceções H/L com (IX/IY+d), deslocamentos extremos e prefixos repetidos |
| DD-CB / FD-CB | `z80_indexed_semantics.rs`, `z80_expanded_opcode_matrix.rs` | PARCIAL | Todas as 256 formas por prefixo, destino de registrador, flags e WZ; o smoke gate não basta |
| ED: aritmética de 16 bits e transferências absolutas | `z80_singlestep_reference_vectors.rs`, `z80_expanded_opcode_matrix.rs` | PARCIAL | Cada forma e par de registradores, carry/half-carry/overflow e wraparound |
| ED: I/R, LD A,I/R, IM e RETN/RETI | `z80_interrupt_semantics.rs`, `z80_singlestep_reference_vectors.rs` | DIRECIONADA | Interações de interrupção e estado interno em todas as fronteiras |
| LDI/LDD/LDIR/LDDR e CPI/CPD/CPIR/CPDR | `z80_ed_block_transfer_compare_boundaries.rs`, `z80_singlestep_reference_vectors.rs` | SHORT_PASS no escopo exercitado | Estados iniciais exaustivos e confirmação independente de flags não documentadas |
| INI/IND/OUTI/OUTD e formas repetitivas | `z80_ed_block_io_reference_vectors.rs` | SHORT_PASS no escopo exercitado | Independência do oráculo e sequência temporal real de portas |
| INT IM 0/1/2 | `z80_interrupt_semantics.rs` | DIRECIONADA | Todos os bytes fornecidos pelo barramento, fronteiras EI e sequência ordenada de acknowledge |
| NMI, HALT, EI/DI | `z80_nmi_halt_semantics.rs`, `z80_interrupt_semantics.rs` | DIRECIONADA | Interleavings de requests, HALT com R, aceitação imediatamente antes/depois de EI |
| Flags X/Y, Q/P, WZ/MEMPTR e R | Vários testes de referência e interrupção acima | PARCIAL / dependente da família | Especificar cada regra e a proveniência; não extrapolar um vetor para todas as instruções |
| Prefixos repetidos e aliases não documentados | Gate de classificação + decoder | CLASSIFICAÇÃO, NÃO SEMÂNTICA | Fixar comportamento de cada combinação com documentação ou oráculo identificado |
| T-states nominais | Vetores de referência e retornos de `step()` | PARCIAL | Ampliar por forma e condição tomada/não tomada |
| M-cycles, traces de barramento e T-states ordenados | Nenhum trace completo validado no contrato atual | PENDENTE | Definir trace, eventos e waits; não alegar ciclo a ciclo |

## 3. Regras para alterar o estado da matriz

Uma linha só pode avançar de **PARCIAL** para **DIRECIONADA** quando houver ao menos um teste que compara explicitamente estado inicial, estado final, flags, PC/SP, efeitos de memória/I/O pertinentes e T-states nominais. Um teste que apenas chama `step()` não atende esse critério.

Uma linha só pode receber **SHORT_PASS** quando o conjunto de testes direcionados passar no SHA candidato exato e os limites declarados estiverem registrados. **LONG_PASS** exige uma campanha prolongada realmente executada. **TEMPORAL_PASS** exige comparação de traces temporais ordenados contra evidência adequada; não pode ser inferido de T-states agregados.

Cada novo vetor externo deve registrar URL, commit, licença e proveniência. O corpus SingleStepTests/Z80 está fixado em `ebe1875d48f374bcfd4b505d8eb8ee751568b5f7`, mas seu README informa ancestralidade no núcleo Z80 do Ares. Ele é útil para regressão diferencial, não um oráculo independente do Ares.

## 4. Aceitação do gate desta PR

- [x] As sete áreas de opcode estão enumeradas explicitamente.
- [x] Cada área percorre todos os 256 bytes.
- [x] Os bytes ED atribuídos e o fallback são contados separadamente.
- [x] O smoke gate exige retorno sem `UnsupportedOpcode` e T-states agregados positivos.
- [x] A documentação separa classificação, evidência semântica e precisão temporal.
- [ ] Testes executados no SHA candidato — executar via GitHub Actions antes de merge.
- [ ] Revisão independente da classificação ED e da matriz documental.
- [ ] Fechamento das lacunas semânticas remanescentes — trabalho posterior, não declarado concluído por esta PR.
