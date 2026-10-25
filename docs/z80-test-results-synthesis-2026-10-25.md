# Consolidação da auditoria e dos testes do Z80 — 2026-10-25

## Resumo executivo

Esta página consolida os resultados verificáveis dos três jobs de validação especializados (funcional, integridade/determinismo e desempenho/cobertura), além da auditoria diferencial SingleStepTests e da auditoria estendida de robustez executadas no GitHub Actions.

**Conclusão:** o commit auditado `78231c692e34eb0c4a8cfabc4dbe1ddbdaa14d50` passou pelos workflows listados abaixo. Isso demonstra que os testes executados nesse commit passaram; **não** demonstra que o Z80 esteja integralmente validado, que o corpus SingleStepTests completo tenha sido executado ou que a CPU seja ciclo-precisa.

- Commit auditado: [`78231c6`](https://github.com/leonardo201800478/sega-emulator/commit/78231c692e34eb0c4a8cfabc4dbe1ddbdaa14d50)
- PR de infraestrutura da Fase 1: [#43 — expandir ferramentas de auditoria diferencial](https://github.com/leonardo201800478/sega-emulator/pull/43)
- Revisão de referência do corpus: [SingleStepTests/z80, commit fixado `ebe1875d48f374bcfd4b505d8eb8ee751568b5f7`](https://github.com/SingleStepTests/z80/tree/ebe1875d48f374bcfd4b505d8eb8ee751568b5f7)
- Ambiente dos workflows: GitHub-hosted Ubuntu, Python 3.14.8. Os resultados não são uma execução local no Windows.

## Resultados confirmados

| Área | Resultado observado | O que permite concluir |
|---|---|---|
| Agente 1 — validação funcional | **499 testes passaram** em 6,17 s; Ruff format e lint passaram | A suíte Python e as verificações de qualidade passaram no commit auditado. |
| Agente 2 — integridade e determinismo | **499 testes passaram em duas execuções** (6,34 s e 5,20 s); `compileall`, `pip check` e integridade do pacote passaram | As duas execuções da suíte tiveram sucesso; imports/compilação e dependências não reportaram erro. A igualdade completa de todos os artefatos só deve ser afirmada nos limites explicitamente verificados pelo job. |
| Agente 3 — desempenho e cobertura | **499 testes passaram** em 3,62 s; Ruff passou; inventário de opcodes gerado | O job de diagnóstico e a geração do inventário terminaram com sucesso. O inventário mede classificação/dispatch, não correção semântica de cada instrução. |
| SingleStepTests diferencial | **25.000/25.000 vetores aprovados**, em 25 arquivos; manifesto de SHA-256 e tamanhos íntegro para os 25 arquivos | O subconjunto selecionado passou no adaptador diferencial, incluindo comparação do estado suportado e T-states agregados. |
| Smoke test do workflow de corpus completo | **3.000/3.000 vetores aprovados**, em 3 arquivos (`00`, `cb 00`, `ed 44`) | Smoke test aprovado; não equivale à execução de todos os arquivos do corpus. |
| Auditoria estendida de robustez | **4.702.901 casos** em 180 s; **0 achados de robustez** e **0 opcodes não suportados registrados** | Nenhuma violação das verificações de robustez observadas nessa execução. Não é um oráculo de correção funcional. |
| CI geral | Sucesso | Os checks do workflow de CI associados ao commit concluíram com sucesso. |

### Execuções e artefatos

- [CI — run 40140361683](https://github.com/leonardo201800478/sega-emulator/actions/runs/40140361683)
- [Agente 1 — Functional Validation — run 40140361697](https://github.com/leonardo201800478/sega-emulator/actions/runs/40140361697) · [relatório JUnit (artefato)](https://github.com/leonardo201800478/sega-emulator/actions/runs/40140361697/artifacts/12578341928)
- [Agente 2 — Integrity and Determinism — run 40140361635](https://github.com/leonardo201800478/sega-emulator/actions/runs/40140361635) · [relatórios JUnit (artefato)](https://github.com/leonardo201800478/sega-emulator/actions/runs/40140361635/artifacts/12578551573)
- [Agente 3 — Performance and Coverage Audit — run 40140361657](https://github.com/leonardo201800478/sega-emulator/actions/runs/40140361657) · [inventário e diagnósticos (artefato)](https://github.com/leonardo201800478/sega-emulator/actions/runs/40140361657/artifacts/12578911189)
- [SingleStepTests diferencial — run 40140361671](https://github.com/leonardo201800478/sega-emulator/actions/runs/40140361671) · [relatório diferencial e vetores (artefato)](https://github.com/leonardo201800478/sega-emulator/actions/runs/40140361671/artifacts/12578184840)
- [Smoke/full-corpus workflow — run 40140361623](https://github.com/leonardo201800478/sega-emulator/actions/runs/40140361623) · [artefato de auditoria](https://github.com/leonardo201800478/sega-emulator/actions/runs/40140361623/artifacts/12579040292)
- [Auditoria estendida — run 40140361649](https://github.com/leonardo201800478/sega-emulator/actions/runs/40140361649) · [relatórios e inventário (artefato)](https://github.com/leonardo201800478/sega-emulator/actions/runs/40140361649/artifacts/12579170580)
- [Diagnóstico de desempenho — run 40140361628](https://github.com/leonardo201800478/sega-emulator/actions/runs/40140361628)

Os artefatos são a fonte primária para os detalhes por teste e por vetor. A retenção de artefatos é limitada pelo GitHub Actions; preservar uma cópia de longo prazo exige baixá-los e arquivá-los deliberadamente.

## Interpretação técnica

### 1. Evidência funcional diferencial

Os 25.000 vetores foram distribuídos por 25 arquivos de 1.000 casos, cobrindo instruções base e amostras dos espaços CB, ED e indexado. Todos os arquivos escolhidos tiveram hash e tamanho conferidos contra o manifesto, e todos os vetores selecionados passaram.

Essa evidência é mais forte do que um teste de despacho de opcode: compara resultados observáveis representados pelo adaptador. Ainda assim, a cobertura é **selecionada**, não exaustiva. O adaptador não compara o traço completo de barramento ciclo a ciclo; a contagem de T-states é agregada por instrução.

### 2. Smoke test não é corpus completo

Apesar do nome do workflow “Z80 full SingleStepTests audit”, a execução associada ao evento de pull request tomou o caminho de smoke test e executou somente os arquivos `00`, `cb 00` e `ed 44` (3.000 vetores). Não devemos descrevê-la como uma execução integral de SingleStepTests.

A execução de 25.000 vetores também é um subconjunto selecionado. **A execução integral de todos os JSONs do corpus, com relatório agregado por família, ainda deve ser confirmada por um workflow executado explicitamente no escopo `full`.**

### 3. Robustez e determinismo

A auditoria estendida executou 4.702.901 casos em três minutos, com zero achados de robustez e zero opcodes não suportados registrados. Isso indica que o núcleo resistiu às verificações/invariantes que o auditor executa e não registrou ocorrências nessas categorias.

O teste de estresse não é um teste diferencial semântico: ausência de exceções ou de violações de invariantes não prova que o resultado de cada instrução corresponde ao hardware. Do mesmo modo, duas execuções bem-sucedidas da suíte são evidência de repetibilidade operacional, mas não substituem uma comparação explícita de hashes completos dos estados/artefatos, quando esse critério for necessário.

### 4. Inventário de opcodes

A matriz estrutural cobre **1.792 células** em sete espaços (base, CB, ED, DD, FD, DD-CB e FD-CB). Ela ajuda a identificar a classificação e o caminho de despacho, inclusive aliases, prefixos ignorados, comportamentos não documentados e ED-NOP.

Uma célula classificada como implementada ou despachada não significa que todos os valores de entrada, flags, efeitos de memória/I/O e temporização daquela instrução estejam corretos. Para isso são necessários vetores semânticos e testes direcionados.

## Resultados históricos que não devem ser misturados com os atuais

Os números abaixo pertencem a revisões/execuções anteriores e **não** são resultados pós-correção do commit auditado acima:

- Linha de base diferencial histórica: **992.309 aprovados e 7.691 divergências em 1.000.000 de casos**; a maior parte das divergências relatadas estava em `BIT (HL)` (6.001), `SCF/CCF` (1.342) e `DAA` (348). Esse resultado é diagnóstico de uma revisão anterior, não retrato do HEAD atual.
- Auditoria direcionada R2 histórica: **13.567/14.000 aprovados**, com 433 divergências em SCF/CCF sem prefixo; a medição antecedeu a última alteração da fórmula de flags e não deve ser apresentada como resultado posterior à correção.
- As aprovações de CI de PRs anteriores validam somente os commits explicitamente citados nos respectivos relatórios, não automaticamente o commit mais recente.

## Limitações ainda abertas

1. **Corpus completo SingleStepTests:** executar no modo `full` contra o commit fixado e arquivar o resumo JSON/Markdown integral por família/opcode.
2. **Oráculo independente:** comparar casos controversos com uma segunda fonte independente, como ZEXDOC/ZEXALL e testes focados de hardware/documentação, sem tratar qualquer emulador como autoridade infalível.
3. **ZEXDOC/ZEXALL:** não declarar aprovação completa sem registrar hash do binário, quantidade de testes efetivamente concluídos, saída final e código de saída. Interrupção por timeout não é aprovação.
4. **Precisão temporal:** a CPU retorna T-states agregados por instrução. Ainda não há evidência aqui de traço completo de ciclos de barramento, contenção ou precisão ciclo a ciclo.
5. **Comportamentos não documentados:** Q, WZ/MEMPTR, flags X/Y, prefixos repetidos/ignorados, aliases ED e semântica de interrupções precisam manter testes dirigidos e referências cruzadas.
6. **Validação in-game:** resultados headless não substituem execução de ROM homebrew legalmente obtida, teste visual/sonoro e comparação controlada com ARES no Windows.
7. **Versão do código:** todos os resultados acima estão vinculados ao SHA auditado `78231c6...`; não os atribuir a commits posteriores sem reexecutar ou verificar a equivalência do código.

## Próximos passos recomendados

1. Disparar o workflow SingleStepTests no escopo `full` e conferir o relatório integral, incluindo T-states e arquivos com divergências.
2. Investigar divergências uma família por vez, confirmar os vetores contra fontes independentes e adicionar regressões unitárias antes de alterar a CPU.
3. Registrar o resultado completo de ZEXDOC/ZEXALL, caso o binário externo esteja disponível e sua proveniência seja conhecida.
4. Só depois de resolver divergências funcionais ampliar otimizações ou avançar em alegações de precisão temporal.
5. Manter esta página versionada junto dos relatórios de execução e atualizar os números sempre com commit, corpus e link do workflow correspondentes.

## Proveniência e licença

O corpus SingleStepTests/Z80 usado nestes workflows está fixado no commit `ebe1875d48f374bcfd4b505d8eb8ee751568b5f7`; o upstream identifica a licença MIT. O harness do projeto não deve versionar ROMs comerciais nem binários externos ZEX. Os relatórios aqui citados foram obtidos dos logs e artefatos do GitHub Actions associados ao commit `78231c692e34eb0c4a8cfabc4dbe1ddbdaa14d50`.
