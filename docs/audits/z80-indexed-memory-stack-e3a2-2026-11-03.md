# Auditoria E3A2 — transferências absolutas, pilha e troca IX/IY

**Data:** 2026-11-03  
**Baseline main de entrada:** `bcc7ef7954bd8cadd148b22c464260810ff63fb3` (após PR #466 e PR #469)  
**Candidato funcional de código:** `5cc45b9150006f3a53beb1493dc5bcc4be3965ae`; **candidato final sincronizado/documentado:** `c102f0b3e1e49b68b7961d4847c0886aa77932d5` — PR #470  
**CI:** [run 41727716404](https://github.com/leonardo201800478/sega-emulator/actions/runs/41727716404) no candidato de código e [run 41728166513](https://github.com/leonardo201800478/sega-emulator/actions/runs/41728166513) no candidato final sincronizado; ambos 4/4 workflows PASS  
**Escopo:** microsequenciador compartilhado em `native/z80_cpu/src/sequencer.rs`. Não representa fechamento integral do Z80 nem integração temporal completa SMS/SG-1000.

## 1. Instruções e semântica implementadas

| Instrução | M-cycles / T-states nominais | Ordem de barramento e commit |
|---|---:|---|
| `LD (nn),IX/IY` | 6 / 20 | Fetch DD/FD + opcode; operandos nn low/high; write low em nn; write high em nn+1. |
| `LD IX/IY,(nn)` | 6 / 20 | Fetch DD/FD + opcode; operandos nn low/high; read low em nn; read high em nn+1; commit do par após a segunda leitura. |
| `PUSH IX/IY` | 4 / 15 | Após o ciclo interno, decrementa SP e escreve high; decrementa SP novamente e escreve low. |
| `POP IX/IY` | 4 / 14 | Read low em SP; incrementa SP; read high em SP+1; incrementa SP e só então faz commit do par. |
| `EX (SP),IX/IY` | 6 / 23 | Read low SP; read high SP+1; 1 T-state interno; write low do par antigo em SP; write high em SP+1; 2 T-states internos antes do commit. SP não muda. |

Os tempos nominais acima seguem o Zilog *Z80 CPU User Manual* UM008011-0816: transferências absolutas pp. 105–111; PUSH p. 117; POP p. 121; EX (SP),IX/IY p. 128. As tabelas de operação confirmam little-endian, ordem dos bytes e contagem nominal de M-cycles/T-states. Os WAITs aumentam T-states sem inventar M-cycles.

## 2. Contratos de estado e bordas

- Prefixos DD/FD repetidos e alternados preservam last-prefix-wins e a contagem M1 acumulada. O microsequenciador não zera M-cycle index ao entrar nas fases de PUSH/EX.
- `nn+1`, `SP-1`, `SP-2` e `SP+1` usam wrap de 16 bits.
- PUSH/POP/EX preservam flags; PC consome todos os bytes de prefixo/opcode; PUSH/POP preservam WZ segundo o comportamento BASE já modelado.
- Transferências absolutas atualizam WZ para nn+1. EX atualiza WZ com a palavra lida da pilha, seguindo o comportamento BASE já existente. O manual Zilog não especifica todos os efeitos não documentados de WZ/MEMPTR; esses pontos não são apresentados como captura direta de silício.
- POP não altera IX/IY antes da leitura do byte alto; EX não altera IX/IY antes do fim dos T-states internos finais.
- WAIT foi injetado em leituras e escritas; snapshots foram restaurados em fases internas e com requests de barramento pendentes.

## 3. Testes direcionados

| Arquivo | Cobertura |
|---|---|
| `indexed_absolute_transfer_e3a2a.rs` | 5 testes: IX/IY store/load, little-endian, WAIT, snapshots, prefixos alternados e wrap FFFF→0000. |
| `indexed_stack_e3a2b.rs` | 4 testes: PUSH/POP IX/IY, ordem high/low, WAIT, SP wrap, flags/WZ/PC, snapshots e M-cycle progression com prefixos repetidos. |
| `indexed_exchange_stack_e3a2c.rs` | 2 testes: EX (SP),IX/IY, ordem read/write, WAIT, SP preservado, wrap SP+1, WZ, flags/PC, snapshots e M-cycle progression. |
| `prefix_fsm.rs` | Ajuste de fixtures antigas que esperavam rejeição de `DD/FD + 0x22`, agora implementado; fallback ainda é testado com uma família indexada pendente. |

## 4. Evidência de validação

| Gate | Resultado |
|---|---|
| `cargo fmt --all -- --check` | PASS |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | PASS |
| Testes direcionados SMS | PASS |
| Testes direcionados SG-1000 | PASS |
| `cargo test --workspace --locked` | PASS |
| `python -m pytest` | PASS |
| `python -m ruff check .` | PASS |
| `python -m ruff format --check .` | PASS |
| Windows x64 — D3D12/video | PASS |
| Windows x64 — WASAPI/XAudio2/audio | PASS |
| Scheduler deterministic stress | PASS |

Os testes de código foram executados no candidato `5cc45b9150006f3a53beb1493dc5bcc4be3965ae`; a execução integral foi repetida após sincronização com a main e atualização documental no candidato `c102f0b3e1e49b68b7961d4847c0886aa77932d5`. Ambos os workflows concluíram 4/4 PASS.

## 5. Limites e estado formal

- E3A2: **IMPLEMENTED / QUICK_PASS / SHORT_PASS** no candidato PR #470.
- E3 completo (E3A1 + E3A2): implementado no microsequenciador para as operações de 16 bits IX/IY.
- Não executados nesta fatia: ZEXDOC, ZEXALL, validação diferencial independente completa, traces de barramento por T-state contra hardware, execução de ROMs/golden masters e integração CPU→SmsTemporalBus/SG-1000.
- `LONG_PASS`, `INTEGRATION_PENDING` e `TEMPORAL_PASS` global não são concedidos por esta auditoria.
- Próxima fase: E4 — instruções indexadas com displacement (IX+d)/(IY+d). DDCB/FDCB permanece E5; fallback de prefixos ignorados permanece E6.
