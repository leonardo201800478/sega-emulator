# Histórico consolidado da modelagem semântica do Z80 — SMS

**Data de corte do histórico original:** 2026-10-28  
**Última PR incluída no histórico original:** [PR #197](https://github.com/leonardo201800478/sega-emulator/pull/197); continuidade em seção pós-merge #227–#229.  
**Base registrada para o histórico original:** `main` em `e8871f31798f904da01c63063aab61ea3b27d803`  
**Escopo:** evidências de inventário, testes semânticos e documentação acumuladas para a CPU Z80 do Sega Master System. Este documento não certifica a completude semântica do Z80 nem precisão temporal ciclo a ciclo.

## 1. Resumo executivo

O trabalho recente estabeleceu uma matriz executável que enumera os espaços de opcode relevantes, refinou a classificação dos prefixos DD/FD e acrescentou vetores dirigidos para instruções indexadas e para DD-CB/FD-CB. A cobertura publicada é útil para regressão e para tornar as lacunas explícitas, mas **a modelagem semântica global permanece aberta**.

A sequência até a PR #197 foi feita principalmente com testes e documentação, sem alteração do runtime da CPU nas PRs #192–#197. Portanto, essas mudanças aumentam a evidência e a capacidade de detectar regressões; não significam, por si só, que toda a semântica das instruções esteja implementada ou correta.

### Estado resumido

| Área | Estado documentado |
|---|---|
| Enumeração/classificação dos espaços de opcode | Implementada para 7 espaços × 256 bytes = 1.792 codificações |
| Prefixos DD/FD | Classificação refinada; semântica ainda parcial |
| IX/IY e deslocamentos | Vetores dirigidos para formas selecionadas e limites assinados |
| DD-CB/FD-CB BIT/RES/SET | Cobertura dirigida ampliada para bits, campos de destino, flags, memória, WZ, PC, R e T-states agregados |
| Bloco ED de transferência/comparação | Campanha de regressão funcional direcionada registrada na PR #191 |
| Oráculo diferencial realmente independente | Pendente |
| Traces ordenados de barramento e validação ciclo a ciclo | Pendente |
| Conclusão semântica global do Z80 | **Não concluída** |

## 2. Histórico por pull request

### PR #189 — proveniência das referências e flags de I/O em bloco

[PR #189](https://github.com/leonardo201800478/sega-emulator/pull/189)

- Esclareceu a ancestralidade do corpus SingleStepTests/Z80: o README da revisão pinada informa que os vetores foram traduzidos do núcleo Z80 do Ares e depois corrigidos. Por isso, o corpus não é um oráculo independente do Ares.
- Separou as informações documentadas pelo manual ZiLOG das descrições de comportamento não documentado em *The Undocumented Z80 Documented*.
- Registrou limites do conhecimento disponível sobre flags das instruções de I/O em bloco, evitando apresentar bits não documentados como garantias do fabricante.
- Não alterou o comportamento do runtime.

### PR #190 — auditoria consolidada dos espaços de opcode

[PR #190](https://github.com/leonardo201800478/sega-emulator/pull/190)

- Criou `docs/architecture/z80-opcode-space-completion-audit.md`.
- Consolidou a auditoria dos espaços Base, CB, ED, DD/FD e DD-CB/FD-CB.
- Registrou 78 bytes ED atribuídos explicitamente e 178 bytes ED encaminhados ao fallback ED-NOP, conforme a auditoria então existente.
- Registrou a contagem de vetores fixos disponível naquele momento e lacunas de flags, WZ/MEMPTR, Q, R, interrupções, wraparound e temporização.
- Definiu critérios de aceitação sem declarar os 256 opcodes semanticamente concluídos.
- A descrição original da PR registrou que os checks de CI não estavam confirmados para seu commit candidato e que não houve validação local naquela sessão. Não se deve reinterpretar essa PR como evidência de CI verde.

### PR #191 — transferência/comparação em bloco ED

[PR #191](https://github.com/leonardo201800478/sega-emulator/pull/191)

- Acrescentou 20 testes de fronteira para LDI, LDD, LDIR, LDDR, CPI, CPD, CPIR e CPDR.
- Cobriu cenários selecionados de wraparound de HL/DE, BC igual a 0000/0001, repetição/término, flags, WZ/MEMPTR e PC perto de 0000/FFFF.
- Acrescentou 10 vetores fixos ao corpus SingleStepTests/Z80; a descrição registrou 118 vetores gerais e 54 de I/O em bloco (172 no total naquela etapa).
- Para CPIR/CPDR, incluiu regressões explícitas para término por correspondência mesmo quando BC não é zero e para o caso de BC inicial zero com wraparound.
- A descrição da PR registrou PASS para os workflows SMS Core Fast Validation, CI e SMS Z80 Platform Validation no candidato `26001b0e410672a977644ab774f4fa7ee3a11408`. Essa evidência pertence àquele SHA candidato; não é, isoladamente, uma declaração de validação pós-merge.
- A documentação da PR enfatizou que o corpus compartilha ancestralidade com Ares e que os T-states são agregados, não traces temporais.

### PR #192 — matriz semântica executável

[PR #192](https://github.com/leonardo201800478/sega-emulator/pull/192)

- Adicionou `native/sms_core/tests/z80_semantic_matrix.rs` e `docs/architecture/z80-semantic-matrix.md`.
- Enumerou 7 espaços × 256 bytes = 1.792 codificações.
- Distinguiu classificação de dispatch de prova semântica: executar um passo e obter T-states positivos não prova o resultado correto nos registradores, flags, memória ou I/O.
- Registrou 78 bytes ED atribuídos e 178 sem atribuição explícita, para os quais o runtime seguia o fallback ED-NOP.
- Identificou o corpus SingleStepTests/Z80 fixado em `ebe1875d48f374bcfd4b505d8eb8ee751568b5f7` e sua ancestralidade no Ares.
- A PR estabeleceu a matriz como gate incremental, não como certificação de completude.

### PR #193 — classificação refinada DD/FD

[PR #193](https://github.com/leonardo201800478/sega-emulator/pull/193)

- Refinou a classificação dos espaços DD e FD em quatro categorias:
  - 81 formas sensíveis a IX/IY;
  - 4 exceções de memória que usam os registradores H/L reais;
  - 167 formas que ignoram o prefixo;
  - 4 bytes de prefixo aninhado.
- Acrescentou exemplos positivos e negativos para evitar classificar todo opcode DD/FD como instrução indexada.
- Não alterou o runtime. A contagem descreve o inventário de dispatch, não a correção de cada execução.

### PR #194 — prefixos DD/FD ignorados

[PR #194](https://github.com/leonardo201800478/sega-emulator/pull/194)

- Adicionou vetores dirigidos para `DD 41` e `FD 41` (`LD B,C`), verificando que a operação conserva a semântica base e o registrador de índice.
- Conferiu registradores, PC, R e T-states agregados.
- São dois casos dirigidos; não constituem prova para todos os prefixos ignorados.
- Não alterou o runtime.

### PR #195 — semântica IX/IY

[PR #195](https://github.com/leonardo201800478/sega-emulator/pull/195)

- Adicionou vetores para `LD B,(IX-128)` e `LD A,(IY+127)`.
- Cobriu `LD IX,nn`, `LD IY,nn`, `INC IX`, `DEC IY`, leitura de IXH/IYL em A e preservação de HL nas formas imediatas.
- Conferiu registradores, flags, PC, R e T-states agregados.
- A documentação manteve explícito que a cobertura é dirigida e não combina todas as formas DD/FD.
- Não declarou precisão ciclo a ciclo.

### PR #196 — DD-CB/FD-CB, prefixos repetidos e wraparound

[PR #196](https://github.com/leonardo201800478/sega-emulator/pull/196)

- Acrescentou um caso de rotação indexada DD-CB com destino em registrador, verificando memória, registrador, flags, PC, R e T-states agregados.
- Acrescentou um caso FD-CB de RES com deslocamento negativo, verificando wraparound e preservação de flags.
- Verificou precedência do último prefixo em sequências DD/DD e DD/FD.
- Atualizou a matriz semântica e manteve as limitações de cobertura explícitas.
- Não alterou o runtime.

### PR #197 — BIT/RES/SET indexados e WZ

[PR #197](https://github.com/leonardo201800478/sega-emulator/pull/197)

- Adicionou `native/sms_core/tests/z80_indexed_cb_bit_res_set_wz.rs`, cobrindo BIT/RES/SET nos oito índices de bit, nos oito campos de destino e nos prefixos DD/FD, com operandos iniciais 00 e FF.
- Verificou resultados de memória e destino em registrador para RES/SET, preservação de flags e de registradores não envolvidos, flags de BIT, WZ/MEMPTR, PC, R e T-states agregados.
- Adicionou `native/sms_core/tests/z80_indexed_cb_reference_vectors.rs`, com oito vetores fixos de BIT indexado, ambos os prefixos e padrões do byte alto do endereço efetivo para observar F3/F5.
- Atualizou a matriz semântica com as fontes usadas e os limites da evidência.
- Os vetores de BIT são derivados de referências documentais; não são resultados de execução de outro emulador nem de hardware.
- A PR não alterou o runtime da CPU.

## 3. Evidência de validação da PR #197

O merge registrado da PR #197 é:

- **Merge commit:** `e8871f31798f904da01c63063aab61ea3b27d803`
- **Commit candidato da branch:** `c96abc6d18b9e331d5b69e13195d6def87e48d1b`
- **URL do merge:** https://github.com/leonardo201800478/sega-emulator/commit/e8871f31798f904da01c63063aab61ea3b27d803

Os registros de execução pós-merge anotados para o SHA de merge são:

| Workflow | Resultado registrado | Execução |
|---|---|---|
| SMS Core Fast Validation | PASS | https://github.com/leonardo201800478/sega-emulator/actions/runs/40686755329 |
| CI | PASS | https://github.com/leonardo201800478/sega-emulator/actions/runs/40686755250 |
| SMS Z80 Platform Validation | PASS | https://github.com/leonardo201800478/sega-emulator/actions/runs/40686755309 |

A branch candidata também teve execuções verdes registradas no SHA `eeaebe49b79a2144a2708f257eb685ac68a11d56`, mas esse SHA é anterior ao commit `c96abc6` registrado como head final da PR. Por isso, o status do candidato anterior não deve ser apresentado como se fosse o status do head final. Para a integração, a evidência relevante acima é a execução pós-merge no merge SHA.

Esta documentação registra os links e os resultados conhecidos; não afirma que comandos locais foram executados nesta sessão.

## 4. Referências técnicas e proveniência

- **Manual do fabricante:** ZiLOG, *Z80 CPU User Manual*, documento UM008011-0816, pp. 247–249, https://www.zilog.com/docs/z80/um0080.pdf. Para BIT indexado, descreve a operação e 20 T-states; alguns bits de flags não são especificados como comportamento garantido pelo fabricante.
- **Comportamento não documentado:** Sean Young, *The Undocumented Z80 Documented*, versão 0.4, §4.1, https://z80.info/zip/z80-documented.pdf. Usado como referência descritiva para comportamento observado de flags; a licença explícita não foi verificada no levantamento documentado.
- **WZ/MEMPTR:** redcode/Z80 Wiki, “MEMPTR”, https://github.com/redcode/Z80/wiki/MEMPTR. A matriz registra a revisão exibida em 2026-01-07; o hash de commit e a licença da página não foram estabelecidos na evidência anotada.
- **Corpus de regressão:** SingleStepTests/Z80, revisão `ebe1875d48f374bcfd4b505d8eb8ee751568b5f7`. O próprio README upstream declara ancestralidade no núcleo Z80 do Ares. Útil para regressão diferencial, mas não independente do Ares.
- **Oráculo independente candidato para trabalho futuro:** redcode/Z80, commit `dcc3f28d3a344b3080e7ae6f1e7b694b45b370d9`, projeto C com licença LGPL-3.0-or-later. Ainda não foi validado neste histórico como harness diferencial integrado e executável; sua adequação para observar todos os estados internos, especialmente WZ/MEMPTR e timing, precisa ser demonstrada antes de usá-lo como oráculo.

## 5. Lacunas que continuam abertas

1. **Rotação e shift:** cobertura semântica completa para RLC/RRC/RL/RR/SLA/SRA/SLL/SRL, todos os destinos, carry de entrada/saída e flags.
2. **BIT/RES/SET fora do recorte indexado:** completar combinações de CB e `(HL)`, além de separar claramente comportamento documentado e não documentado.
3. **Base, ALU e DAA:** ampliar vetores de todas as formas e casos-limite de flags, com oráculo de proveniência clara.
4. **Saltos e fluxo de controle:** condições tomadas/não tomadas, limites de deslocamento, wraparound, CALL/RET/RST e efeitos WZ.
5. **Pilha e memória:** SP em 0000/FFFF, ordem de efeitos, combinações de registradores e fronteiras de endereço.
6. **IX/IY e prefixos:** cobrir sistematicamente as formas indexadas, exceções H/L, prefixos repetidos e aliases não documentados.
7. **ED:** completar as formas atribuídas e revisar o fallback das formas não atribuídas, sem presumir que ED-NOP representa todos os comportamentos de CPU alvo.
8. **Interrupções e estados especiais:** aprofundar IM 0/1/2, NMI, EI/DI, HALT, R, acknowledge e fronteiras de aceitação.
9. **Estado interno:** ampliar Q/P, WZ/MEMPTR, R e flags não documentadas por família e sequência.
10. **Validação diferencial:** criar um harness isolado contra implementação independente e reproduzir divergências campo a campo.
11. **Temporização:** modelar e validar progressivamente M-cycles, T-states, acessos ao barramento, waits, interrupções e ordem dos eventos. T-states agregados por instrução não equivalem a traces ciclo a ciclo.
12. **Revisão independente:** confirmar as regras e vetores com fontes independentes e evidência reproduzível.

## 6. Critérios para fechar a modelagem semântica

Não declarar a etapa concluída até que:

- [ ] cada família e cada forma de opcode suportada esteja mapeada para testes semânticos com limites explícitos;
- [ ] os testes comparem estado inicial/final, flags, PC/SP, memória/I/O e estados internos pertinentes;
- [ ] divergências tenham vetores mínimos reproduzíveis e classificação da causa;
- [ ] a procedência, revisão e licença de corpora/oráculos estejam registradas;
- [ ] o harness diferencial esteja executável e isolado do runtime, com resultados no SHA candidato exato;
- [ ] os workflows aplicáveis passem no candidato exato e, após merge, no SHA de integração;
- [ ] as pendências não verificadas permaneçam identificadas como BLOCKED ou NOT RUN, sem serem tratadas como PASS.

A conclusão semântica e a conclusão temporal são gates separados. O estado `TEMPORAL_PASS` requer comparação de traces temporais ordenados com evidência adequada; não pode ser inferido de uma suíte de instruções com T-states agregados.

## 7. Estado no corte desta documentação

- **Inventário de opcode:** IMPLEMENTED para a matriz de classificação existente.
- **Testes semânticos dirigidos:** IMPLEMENTED nas famílias descritas, com escopos limitados.
- **QUICK_PASS / SHORT_PASS:** somente nos escopos e SHA explicitamente associados aos workflows listados; não implica completude universal.
- **LONG_PASS:** não reivindicado por este documento.
- **INTEGRATION_PENDING:** não se aplica à PR #197, já integrada; aplica-se às próximas mudanças que ainda não tiverem sido integradas e verificadas.
- **TEMPORAL_PASS:** não reivindicado.
- **Modelagem global das lacunas Z80:** **ABERTA**.
- **Próxima etapa recomendada:** implementar e validar o harness diferencial independente, começando por DD-CB/FD-CB, sem modificar o runtime até haver divergência reproduzível.


## Continuação após PR #197 — PRs #227–#229 (2026-10-29)

Este histórico foi originalmente fechado na PR #197. A seção abaixo atualiza o estado sem reescrever o registro histórico de cada etapa.



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


As PRs #227–#229 não demonstram a conclusão semântica global do Z80. O trabalho subsequente deve preservar o método incremental e vincular cada novo estado de validação ao SHA candidato e aos workflows efetivamente concluídos.
