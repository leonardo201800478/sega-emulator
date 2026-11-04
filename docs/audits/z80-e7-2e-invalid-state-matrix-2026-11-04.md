# Auditoria E7.2E — matriz de estados inválidos

**Data:** 2026-11-04  
**Baseline main:** `644b2c22879d63447f63b730ed9415db11e81fab`  
**PR:** [#506](https://github.com/leonardo201800478/sega-emulator/pull/506)  
**HEAD validado:** `73ab196feb09aec2b6cdf21b73ff4b7d1cc0dde1`  
**Workflow:** [CI #41860306541](https://github.com/leonardo201800478/sega-emulator/actions/runs/41860306541)  
**Estado:** PASS nos gates convencionais deste HEAD; E7.2E permanece IN_PROGRESS.

## Escopo implementado

O teste `snapshot_restore_phase_matrix_e7_2e.rs` reutiliza o corpus que alcança as 70 variantes de `Z80MicroSequencerPhase`, coleta um estado válido representativo por fase e aplica mutações inválidas.

As mutações verificadas incluem:

- `mcycle_index` e `tstate_index` fora dos limites coerentes da fase;
- request ID não alocado e `next_request_id` não monotônico;
- endereço incompatível com a fase;
- operação/payload incompatíveis;
- request com duração base zero;
- remoção de request pendente obrigatório;
- fase incompatível com request pendente;
- WAIT sem request pendente;
- remoção de opcode quando exigido.

Cada estado mutado deve ser recusado por `encode()` e por `Z80MicroSequencer::restore_state()`. O teste exige pelo menos 210 mutações no conjunto de 70 fixtures. Isso comprova a matriz exercitada, não todas as combinações possíveis de payloads e contextos.

## Defeito identificado e correção

A primeira execução da matriz identificou que fases imediatas com prefixo DD/FD ignorado aceitavam valores arbitrariamente altos de M-cycle, porque a validação usava apenas um limite inferior (`mcycle_index >= 2`) sem relacionar o número de M1s ao tempo acumulado.

A validação foi reforçada para derivar o mínimo de T-states a partir da contagem de M1s de prefixo, com operações aritméticas checked. O mesmo critério foi aplicado às fases imediatas correlatas que tinham a mesma forma de validação.

A matriz passou no HEAD final da PR #506.

## CI — HEAD `73ab196feb09aec2b6cdf21b73ff4b7d1cc0dde1`

| Gate | Resultado |
|---|---|
| `cargo fmt --all -- --check` | PASS |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | PASS |
| `cargo test -p sega-sms-core --locked` | PASS |
| `cargo test -p sega-sg1000-core --locked` | PASS |
| `cargo test --workspace --locked` | PASS |
| `python -m ruff format --check .` | PASS |
| `python -m ruff check .` | PASS |
| `python -m pytest -q` | PASS |
| Windows x64 D3D12/vídeo compile e testes | PASS |
| Windows x64 WASAPI/XAudio2/áudio compile e testes | PASS |
| Scheduler deterministic stress | SKIPPED |

Os resultados acima vêm do GitHub Actions; não representam execução local nesta sessão. Scheduler SKIPPED não é PASS.

## Limitações remanescentes

- Um fixture por variante de fase não equivale à validação de todas as combinações de opcode, prefixo, request e payload.
- Casos `UnsupportedOpcode` não são contados como cobertura semântica.
- WAIT 0/1/múltiplo por família de barramento e fronteiras ainda precisa de matriz própria.
- Os T-states terminais dos cenários IM0 ainda precisam ser fixados contra referência técnica.
- Não há evidência de codec binário persistente versionado para o estado completo da máquina SMS; o DTO SMS em memória não deve ser confundido com o codec binário do estado da CPU.
- A sequência integral de `SmsTemporalTraceEvent` e a equivalência sistêmica contínua/retomada continuam dependentes das próximas fatias E7.2E/E9.

**Decisão:** esta PR melhora a integridade dos snapshots e passa nos gates convencionais do HEAD registrado. Não encerra E7.2E, não encerra E7 e não autoriza iniciar E8. Não atribuir LONG_PASS, DIFFERENTIAL_PASS ou TEMPORAL_PASS.
