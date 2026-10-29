# Matriz semântica executável do Z80 — SMS

**Base histórica da matriz:** branch `audit/z80-prefix-classification-matrix`, criada a partir de `280aabdb4a0567e5238da9bf1eb971ad5d594c85`. O estado corrente após PRs #227–#229 está registrado ao final.
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
| DD-CB / FD-CB | `z80_indexed_cb_semantics.rs`, `z80_indexed_cb_bit_res_set_wz.rs`, `z80_indexed_cb_reference_vectors.rs`, `z80_expanded_opcode_matrix.rs` | DIRECIONADA — BIT/RES/SET cobertos; vetores fixos de BIT indexado conferem flags X/Y com o byte alto do endereço efetivo e WZ com IX/IY+d | Semântica completa das famílias de rotação/shift, execução diferencial contra um segundo núcleo ou hardware e traces temporais; fontes documentais não equivalem a execução de oráculo independente |
| ED: aritmética de 16 bits e transferências absolutas | `z80_singlestep_reference_vectors.rs`, `z80_expanded_opcode_matrix.rs` | PARCIAL | Cada forma e par de registradores, carry/half-carry/overflow e wraparound |
| ED: I/R, LD A,I/R, IM e RETN/RETI | `z80_interrupt_semantics.rs`, `z80_singlestep_reference_vectors.rs` | DIRECIONADA | Interações de interrupção e estado interno em todas as fronteiras |
| LDI/LDD/LDIR/LDDR e CPI/CPD/CPIR/CPDR | `z80_ed_block_transfer_compare_boundaries.rs`, `z80_singlestep_reference_vectors.rs` | SHORT_PASS no escopo exercitado | Estados iniciais exaustivos e confirmação independente de flags não documentadas |
| INI/IND/OUTI/OUTD e formas repetitivas | `z80_ed_block_io_reference_vectors.rs` | SHORT_PASS no escopo exercitado | Independência do oráculo e sequência temporal real de portas |
| INT IM 0/1/2 | `z80_interrupt_semantics.rs` | DIRECIONADA | Todos os bytes fornecidos pelo barramento, fronteiras EI e sequência ordenada de acknowledge |
| NMI, HALT, EI/DI | `z80_nmi_halt_semantics.rs`, `z80_interrupt_semantics.rs` | DIRECIONADA | Interleavings de requests, HALT com R, aceitação imediatamente antes/depois de EI |
| Flags X/Y, Q/P, WZ/MEMPTR e R | `z80_indexed_cb_reference_vectors.rs` compara oito casos fixos de BIT indexado com valores esperados documentados externamente; `z80_indexed_cb_bit_res_set_wz.rs` amplia a matriz de estado | PARCIAL / dependente da família | Executar uma implementação de oráculo separada ou teste em hardware; ampliar a outros usos de WZ e variantes de CPU |
| Prefixos repetidos e aliases não documentados | `z80_indexed_prefix_wraparound.rs` + gate de classificação | DIRECIONADA em prefixos DD/DD e DD/FD | Fixar comportamento de cada combinação com documentação ou oráculo identificado |
| T-states nominais | Vetores de referência e retornos de `step()` | PARCIAL | Ampliar por forma e condição tomada/não tomada |
| M-cycles, traces de barramento e T-states ordenados | Nenhum trace completo validado no contrato atual | PENDENTE | Definir trace, eventos e waits; não alegar ciclo a ciclo |

## 3. Regras para alterar o estado da matriz

Uma linha só pode avançar de **PARCIAL** para **DIRECIONADA** quando houver ao menos um teste que compara explicitamente estado inicial, estado final, flags, PC/SP, efeitos de memória/I/O pertinentes e T-states nominais. Um teste que apenas chama `step()` não atende esse critério.

Uma linha só pode receber **SHORT_PASS** quando o conjunto de testes direcionados passar no SHA candidato exato e os limites declarados estiverem registrados. **LONG_PASS** exige uma campanha prolongada realmente executada. **TEMPORAL_PASS** exige comparação de traces temporais ordenados contra evidência adequada; não pode ser inferido de T-states agregados.

Cada novo vetor externo deve registrar URL, commit, licença e proveniência. O corpus SingleStepTests/Z80 está fixado em `ebe1875d48f374bcfd4b505d8eb8ee751568b5f7`, mas seu README informa ancestralidade no núcleo Z80 do Ares. Ele é útil para regressão diferencial, não um oráculo independente do Ares.

### Fontes para os vetores BIT indexados (sem importar código de terceiros)

- **Manual do fabricante:** Zilog, *Z80 CPU User Manual*, documento `UM008011-0816`, pp. 247–249; cópia de referência fornecida no projeto (`um0080.pdf`). Define BIT `b,(IX/IY+d)`, 20 T-states, H=1, N=0, C preservado e memória inalterada; declara S e P/V como desconhecidos. URL de referência: https://www.zilog.com/docs/z80/um0080.pdf.
- **Comportamento não documentado:** Sean Young, *The Undocumented Z80 Documented*, versão 0.4, §4.1: para BIT indexado, F3/F5 derivam dos bits 3/5 do byte alto do endereço efetivo; descreve também S, Z, H, P/V, N e C observados. URL: https://z80.info/zip/z80-documented.pdf. Licença explícita não verificada; nenhum texto ou código-fonte foi copiado para o runtime.
- **WZ/MEMPTR:** redcode/Z80 Wiki, página “MEMPTR”, revisão exibida em 2026-01-07; lista BIT/RES/SET indexados com WZ = XY + deslocamento. URL: https://github.com/redcode/Z80/wiki/MEMPTR. Hash de commit e licença da página wiki não expostos pela fonte consultada; usada como referência factual, não como código importado.
- **Limite da evidência:** os oito valores esperados são vetores fixos derivados dessas referências, não resultado de execução de outro emulador nem de CPU física. A confirmação diferencial executável contra um segundo núcleo independente continua pendente.

## 4. Aceitação desta atualização

- [x] Os sete espaços de opcode continuam enumerados integralmente.
- [x] DD/FD distinguem formas index-sensitive, exceções H/L, prefixos ignorados e prefixos aninhados.
- [x] Vetores dirigidos verificam deslocamentos assinados extremos, par indexado e registradores de índice altos/baixos.
- [x] Vetores DD-CB/FD-CB verificam efeito de memória, destino em registrador, flags e contadores arquiteturais em casos direcionados.
- [x] `z80_indexed_cb_bit_res_set_wz.rs` cobre BIT/RES/SET para os 8 índices de bit, os 8 campos de destino e DD/FD, com dados 00/FF; verifica destino de registrador, memória, flags, WZ, PC, R e T-states agregados.
- [x] `z80_indexed_cb_reference_vectors.rs` verifica oito resultados fixos para BIT indexado, cobrindo todas as combinações de F3/F5 no byte alto, ambos os prefixos, flags, WZ, PC, R, memória e T-states; fontes e limites estão registrados.
- [x] O runtime não foi alterado nesta atualização; qualquer correção futura depende de divergência reproduzível contra evidência independente.
- [x] Prefixos DD repetidos e sequência DD/FD verificam precedência do último prefixo, PC, R e T-states agregados.
- [x] Um vetor de deslocamento negativo verifica wraparound do endereço efetivo para FFFF.
- [x] CI, SMS Core Fast Validation e SMS Z80 Platform Validation passaram em um candidato anterior da PR #197; o status do SHA atual deve ser confirmado pelos checks vinculados à PR.
- [ ] Executar comparação diferencial com um segundo núcleo independente ou em hardware; a referência documental adicionada não satisfaz esse critério.
- [ ] Revisão independente das semânticas e contagens.
- [ ] Fechamento das lacunas remanescentes — não declarado concluído por esta atualização.


## Estado pós-merge — PRs #227 e #229

A matriz original continua sendo um gate incremental; os testes adicionados depois de sua criação não convertem a classificação de 1.792 codificações em prova de correção global. A PR #229 acrescenta cobertura explícita para condições tomadas/não tomadas em JR/JP/CALL/RET/DJNZ e limites de PC/SP, além da regressão para WZ em RET condicional. A PR #227 acrescenta observação temporal de uma seleção restrita de instruções ED.



## Estado consolidado após as PRs #227–#229 — 2026-10-29

**SHA de `main` verificado após os merges:** `a662b7256ed5b92d06fd33875e4031b932e25945`.  
**CI da `main` nesse SHA:** PASS — [workflow 40792001983](https://github.com/leonardo201800478/sega-emulator/actions/runs/40792001983).  
**Limite:** o CI verde comprova apenas os checks executados pelo workflow; não certifica completude global do Z80 nem fidelidade física/ciclo a ciclo.

### Mudanças integradas

- **PR #227**, candidato `d4447db5f0dce9a2ecde9e50c97f39b10ce8da17`, merge `28da159634aa60b9a804f3bd7f781ccf82f3e1cf`: trace por T-state para LDI/LDD/LDIR/LDDR e CPI/CPD/CPIR/CPDR sem prefixos DD/FD, além de trace M-cycle associado. Validação de plataforma no candidato: [run 40782245521](https://github.com/leonardo201800478/sega-emulator/actions/runs/40782245521) — PASS.
- **PR #229**, candidato `56cf4743bf0f05660f21a798de72e19afcfab34a`, merge `7b6fef3300c753c2b089f6b0dd287f8120b9f93f`: testes determinísticos para JR, JP, CALL, RET e DJNZ condicionais, caminhos tomados/não tomados, fronteiras de deslocamento, wraparound de PC/SP, ordem de acesso à pilha e T-states nominais; correção de `RET cc` para atualizar WZ/MEMPTR com o endereço de retorno somente quando a condição é tomada. [Fast Validation](https://github.com/leonardo201800478/sega-emulator/actions/runs/40791517040), [CI](https://github.com/leonardo201800478/sega-emulator/actions/runs/40791516982) e [SMS Z80 Platform Validation](https://github.com/leonardo201800478/sega-emulator/actions/runs/40791516943) — PASS no candidato exato.
- **PR #228**, candidato documental `a2c0850c20232c95a5210a39fbd53cc7aeb5101e`, merge `a662b7256ed5b92d06fd33875e4031b932e25945`: auditoria de continuidade incorporada; [CI documental](https://github.com/leonardo201800478/sega-emulator/actions/runs/40791436529) — PASS.

### Estado técnico depois dos merges

| Área | Estado | Limite/evidência |
|---|---|---|
| Executor Z80 Rust em `native/sms_core` | IMPLEMENTED | Caminho ativo consolidado; não significa que todos os estados de todas as instruções estejam provados. |
| Testes de controle de fluxo condicional | IMPLEMENTED; SHORT_PASS no candidato #229 | Cobertura adicionada às famílias JR/JP/CALL/RET/DJNZ e aos limites declarados; não é prova combinatória de todos os estados. |
| Trace ED de bloco | IMPLEMENTED; SHORT_PASS no candidato #227 | Restrito às oito instruções indicadas e ao perfil de observação documentado. |
| Classificação dos espaços de opcode | IMPLEMENTED | Enumeração/classificação não equivale à correção semântica integral. |
| Semântica global, flags não documentadas, Q/P, WZ, interrupções e aliases | INCOMPLETO / auditoria contínua | Exigir vetores discriminantes e proveniência explicitada por família. |
| Integração temporal com scheduler/dispositivos SMS | INTEGRATION_PENDING | A instrumentação atual não controla o scheduler nem representa todos os pinos/esperas. |
| Fidelidade ciclo a ciclo integral | TEMPORAL_PASS NÃO DECLARADO | Não há evidência para afirmar cobertura temporal completa ou equivalência física. |

### Proveniência e regras de interpretação

O corpus SingleStepTests/Z80 fixado em `ebe1875d48f374bcfd4b505d8eb8ee751568b5f7` tem ancestralidade no núcleo Z80 do Ares; é útil para regressão, mas não é um oráculo independente de hardware. Separar sempre comportamento descrito pelo manual ZiLOG, comportamento documentado por referências de terceiros, observação em emuladores e hipótese ainda não comprovada. Não promover um estado de validação com base apenas no número de opcodes enumerados ou no CI geral.


A próxima revisão desta matriz deve relacionar os novos arquivos de teste com as linhas/famílias correspondentes e separar PASS direcionado de cobertura total. A suíte SingleStepTests compartilha ancestralidade com Ares; buscar um segundo oráculo independente continua pendente.
