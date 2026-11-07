# Ledger de evidências

Cada registro deve vincular resultado a código e teste identificáveis, informar comando, ambiente, seeds/configuração, status e limitações. Use somente `PASS`, `FAIL`, `BLOCKED` ou `NOT RUN`; interrupções devem ser descritas explicitamente e nunca convertidas em PASS.

## Registros

| Evidência | Componente | Resultado |
|---|---|---|
| [Scheduler long stress — 2026-11-07](scheduler-stress-long-2026-11-07.md) | MachineScheduler | PASS — 320.000 operações + 4.000.000 dispatches |

## Regras

- Resultado local fornecido pelo usuário deve ser identificado como tal.
- Resultado de branch só é transferível à main após comparação de conteúdo dos arquivos relevantes; não presumir equivalência apenas por ancestralidade ou nome de branch.
- Resultado de teste direcionado não equivale a `cargo test --workspace`.
- Testes físicos, CI e validação de gameplay devem ser registrados separadamente.
- Registrar hashes de artefatos/traces quando existirem; não inventar artefatos ausentes.
