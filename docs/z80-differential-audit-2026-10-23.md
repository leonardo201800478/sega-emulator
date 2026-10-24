# Auditoria diferencial Z80 — 23/10/2026

## Objetivo e escopo

Registrar os resultados do primeiro lote de um milhão de casos SingleStepTests e as correções propostas a partir das divergências. Este documento separa a medição da revisão antiga, as mudanças feitas na branch de trabalho e as validações ainda pendentes.

- Repositório: [leonardo201800478/sega-emulator](https://github.com/leonardo201800478/sega-emulator)
- Branch de trabalho: `fix/z80-single-step-audit`
- Pull request: [#14 — Corrige flags Z80 em DAA, BIT (HL) e SCF/CCF](https://github.com/leonardo201800478/sega-emulator/pull/14)
- Commit testado pelo lote reportado: [`4a6eac4468645843c2cab22aa653413db4478221`](https://github.com/leonardo201800478/sega-emulator/commit/4a6eac4468645843c2cab22aa653413db4478221)
- Corpus declarado pelo relatório: `SingleStepTests/z80 v1`, 1.000 arquivos, 1.000.000 de casos.
- Natureza desta etapa: análise de resultado diferencial e correções propostas; **o relatório não é uma execução pós-correção**.

## Resultado da execução reportada

| Métrica | Resultado |
|---|---:|
| Casos processados | 1.000.000 |
| Casos aprovados | 992.309 |
| Divergências | 7.691 |
| Taxa de aprovação | 99,2309% |
| Taxa de divergência | 0,7691% |
| Tempo registrado na sessão | 100,29 s |

A taxa é calculada sobre o total reportado. Os resultados vieram do pacote de auditoria recebido, que contém `report.json`, `checkpoint.json` e `case_results.csv`.

## Divergências agrupadas

| Família | Divergências reportadas | Interpretação inicial |
|---|---:|---|
| `CB 46/4E/56/5E/66/6E/76/7E` — `BIT b,(HL)` | 6.001 | As flags não documentadas F3/F5 podem depender do estado interno WZ/MEMPTR, não simplesmente do valor de HL. |
| `SCF` / `CCF`, incluindo casos prefixados | 1.342 | A geração de F3/F5 em Z80 NMOS pode depender do estado Q e do contexto da instrução anterior. Casos prefixados devem ser analisados separadamente. |
| `DAA`, incluindo casos prefixados | 348 | A seleção da correção decimal no caminho de subtração divergia em casos como A=`F3h`, F=`4Eh`. |
| **Total** | **7.691** | Os agrupamentos precisam ser reavaliados após executar a mesma auditoria com o código corrigido. |

Essas contagens descrevem a execução anterior e não devem ser apresentadas como contagens atuais após as mudanças.

## Caso reproduzível de DAA

Um caso do corpus começa com A=`F3h` e F=`4Eh`. O estado esperado reportado é A=`93h` e F=`87h`. O caso foi convertido em regressão unitária para evitar que a divergência volte a ocorrer.

A regra de correção foi ajustada para considerar os critérios de A e os flags H/C anteriores mesmo quando N indica subtração. O teste unitário e o oráculo auxiliar foram atualizados em conjunto. O teste ainda precisa ser executado no ambiente de CI/local para ser considerado aprovado.

## Alterações de código registradas na branch

As alterações atuais do PR #14 afetam:

- `src/sega_emulator/cpu/z80.py`
  - corrige a seleção das correções de `DAA` para o caso de regressão;
  - introduz estado interno `wz` e `q`;
  - usa o byte alto de `wz` para F3/F5 em `BIT b,(HL)`;
  - implementa a regra proposta de F3/F5 em `SCF`/`CCF` baseada em Q;
  - atualiza Q em caminhos selecionados, incluindo DAA e operações CB que testam bits.
- `tests/unit/test_z80.py`
  - adiciona a regressão de DAA;
  - acrescenta testes direcionados para SCF/CCF e BIT (HL).

**Importante:** a existência desses testes no código não significa que tenham passado. A execução de pytest/Ruff pós-alteração e a nova auditoria diferencial ainda precisam ser confirmadas.

## Limitações técnicas em aberto

1. **Ciclo de vida de Q:** o estado Q ainda não tem atualização centralizada e validada para todas as instruções. Casos com prefixos DD/FD antes de SCF/CCF precisam de teste explícito; não declarar a família resolvida apenas pelos testes atuais.
2. **Atualização de WZ/MEMPTR:** inicializar WZ e usá-lo em BIT (HL) permite reproduzir casos que fornecem WZ inicial, mas não implementa ainda a atualização correta de WZ em todas as instruções.
3. **Cobertura do corpus:** SingleStepTests valida os estados de entrada e saída de cada caso conforme os campos incluídos no harness; não prova por si só precisão ciclo a ciclo, temporização de barramento ou compatibilidade geral com ROMs.
4. **Independência do oráculo:** divergências devem ser avaliadas com a especificação aplicável, documentação técnica e/ou outra referência independente; não escolher automaticamente uma implementação por maioria.
5. **Variante física:** comportamentos não documentados precisam ser associados ao modelo de Z80/NMOS/derivado escolhido para a plataforma SG-1000.

## Próximo procedimento de validação

### Testes curtos locais

No PowerShell, a partir da raiz do clone e com o ambiente virtual do projeto:

```powershell
.\.venv\Scripts\python.exe -m pytest tests/unit/test_z80.py -q
.\.venv\Scripts\python.exe -m pytest -q
.\.venv\Scripts\python.exe -m ruff check .
.\.venv\Scripts\python.exe -m ruff format --check .
git diff --check
```

Registrar o resultado real de cada comando, sem inferir sucesso a partir da existência dos testes.

### Auditoria longa no Google Colab

1. Usar o notebook de auditoria atualizado que adapta o harness para os campos de estado inicial `wz` e `q`.
2. Confirmar que o notebook clona/obtém explicitamente a branch/commit corrigido, e não `main` por padrão.
3. Salvar no diretório de saída do Colab o relatório, checkpoint e CSV completos.
4. Registrar no relatório o SHA exato do código testado, branch, corpus, total processado, pass/fail, duração e versão do harness.
5. Comparar as divergências por opcode com a execução anterior e inspecionar as divergências remanescentes.
6. Só atualizar este documento com números pós-correção quando o novo artefato tiver sido recebido e conferido.

## Critérios de aceitação desta frente

- Os testes curtos relacionados às correções passam.
- Ruff e verificação de whitespace passam.
- A auditoria longa confirma o SHA corrigido e produz artefatos completos.
- As divergências das três famílias são recontadas e as remanescentes classificadas.
- Q/WZ, prefixos e flags são descritos como parcialmente validados até haver evidência suficiente para cada caminho.
- O PR continua sem merge até a validação ser revisada.

## Estado no fechamento deste registro

- Resultado de referência anterior: **1.000.000 de casos, 992.309 aprovados, 7.691 divergências**.
- Correções propostas: commitadas na branch de trabalho e reunidas no PR #14.
- Execução pós-correção: **pendente**.
- Suíte unitária/Ruff pós-correção: **não confirmada neste registro**.
- Precisão ciclo a ciclo: **não implementada/demonstrada**.


---

## Validação curta após as correções — 23/10/2026

A revisão [`a21dfdcbc02b09b7507e109b5a28be09a3674f52`](https://github.com/leonardo201800478/sega-emulator/commit/a21dfdcbc02b09b7507e109b5a28be09a3674f52) passou na [CI 39904100544](https://github.com/leonardo201800478/sega-emulator/actions/runs/39904100544): Ruff format/check aprovados e **367 testes aprovados em 5,41 s**. A revisão contém correções de DAA, atualização centralizada de Q e atualizações parciais de WZ/MEMPTR, além de testes dirigidos.

Este resultado valida a suíte curta do repositório, não o corpus de um milhão de casos. A auditoria diferencial pós-correção permanece pendente; os números da linha de base acima não foram alterados. O próximo lote deve comparar também os campos finais `wz` e `q` quando fornecidos pelo corpus, além de registrar SHA, corpus, tempo e artefatos completos.


### Validação adicional: ED-NOP e cobertura de decodificação (23/10/2026)

A revisão [`88bc6d35f3c3433c01f0451688092d6f8c70f7a1`](https://github.com/leonardo201800478/sega-emulator/commit/88bc6d35f3c3433c01f0451688092d6f8c70f7a1) passou na [CI 39904473410](https://github.com/leonardo201800478/sega-emulator/actions/runs/39904473410): **372 testes aprovados em 5,62 s**, com Ruff format/check aprovados. A cobertura agora trata bytes ED sem operação atribuída como ED-NOP de 8 T-states, preservando handlers para aliases conhecidos; os testes percorrem todos os 256 segundos bytes ED.

A nova auditoria diferencial completa ainda não foi executada. Os resultados de 1.000.000 de casos acima continuam sendo a linha de base anterior às correções. A próxima medição deve usar o notebook v5, comparar WZ/Q quando presentes no estado final e preservar todos os detalhes de divergência no CSV.


---

## Auditoria direcionada pós-correção inicial — 24/10/2026

Foi recebido e inspecionado o arquivo `z80-targeted-audit-pr14-results.zip`, produzido contra o commit `27fadd182c65ffbd0ab899343ed1197c3f265515` da branch `fix/z80-single-step-audit`. O lote cobriu 14 arquivos do corpus `SingleStepTests/z80 v1`, com 1.000 vetores por arquivo.

| Métrica | Resultado |
|---|---:|
| Vetores processados | 14.000 |
| Aprovados | 13.531 |
| Divergentes | 469 |
| Aprovação | 96,65% |
| Divergência | 3,35% |

### Resultado por família

| Família | Aprovados | Divergentes |
|---|---:|---:|
| `BIT (HL)` — 8 opcodes | 8.000 | 0 |
| `DAA`, incluindo `DD 27` | 2.000 | 0 |
| `SCF/CCF`, sem prefixo | 2.000 | 0 |
| `DD SCF` (`DD 37`) | 761 | 239 |
| `DD CCF` (`DD 3F`) | 770 | 230 |
| **Total** | **13.531** | **469** |

As 469 divergências eram exclusivamente nos campos `F` e `Q`; o campo `F` diferia apenas nos bits F3/F5. Não foram reportadas divergências de ciclos nesse lote. O padrão aponta para o tratamento de Q ao executar `SCF/CCF` após um prefixo DD ignorado.

### Correção subsequente

A inspeção dos casos `DD 37` e `DD 3F` mostrou que, quando Q anterior é diferente de zero, a implementação considerava apenas os bits X/Y de A e descartava os bits X/Y presentes em Q. Foi corrigida a expressão para combinar os bits X/Y de A com os bits correspondentes de Q anterior; quando Q anterior é zero, preserva-se a regra já exercitada pelo corpus que combina A e F.

Foram adicionados testes para:
- Q não nulo contendo F3/F5 em SCF/CCF;
- SCF e CCF precedidos por prefixo DD ignorado, com estado inicial reproduzível semelhante aos vetores divergentes.

**Estado da validação:** o ZIP recebido mede o commit anterior à correção acima. Os commits que aplicam a correção e adicionam regressões ainda precisam passar na CI e na repetição dos 14.000 vetores. Não declarar resolvida a família DD SCF/CCF até confirmar esses resultados.


## Segunda auditoria direcionada e revisão Q/F — 24/10/2026

O ZIP `z80-targeted-audit-pr14-r2-results.zip` foi executado contra `6434fdaea85b32e7335ddf9a37730d6e70a083ae`, antes da correção mais recente:

| Métrica | Resultado |
|---|---:|
| Vetores | 14.000 |
| Aprovados | 13.567 (96,91%) |
| Divergentes | 433 (3,09%) |
| BIT (HL) | 8.000/8.000 |
| DAA, incluindo DD 27 | 2.000/2.000 |
| SCF | 777/1.000; 223 divergências |
| CCF | 790/1.000; 210 divergências |
| DD SCF / DD CCF | 1.000/1.000 em cada arquivo |

As divergências restantes no lote estão nos campos `F` e `Q` de SCF/CCF sem prefixo; os T-states não divergiram. A expressão da branch usava os bits de Q diretamente quando Q era diferente de zero. A correção candidata agora calcula os bits não documentados como `((Q XOR F) OR A) & 0x28`, tanto em SCF quanto em CCF.

Commits: [código](https://github.com/leonardo201800478/sega-emulator/commit/9f936ec913e09f6aaf4219682d8ddf95f25603dd) e [testes](https://github.com/leonardo201800478/sega-emulator/commit/58671c8f09d2462723fb2c0b85a823789572ad65). Essa é uma correção candidata: testes unitários/Ruff ainda precisam ser executados. Não enviar ao Colab antes de os testes rápidos passarem; o ZIP acima não valida esses novos commits.
