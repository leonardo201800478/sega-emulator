# Matriz de cobertura de referência — Z80 do SMS

## Baseline

- Repositório: `leonardo201800478/sega-emulator`
- Base desta auditoria: `53db203732524e824e670073772cb4cb082937a9`
- Corpus externo de referência: [SingleStepTests/Z80](https://github.com/SingleStepTests/z80/tree/ebe1875d48f374bcfd4b505d8eb8ee751568b5f7), commit fixado `ebe1875d48f374bcfd4b505d8eb8ee751568b5f7`, licença MIT. **Não é um oráculo independente de Ares**: o README upstream declara que os vetores foram criados traduzindo o núcleo Z80 do Ares e corrigindo alguns bugs.
- Suíte de referência Rust: **172 vetores** — 118 vetores gerais em `z80_singlestep_reference_vectors.rs` e 54 vetores de I/O em bloco em `z80_ed_block_io_reference_vectors.rs` (16 da PR #184, 14 da PR #186 e 24 da PR #187). Os 16 novos vetores passaram sem falhas no SHA integrado `5f39196967a0603185d2dd886711682319f1e381`.
- Este documento separa explicitamente **classificação/execução sem erro** de **correção semântica comparada a um oráculo**.

## Atualização de validação — 2026-10-28 (PR #184)

- Integrados 8 vetores `ED B2` (INIR) e 8 vetores `ED BB` (OTDR) do corpus fixado SingleStepTests/Z80.
- Os vetores comparam estado final dos registradores principais e alternativos, flags, `WZ/MEMPTR`, `R`, `P/Q`, IFF1/IFF2, IM, atraso de EI, RAM alterada, portas de I/O e contagem agregada de T-states.
- Resultado no SHA de merge `5f39196967a0603185d2dd886711682319f1e381`: 16/16 vetores iniciais passaram; CI, SMS Core Fast Validation e SMS Z80 Platform Validation passaram. A PR #186 adiciona mais 14 vetores de fronteira para `B=0`, `L=00/FF`, `H=00/FF` e `F=00/FF`; os 14/14 passaram no SHA candidato `4ea8675f824a30ea34947fa51dfc4b47c349ed91`, com formatação, Clippy, testes do workspace, testes focados SMS e qualidade verdes.
- Os testes usam o comprimento do traço externo como total de T-states, mas não comparam eventos de barramento ordenados. Não constituem evidência de hardware nem cobertura combinatória completa.

## Ampliação de casos de fronteira — PR #186

- Adiciona 7 vetores para INIR (`ED B2`) e 7 para OTDR (`ED BB`) do mesmo corpus fixado.
- Casos selecionados cobrem `B=0` (decremento com wrap para `FF`), limites do byte baixo/alto de `HL` e valores extremos das flags de entrada.
- O objetivo é aumentar a diversidade de estado de entrada para flags e efeitos de memória/I/O, não alegar cobertura de todos os endereços de 16 bits nem de todas as combinações de flags.

## Cobertura ampliada das oito instruções de I/O em bloco — PR #187

- Adicionados 24 vetores do corpus fixado SingleStepTests/Z80: 4 para cada uma das instruções `INI` (`ED A2`), `OUTI` (`ED A3`), `IND` (`ED AA`), `OUTD` (`ED AB`), `OTIR` (`ED B3`) e `INDR` (`ED BA`).
- A seleção inclui estado ordinário, `B=0`, `L=FF` e `F=FF`, quando disponível na família. Os 24/24 vetores passaram no SHA candidato `e09967f12076366db13f5eea52178afca51af994`; os checks Rust, Clippy, formatação, SMS focado e qualidade concluíram com sucesso.
- O código foi integrado pela PR #187 no SHA `c226ddfccd6961ecf63d8751bc9edf10d2465a9b`. Os workflows disparados no SHA de merge devem ser acompanhados separadamente; o resultado do candidato não deve ser confundido com um resultado pós-merge.
- A cobertura de referência agora tem 172 vetores contabilizados (118 gerais + 16 iniciais + 14 de fronteira + 24 das demais instruções de I/O em bloco). A contagem descreve vetores versionados, não cobertura combinatória de todos os opcodes/estados.

## Proveniência e independência do corpus

O README do corpus SingleStepTests/Z80 no commit fixado informa que os vetores foram criados por tradução do núcleo Z80 do Ares, seguida de correções de bugs. Portanto, os vetores são externos ao repositório Sega Emulator e úteis para regressão/diferencial, mas **não contam como oráculo de implementação independente do Ares**. A linhagem comum reduz a força da evidência quando o comportamento comparado também veio do Ares. Fonte: [README no commit fixado](https://github.com/SingleStepTests/z80/blob/ebe1875d48f374bcfd4b505d8eb8ee751568b5f7/README.MD).

## Níveis de evidência

| Nível | O que o teste verifica | O que não prova |
|---|---|---|
| Classificação | O decoder executa sem retornar `UnsupportedOpcode`/erro para o byte testado. | Flags, memória, registradores internos ou T-states corretos. |
| Vetor semântico | Estado final comparado ao estado fixo do SingleStepTests. | Cobertura de outros estados/valores para a mesma instrução. |
| Efeito de memória/I/O | Bytes de RAM e valores/endereço de porta listados pelo vetor são comparados. | Efeitos não listados no vetor ou ordenação temporal completa do barramento. |
| Temporização agregada | O total nominal de T-states é comparado. | M-cycles, waits, contensão ou eventos ordenados em cada T-state. |

## Matriz por espaço de opcode

| Espaço | Classificação exercitada | Vetores semânticos de referência observados | Lacuna principal |
|---|---|---|---|
| Base `00–FF` | Todos os 256 bytes passam pelo teste `every_primary_and_prefixed_encoding_is_classified_without_fallback_errors`. | NOP; loads; INC/DEC; aritmética; saltos relativos/absolutos/condicionais; CALL/RET/RST; pilha; troca de bancos; flags e `DAA`; loads indiretos; `LD (nn),A`; `EX (SP),HL`; `JP (HL)`; `LD SP,HL`; `DJNZ`. | A maioria dos 256 opcodes ainda não tem vários vetores por condição, limite e valor de entrada. |
| `CB` | Os 256 segundos bytes são executados no teste de classificação. | `RLC B`, `RLC (HL)`, `BIT 7,(HL)`, `BIT 0,(HL)`, `RES 0,(HL)`. | Cobertura semântica esparsa para RLC/RRC/RL/RR/SLA/SRA/SLL/SRL, BIT, RES e SET em todos os destinos. |
| `ED` | Os 256 segundos bytes são executados no teste de classificação. | SBC/ADC HL; loads indiretos de pares; NEG; RETN/RETI; IM 0/1/2; loads de I/R; LDI/LDD/CPI/CPD; LDIR/LDDR/CPIR/CPDR; INI/IND/OUTI/OUTD e variantes repetidas; IN/OUT por C. Os vetores externos incluem oito estados adicionais de INIR e oito de OTDR; a suíte geral inclui dois vetores fixos para cada ED A0/A1/A8/A9/B0/B1/B8/B9. A PR #191 acrescenta 20 testes dirigidos para os oito opcodes, incluindo término por correspondência em CPDR com BC ainda diferente de zero e correspondência com BC inicial zero. O corpus fixado não tem vetor externo CPDR de término por correspondência entre seus 1000 estados, limitação registrada explicitamente na documentação específica do bloco. | Muitas formas ED e várias condições/estados por opcode ainda carecem de vetores; a validação não é combinatória. |
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


## Inventário completo de codificações — revisão de 2026-10-28

A matriz enumerada por codificação está em [z80-sms-opcode-matrix.csv](z80-sms-opcode-matrix.csv). O CSV contém **1.792 entradas de encoding fixo** (256 em cada espaço BASE, CB, ED, DD, FD, DDCB e FDCB), mais **11 classes de sequência de prefixos**; são 1.803 registros de dados.

### Como interpretar os estados

- `decoder path present; classification test only`: o encoding é reconhecido pelo caminho de decoder/classificação. Isso **não** certifica resultados, flags, efeitos de memória, PC/SP, WZ/MEMPTR, Q, R ou T-states.
- `exhaustive_internal_model`: a família ALU indicada tem varredura exaustiva por modelo matemático no candidato da PR #201; não equivale a oráculo físico.
- `selected_unit_and_reference_vectors` / `sparse_reference_vectors`: há testes selecionados, mas não cobertura semântica completa por opcode e estado.
- `NOT_VERIFIED` na coluna temporal significa que não há prova por eventos de barramento ordenados a cada T-state. A contagem nominal agregada não muda isso.
- Os resultados `z80full` e `z80ccf` são resultados agregados da suíte externa, sem atribuição individual de PASS por opcode no relatório do harness.
### Revisão de aceitação

- **Inventário dos espaços fixos:** completo como enumeração, 1.792 entradas, mais 11 classes de sequência.
- **Classificação individual:** aliases, encodings não atribuídos, instruções não documentadas e casos indexados estão identificados; a semântica de todas as entradas ainda não está demonstrada. ED não atribuído/aliases e contextos DD/FD exigem confirmação por evidência de alvo e testes de sequência.
- **Correção semântica global:** **NOT PASS**; não se infere da classificação nem dos resultados agregados da suíte externa.
- **Etapa 2 (ALU/flags): SHORT_PASS** no candidato da PR #201 `fca5adc0b4baef1c8acb6369d2d4a992405adb25`. Workflows: [CI/quality](https://github.com/leonardo201800478/sega-emulator/actions/runs/40740205428), [SMS Core Fast Validation](https://github.com/leonardo201800478/sega-emulator/actions/runs/40740205236), [SMS Z80 Platform Validation](https://github.com/leonardo201800478/sega-emulator/actions/runs/40740205501).
- **Etapas 3–10:** não concluídas nesta alteração.
- **TEMPORAL_PASS:** não declarado.

O CSV é uma base de auditoria e planejamento, não um certificado de completude semântica. Para concluir a auditoria global, cada família ainda precisa de vetores fixos por opcode/forma, estados de fronteira, referência/procedência e regressão no SHA candidato exato.


## Atualização — destinos DDCB/FDCB (PR #211, 2026-10-28)

- A PR #211 foi integrada no SHA `90b446d768b6c534487151457f3d56fa5d6d5a39`; arquivo de teste: `native/sms_core/tests/z80_indexed_cb_rotate_shift_destinations.rs`.
- A nova suíte percorre as oito operações rotate/shift (`RLC`, `RRC`, `RL`, `RR`, `SLA`, `SRA`, `SLL`, `SRL`), os dois prefixos `DD CB`/`FD CB` e todos os oito destinos codificados (`B,C,D,E,H,L,(IX/IY+d),A`). Os casos verificam valor escrito na memória, cópia para o registrador selecionado, flags, PC, R, WZ, T-states nominais e preservação do índice não selecionado.
- O teste usa os mesmos vetores fixos de resultado/flags já existentes em `z80_indexed_cb_rotate_shift_vectors.rs`; amplia a cobertura dos destinos, mas **não cria um oráculo independente novo** e **não valida a ordem dos eventos de barramento ciclo a ciclo**.
- Os testes já existentes em `z80_indexed_cb_bit_res_set_wz.rs` percorrem BIT/RES/SET, bits e destinos codificados para IX/IY, incluindo preservação de flags em RES/SET. Com a PR #211, a lacuna registrada anteriormente para destinos de rotate/shift indexados foi coberta por testes dirigidos.
- Checks no SHA candidato `2ec52ca7b2f169709bb5f5880fc5a5113a77abec`: CI/quality PASS; SMS Core Fast Validation PASS; SMS Z80 Platform Validation PASS; Rust formatting PASS; Workspace Clippy PASS; Workspace Rust tests PASS; suíte externa fixada de conformidade Z80 PASS. A checagem Sourcery foi SKIPPED por limite de uso e não conta como revisão aprovada.
- O merge não modifica código de produção. O SHA integrado de `main` é `90b446d768b6c534487151457f3d56fa5d6d5a39`.

### Limite do marco

Este avanço fecha a lacuna específica de destinos das operações rotate/shift indexadas, não a auditoria semântica global. A issue [#158 — matriz completa de opcodes e semântica](https://github.com/leonardo201800478/sega-emulator/issues/158) permanece aberta: faltam vetores semânticos para várias famílias/condições e estados de fronteira. A issue [#159 — interrupções, estado e temporalidade](https://github.com/leonardo201800478/sega-emulator/issues/159) também permanece aberta. `TEMPORAL_PASS` não é declarado, pois nenhum trace ordenado de barramento foi comparado ciclo a ciclo.
