# Auditoria de resolução geral dos prefixos Z80 — E6

**Data:** 2026-11-04  
**PR:** [#486 — E6 general prefix resolution](https://github.com/leonardo201800478/sega-emulator/pull/486)  
**Baseline main antes da E6:** `f4130104fa971289507c0b15ab2d8eabd1dd5b00` (E5 integrada pela PR #485)  
**HEAD com validação funcional concluída:** `b66b97cb6202b270e857f6cfb55be85a0ea62a6b`  
**HEAD atual da PR:** `6b533a186b1f3d1891216778ba8332e394470857`  
**CI funcional anterior:** [#41757983537](https://github.com/leonardo201800478/sega-emulator/actions/runs/41757983537) — Rust, Python e Windows x64 PASS; Scheduler SKIPPED.  
**CI do HEAD atual:** #41758908964 — execução pendente no momento desta revisão.

## 1. Objetivo e limites

Concluir a resolução geral DD/FD/ED/CB no microsequenciador Z80 compartilhado, sem duplicar decoders ou handlers de hardware.

A E6 fecha o roteamento dos prefixos e o fallback para instruções BASE/ED que já possuem implementação no sequenciador. Não amplia o universo de instruções BASE semanticamente implementadas. OpCodes ainda ausentes continuam retornando `UnsupportedOpcode` e devem ser tratados na auditoria E7.

## 2. Implementação

### 2.1 DD/FD e fallback BASE

- OpCodes classificados como `Ignored` seguem diretamente ao dispatch BASE com o byte terminal já obtido.
- Não é emitido novo fetch nem releitura do opcode.
- O estado IX/IY permanece como contexto de timing/snapshot durante a instrução prefixada; a classificação `Ignored` impede a substituição semântica indevida de IX/IY e H/L.
- Os custos M1 dos prefixos são preservados em T-states, M-cycles, PC e R.
- Last-prefix-wins permanece válido em sequências repetidas e alternadas, sem limite artificial de prefixos.

### 2.2 ED depois de DD/FD

- ED assume o decoder existente e cancela a seleção semântica de IX/IY.
- Os M-cycles adicionais dos DD/FD anteriores são preservados nas fases ED retomáveis.
- Transições internas ED que antes atribuíam valores absolutos a `mcycle_index` foram ajustadas para manter o offset dos prefixos anteriores, preservando o comportamento sem prefixos.
- A validação de snapshots distingue o M-cycle-base de cada fase ED e aceita o offset adicional de M1 quando ele está representado no estado.

### 2.3 CB

- DD/FD + CB continua na rota indexed-CB DDCB/FDCB concluída na E5.
- A E6 reutiliza essa implementação, sem criar um segundo decoder.

## 3. Cobertura de testes E6

Suíte nova: `native/z80_cpu/tests/prefix_fallback_e6.rs` — **9/9 PASS** no CI #41757983537.

| Caso | Evidência |
|---|---|
| DD/FD simples, repetidos e alternados | Fallback BASE com EX DE,HL; preserva HL/IX/IY e last-prefix-wins |
| WAIT e snapshots dos fetches | WAIT aditivo e round-trip antes/depois dos fetches |
| Operando BASE imediato | MemoryRead non-M1 sem novo opcode fetch; PC/R preservados |
| ALU imediato BASE | ADD A,n com WAIT e snapshot do operando |
| Desvio relativo BASE | Operand read, ciclo interno e snapshot |
| BASE memory read | LD A,(BC) com snapshot |
| BASE memory write | LD (DE),A com snapshot |
| DD ED absolute transfer | LD (nn),BC reutiliza ED e preserva snapshots nos quatro estágios |

Suíte `native/z80_cpu/tests/prefix_fsm.rs`:
- Expectativa de fallback DD EB atualizada.
- Cadeia de 257 prefixos repetidos/alternados, sem limite artificial.
- Valida endereço do operando, PC, R, M-cycles e T-states.

Regressões preservadas:
- Z80 unit tests: 216/216 PASS.
- Indexed-CB E5: 8/8 PASS.

## 4. Gates e evidências

CI #41757983537 no HEAD `b66b97cb6202b270e857f6cfb55be85a0ea62a6b`:

| Gate | Resultado |
|---|---|
| `cargo fmt --all -- --check` | PASS |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | PASS |
| `cargo test --workspace --locked` | PASS |
| `python -m pytest` | PASS |
| `python -m ruff check .` | PASS |
| `python -m ruff format --check .` | PASS |
| Windows x64 D3D12 compile + video-runtime tests | PASS |
| Windows x64 WASAPI/XAudio2 compile + audio-output tests | PASS |
| Scheduler deterministic stress | SKIPPED — a PR não altera o Scheduler |

O HEAD `6b533a186b1f3d1891216778ba8332e394470857` contém o refinamento da validação do offset de M-cycles nas fases ED e aguarda a execução final do CI #41758908964.

## 5. Critérios de aceitação E6

- [x] DD/FD simples e repetidos/alternados.
- [x] Last-prefix-wins.
- [x] Fallback BASE sem refetch.
- [x] Sem substituição IX/IY/H/L para opcodes ignorados.
- [x] PC, R, M1 e custo temporal dos prefixos preservados.
- [x] WAIT e snapshots nos fetches e nas fases BASE testadas.
- [x] DD/FD + ED reutiliza decoder ED e preserva M-cycles nas fases testadas.
- [x] DD/FD + CB reutiliza DDCB/FDCB E5.
- [x] Cadeia longa sem limite artificial (257 prefixos).
- [x] Scorecard, matriz semântica e estratégia de resolução atualizados.

## 6. Limitações remanescentes — fora do escopo E6

- Instruções BASE ainda não implementadas continuam `UnsupportedOpcode`.
- E7 deve auditar integralmente BASE/CB/ED/DD/FD/DDCB/FDCB, flags, WZ/MEMPTR, interrupções, HALT e todos os estados arquiteturais.
- ZEXDOC/ZEXALL, SingleStepTests em revisão fixada e oráculos independentes: NOT RUN nesta E6.
- LONG_PASS, DIFFERENTIAL_PASS e TEMPORAL_PASS: não reivindicados.
- Integração temporal sistêmica com Scheduler, barramentos e Master System permanece INTEGRATION_PENDING.

## 7. Próxima etapa

Após CI final aprovado, merge da PR #486 e confirmação do novo SHA remoto da main, iniciar exclusivamente **E7 — auditoria e fechamento semântico integral do Z80**.