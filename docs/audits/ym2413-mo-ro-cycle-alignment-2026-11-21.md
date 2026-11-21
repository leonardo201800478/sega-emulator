# YM2413 MO/RO cycle alignment audit — corrected 2026-11-21

## Scope and evidence boundary

This audit reconciles the measured MO/RO output-slot maps with the pinned
Nuked-OPLL software schedule at commit
`1269cf5a783b65583b50fa2464d08be75830aaa0`. It corrects the intermediate
claim that both pins share one cycle-to-slot transform. MO and RO use
different offsets. It also corrects the interpretation of
`rm_enable & 0x40`: it is a delayed mode-write pipeline bit, not a
persistent Rhythm Mode flag.

The measured slot maps come from the project's reverse-engineering evidence.
Nuked's `output_m/output_r` behavior is a software model, not physical
timing evidence. Its RO switch is explicitly marked `TODO: This might be
incorrect`.

## Clock order and pipeline

At the beginning of `OPLL_Clock`, Nuked copies the previously computed
`output_m/output_r` into the caller's buffer. It then shifts
`rm_enable >>= 1`, applies a pending mode-register write, calls
`OPLL_Channel`, and only afterwards calls `OPLL_Operator`. The operator
updates `ch_out` after the channel stage.

Consequences:

1. The output buffer is one `OPLL_Clock` call behind the pin values just
   computed by `OPLL_Channel`.
2. `OPLL_Channel` consumes the previous operator's `ch_out` latch.
3. An adapter must preserve both pipeline boundaries; routing the current
   operator result in the same pre-operator channel stage is incorrect.

Pinned source:
https://github.com/nukeykt/Nuked-OPLL/blob/1269cf5a783b65583b50fa2464d08be75830aaa0/opll.c

## MO cycle-to-slot transform

Nuked computes:

```c
ismod = (cycles / 3) & 1;
mute_m = ismod || ((rm_enable & 0x40) && (cycles + 15) % 18 >= 12);
```

Ignoring the separate transition mask for a moment, the base MO active
windows are cycles 0..2, 6..8 and 12..14. These align with the nine measured
melodic carrier positions 3..5, 9..11 and 15..17 using:

```text
mo_slot = (nuked_cycle + 3) % 18
```

In measured Rhythm Mode, channels 7..9 are repurposed for percussion and MO
retains only measured slots 3..5 and 9..11. These correspond to Nuked cycles
0..2 and 6..8 under the same `+3` coordinate. The persistent six-channel
restriction must be sourced from the actual Rhythm Mode register/state and
the measured output map; the transient `rm_enable & 0x40` mask in Nuked
does not establish that persistent gate.

## RO cycle-to-slot transform

Nuked's conditional `mute_r` switch selects cycles 16, 17, 0..5, 9 and 10
when `rm_enable & 0x40` is set. These correspond to the measured RO
occurrence positions 13..17, 0..2, 6 and 7 using:

```text
ro_slot = (nuked_cycle + 15) % 18
```

This matches the measured occurrence map: original outputs at slots 13..17
and delayed repeats at slots 0, 1, 2, 6 and 7. The source labels its RO
switch as potentially incorrect, and the condition is tied to the delayed
mode-write pipeline. Therefore this is a software schedule correspondence,
not proof of steady-state physical RO gating.

## Why rm_enable is not a stable mode flag

At the start of every `OPLL_Clock`, Nuked shifts `rm_enable` right by one.
The mode-write handler loads its upper pipeline bit when register 0x0e is
written. Consequently bit 6 is a delayed write-pipeline state; it is not a
persistent Rhythm Mode boolean. Bit 7 is likewise a delayed pipeline bit
used by the operator's rhythm-source path. The persistent register image
and the delayed `rm_enable` pipeline must remain separate inputs to any
adapter.

Do not synthesize a constant `rm_enable=0x40` or `0x80` to represent
steady Rhythm Mode.

## Sign-dependent neutral output

For the original YM2413 branch (excluding the YM2413B/YMF281B special
branch), `OPLL_Channel` derives:

```c
sign = ch_out >> 8;
if (ch_out >= 0) {
    ch_out++;
    sign++;
}
output_pin = muted ? sign : ch_out;
```

Thus the software neutral token is `-1` when the prior `ch_out` is
negative and `+1` when it is nonnegative. Active positive values are also
incremented before assignment. This is a signed software representation;
it must not be confused with the signed measurement-domain amplitude token
or claimed as a proven physical raw DAC input word.

The current `Ym2413DacCode::from_operator_channel_output` helper performs
only a one's-complement-to-signed-measured-amplitude conversion. It does
not implement the `OPLL_Channel` positive increment, the sign-dependent
neutral token, or the delayed output buffer. It remains an isolated
conversion helper and is not a pin adapter.

## Integration consequence

MO and RO require independent cycle-to-slot transforms:

- MO: `(cycle + 3) % 18`.
- RO: `(cycle + 15) % 18`.

The current `Ym2413RhythmOutputService::process_slot` accepts one shared
slot index and produces both pin results. It cannot be called once with a
single raw Nuked cycle and correctly align both pins. A future adapter must
route the MO event using the MO coordinate and service the RO retention path
using the RO coordinate. The two pin schedules and their mode gates remain
independent; the RO retention cursor must advance only on the RO schedule.

The adapter must also preserve the previous-`ch_out` latch, the additional
output-buffer delay, the delayed `rm_enable` pipeline, and the
sign-dependent neutral token. No core integration is made by this audit.

## Required adapter design

1. Convert cycle to MO slot with `mo_output_slot_for_nuked_cycle(cycle)`.
2. Convert cycle to RO slot with `ro_output_slot_for_nuked_cycle(cycle)`.
3. Consume the prior `ch_out` latch at the channel stage; update the next
   `ch_out` only after the current operator stage; preserve the extra output
   buffer delay when capturing host-facing pin samples.
4. Keep persistent Rhythm Mode register state separate from transient
   `rm_enable` bits. Apply MO's six-channel gate from the persistent mode
   state and measured MO map; do not infer it from bit 6.
5. Keep RO source selection and active/muted gating separate. Treat Nuked's
   RO gate as a candidate software behavior because the source itself marks
   it uncertain.
6. Represent the original YM2413 sign-dependent neutral token and positive
   active-value increment explicitly. Keep this software pin-code
   representation separate from `Ym2413DacCode` measured amplitudes and
   from any raw nine-bit DAC-word hypothesis.
7. Differentially compare complete `output_m/output_r` traces across
   register writes, mode transitions, all 18 cycles and both output-buffer
   phases. Treat Nuked agreement as software-model compatibility, not
   physical timing certification.
8. Preserve the measured MO/RO slot evidence independently of Nuked's
   uncertain RO gating implementation.

## Decision

The shared-offset claim is withdrawn. MO uses the `+3` coordinate and RO
uses the `+15` coordinate. The earlier interpretation of
`rm_enable & 0x40` as a persistent mode flag is also withdrawn. Separate
per-pin cycle mapping is required before the existing combined output
service can be adapted safely. No core integration is made by this audit.
