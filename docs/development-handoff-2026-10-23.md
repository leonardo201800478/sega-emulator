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
4. Revisar o harness do notebook de auditoria direcionada e confirmar que ele seleciona explicitamente o commit/branch corrigido, que inicializa WZ/Q quando fornecidos e que salva os artefatos no diretório de saída do Colab.
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


---

## Continuação — Q/WZ e regressões direcionadas (23/10/2026)

- Branch: `fix/z80-single-step-audit`.
- HEAD confirmado: [`a21dfdcbc02b09b7507e109b5a28be09a3674f52`](https://github.com/leonardo201800478/sega-emulator/commit/a21dfdcbc02b09b7507e109b5a28be09a3674f52).
- CI: [execução 39904100544](https://github.com/leonardo201800478/sega-emulator/actions/runs/39904100544) aprovada; Ruff format/check aprovados e **367 testes passaram em 5,41 s**.
- PR #14 continua aberto como draft; não houve merge nem alteração direta em `main`.

### Mudanças na revisão atual

- Corrigida a regra de `DAA` para cobrir a regressão `A=F3h, F=4Eh`, mantendo o ajuste de nibble e o carry coerentes com os oráculos presentes no repositório.
- Centralizadas as escritas internas de flags por `_set_flags`, que atualiza `Q`; o início de uma instrução limpa Q, preservando o valor anterior durante a decodificação aninhada de prefixos.
- Adicionadas regressões para Q após instrução que preserva flags, após instrução que modifica flags e com prefixo DD ignorado.
- Implementadas atualizações parciais de `WZ/MEMPTR` em acessos indiretos/absolutos, operações de 16 bits, saltos/chamadas/retornos, I/O imediato e por C, acessos indexados e parte das instruções ED/de bloco.
- Adicionados testes de WZ para acessos de memória, `ADD HL,BC`, `JP nn`, `LD HL,(nn)` e `LD A,(IX+d)`.
- Atualizados `docs/z80-undocumented-behavior-research.md` e `docs/z80-coverage.md` para distinguir implementação parcial de validação concluída.

### Validação ainda necessária

O resultado de 1.000.000 de casos (992.309 aprovados e 7.691 divergências) continua sendo a **linha de base anterior às correções**. Não existe ainda resultado diferencial pós-correção.

O notebook de auditoria v5 compara `WZ/MEMPTR` e `Q` quando o corpus fornece estado final, conta campos divergentes a partir do CSV completo e fixa o SHA esperado. Executá-lo no Google Colab e preservar o ZIP completo antes de atualizar qualquer estatística do relatório.

A implementação de WZ/Q permanece parcial até a auditoria longa e os testes externos. A temporização ciclo a ciclo e a ordem de barramento continuam fora do escopo desta correção.


### Complemento — ED-NOP e cobertura de 256 bytes ED (23/10/2026)

- Revisão de código/testes: [`88bc6d35f3c3433c01f0451688092d6f8c70f7a1`](https://github.com/leonardo201800478/sega-emulator/commit/88bc6d35f3c3433c01f0451688092d6f8c70f7a1).
- CI: [execução 39904473410](https://github.com/leonardo201800478/sega-emulator/actions/runs/39904473410) aprovada; Ruff format/check aprovados e **372 testes passaram em 5,62 s**.
- Os bytes ED sem operação atribuída agora retornam como ED-NOP de 8 T-states; aliases conhecidos continuam tratados por handlers explícitos. Os testes cobrem ED-NOPs representativos e verificam a decodificação dos 256 segundos bytes ED.
- Fontes de engenharia reversa e a ressalva sobre a ausência de especificação oficial para estes bytes foram registradas em `docs/z80-undocumented-behavior-research.md`.

A CI curta está verde nesta revisão. Ainda não foi executada a auditoria longa pós-correção; usar o notebook de auditoria direcionada fixado no SHA acima para comparar também `WZ/MEMPTR` e `Q`, preservar o CSV completo e devolver o ZIP antes de alterar a linha de base.


---

## Retomada após auditoria direcionada PR #14 — 24/10/2026

- Resultado recebido: `z80-targeted-audit-pr14-results.zip`.
- Commit efetivamente testado pelo ZIP: `27fadd182c65ffbd0ab899343ed1197c3f265515`.
- 14.000 vetores: 13.531 aprovados e 469 divergentes.
- `BIT (HL)`: 8.000/8.000 aprovados.
- `DAA`: 2.000/2.000 aprovados.
- `SCF/CCF` sem prefixo: 2.000/2.000 aprovados.
- Divergências remanescentes: 239 em `DD 37` e 230 em `DD 3F`, somente nos bits F3/F5 de `F` e no campo `Q` derivado.
- Corrigido o uso do latch Q anterior em `SCF/CCF` quando Q não é zero; adicionados testes dirigidos para SCF/CCF com prefixo DD ignorado.
- Commits da correção/testes/documentação: `5697bdbd4dea6d1cc8c495b1a12a8e58d87524c7`, `c26a66d585c091ad9137eb1e03b134fcd256216f`, `333c7a6601159f54430b183a85a9d321a6970e5c`, `1ce9c186f9efd72e156d08077367967eed2ed932`.
- A correção ainda não foi validada por CI nem pela reexecução do corpus completo de 14.000 casos.
- O notebook de repetição foi preparado para testar o commit `1ce9c186f9efd72e156d08077367967eed2ed932` e salvar os artefatos em `MyDrive/SegaEmulator/Z80TargetedAudit_PR14_R2/`.
- PR #14 continua draft e sem merge. Não declarar SCF/CCF prefixados resolvidos antes de receber o ZIP pós-correção.


### Ajustes de regressão após revisão dos casos DD — 24/10/2026

- Ajustadas as expectativas unitárias de SCF/CCF para refletir os bits F3/F5 combinados com Q anterior, inclusive quando a instrução anterior escreveu flags.
- Commit de testes: [e06727c07782285b0f9e63fe0f770471c084e870](https://github.com/leonardo201800478/sega-emulator/commit/e06727c07782285b0f9e63fe0f770471c084e870).
- Estes testes ainda aguardam execução na CI. A auditoria direcionada pós-correção também precisa ser repetida.
