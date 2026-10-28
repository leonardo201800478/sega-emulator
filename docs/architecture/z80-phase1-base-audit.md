# Fase 1 — auditoria semântica direcionada da CPU Z80 do SMS

## Estado e limite da conclusão

Estado desta auditoria: **AUDIT_COMPLETE / IMPLEMENTATION_PENDING** até que a PR #201 receba os novos checks no SHA final e seja revisada. Este relatório fecha a auditoria direcionada desta fase; não declara conformidade semântica completa dos 256 opcodes BASE nem precisão temporal ciclo a ciclo.

Base examinada: `main` em `2db0e51d93f2274457d5155c6ed3fef32cd4b870`; candidato de implementação e testes: PR #201, branch `test/z80-exhaustive-alu-vectors`. A base remota deve ser revalidada antes de integrar.

## Defeitos confirmados

1. **DAA — correção do dígito baixo em estados de subtração.** A implementação anterior só adicionava `0x06` quando `H=1` ou quando `N=0` e o nibble baixo de A era maior que 9. Isso omitia a correção quando `N=1`, `H=0` e o nibble baixo de A era A–F. A alteração proposta na PR #201 remove a condição `N=0`. A correção está coberta pelo modelo interno exaustivo, mas esse modelo usa a mesma regra de seleção de correção; por isso, não é evidência independente suficiente isoladamente.
2. **Matriz documental — opcodes 0x22 e 0x2A.** A matriz na branch da PR #203 identifica incorretamente `0x22` como `LD (HL),nn` e `0x2A` como `LD nn,(HL)`. O decoder observado implementa, respectivamente, `LD (nn),HL` e `LD HL,(nn)`. Isto é um erro de rótulo documental, não evidência de defeito no decoder. A correção deve permanecer restrita à PR documental.

## Evidência e independência

- Os testes exaustivos de ADD/ADC/SUB/SBC/CP e AND/XOR/OR usam aritmética alargada, faixa de inteiros com sinal, nibble carry/borrow e paridade. São um modelo matemático interno, não captura de hardware.
- Os testes de INC/DEC, rotações do acumulador, flags e DAA ampliam a cobertura combinatória das famílias testadas.
- Foram acrescentados vetores fixos de DAA derivados do exemplo BCD do *Zilog Z80 CPU User Manual*, documento UM008011-0816, pp. 173–174: 0x15 + 0x27 → 0x42 e 0x42 − 0x27 → 0x15. Os resultados esperados são literais, não calculados pela função de referência exaustiva. Os checks do SHA final ainda precisam confirmar a execução desses vetores.
- A suíte externa `raxoft/z80test` é fixada por commit e executa o núcleo SMS com barramento plano de 64 KiB. Ela complementa os vetores, mas não valida mapper, VDP, PSG, interrupções dirigidas por dispositivos nem eventos ordenados do barramento por T-state. Os checks aprovados no SHA anterior da PR #201 não valem automaticamente para um SHA posterior.

## Cobertura BASE: o que está e o que não está provado

A matriz da PR #203 contém 256 entradas BASE. Na classificação documental examinada, 104 entradas são marcadas `exhaustive_internal_model` e 152 `sparse_selected_vectors`; esses rótulos são indicadores de evidência, não prova de que cada combinação de operandos, flags, condição e efeito lateral foi validada. A auditoria direcionada desta fase cobre as famílias ALU de 8 bits, DAA, INC/DEC e rotações/flags de acumulador por testes combinatórios, mas não transforma os demais 152 opcodes em conformidade exaustiva.

Pendências sem defeito confirmado nesta fase:
- Loads, transferências de 16 bits, saltos, chamadas/retornos, pilha, RST, I/O e troca de bancos precisam de vetores por caminhos tomados/não tomados, limites de PC/SP, wraparound e efeitos de memória.
- Flags X/Y, WZ/MEMPTR e Q precisam de sequências fixas adicionais por família; a classificação do decoder não é suficiente.
- A contagem nominal de T-states não prova ordem de leituras/escritas nem precisão ciclo a ciclo. A evidência temporal permanece `NOT_VERIFIED`.

## Critérios de saída da Fase 1

- [x] Confirmar o SHA remoto de base e identificar o candidato PR existente.
- [x] Separar o defeito DAA confirmado das limitações de independência do modelo.
- [x] Registrar os dois erros de rótulo da matriz separadamente de defeitos do decoder.
- [x] Definir regressões fixas independentes para os exemplos BCD documentados.
- [x] Registrar as lacunas de cobertura e não equiparar classificação a conformidade.
- O estado dos checks é deliberadamente consultado na aba **Checks** da [PR #201](https://github.com/leonardo201800478/sega-emulator/pull/201), sempre no HEAD atual. Resultados de SHAs anteriores não são transferíveis.
- A suíte externa deve reportar zero falhas e terminar sem timeout no mesmo SHA em que os vetores fixos estão presentes.
- A integração permanece separada desta auditoria: não fazer merge automático; exigir revisão dos checks no SHA candidato.

## Referências

- Zilog, *Z80 CPU User Manual*, UM008011-0816, pp. 173–174 (instrução DAA e exemplo BCD).
- [PR #201 — vetores ALU exaustivos e suíte externa](https://github.com/leonardo201800478/sega-emulator/pull/201).
- [PR #203 — matriz de opcodes SMS](https://github.com/leonardo201800478/sega-emulator/pull/203).
- [SingleStepTests/Z80](https://github.com/SingleStepTests/z80/tree/ebe1875d48f374bcfd4b505d8eb8ee751568b5f7): útil para regressão diferencial, mas derivado de Ares conforme a documentação upstream; não é oráculo independente de Ares.
