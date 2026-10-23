# Registro de desenvolvimento e ponto de retomada — 23/10/2026

## Objetivo

Registrar o estado do projeto após a primeira auditoria diferencial de um milhão de casos Z80, as correções propostas e a atualização da documentação. Este documento é o ponto de retomada mais recente; o registro de 22/10/2026 continua preservado como histórico.

## Repositório e Git

- Repositório: [leonardo201800478/sega-emulator](https://github.com/leonardo201800478/sega-emulator)
- Branch principal: `main` — não foi alterada diretamente por esta etapa.
- Branch de trabalho: `fix/z80-single-step-audit`
- PR: [#14 — Corrige flags Z80 em DAA, BIT (HL) e SCF/CCF](https://github.com/leonardo201800478/sega-emulator/pull/14)
- Estado do PR no início desta etapa: aberto como draft, não mesclado.
- Último commit de documentação confirmado nesta etapa: [`4b5a723ab17625b5fbaecb823ebd842fb17c2770`](https://github.com/leonardo201800478/sega-emulator/commit/4b5a723ab17625b5fbaecb823ebd842fb17c2770)
- O PR também contém alterações de código/testes em commits anteriores. Conferir o HEAD do PR antes de iniciar novos trabalhos.

As atualizações de documentação foram feitas diretamente no GitHub, na branch do PR. Não foi executado merge.

## Resultado da auditoria diferencial anterior

Corpus declarado: `SingleStepTests/z80 v1`, 1.000 arquivos, 1.000.000 de casos.

- Commit medido: [`4a6eac4468645843c2cab22aa653413db4478221`](https://github.com/leonardo201800478/sega-emulator/commit/4a6eac4468645843c2cab22aa653413db4478221), branch `main`.
- Aprovados: 992.309.
- Divergências: 7.691.
- Taxa de aprovação: 99,2309%.
- Tempo registrado: 100,29 s.
- Grupos: 6.001 divergências em BIT b,(HL), 1.342 em SCF/CCF e 348 em DAA.

Esse resultado é uma **linha de base anterior às correções**, não o resultado do código atual. Não usar esses números para declarar que o PR passou ou falhou.

## Alterações de código em avaliação

No PR #14:

- `src/sega_emulator/cpu/z80.py`: ajuste de DAA; estado auxiliar WZ e Q; uso do byte alto de WZ nas flags F3/F5 de BIT (HL); comportamento de F3/F5 em SCF/CCF condicionado ao Q.
- `tests/unit/test_z80.py`: regressão DAA com A=`F3h` e F=`4Eh`, teste direcionado de BIT (HL) com WZ diferente de HL e casos parametrizados de SCF/CCF.

### Limitações ainda abertas

- O ciclo de vida de Q não está implementado/validado de forma abrangente para todas as instruções e prefixos. Em particular, investigar DD/FD antes de SCF/CCF.
- WZ/MEMPTR ainda não recebe atualização correta em todas as famílias de instruções.
- Não há evidência registrada aqui de execução pós-correção de pytest/Ruff.
- Não foi concluída uma segunda auditoria longa com o SHA corrigido.
- A CPU ainda usa T-states nominais agregados por instrução; não é um modelo de barramento ciclo a ciclo.
- A máquina SG-1000 completa ainda não está integrada: mapa de I/O, VDP, PSG, entrada, sincronização audiovisual e frontend continuam etapas futuras.

## Documentação atualizada nesta etapa

- `README.md`: status da CPU e link para a auditoria diferencial e este registro.
- `docs/z80-differential-audit-2026-10-23.md`: relatório de resultados, famílias divergentes, regressões, limitações e procedimento de repetição.
- `docs/z80-code-audit-2026-10-22.md`: adendo com evidência diferencial nova, sem reescrever a auditoria estática original.
- `docs/z80-coverage.md`: distinção entre caminho implementado, teste presente e validação diferencial pós-correção.
- `docs/z80-undocumented-behavior-research.md`: ligação entre as hipóteses WZ/Q/DAA e as regressões concretas.
- Este arquivo: novo ponto de retomada datado de 23/10/2026.

## Próximos passos obrigatórios

1. Inspecionar o HEAD do PR e os arquivos atuais de `z80.py` e `test_z80.py`; não presumir que o ciclo de vida de Q esteja concluído.
2. Executar os testes curtos no clone local do Windows:
   ```powershell
   .\.venv\Scripts\python.exe -m pytest tests/unit/test_z80.py -q
   .\.venv\Scripts\python.exe -m pytest -q
   .\.venv\Scripts\python.exe -m ruff check .
   .\.venv\Scripts\python.exe -m ruff format --check .
   git diff --check
   ```
3. Registrar literalmente os resultados reais. Se houver falha, corrigir em mudança pequena antes de enviar o lote longo.
4. Revisar o harness do notebook v4 e confirmar que ele seleciona explicitamente o commit/branch corrigido, que inicializa WZ/Q quando fornecidos e que salva os artefatos no diretório de saída do Colab.
5. Executar a auditoria longa no Colab, registrando SHA, corpus, versão do harness, total, pass/fail, tempo e caminho dos artefatos.
6. Comparar os grupos de divergência com a linha de base. Não atualizar números pós-correção sem um ZIP/relatório realmente gerado pela nova execução.
7. Só depois de testes e auditoria revisados, decidir se o PR pode sair de draft e ser mesclado. Não mesclar automaticamente.

## Critérios de aceitação

- Testes curtos, suíte completa, Ruff e verificação de whitespace executados e registrados.
- Auditoria pós-correção confirmadamente executada sobre o SHA correto.
- Divergências remanescentes classificadas e regressões mantidas.
- Nenhuma afirmação de precisão ciclo a ciclo sem modelo de eventos de barramento e testes temporais.
- Nenhuma alteração destrutiva ou merge sem revisão explícita.

## Nota final

O projeto avançou da auditoria estática para uma primeira medição diferencial em larga escala. O principal ganho atual é ter uma linha de base quantificada e divergências agrupadas em famílias investigáveis. A correção definitiva dessas famílias ainda depende da validação local e da nova execução diferencial.
