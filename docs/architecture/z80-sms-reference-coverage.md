# Matriz de cobertura de referência — Z80 do SMS

## Baseline

- Repositório: `leonardo201800478/sega-emulator`
- Base desta auditoria: `53db203732524e824e670073772cb4cb082937a9`
- Corpus independente: [SingleStepTests/Z80](https://github.com/SingleStepTests/z80/tree/ebe1875d48f374bcfd4b505d8eb8ee751568b5f7), commit fixado `ebe1875d48f374bcfd4b505d8eb8ee751568b5f7`, licença MIT.
- Suíte `native/sms_core/tests/z80_singlestep_reference_vectors.rs`: 108 testes de vetor. O candidato da PR #168 passou nos workflows Rust e SMS com 108 testes aprovados, nenhum ignorado.
- Este documento separa explicitamente **classificação/execução sem erro** de **correção semântica comparada a um oráculo**.

## Níveis de evidência

| Nível | O que o teste verifica | O que não prova |
|---|---|---|
| Classificação | O decoder executa sem retornar `UnsupportedOpcode`/erro para o byte testado. | Flags, memória, registradores internos ou T-states corretos. |
| Vetor semântico | Estado final comparado ao estado fixo do SingleStepTests. | Cobertura de outros estados/valores para a mesma instrução. |
| Efeito de memória/I/O | Bytes de RAM e valores/endereço de porta listados pelo vetor são comparados. | Efeitos não listados no vetor ou ordenação temporal completa do barramento. |
| Temporização agregada | O total nominal de T-states é comparado. | M-cycles, waits, contensão ou eventos ordenados em cada T-state. |

## Matriz por espaço de opcode

| Espaço | Classificação exercitada | Vetores semânticos independentes observados | Lacuna principal |
|---|---|---|---|
| Base `00–FF` | Todos os 256 bytes passam pelo teste `every_primary_and_prefixed_encoding_is_classified_without_fallback_errors`. | NOP; loads; INC/DEC; aritmética; saltos relativos/absolutos/condicionais; CALL/RET/RST; pilha; troca de bancos; flags e `DAA`; loads indiretos; `LD (nn),A`; `EX (SP),HL`; `JP (HL)`; `LD SP,HL`; `DJNZ`. | A maioria dos 256 opcodes ainda não tem vários vetores por condição, limite e valor de entrada. |
| `CB` | Os 256 segundos bytes são executados no teste de classificação. | `RLC B`, `RLC (HL)`, `BIT 7,(HL)`, `BIT 0,(HL)`, `RES 0,(HL)`. | Cobertura semântica esparsa para RLC/RRC/RL/RR/SLA/SRA/SLL/SRL, BIT, RES e SET em todos os destinos. |
| `ED` | Os 256 segundos bytes são executados no teste de classificação. | SBC/ADC HL; loads indiretos de pares; NEG; RETN/RETI; IM 0/1/2; loads de I/R; LDI/LDD/CPI/CPD; LDIR/LDDR/CPIR/CPDR; INI/IND/OUTI/OUTD e variantes repetidas; IN/OUT por C. | Muitas formas ED, incluindo aliases e opcodes não documentados, ainda carecem de vetores de referência. |
| `DD`/`FD` | Os 256 bytes seguintes a cada prefixo são executados no teste de classificação. | `LD IX/IY,nn`; `LD A,(IX+d)`; `LD (IX+d),n`; indexed-CB e casos de BIT. | Instruções indexadas de 16 bits, substituições de H/L, prefixos repetidos e combinações de prefixos precisam de mais vetores semânticos. |
| `DD CB d op`/`FD CB d op` | As 256 codificações de cada prefixo indexado são exercitadas no teste `every_indexed_cb_encoding_is_classified_for_ix_and_iy`. | Quatro vetores independentes BIT em `(IX+d)`/`(IY+d)`, incluindo WZ e flags XY. | Rotates/shifts, RES e SET com escrita de memória e cópia opcional para registrador ainda precisam de vetores semânticos. |

## Estado interno e interrupções

| Área | Evidência existente | Trabalho restante |
|---|---|---|
| Flags e latch Q | Vetores de flags, `EX AF,AF'`, `POP AF`, `SCF/CCF`, `DAA`, `CPIR` e instruções de bloco. | Matriz sistemática para flags documentadas e não documentadas, inclusive sequências de instruções dependentes de Q. |
| WZ/MEMPTR | Vetores que revelaram e corrigiram WZ em `LD (nn),A`, `EX (SP),HL`, `JP cc,nn`, `CALL cc,nn`, `RST`, block-repeat e BIT indexado. | Auditar WZ por família de opcode, condições tomadas/não tomadas, wraparound e efeitos dos prefixos. |
| Interrupções | Testes unitários para NMI, INT IM0/1/2, atraso de EI, IFF1/IFF2, HALT e RETN/RETI. Vetores de instrução cobrem IM e RETI. | Oráculo independente para aceitação de interrupção, estados de borda, ordenação de leitura/escrita e T-states da entrada. |
| Refresh R | Testes unitários de incremento de R, bit 7 e fetches M1. | Vetores independentes por prefixos repetidos, interrupções e wraparound de R. |
| Memória e I/O | Vetores que verificam bytes finais de RAM e portas de entrada/saída listadas no corpus. | Aumentar os vetores para endereços espelhados, wraparound, operações simultâneas e todas as famílias de I/O. |

## Correções encontradas pela auditoria

As seguintes divergências foram reproduzidas contra vetores fixos e corrigidas nas PRs #161, #163, #164 e #165:

1. `LD (nn),A`: WZ/MEMPTR incorreto.
2. `EX (SP),HL`: WZ/MEMPTR não recebia a palavra trocada.
3. `JP cc,nn` e `CALL cc,nn`: WZ não era atualizado para o alvo em todos os caminhos.
4. `RST`: WZ não era atualizado para o vetor.
5. `POP AF`: o latch Q era atualizado indevidamente.
6. `CPIR/CPDR` e demais instruções de bloco repetidas: flags X/Y dos T-states adicionais vinham da fonte incorreta.
7. Instruções de bloco de I/O repetidas: PV/H não refletiam corretamente os efeitos adicionais de repetição.

O vetor CPIR anteriormente ignorado foi reativado. No SHA candidato da PR #164, os 89 vetores passaram sem ignorados; os incrementos subsequentes elevaram a suíte a 108 vetores aprovados no candidato da PR #168.

## Próximos passos de auditoria

1. Construir vetores semânticos para todas as 256 instruções base, com mais de um estado por opcode quando houver condições, flags ou efeitos de memória.
2. Cobrir todas as 256 instruções CB, ED e indexed-CB com oráculos independentes, separando instruções documentadas, aliases e comportamentos não documentados.
3. Ampliar os vetores de interrupção, EI delay, HALT, R, IFF1/IFF2 e retorno de interrupção.
4. Adicionar um modelo de trace de barramento com eventos ordenados e timestamps antes de afirmar precisão em nível M-cycle/T-state.
5. Manter as regressões atuais e executar `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings` e `cargo test --workspace --locked` no SHA candidato de cada PR.

## Conclusão

A maior parte do trabalho de CPU/ALU que existia no SG-1000 já estava presente no SMS. A mescla correta preservou os aprimoramentos específicos do SMS e usou vetores independentes para identificar falhas reais. Os 108 vetores aprovados são um avanço verificável, mas **não** equivalem a cobertura semântica completa nem a precisão ciclo a ciclo.
