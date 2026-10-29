# Auditoria de estado e documentação do projeto

**Data da auditoria:** 2026-10-29  
**Repositório:** `leonardo201800478/sega-emulator`  
**Base auditada:** `main` em `5d29b1790aa7e8d7af47de9ee2563a1c978b62d7`  
**Tipo:** auditoria documental e de metadados do GitHub; não é auditoria linha a linha de todos os módulos nem validação de hardware real.

## Resumo executivo

- A branch padrão `main` aponta para o commit de merge da PR #157: `5d29b1790aa7e8d7af47de9ee2563a1c978b62d7`.
- A PR #157 integrou a consolidação do Z80 SG-1000, com cinco arquivos alterados e nenhum arquivo em `native/sms_core/`.
- Os seis workflows associados ao candidato `baa56568ca2ccbb3963b86207c49fb12f32ab342` foram reportados como concluídos com sucesso na PR #157.
- O mantenedor confirmou manualmente que jogos SG-1000 funcionam e que não percebeu regressões.
- Essas evidências não provam compatibilidade universal, correção integral da ISA Z80, equivalência com hardware real ou precisão ciclo a ciclo.
- A prioridade atual é concluir o core SMS. As issues #158 e #159 definem as próximas auditorias de CPU.

## Estado por subsistema

| Subsistema | Estado observado | Limite / próximo passo |
|---|---|---|
| SG-1000 runtime Rust | Funcional no escopo manual reportado | Refinar CPU e temporização posteriormente; preservar regressões |
| Z80 SG-1000 | Consolidado pela PR #157 | T-states nominais não são precisão de barramento ciclo a ciclo |
| SMS machine/core | Existe em `native/sms_core`; CPU contém implementação consolidada e testes dirigidos | Semântica global ainda não certificada; continuar auditoria #158 |
| Z80 SMS: espaços de opcode | Teste de classificação cobre sete espaços de 256 codificações | Classificação/dispatch não equivale à validação de todos os efeitos semânticos |
| Z80 SMS: referência | Há vetores fixos e suítes de referência no repositório/CI | Verificar proveniência e ancestralidade dos oráculos; não presumir independência |
| Temporalidade SMS | Há contrato de traces M-cycle/T-state implementado para um subconjunto de oito instruções ED sem DD/FD | Cobertura parcial; waits, pinos completos, scheduler e comparação independente continuam pendentes |
| Frontend Qt/PySide6 | A documentação e workflows descrevem integração com runtime Rust | Em cada alteração de binding/package, provar o caminho da extensão realmente importada |
| Game Gear, Mega Drive, Sega CD e 32X | Fora do foco atual | Não expandir escopo antes de concluir SMS |

## Evidências de validação

Workflows associados ao candidato da PR #157:

- [CI](https://github.com/leonardo201800478/sega-emulator/actions/runs/40817249815) — PASS.
- [Rust SG-1000 Workspace](https://github.com/leonardo201800478/sega-emulator/actions/runs/40817249821) — PASS.
- [SMS Core Fast Validation](https://github.com/leonardo201800478/sega-emulator/actions/runs/40817249865) — PASS.
- [SMS Z80 Platform Validation](https://github.com/leonardo201800478/sega-emulator/actions/runs/40817249819) — PASS.
- [SG-1000 Sequential Validation](https://github.com/leonardo201800478/sega-emulator/actions/runs/40817250047) — PASS; é automação CI sequencial, não três agentes independentes.
- [Qt Rust SG-1000 runtime integration](https://github.com/leonardo201800478/sega-emulator/actions/runs/40817249925) — PASS.

Os resultados pertencem ao SHA candidato indicado, não automaticamente a commits futuros. Não foi observada uma execução pós-merge para o commit de merge na consulta realizada durante esta auditoria; essa validação deve ser consultada separadamente.

## Auditoria da documentação

### Documentos atualizados nesta alteração

- `README.md`: estado atual, commit de merge, evidência funcional reportada, limites de fidelidade e foco SMS.
- `docs/architecture/migration-control-board.md`: estado corrente explicitado antes dos snapshots históricos.
- `docs/architecture/master-system-roadmap.md`: prioridade SMS e gates #158/#159.
- `docs/architecture/z80-progress-through-pr-197.md`: adendo pós-PR #157, mantendo as seções históricas associadas aos SHAs originais.
- `CHANGELOG.md`: registro da integração #157 e da prioridade seguinte.

### Riscos documentais identificados

1. Vários documentos têm snapshots datados de branches/SHAs antigos. Não é correto reescrever silenciosamente essas bases como se fossem observações atuais.
2. A documentação de migração contém checkboxes de fases históricas que podem parecer pendências atuais. O estado corrente foi acrescentado em destaque; uma revisão por fase ainda deve correlacionar cada item ao código e aos gates atuais.
3. A matriz semântica de Z80 registra cobertura de classificação e vetores dirigidos, mas não cobre todas as combinações de estados/flags/efeitos.
4. O modelo temporal é parcial: o trace não controla o scheduler, a fase de refresh é inferida e a cobertura temporal está restrita a um subconjunto.
5. Algumas auditorias históricas descrevem arquitetura proposta ou estado de PR antigo. Devem continuar rotuladas como propostas/históricas, não como especificação da implementação atual.
6. Esta revisão inspecionou metadados do repositório, README e documentos-chave de arquitetura/CPU. Não afirma que cada um dos documentos existentes foi revisado linha por linha.

## Próximos passos priorizados

1. **P0 — SMS opcode/semântica:** executar a auditoria definida na [issue #158](https://github.com/leonardo201800478/sega-emulator/issues/158), produzir matriz por opcode/formato e acrescentar vetores fixos apenas para discrepâncias reproduzíveis.
2. **P1 — interrupções/estado/tempo:** executar [issue #159](https://github.com/leonardo201800478/sega-emulator/issues/159), separando estado semântico, T-states agregados e sequência de barramento.
3. **P1 — referência independente:** documentar versões, SHA, licença e ancestralidade dos oráculos; usar um segundo oráculo apenas se a independência for defensável.
4. **P1 — gate por candidato:** executar format, Clippy, testes do workspace, validação SMS direcionada, testes de integração pertinentes e workflows aplicáveis no mesmo SHA.
5. **P2 — temporização:** expandir traces somente com contratos e vetores de aceitação concretos; não promover a `TEMPORAL_PASS` antes da comparação adequada.
6. **P3 — SG-1000:** retomar melhorias após o marco SMS, sem duplicar CPU ou reabrir mudanças já validadas sem evidência de bug.

## Classificação

- Implementação e workflows do candidato PR #157: `PASS` conforme evidências vinculadas.
- Teste manual SG-1000: `PASS` reportado pelo mantenedor.
- Completude semântica global do Z80 SMS: `IN PROGRESS`.
- Precisão ciclo a ciclo SMS: `IN PROGRESS`; não declarar `TEMPORAL_PASS`.
- Revisão linha a linha de toda a documentação do repositório: `NOT COMPLETE`; esta PR atualiza os documentos centrais e registra explicitamente o escopo da auditoria.
