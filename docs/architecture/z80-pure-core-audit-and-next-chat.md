# Auditoria do Z80 puro — estado, evidências e continuidade

**Base inspecionada originalmente:** `main` / commit `9a0d424cf610d8cecb4818b1a9bc9fbc4572dbef` (merge da PR #226). **Estado corrente atualizado em seção posterior:** `main` / `a662b7256ed5b92d06fd33875e4031b932e25945`.
**Escopo desta auditoria:** CPU Z80 implementada em Rust no `native/sms_core`, sua ALU, contrato mínimo de barramento, testes unitários/diferenciais e instrumentação de trace. Não avaliar a prontidão da máquina SMS nem bloquear a auditoria da CPU por dispositivos SMS ainda não implementados.
**Natureza:** auditoria estática do código remoto e das evidências de CI já disponíveis no histórico. Não afirmar que todos os comandos foram executados localmente nesta sessão.

## 1. Decisão de escopo

“Z80 puro” significa testar a CPU isolada de VDP, PSG, mapper SMS, portas específicas do SMS e scheduler da máquina. Os testes podem fornecer um barramento de teste determinístico com memória e I/O simulados. A CPU não deve depender de dispositivos da máquina para demonstrar semântica de instrução.

O barramento é relevante somente como contrato de observação e para verificar chamadas/ordem de acesso da CPU. O trace M-cycle-level recentemente adicionado é parte da CPU/barramento e deve ser validado separadamente de qualquer integração de hardware.

## 2. Arquivos e componentes examinados

- `native/sms_core/src/cpu.rs`: decoder/executor Z80 ativo no SMS, estado dos registradores, flags, prefixos, interrupções, contagem nominal de T-states e trace inicial.
- `native/sms_core/src/alu.rs`: operações aritméticas/lógicas auxiliares da CPU.
- `native/sms_core/src/bus.rs`: contrato `CpuBus` e eventos observáveis.
- `native/sms_core/tests/z80_ed_block_temporal_trace.rs`: testes de trace das oito instruções ED de transferência/comparação.
- `docs/architecture/z80-opcode-space-completion-audit.md`: inventário semântico anterior, cujo cabeçalho cita outra base; seus números são contexto histórico e devem ser reconciliados com o código no SHA atual antes de tratá-los como contagem atual.
- `docs/architecture/z80-temporal-bus-model.md`: contrato temporal e limites conhecidos.
- `docs/architecture/z80-coverage-gap-model.md`: matriz histórica de lacunas e critérios de aceitação; parte dela refere-se ao runtime Python legado e a branches/SHA anteriores, portanto não é evidência direta do runtime Rust atual.

Links de código na base auditada:
- [CPU](https://github.com/leonardo201800478/sega-emulator/blob/9a0d424cf610d8cecb4818b1a9bc9fbc4572dbef/native/sms_core/src/cpu.rs)
- [ALU](https://github.com/leonardo201800478/sega-emulator/blob/9a0d424cf610d8cecb4818b1a9bc9fbc4572dbef/native/sms_core/src/alu.rs)
- [Barramento Z80](https://github.com/leonardo201800478/sega-emulator/blob/9a0d424cf610d8cecb4818b1a9bc9fbc4572dbef/native/sms_core/src/bus.rs)
- [Testes de trace ED](https://github.com/leonardo201800478/sega-emulator/blob/9a0d424cf610d8cecb4818b1a9bc9fbc4572dbef/native/sms_core/tests/z80_ed_block_temporal_trace.rs)

## 3. O que está implementado ou já tem evidência

### 3.1 Runtime e decoder

- O núcleo SMS contém um executor Z80 Rust consolidado, com famílias de instruções base e espaços de prefixo CB, ED, DD/FD e DDCB/FDCB.
- A CPU mantém estado além dos registradores primários, incluindo registradores alternativos, IX/IY, I/R, IFF1/IFF2, modo de interrupção, HALT e estado auxiliar usado para comportamentos como WZ/MEMPTR e flags dependentes do histórico.
- O contrato da CPU retorna T-states nominais por instrução. Isso permite testar duração agregada, mas não prova os instantes reais de cada acesso ao barramento.
- Há testes direcionados para diversas famílias e casos de borda. A existência de um caminho de decodificação para um byte não prova que todas as combinações de estado de entrada estejam corretas.

### 3.2 Trace temporal inicial

- Foi adicionado observador opcional de eventos ao contrato do barramento.
- A primeira fatia registra eventos em nível de M-cycle para LDI, LDD, LDIR, LDDR, CPI, CPD, CPIR e CPDR, sem prefixos DD/FD.
- A suíte dedicada inclui casos de terminação por BC chegando a zero, término de CPIR por igualdade e wraparound de BC=0 para FFFF quando a repetição prossegue.
- A documentação delimita corretamente que não há amostras completas de pinos por T-state, modelagem completa de WAIT/refresh, integração com scheduler nem validação temporal de todo o conjunto de instruções.

### 3.3 Evidências de CI pré-merge da PR #226

No SHA candidato `1f941122ae6763caff4a46b8523af154ef742623`, os seguintes workflows foram registrados como aprovados:
- SMS Core Fast Validation — run [40780745233](https://github.com/leonardo201800478/sega-emulator/actions/runs/40780745233).
- CI — run [40780745373](https://github.com/leonardo201800478/sega-emulator/actions/runs/40780745373).
- SMS Z80 Platform Validation — run [40780745207](https://github.com/leonardo201800478/sega-emulator/actions/runs/40780745207).
- Rust SG-1000 Workspace — run [40780745212](https://github.com/leonardo201800478/sega-emulator/actions/runs/40780745212).
- SG-1000 — Three Agent Sequential Validation — run [40780745369](https://github.com/leonardo201800478/sega-emulator/actions/runs/40780745369).

Esses são resultados do SHA candidato da PR, não uma alegação de que todos os checks foram novamente executados no merge commit. Para o merge SHA `9a0d424cf610d8cecb4818b1a9bc9fbc4572dbef`, a consulta realizada nesta auditoria não retornou workflow runs associados nem status combinados; logo, os checks pós-merge ficam **NOT VERIFIED** nesta rodada. Não inferir PASS ou FAIL a partir da ausência de resultados.

O check Sourcery da PR foi ignorado/skipped; não deve ser contado como aprovação nem como revisão humana.

## 4. Limites e melhorias prioritárias

### P0 — fechar a validade semântica por família

1. Gerar uma matriz versionada dos 256 bytes do espaço base, CB, ED, DD, FD, DDCB e FDCB.
2. Para cada codificação, classificar: instrução documentada, alias, prefixo ignorado, operação não atribuída/ED-NOP, ou comportamento específico de variante. Não contar “executou sem erro” como semântica correta.
3. Ligar cada caso a expectativas de estado: registradores, flags S/Z/Y/H/X/PV/N/C, PC/SP, WZ/MEMPTR, Q/P quando aplicável, R, memória, I/O e T-states nominais.
4. Identificar instruções com fallback intencional e testar o fallback explicitamente; detectar quedas acidentais para o decoder base em instruções sensíveis a IX/IY.
5. Cobrir os casos de H/L versus IXH/IXL/IYH/IYL, especialmente exceções envolvendo `(IX/IY+d)`.

### P0 — vetores diferenciais confiáveis

1. Auditar procedência, SHA, esquema e licença do corpus externo fixado; preservar a versão e os hashes usados.
2. Medir cobertura por família e condição, não apenas quantidade total de vetores.
3. Comparar campo a campo e reportar o primeiro desvio com opcode, estado inicial, estado esperado/obtido e endereço.
4. Classificar a independência do oráculo. O corpus SingleStepTests tem ancestralidade no núcleo Z80 do Ares; é útil, mas não é prova independente de hardware.
5. Acrescentar vetores para todos os modos de interrupção, atraso de EI, NMI/INT, HALT, wraparound, pilha, flags e estados ocultos em sequências curtas.
6. Manter fixtures pequenos e legalmente obtidos; não adicionar ROMs comerciais.

### P1 — semântica das flags e estado oculto

- Varredura sistemática das operações ALU, DAA, INC/DEC, aritmética 16-bit, rotações/shifts, BIT, RLD/RRD e instruções indexadas.
- Verificar preservação/alteração de cada flag, incluindo X/Y não documentadas, carry de entrada, half-carry, overflow/paridade e N.
- Validar WZ/MEMPTR, Q, estado P, refresh R e registradores alternativos em sequências de instruções, não apenas em execuções isoladas.
- Separar fatos documentados, observações de oráculo, inferências e hipóteses específicas de variante.

### P1 — interrupções e instruções de bloco

- Verificar acceptance boundary de INT/NMI, EI delay, IFF1/IFF2, modos IM0/1/2, acknowledge, stack e refresh.
- Cobrir LDI/LDD/LDIR/LDDR, CPI/CPD/CPIR/CPDR e também I/O em bloco INI/IND/INIR/INDR/OUTI/OUTD/OTIR/OTDR.
- Para instruções repetitivas, testar cada iteração, BC=0, igualdade, término por flags, wraparound e efeito de interrupções entre iterações.

### P1 — temporização e trace do barramento

- Não declarar precisão ciclo a ciclo com base na soma nominal de T-states.
- Expandir gradualmente o trace: fetch/M1, leituras/escritas, I/O, acknowledge, refresh, posição temporal, endereço/dados opcionais e waits, quando sustentados por documentação.
- A primeira fase de oito instruções ED precisa ser confrontada com o corpus fixado, mas esse corpus tem ancestralidade comum e não constitui prova física.
- Adicionar testes que comprovem que ativar/desativar o observador não muda o estado, o resultado ou a duração nominal da CPU.
- Testar continuidade temporal, ausência de eventos duplicados, wraparound e ordenação determinística.

### P2 — processo, cobertura e rastreabilidade

- Reconciliar os documentos históricos cujo SHA de auditoria difere da base atual.
- Gerar relatório reproduzível de cobertura de opcode a partir dos testes realmente executados.
- Distinguir estados `IMPLEMENTED`, `QUICK_PASS`, `SHORT_PASS`, `LONG_PASS`, `INTEGRATION_PENDING` e `TEMPORAL_PASS`.
- Fazer cada PR pequena, com critérios de aceitação explícitos e checks relevantes no SHA candidato exato.
- Não reescrever a CPU nem extrair um crate compartilhado apenas por preferência; exigir benefício demonstrável e regressões equivalentes.

## 5. Critérios de aceitação para declarar a CPU Z80 pura validada

A validação do Z80 puro pode avançar independentemente da máquina SMS, usando barramento de teste. Para declarar uma etapa aprovada, exigir:
1. matriz de codificações e aliases versionada;
2. testes semânticos por família, não só smoke tests;
3. vetores diferenciais com provenance/SHA e resultado campo a campo;
4. cobertura explícita de flags, estados ocultos, interrupções e wraparound;
5. testes determinísticos de memória/I/O e da ordem de acesso observável;
6. fmt, Clippy e testes Rust pertinentes no SHA candidato;
7. status dos workflows do SHA exato documentado como PASS/FAIL/BLOCKED/NOT RUN;
8. limitações remanescentes registradas; nunca chamar de TEMPORAL_PASS sem validação de trace temporal apropriada.

## 6. Estado desta auditoria e inventário atualizado (2026-10-28)

### Base e CI

- **Base remota confirmada:** `main` em `9a0d424cf610d8cecb4818b1a9bc9fbc4572dbef`.
- **PASS no SHA exato:** [CI geral](https://github.com/leonardo201800478/sega-emulator/actions/runs/40781212247), [SMS Z80 Platform Validation](https://github.com/leonardo201800478/sega-emulator/actions/runs/40781212283), [SMS Core Fast Validation](https://github.com/leonardo201800478/sega-emulator/actions/runs/40781212259), [Rust SG-1000 Workspace](https://github.com/leonardo201800478/sega-emulator/actions/runs/40781212268) e [SG-1000 — Three Agent Sequential Validation](https://github.com/leonardo201800478/sega-emulator/actions/runs/40781212255).
- Os checks consultados no SHA exato incluem Rust formatting, Workspace Rust tests, Workspace Clippy e Pinned external Z80 conformance suite; todos retornaram `success`.
- **Limite:** esses resultados validam os checks executados, não a conformidade semântica completa do Z80. Não foram executados comandos locais nesta sessão.
- **IMPLEMENTED:** executor Z80 Rust consolidado em `native/sms_core`; não criar um runtime duplicado.
- **TEMPORAL_PASS:** não declarado.

### Inventário real da matriz versionada

O arquivo `docs/architecture/z80-sms-opcode-matrix.csv` no SHA auditado contém 1.803 registros: 256 para cada um dos sete espaços BASE, CB, ED, DD, FD, DDCB e FDCB, mais 11 sequências de prefixos. Os rótulos atuais de evidência semântica totalizam:

| Rótulo da matriz | Registros |
|---|---:|
| `exhaustive_internal_model` | 376 |
| `sparse_selected_vectors` | 365 |
| `sparse_reference_vectors` | 1.008 |
| Outras categorias | 54 |
| `temporal_evidence = NOT_VERIFIED` | 1.803 |

A matriz ED classifica 78 bytes como atribuídos e 178 como fallback ED-NOP. A matriz semântica descreve a cobertura dos grupos como parcial/direcionada em várias famílias. Esses rótulos são inventário de evidência, não certificação de comportamento correto. A presença de um vetor para uma codificação não garante cobertura dos dois caminhos condicionais, dos limites de estado ou de todos os efeitos laterais.

### Lacuna prioritária identificada

A primeira fatia de maior impacto proposta é a **cobertura dos caminhos condicionais de controle de fluxo**. Na matriz, `DJNZ`, `JR NZ/Z/NC/C` e as famílias condicionais `JP`, `CALL` e `RET` estão marcadas com evidência esparsa. Isso demonstra uma lacuna de evidência, não um defeito funcional já confirmado.

Critérios de aceitação antes de modificar o executor:

1. Cobrir condição tomada e não tomada para cada condição de `JR`, `JP`, `CALL` e `RET`, além dos dois caminhos de `DJNZ`.
2. Comparar `PC`, `SP`, flags preservadas, conteúdo/ordem da pilha e T-states nominais.
3. Cobrir deslocamentos relativos `-128`, `-1`, `0`, `+1`, `+127` e wraparound de PC.
4. Cobrir SP próximo de `0000` e `FFFF` em CALL/RET, incluindo wraparound e ordem de bytes.
5. Usar expectativas fixas, documentar procedência dos vetores externos e separar resultados de modelos internos dos resultados de oráculos externos. SingleStepTests/Z80 tem ancestralidade no Ares e não é prova independente de hardware.
6. Rodar testes direcionados e os checks Rust pertinentes no SHA candidato exato; não transferir resultados de SHAs anteriores.

Próxima implementação recomendada: começar com testes determinísticos para os caminhos condicionais e os limites de PC/SP, sem alterar o decoder até que uma falha reproduzível seja demonstrada. Registrar primeiro a lacuna e a aceitação; depois, implementar uma correção mínima somente se os testes revelarem divergência.

### Execução da primeira fatia de testes

A PR [#229 — testes de caminhos condicionais e limites](https://github.com/leonardo201800478/sega-emulator/pull/229) foi aberta sem alterações no runtime, a partir de `main` em `9a0d424cf610d8cecb4818b1a9bc9fbc4572dbef`. HEAD candidato consultado: `78553c9f6a78b408c14efc3d00ca0113ec580e42`.

- **PASS:** [SMS Core Fast Validation](https://github.com/leonardo201800478/sega-emulator/actions/runs/40790980771), incluindo formatação, lint e testes direcionados do núcleo SMS.
- **PASS:** [CI geral](https://github.com/leonardo201800478/sega-emulator/actions/runs/40790980752).
- **PASS:** Rust formatting, Workspace Clippy e Workspace Rust tests no workflow [SMS Z80 Platform Validation](https://github.com/leonardo201800478/sega-emulator/actions/runs/40790980785).
- **PASS:** suíte externa completa Z80 no mesmo workflow; o job também executou a suíte SCF/CCF. O log reportou 204 testes, 0 falhas, 88.340.467 instruções e 602.728.667 T-states para o teste CCF. Isso é evidência de execução dessa suíte, não prova independente de hardware.
- Os sete testes novos passaram na validação rápida. A revisão dos efeitos laterais revelou uma lacuna adicional em `RET cc`: quando a condição é verdadeira, o executor atualiza PC/SP mas não WZ/MEMPTR. A tabela externa de MEMPTR lista `RET Z` como atualizando MEMPTR com o valor de retorno da pilha quando Z é verdadeiro (https://github.com/redcode/Z80/wiki/MEMPTR; consulta em 2026-10-29). O manual Zilog documenta a operação arquitetural de RET cc, mas não especifica o latch não documentado.
- A PR #229 foi ampliada com uma regressão que inicializa WZ com sentinela e exige que WZ receba o destino quando RET cc é tomado e permaneça igual quando não é tomado. A correção mínima atribui `self.wz = self.registers.pc` apenas no caminho tomado.
- **PASS no SHA exato `b2576d1a418b79454bcd9be031508f9ecd957775`:** [SMS Core Fast Validation](https://github.com/leonardo201800478/sega-emulator/actions/runs/40791302843), [CI geral](https://github.com/leonardo201800478/sega-emulator/actions/runs/40791302820) e [SMS Z80 Platform Validation](https://github.com/leonardo201800478/sega-emulator/actions/runs/40791302967). A validação completa inclui formatação Rust, Clippy, testes do workspace e suíte externa Z80/SCF-CCF; o teste direcionado `conditional_ret_covers_both_paths_and_stack_read_order` passou.
- A PR permanece aberta; não houve merge.

## 7. Prompt para continuar em um novo chat

Use o prompt abaixo como instrução de retomada. Ele está intencionalmente restrito ao Z80 puro.

```text
Projeto: Sega Emulator — auditoria e validação do Z80 puro.

Repositório oficial: https://github.com/leonardo201800478/sega-emulator
Último SHA conhecido: 9a0d424cf610d8cecb4818b1a9bc9fbc4572dbef (merge da PR #226).
Documento de continuidade: docs/architecture/z80-pure-core-audit-and-next-chat.md.

OBJETIVO
Validar somente a CPU Z80 Rust isolada. NÃO tentar completar ou auditar a máquina SMS, VDP, PSG, mapper, frontend ou scheduler. Use um barramento de teste determinístico, mocks e vetores externos para testar CPU, ALU, flags, registradores, interrupções, memória/I/O observável, ciclos nominais e trace da CPU.

MÉTODO OBRIGATÓRIO
1. Consulte o GitHub antes de agir. Confirme branch main, SHA real, PRs abertas, árvore, histórico, arquivos, testes e workflows. O SHA acima é apenas ponto de partida, não uma suposição de HEAD atual.
2. Leia o documento de auditoria e as fontes/testes atuais. Reconcilie documentação que menciona SHAs anteriores.
3. Inventarie os espaços base, CB, ED, DD, FD, DDCB e FDCB e classifique cada codificação como instrução, alias, prefixo ignorado, fallback intencional ou lacuna.
4. Defina critérios de aceitação antes de alterar código. Não reescreva o núcleo nem duplique implementações.
5. Priorize a cobertura semântica: registradores, flags S/Z/Y/H/X/PV/N/C, PC/SP, WZ/MEMPTR, Q/P, R, bancos alternativos, wraparound, memória/I/O, interrupções, HALT e instruções ED em bloco.
6. Use vetores fixos e um oráculo de procedência documentada. O corpus SingleStepTests/Z80 usado pelo projeto tem ancestralidade no Ares; não o descreva como evidência independente de hardware. Preserve SHA e hashes.
7. Para qualquer falha, registre o primeiro desvio: opcode, estado inicial, esperado, obtido, campo divergente e fixture mínima. Faça uma correção pequena e um teste de regressão por vez.
8. Avalie separadamente contagem nominal de T-states e trace M-cycle-level. Não declare precisão ciclo a ciclo por causa de contagens agregadas. O trace inicial cobre LDI/LDD/LDIR/LDDR/CPI/CPD/CPIR/CPDR sem DD/FD; waits, refresh/pinos por T-state e validação temporal ampla continuam pendentes.
9. Execute os checks Rust pertinentes: cargo fmt --all -- --check; cargo clippy --workspace --all-targets --locked -- -D warnings; cargo test --workspace --locked. Confirme se o workspace inclui o crate testado; execute explicitamente testes específicos que possam estar fora dele. Não diga que executou algo sem evidência.
10. Verifique os workflows no SHA candidato exato. PASS, FAIL, BLOCKED e NOT RUN devem ser separados; checks ausentes/skipped não são PASS.
11. Use branch e PR para alterações não triviais. Nunca edite main diretamente, faça force-push, resete mudanças destrutivamente ou faça merge sem autorização e critérios cumpridos.
12. Mantenha a máquina SMS fora do escopo. A falta de VDP/PSG/scheduler não pode impedir testes isolados da CPU.
13. Atualize este documento ou a documentação dedicada com o que foi implementado, testes, resultados, SHA, evidências, limitações e próximo passo.

ESTADOS DE VALIDAÇÃO
Use IMPLEMENTED, QUICK_PASS, SHORT_PASS, LONG_PASS, INTEGRATION_PENDING e TEMPORAL_PASS. Não use TEMPORAL_PASS sem evidência adequada de traces temporais.

PRÓXIMO PASSO
Após verificar o HEAD remoto, inventarie os testes de CPU/ALU e os vetores disponíveis no SHA atual; produza uma matriz de cobertura real por família/opcode/condição e identifique a primeira lacuna de maior impacto. Não faça mudanças antes de mostrar os critérios de aceitação e a evidência da lacuna.
```


## Atualização de continuidade — pós-merge #227–#229

A base atual desta auditoria é `main` no SHA `a662b7256ed5b92d06fd33875e4031b932e25945`, não o SHA histórico indicado na primeira versão do documento. Os links para código versionado nas seções anteriores são snapshots históricos; para decisões atuais, abrir os mesmos caminhos em `main` ou fixar o SHA acima.



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


**Próximo passo:** inventariar o estado real do executor e os testes por família, procurar uma lacuna reproduzível de maior impacto (incluindo WZ/MEMPTR, Q/P, flags e interrupções), definir critérios de aceitação e somente então alterar o runtime. Não duplicar o executor em `native/z80_core`; confirmar seu papel isolado antes de qualquer mudança.
