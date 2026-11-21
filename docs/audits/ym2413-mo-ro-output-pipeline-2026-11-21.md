# YM2413 MO/RO output-pipeline implementation and differential

Date: 2026-11-21

Reference implementation: Nuked-OPLL commit
1269cf5a783b65583b50fa2464d08be75830aaa0, original YM2413 chip type.

## Scope

This report documents a separate serial digital output-pipeline model in
native/ym2413/src/output_pipeline.rs. It does not replace the measured-amplitude
model in rhythm_output.rs and does not claim to model the physical DAC word,
analog voltage, or silicon behavior.

The existing measured MO/RO retention service remains unchanged. The new
pipeline produces Nuked-compatible signed output-buffer integers and preserves
the channel and output-buffer state boundaries required for deterministic
replay.

## Reference call order

The pinned OPLL_Clock source establishes this order:

1. Copy the previous output_m and output_r latches into the caller's buffer.
2. Shift rm_enable.
3. Apply OPLL_DoModeWrite when the pending mode-register write is qualified.
4. Execute OPLL_Channel using the previous ch_out latch.
5. Execute OPLL_Operator, which computes the next ch_out latch.

The Rust process_cycle API mirrors these boundaries. It returns the previously
latched buffer values, computes the next channel-stage outputs from the old
ch_out, then stores the operator result supplied by the caller as the next
ch_out value.

The caller supplies a mode-write value only at the exact OPLL_DoModeWrite
phase. Address/data bus synchronization and the generation of write_d_en are
not implemented by this output-only module.

## Signed rm_enable semantics

The pinned opll.h declares rm_enable as int8_t. OPLL_Clock uses the compound
right-shift operation on that signed field. The reference build used by the
workflow performs an arithmetic right shift for negative values; the Rust
model uses i8 arithmetic shift to reproduce that observed behavior.

When register 0x0e bit 5 is written, OPLL_DoModeWrite loads bit 7 after the
per-clock shift. Under the pinned reference compiler, the signed shift
propagates the sign bit: the enabled state evolves through bit patterns
0x80, 0xc0, 0xe0, 0xf0, and eventually 0xff. A disable write clears bit 7
after the current shift while preserving the lower seven bits, so bit 6 can
remain active for one final channel stage before shifting out.

This is not modeled as a persistent boolean flag. The complete signed shift
register is part of the snapshot. C's right shift of a negative signed integer
is implementation-defined; the claim here is equivalence to the pinned
Nuked build used in the Linux CI workflow, not a universal claim for every C
compiler.

## Channel output and mute behavior

For the original YM2413 path in the pinned source:

- Start from the previous ch_out latch.
- Derive the neutral code from arithmetic ch_out >> 8.
- When ch_out is nonnegative, increment both the active code and neutral code.
- MO mute is driven by the modulator-cycle phase and the bit-6 Rhythm Mode
  pipeline gate combined with the MO cycle-coordinate condition.
- RO is neutral except for the explicit cycle set selected while bit 6 is
  active.
- The newly calculated output_m/output_r values are copied to the external
  buffer on the next OPLL_Clock invocation.

The Rust module implements the same original-YM2413 branch. It does not model
the YM2413B/YMF281B alternate mute-to-zero behavior or DS1001 behavior.

## Slot coordinates

The independent coordinates remain:

- MO slot = (Nuked cycle + 3) % 18
- RO slot = (Nuked cycle + 15) % 18

The output-pipeline comparator reports both coordinates on divergence. The
pipeline itself does not collapse them into one shared offset.

## Differential evidence

Workflow: YM2413 Nuked MO/RO output-pipeline differential
Run: 44930418493
Head: f18848a450f1209c9777cdcc8238a4db1c9c5939

Result: PASS.

- 1,200 consecutive OPLL_Clock calls compared.
- Nuked trace captured twice and byte-identical.
- Qualified register-0x0e writes observed: 0x3f and 0x00.
- Nonzero ch_out samples: 181.
- Compared buffer MO/RO values, post-call output_m/output_r, signed rm_enable,
  previous and next ch_out latches, and serial cycle progression.
- No mismatch was reported.

The comparator reports call index, cycle, independent MO/RO slots,
register-0x0e write data, ch_out before/after, expected and actual buffer
values, expected and actual output latches, and rm_enable before/after.

The external oracle adapter smoke workflow also passed on this head:
run 44930418525. Rhythm reset differential passed:
run 44930418590.

The previous full CI run for the initial pipeline and differential implementation
passed at head 9d304b3949f7dd28bc9caefb75f88980808daef2 (run 44930236424).
The latest CI for the enhanced divergence diagnostics is run 44930418522;
Rust formatting and documentation hygiene passed, while the remaining Rust
quality and unit-test steps were still running when this report was written.

## Limitations and remaining work

1. This is a strict software differential against Nuked-OPLL, not a physical
   YM2413 capture or independent silicon validation.
2. The output code is not the measured DAC amplitude token. Ym2413DacCode and
   its signed-zero distinction remain separate and unchanged.
3. The new module accepts already-qualified register-0x0e writes. It does not
   yet own the address/data bus synchronizer or reproduce write_d_en itself.
4. The caller supplies the operator result that becomes the next ch_out latch.
   The module does not replace or reimplement PG, EG, operators, or rhythm
   synthesis.
5. The module is not yet integrated into the production YM2413 core,
   MachineScheduler, PCM generation, or the existing measured MO/RO retention
   service.
6. Nuked's own RO cycle switch is marked TODO in the reference source. A pass
   against Nuked establishes software equivalence, not that this RO schedule
   is physically correct.

Status: the isolated cycle-ordered digital output pipeline is implemented and
differentially validated against the pinned software reference. Full MO/RO
hardware closure and production integration remain open.


## Status CI e fronteira de integração

CI #44930418522 terminou SUCCESS após rerun do job cancelado. O código do
pipeline e o exemplo diferencial passaram por Rustfmt, Clippy e testes do
workspace. O workflow PG #44930418468 terminou SUCCESS, mas mantém os
relatórios de candidatos divergentes já caracterizados. EG #44930418372 ainda
estava executando a matriz de 38 configurações no momento do checkpoint.

A inspeção do core confirmou que Ym2413Core já é a autoridade do rm_enable
assinado e executa o shift e a escrita de modo por slot. O pipeline isolado
mantém seu próprio rm_enable apenas para reproduzir a sequência do Nuked no
differential. A integração de produção deve receber o rm_enable pós-update
do core, sem duplicar a temporização.

A inspeção do serviço PG também confirmou que slots de Rhythm Mode sem seed de
ruído são deliberadamente ignorados para não fabricar estado do operador.
Nesses ciclos, não existe um próximo ch_out validado. A integração correta
precisa propagar saída desconhecida explicitamente (e não substituí-la por
zero ou pelo último valor) e incluir essa validade no frame e nos snapshots.

Portanto, a integração ao serviço PG não foi feita nesta etapa. A fronteira
técnica está identificada; a próxima implementação deve introduzir entrada
opcional de ch_out e rm_enable externo pós-update, alinhando o cursor do
pipeline ao cursor de Ym2413Core.


## Resultados finais PG/EG no mesmo checkpoint

PG Differential #425, run 44930418468, terminou SUCCESS:
2.600 chamadas e 46.800 valores de fase. A comparação PG base reportou zero
divergências após floor normalization, mas 256 diferenças em unidade comum
exata/half-LSB. PM teve 404.965 diferenças na posição sem rotação em 524.288
tuplas; a melhor rotação cíclica foi 6. A coincidência da tabela MAME com o
candidato Rust após offset 6 e truncamento em direção a zero é uma
caracterização, não evidência de alinhamento físico definitivo.

EG Characterization #598, run 44930418372, terminou SUCCESS na matriz de 38
configurações AR/DR/KSR:
- 243.884 amostras;
- 44.064 divergências de nível;
- 71 divergências de estágio;
- 1.140.000 replays de sequenciador e 1.140.000 replays de timer sem
  divergências de campo;
- 506.768 amostras de replay do operador sem divergências de nível/estágio.

O controle baseline registrou 56.270 divergências de nível e 266 de estágio.
O candidato external_rate_phase eliminou divergências nessa caracterização,
mas usa fase externa/oráculo e permanece teacher-forced; não pode ser aceito
como correção independente.

CI #44930418522 terminou SUCCESS após rerun do job cancelado. Assim, o código
do pipeline isolado, seu exemplo e o diferencial estão compilados e cobertos
pelo CI. A caracterização PG/EG continua sem fechamento.
