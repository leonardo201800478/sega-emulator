# Auditoria de integração das PRs abertas — 2026-11-05

**Baseline auditada:** `main` = `1da784f556b9524f5a6f43ce8a7c525e68354f9e`.  
**Escopo:** todas as PRs abertas no repositório na data da auditoria.  
**Regra de integração:** somente código/documentação revisados, reconciliados com a main e com gates aplicáveis PASS no HEAD exato. `SKIPPED`, `NOT RUN`, CI de SHA anterior e descrição do autor não equivalem a PASS.

## Resultado executivo

Foram encontradas **9 PRs abertas**. **Nenhuma estava apta para merge nesta auditoria.** Duas PRs não-Draft (#447 e #424) ainda contêm pendências declaradas e divergência substancial da main; as demais são Draft, propostas incompletas ou o contrato permanente que deve permanecer aberto até o encerramento de todo o projeto.

Nenhuma PR foi mesclada ou fechada nesta operação. Não foi feito commit direto na main. A ausência de merge é deliberada: não há justificativa técnica para promover branches antigas e divergentes sem reconciliação e validação do resultado final.

## Inventário e decisão

| PR | Estado | Divergência em relação à main auditada | Validação observada no HEAD consultado | Decisão |
|---|---|---:|---|---|
| [#533 — contrato permanente de conclusão](https://github.com/leonardo201800478/sega-emulator/pull/533) | Draft | 1 commit à frente / 28 atrás | Jobs de build/teste SKIPPED; classificação de impacto PASS | **Manter Draft e aberta.** É o gate permanente final e só pode ser concluída após P0–P9. A branch contém apenas `docs/project-completion-contract.md`. |
| [#530 — contrato temporal multiplataforma](https://github.com/leonardo201800478/sega-emulator/pull/530) | Draft | 4 à frente / 29 atrás | Apenas classificação de impacto PASS; demais jobs SKIPPED | **Não integrar ainda.** Documento propositivo com baseline antiga; revisar cada obrigação temporal contra ADR-0016, remover qualquer interpretação de microsequenciamento universal e atualizar referências antes de reconsiderar. |
| [#527 — auditorias independentes E7](https://github.com/leonardo201800478/sega-emulator/pull/527) | Draft | 4 à frente / 31 atrás | Jobs de teste/build SKIPPED; classificação de impacto PASS | **Não integrar ainda.** Conteúdo documental útil, mas precisa de revisão das evidências, da cadeia de independência dos oráculos, dos achados C1 e do scorecard contra a main atual. Não tratar seus relatórios como autoridade normativa enquanto não revisados. |
| [#526 — MachineInstant racional](https://github.com/leonardo201800478/sega-emulator/pull/526) | Draft | 54 à frente / 31 atrás | Rust, Scheduler stress, PyO3 e classificação PASS; Python/Windows/lock refresh SKIPPED | **Não integrar.** A própria PR declara C1 incompleta, overflow racional intermediário ainda não resolvido formalmente e migração pendente de Scheduler.now, clock domains e consumidores. Além disso, a ADR-0016 não autoriza timeline racional sem dependência observável concreta. |
| [#514 — M-cycles por bordas T-state SMS](https://github.com/leonardo201800478/sega-emulator/pull/514) | Draft | 27 à frente / 75 atrás | Rust, Python e Windows D3D12 PASS; Scheduler stress SKIPPED | **Não integrar.** A própria auditoria da PR identifica lacunas de fase WAIT/TW: amostragem falling-edge, TW automático de I/O e WAIT durante TW obrigatório. E7.4.1B permanece INCOMPLETE/BLOCKED. |
| [#512 — contrato retomável de M-cycle](https://github.com/leonardo201800478/sega-emulator/pull/512) | Draft | 7 à frente / 75 atrás | Rust, Python, Windows D3D12 e Scheduler stress PASS no HEAD reportado | **Não integrar.** Contrato ainda não conectado ao sequenciador/barramento/Scheduler; revisão identifica WAIT não amostrado durante TW obrigatório, TW automático I/O/INT ausente e falta de falling-edge sampling. CI verde não fecha essas lacunas semânticas. |
| [#447 — sequenciar LD IX/IY imediato e operações indexadas](https://github.com/leonardo201800478/sega-emulator/pull/447) | Aberta, não-Draft | 68 à frente / 390 atrás | Rust, Python, Scheduler stress e Windows D3D12 PASS no HEAD consultado | **Não integrar.** A descrição declara pendentes diferenciais, matriz completa DD/FD, flags não documentadas e validação de barramento; declara explicitamente não fazer merge antes dessas pendências. O branch está muito divergente e o runtime SMS normal não consome o caminho experimental. |
| [#424 — SRAM do Sega mapper SMS](https://github.com/leonardo201800478/sega-emulator/pull/424) | Aberta, não-Draft | 51 à frente / 528 atrás | Rust, Python e Windows D3D12 PASS; Scheduler stress SKIPPED | **Não integrar.** A própria PR declara ausência de teste in-game, configuração automática de capacidade/caminho e integração de metadata pendentes, além de manter INTEGRATION_PENDING. O branch está extremamente divergente e exige port seletivo/revisão contra o código atual, não merge direto. |
| [#338 — auditoria VDP e backends gráficos nativos](https://github.com/leonardo201800478/sega-emulator/pull/338) | Draft | 14 à frente / 1086 atrás | Rust quality/unit regression FAIL; Python PASS | **Não integrar.** CI Rust falhou, trabalho de auditoria/backend está incompleto e a branch está muito divergente. O plano de backends não deve antecipar fases nem misturar apresentação com o runtime temporal do VDP. |

## Observações técnicas por grupo

### PRs de Z80 / temporal

- E5/E6 e os incrementos já integrados permanecem no histórico da main; não reintroduzir código antigo por cherry-pick sem comparação semântica.
- #512 e #514 são partes encadeadas da proposta E7.4.1, mas as duas têm bloqueios explícitos de WAIT/TW e não devem ser integradas separadamente.
- #526 é uma migração estrutural de alto impacto em 10 arquivos, com 31 commits de divergência em relação à main e risco de alterar o contrato temporal sem consumidor demonstrado. Manter isolada até que um caso observável exija a mudança.
- #530 é documental e não altera runtime, mas descreve um contrato mais abrangente do que a necessidade já estabelecida pela ADR-0016. Documento técnico pode informar futuras tarefas, mas não pode sobrepor a ADR nem criar gates universais.
- #527 pode preservar resultados de pesquisa como auditoria histórica, mas suas afirmações precisam ser confrontadas com as fontes e com o estado atual antes de adotar qualquer conclusão.

### SMS / VDP / SRAM

- #424 contém funcionalidade potencialmente útil, mas a ausência de teste in-game e o grau de divergência impedem considerá-la pronta. Próxima ação correta: reavaliar o requisito SRAM e portar somente a implementação ainda ausente, em branch nova baseada na main atual, com testes de mapper, save/load e regressões SMS/SG-1000.
- #338 não é uma implementação concluída de renderer/backend. A falha de Rust CI é bloqueio objetivo; o escopo amplo deve ser decomposto em tarefas pequenas após auditoria VDP e evidência de gameplay.

## Estado Z80 integrado na baseline

Na main auditada, as PRs #531 e #536 estão integradas:
- #531: caminho VDP SMS → linha INT do Z80, VBlank/Line IRQ e aceitação/acknowledgement cobertos por testes de integração.
- #536: semântica funcional do RESET Z80, limpeza/preservação de estado e monotonicidade temporal da máquina cobertas por testes unitários e de integração.

Isso **não** encerra E7. A auditoria semântica global e a divergência das flags de I/O em bloco continuam abertas conforme a matriz Z80. E8 permanece bloqueada até fechamento formal de E7; não declarar validação diferencial completa nem TEMPORAL_PASS sistêmico.

## Ações recomendadas após esta auditoria

1. Preservar #533 como Draft permanente.
2. Não tentar merge/rebase automático das branches antigas.
3. Fazer revisão documental isolada de #530 e #527, reaproveitando apenas conteúdo comprovado e alinhado à ADR-0016.
4. Reabrir #447 como trabalho de port seletivo apenas se a auditoria do código atual confirmar lacuna funcional real; não presumir que a branch antiga representa a próxima tarefa.
5. Reavaliar #424 em branch nova sobre a main, após confirmar o escopo P1 e o formato de metadata/host para SRAM.
6. Manter #512/#514/#526 em espera até evidência de gameplay que exija essa granularidade temporal.
7. Corrigir a falha Rust e reduzir o escopo de #338 antes de qualquer nova revisão.
8. Executar os gates completos no SHA final de cada candidato e classificar individualmente PASS/FAIL/BLOCKED/NOT RUN.

## Limites desta auditoria

A auditoria usou os metadados/diffs das PRs, comparação de commits com a main e os check-runs reportados pelos HEADs consultados. Não executou localmente cargo/pytest nem testou jogos/ROMs. Os resultados de CI foram lidos como evidência do SHA reportado, não como validação do estado atual da main. Onde a PR declara testes in-game, diferenciais ou integrações como pendentes, isso foi mantido como pendência, sem inferir sucesso.


---

## Aditivo de PRs E7.7 — atualização após auditoria dos HEADs (2026-11-05)

Esta atualização substitui a fotografia operacional anterior das PRs #538–#540, preservando o histórico acima. Nenhuma PR foi mesclada.

| PR | HEAD atual consultado | CI / classificação | Estado |
|---|---|---|---|
| #537 | `26a3b9fe13a2797281a262d7e1dd7607e8158e12` | Run #42025922287 foi iniciado no HEAD anterior `c92643f1036ba90a9e780a389067134986423eb9`; o HEAD atual contém somente documentação adicional e não tem run próprio observado | OPEN; HEAD atual NOT RUN |
| #538 | `6268d586c84c9e96a4b3edb0ec63479bdfb62ad9` | O código/teste passou no HEAD anterior `4a78a04c0851acef3951f0faa78065a9f36d601b`; o HEAD atual contém apenas remoção da sobreposição no scorecard e ainda não tem CI observado | OPEN; HEAD atual NOT RUN |
| #539 | `f3afc000c2805559d83d06b693ed1f2f634cf39b` | Run #42014043705: Rust, Python, PyO3 e Windows D3D12 PASS; Scheduler stress, Qt, PowerShell e lock refresh SKIPPED | OPEN |
| #540 | `6a7fadfc245ac40c44ebfdfaa8fd675ae7123c63` | Run #42015058399: Rust, Python, PyO3 e Windows D3D12 PASS; Scheduler stress, Qt, PowerShell e lock refresh SKIPPED | OPEN |
| #541 | `5f60c1f4fa0357c05379f5e1d5857ca49f267fbb` | Run #42025639950: Rust (fmt, Clippy, SMS, SG-1000 e workspace), Python, PyO3 e Windows D3D12 PASS; Scheduler stress, Qt, PowerShell e lock refresh SKIPPED | OPEN |

### Decisões e limites

- **#538:** 48.000 combinações determinísticas para os oito opcodes Block I/O. O teste é uma referência matemática independente dos helpers de produção, derivada de pesquisa NMOS; não é oráculo de silício. O CI PASS refere-se ao HEAD anterior indicado, não ao HEAD atual após a remoção da edição concorrente do scorecard.
- **#539:** correção localizada do latch `p` para o erratum P/V de LD A,I/R no aceite de INT. O CI PASS confirma a suíte no HEAD exato; hardware físico e equivalência CMOS/clones permanecem NOT RUN/NOT VERIFIED.
- **#540:** 126 casos da família LD r,r' DD/FD. Não cobre ALU, INC/DEC ou todas as famílias DD/FD.
- **#541:** 10.816 casos ALU por amostragem de 13 valores de A e 13 operandos, e 4.096 casos INC/DEC para todos os bytes/carry. O oráculo ALU/INC/DEC compara com H/L BASE que compartilha helpers; portanto não é validação aritmética independente de silício. A correção de rustfmt foi publicada no HEAD atual e o CI completo aplicável concluiu sucesso.
- **#537:** a reconciliação documental foi atualizada para centralizar a sobreposição do scorecard com #538. O run #42025922287 refere-se ao HEAD anterior `c92643f1036ba90a9e780a389067134986423eb9`; o HEAD atual `26a3b9fe13a2797281a262d7e1dd7607e8158e12` não possui CI próprio observado.
- Jobs SKIPPED permanecem SKIPPED, não PASS. Nenhuma validação local foi executada nesta sessão: o ambiente não possui `cargo`, `rustc` nem `pwsh`; os scripts `project.ps1 audit/locks/validate` permanecem NOT RUN.
- A ausência de Scheduler stress/Qt/PowerShell/lock refresh deve ser mantida explícita. Sua obrigatoriedade precisa ser avaliada por impacto/paths e regras do workflow, não promovida a PASS.

### Estado global

E7.7 continua IN_PROGRESS. E8 permanece BLOCKED. Não há autorização técnica para merge nesta fotografia: #537 ainda está em CI; #538 teve alteração posterior sem CI no HEAD atual; #539–#541 têm CI convencional aplicável PASS, mas as lacunas semânticas globais DD/FD permanecem abertas.
