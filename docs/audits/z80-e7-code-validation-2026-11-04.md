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

A auditoria dos nomes de testes no módulo `#[cfg(test)]` de `sequencer.rs` não encontrou testes positivos dedicados a essas fases ED. O teste ED localizado valida somente a rejeição explícita de um subopcode não suportado, sem request fantasma.

**Conclusão A:** executor atômico ED = IMPLEMENTED com testes internos dirigidos; ED microsequenciado = IMPLEMENTED em parte do dispatch, mas validação funcional positiva = NOT RUN / não demonstrada nesta revisão. Não atribuir SHORT_PASS global às famílias ED.

### Revisão B — microsequenciamento e barramento

**Arquivos examinados**
- `native/z80_cpu/src/sequencer.rs`.
- `native/z80_cpu/src/micro_ops.rs` — blob `2e3ad96764df76356f017664e956724186bdb71e`.

O sequenciador representa requests suspensíveis e fases específicas para operações ED. Os helpers de barramento atribuem durações nominais e registram operações de memória e I/O; o handshake preserva identidade do request, WAIT e validação da resposta.

Limitações encontradas:
1. `Z80BusOperation` contém somente `OpcodeFetch`, `MemoryRead`, `MemoryWrite`, `PortRead` e `PortWrite`. Não existe operação de acknowledge de INT com semântica M1/IORQ.
2. A operação `OpcodeFetch` não distingue o sinal especial de RETI necessário para notificação de daisy-chain. A fase `EdReturn` restaura IFF1 a partir de IFF2, mas não emite um evento de RETI para periféricos.
3. Os ciclos ED codificados e suas durações nominais não foram comparados aqui com traces independentes por T-state. Portanto, não há evidência TEMPORAL_PASS para ED.
4. Não foi localizada uma suíte microsequenciada ED que valide a ordem de leitura/escrita, WAIT em cada request e retomada contínua-versus-blocos.

**Conclusão B:** fases ED existentes = IMPLEMENTED; cobertura de requests ED e temporalidade independente = NOT RUN. A modelagem atual não permite representar o acknowledge de INT nem o sinal especial de RETI.

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
| ED no microsequenciador | IMPLEMENTED parcial | Handlers e fases presentes; falta suíte positiva ED dedicada |
| ED flags e estado completo | NOT RUN | Não há matriz de resultados retomáveis por família |
| ED WAIT e snapshots | NOT RUN | Handshake/codec genéricos existem; falta validação específica das fases ED |
| ED M-cycles/T-states | NOMINAL_ONLY | Durações codificadas, sem trace temporal independente |
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

Os resultados acima são evidência do workflow existente na baseline; não representam execução de uma nova suíte E7 ED. O Scheduler stress não foi aprovado.

**Execução local nesta sessão:** BLOCKED. O transporte Git no ambiente de execução falhou ao resolver `github.com` via DNS. A API GitHub permaneceu acessível para leitura remota. Não foram executados localmente `cargo fmt`, `cargo clippy`, `cargo test`, `pytest` ou Ruff nesta sessão.

## 5. Próximas ações obrigatórias para fechar E7

1. Criar uma suíte `ed_microsequencer_e7.rs` com casos positivos para cada família ED documentada e aliases, exercitando o microsequenciador — não somente o executor atômico.
2. Verificar estado completo: AF/BC/DE/HL, IX/IY, I/R, IFF1/IFF2, IM, EI delay, PC/SP, WZ, Q/P, memória e I/O.
3. Cobrir WAIT e round-trip do codec em cada request/fase ED suspensível; comparar execução contínua com execução em blocos.
4. Confrontar os resultados com oráculo independente fixado e documentar exclusões/indeterminações de flags não documentadas.
5. Representar o evento RETI/daisy-chain sem confundi-lo com RETN.
6. Implementar E7.2A–E7.2F conforme o contrato de interrupções: NMI, INT acknowledge, IM 0/1/2, HALT/EI delay e integração dos adaptadores.
7. Executar os gates completos no SHA candidato e atualizar scorecard com evidência reproduzível.

## 6. Decisão

**Não aprovar fechamento global de E7 nesta baseline.** O código contém implementação significativa, mas faltam testes positivos do caminho ED retomável, validação temporal independente e o serviço de interrupções no microsequenciador.

Nenhum código de hardware foi alterado nesta auditoria. Este relatório e a reconciliação do baseline do scorecard são alterações documentais destinadas a registrar o estado real, sem transformar ausência de evidência em aprovação.
