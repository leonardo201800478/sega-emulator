# Auditoria de código E7 — semântica ED, microsequenciador, snapshots e interrupções

**Data:** 2026-11-04  
**Baseline auditada:** `main` — `fd1d46925aecb29991580710131780749a980641`  
**Escopo:** revisão estática independente do executor atômico, microsequenciador retomável, operações de barramento, codec e evidências de testes da etapa E7.  
**Natureza:** auditoria de código e evidência remota. Não substitui execução local dos gates, comparação diferencial externa ou traces temporais independentes.

## 1. Resumo executivo

**Resultado global E7: INTEGRATION_PENDING.**

A revisão confirma que o executor atômico possui cobertura de testes dirigida para várias famílias ED. O microsequenciador também contém handlers para um subconjunto amplo de instruções ED, incluindo operações aritméticas, I/O, transferência/compare de blocos, retorno e movimentação de nibbles.

Entretanto, a presença de handlers no microsequenciador não está acompanhada por uma suíte positiva ED equivalente. A inspeção das funções de teste em `native/z80_cpu/src/sequencer.rs` encontrou somente o teste `unsupported_ed_opcode_does_not_leave_a_phantom_pending_request` explicitamente relacionado a ED; não foi localizada uma matriz positiva que execute as famílias ED pelo caminho retomável, compare estado completo, percorra WAIT e valide snapshots em cada fase.

Além disso, E7.2 permanece incompleta: o microsequenciador não arbitra NMI/INT, não possui operação de acknowledge de INT e não implementa a saída de HALT por interrupção. Esses bloqueios impedem declarar o fechamento semântico e temporal de E7.

## 2. Revisões independentes

### Revisão A — semântica arquitetural e ED

**Arquivos examinados**
- `native/z80_cpu/src/cpu.rs` — blob `c6a4cb5787fc51334a7894e4951355a95e653241`.
- `native/z80_cpu/src/sequencer.rs` — blob `fdb54e0d45b1b2e5e41178927945ce8322c4d6ed`.
- `native/z80_cpu/tests/base_sequencer_coverage.rs` — blob `0bca455a9ddc066bd209aa54b221731adbc745d4`.

**Executor atômico**

Foram localizados testes dirigidos para:
- `all_256_ed_encodings_match_nominal_cycle_vectors`;
- `every_ed_fallback_encoding_is_an_eight_tstate_no_op`;
- `ed_absolute_word_loads_and_stores_cover_all_register_pairs`;
- `ed_adc_sbc_hl_vectors_cover_signed_overflow_half_carry_and_borrow`;
- `ed_neg_aliases_produce_identical_results_and_flags`;
- `ed_interrupt_mode_aliases_select_the_documented_mode`;
- `ed_retn_reti_aliases_restore_iff1_and_pop_return_address`;
- `ed_ld_a_i_and_ld_a_r_update_flags_and_preserve_carry`;
- `ed_rrd_rld_match_nibble_transfer_and_documented_flags`;
- `ed_block_transfer_vectors_check_copy_direction_counter_flags_and_repeat`;
- `ed_block_compare_vectors_check_match_stop_repeat_and_flags`;
- `ed_block_io_vectors_check_ports_memory_counter_and_repeat`;
- `ed_in_out_register_forms_use_bc_port_and_expected_data`;
- `ed_ld_i_a_and_ld_r_a_copy_accumulator_without_touching_flags`;
- `ed_cpd_decrements_hl_and_cpdr_stops_on_match`;
- `ed_adc_sbc_hl_decode_all_four_register_pairs`;
- `ed_decrement_block_io_variants_update_hl_and_ports`.

Esses testes pertencem ao executor atômico em `cpu.rs`. Eles não demonstram que a mesma instrução está correta no microsequenciador retomável e não são um oráculo externo independente.

**Microsequenciador**

A função `execute_ed_register_control()` contém dispatch para NEG, IM 0/1/2, LD I/R, IN/OUT, transferências absolutas, operações de bloco, RETN/RETI, RRD/RLD e ADC/SBC HL. Há fases explícitas como `EdRegisterInternal`, `EdPortRead`, `EdAbsoluteTransfer`, `EdReturn`, `EdBlockTransferRead/Write`, `EdBlockCompareRead/Internal` e `EdBlockIo*`.

**Correção da revisão anterior:** a afirmação de que não havia testes positivos ED no microsequenciador estava incorreta. O blob de `sequencer.rs` examinado nesta auditoria (`fdb54e0d45b1b2e5e41178927945ce8322c4d6ed`) já continha grupos de testes positivos sob `#[cfg(test)]`. A revisão corrigida identificou **26 testes ED específicos** executados com sucesso no CI pós-merge #41775300143, no SHA `5768705658522deb46a0fb0556b51923ae8166b8`:
- ADC/SBC HL: todos os pares de registradores, M-cycles internos, WAIT e snapshots.
- IN/OUT (C): todas as codificações, trace de portas, WAIT e snapshots.
- NEG, IM 0/1/2, LD I/R e fases internas retomáveis.
- Transferências absolutas ED: oito codificações, ordem de barramento, wrap e snapshots.
- RETN/RETI: aliases, ordem da pilha, IFF1/IFF2 e snapshots.
- RRD/RLD: rotação de nibbles, flags, WAIT e snapshots.
- LDI/LDD/LDIR/LDDR e CPI/CPD/CPIR/CPDR: repetição, wrap, flags, ordem, WAIT e snapshots.
- Block I/O: oito opcodes, wrap de contador, portas, flags, WZ, WAIT e snapshots.

O job Rust registrou **216 testes unitários aprovados no crate `sega_z80_cpu`**, incluindo esses 26 casos ED. A evidência é positiva para os escopos testados, mas os testes de nome `*_differential_tests` comparam o microsequenciador ao executor atômico interno do projeto; essa comparação não é um oráculo externo independente.

**Conclusão A corrigida:** ED microsequenciado = IMPLEMENTED para as famílias cobertas, com QUICK_PASS sustentado pelos 26 testes ED no CI #41775300143. Não promover o conjunto ED inteiro a SHORT_PASS global, DIFFERENTIAL_PASS ou TEMPORAL_PASS: ainda faltam cobertura comprovada de todos os campos/casos elegíveis, oráculo externo independente e traces elétricos independentes por T-state.

### Revisão B — microsequenciamento e barramento

**Arquivos examinados**
- `native/z80_cpu/src/sequencer.rs`.
- `native/z80_cpu/src/micro_ops.rs` — blob `2e3ad96764df76356f017664e956724186bdb71e`.

O sequenciador representa requests suspensíveis e fases específicas para operações ED. Os helpers de barramento atribuem durações nominais e registram operações de memória e I/O; o handshake preserva identidade do request, WAIT e validação da resposta.

Limitações encontradas:
1. `Z80BusOperation` contém somente `OpcodeFetch`, `MemoryRead`, `MemoryWrite`, `PortRead` e `PortWrite`. Não existe operação de acknowledge de INT com semântica M1/IORQ.
2. A operação `OpcodeFetch` não distingue o sinal especial de RETI necessário para notificação de daisy-chain. A fase `EdReturn` restaura IFF1 a partir de IFF2, mas não emite um evento de RETI para periféricos.
3. Os ciclos ED codificados e suas durações nominais não foram comparados aqui com traces independentes por T-state. Portanto, não há evidência TEMPORAL_PASS para ED.
4. Os 26 testes ED identificados validam ordem de requests em escopos específicos, WAIT e round-trip de snapshots nas fases cobertas. Não constituem, porém, trace elétrico independente por T-state nem comparação externa. A equivalência contínua-versus-blocos não está demonstrada de forma global para todas as famílias ED.

**Conclusão B corrigida:** handlers, requests e fases ED = IMPLEMENTED nos escopos cobertos; validação funcional positiva = QUICK_PASS nesses casos. A temporalidade ED independente permanece NOT RUN. A modelagem ainda não representa o acknowledge de INT nem o sinal/evento especial de RETI para daisy-chain.

### Revisão C — estado, snapshots e interrupções

**Arquivos examinados**
- `native/z80_cpu/src/lib.rs` — schema `Z80_MICRO_SEQUENCER_STATE_VERSION = 28`.
- `native/z80_cpu/src/snapshot.rs` — blob `7222d5a5416517666cccada8e54e61f54fb4d3da`.
- `native/z80_cpu/src/sequencer.rs`.
- `docs/audits/z80-e7-2-interrupt-contract-2026-11-04.md`.

O estado e o codec já carregam `iff1`, `iff2`, `interrupt_mode`, `ei_delay`, `nmi_pending`, `int_pending` e `halted`. Isso preserva campos arquiteturais existentes, mas não significa que exista uma continuação de serviço NMI/INT.

Confirmações:
- `begin_instruction()` inicia diretamente um request `OpcodeFetch`; não faz arbitragem NMI > INT elegível > fetch normal.
- Não há API pública de entrada de NMI/INT no microsequenciador.
- Não há fases de serviço NMI, empilhamento retomável do PC por NMI, acknowledge INT ou dispatch IM0/IM1/IM2 no microsequenciador.
- A retenção M1 de HALT está implementada, mas a saída de HALT por interrupção permanece ausente.
- `retire()` consome `ei_delay` após a instrução seguinte, mas não existe arbitragem de interrupção que use esse estado.
- O teste atômico `z80_nmi_halt_semantics.rs` cobre parte do resultado arquitetural final, não o caminho microsequenciado, WAIT ou snapshots de serviço.

**Conclusão C:** serialização de campos preparatórios = IMPLEMENTED; serviço retomável de NMI/INT, EI delay ligado à arbitragem e saída de HALT = NOT IMPLEMENTED no microsequenciador.

## 3. Matriz de evidência E7

| Área | Estado nesta auditoria | Evidência / lacuna |
|---|---|---|
| BASE estrutural | IMPLEMENTED | Inventário de 252 codificações no teste BASE; não prova semântica integral |
| ED no executor atômico | IMPLEMENTED | Testes dirigidos ED em `cpu.rs`; sem diferencial externo |
| ED no microsequenciador | IMPLEMENTED / QUICK_PASS (escopos testados) | 26 testes ED positivos no CI #41775300143; comparação interna com executor atômico e vetores dirigidos |
| ED flags e estado completo | PARTIAL | Estado arquitetural, flags e efeitos cobertos por família; não há matriz completa de todos os campos/casos elegíveis |
| ED WAIT e snapshots | QUICK_PASS (fases cobertas) | WAIT e round-trip testados em requests/fases ED selecionados; não generalizar a todas as fases |
| ED M-cycles/T-states | NOMINAL_ONLY | M-cycles/T-states e ordem de requests testados internamente; sem trace elétrico independente por T-state |
| RETN/RETI | PARTIAL | Restauração IFF1 presente; sinal/evento RETI para daisy-chain não representado |
| HALT hold M1 | IMPLEMENTED | E7.1 integrada pela PR #490 |
| NMI no microsequenciador | NOT IMPLEMENTED | Sem arbitragem/fases de serviço retomável |
| INT acknowledge / IM 0/1/2 | NOT IMPLEMENTED | Sem operação M1/IORQ e dispatch temporal |
| EI delay + arbitragem | NOT IMPLEMENTED | Contador existe; aceitação de INT não o consulta |
| E7 global | INTEGRATION_PENDING | Critérios E7.2 e validação ED retomável permanecem pendentes |

## 4. Gates e CI observados

Workflow da `main`: [#41768733643](https://github.com/leonardo201800478/sega-emulator/actions/runs/41768733643), commit `fd1d46925aecb29991580710131780749a980641`.

| Gate/job | Resultado observado |
|---|---|
| Rust fmt | PASS |
| Rust Clippy | PASS |
| SMS / SG-1000 / workspace tests | PASS |
| Python pytest / Ruff | PASS |
| Windows x64 D3D12 / WASAPI / XAudio2 | PASS |
| Scheduler deterministic stress | SKIPPED |

O CI pós-merge #41775300143 foi executado sobre o SHA `5768705658522deb46a0fb0556b51923ae8166b8`: Rust quality/unit regression PASS (incluindo 216 testes unitários do crate Z80, com os 26 casos ED acima), Python lint/regression PASS e Windows x64 D3D12/WASAPI/XAudio2 PASS. Scheduler deterministic stress SKIPPED; não contabilizar como PASS.

**Execução local nesta sessão:** BLOCKED. O transporte Git no ambiente de execução falhou ao resolver `github.com` via DNS. A API GitHub permaneceu acessível para leitura remota. Não foram executados localmente `cargo fmt`, `cargo clippy`, `cargo test`, `pytest` ou Ruff nesta sessão.

## 5. Próximas ações obrigatórias para fechar E7

1. Manter a cobertura ED existente rastreável por família; ampliar somente onde a matriz de evidências apontar lacuna concreta, sem criar uma segunda suíte redundante.
2. Auditar os campos arquiteturais ainda não cobertos de forma completa por família: AF/BC/DE/HL, IX/IY, I/R, IFF1/IFF2, IM, EI delay, PC/SP, WZ, Q/P, memória e I/O.
3. Planejar comparação ED com oráculo externo fixado, distinguindo-a da comparação atual com o executor atômico interno; documentar flags indeterminadas e divergências.
4. Representar o evento RETI/daisy-chain sem confundi-lo com RETN.
5. Implementar E7.2A–E7.2F conforme o contrato de interrupções, começando por NMI retomável; depois INT acknowledge, IM 0/1/2, HALT/EI delay e integração dos adaptadores.
6. Executar os gates completos no SHA candidato e atualizar scorecard/auditoria com evidência reproduzível.

## 6. Decisão

**Não aprovar fechamento global de E7 nesta baseline.** Há validação funcional positiva do caminho ED retomável em 26 casos específicos, mas permanecem lacunas de cobertura completa, oráculo diferencial externo, traces temporais independentes e serviço de interrupções no microsequenciador. E7.2 continua pendente.

Nenhum código de hardware foi alterado nesta auditoria. Este relatório e a reconciliação do baseline do scorecard são alterações documentais destinadas a registrar o estado real, sem transformar ausência de evidência em aprovação.
