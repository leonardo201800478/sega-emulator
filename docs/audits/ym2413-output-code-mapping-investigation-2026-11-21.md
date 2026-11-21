# YM2413 output-code mapping investigation — 2026-11-21

## New evidence

A focused review of andete's 2015-03-27 operator model and 2015-04-09 sine/EXP reconstruction found a candidate arithmetic mapping that was not recorded in the earlier DAC evidence audit.

The 2015-03-27 model computes the carrier result as:

```cpp
auto c = lookupExp(s2) >> 4;
cout << 255 - c << endl;
```

The author states that this model can exactly predict the measured output data. The 2015-04-09 explanation says that `lookupExp` uses one's-complement negation, which preserves two representations of zero. The 2015-03-11 volume measurements explicitly identify 512 output levels spanning the 9-bit range 0..511 and distinguish `-0` and `+0`.

Sources:
- andete / SMS Power, 2015-03-27 operator and feedback model: https://www.smspower.org/Development/YM2413ReverseEngineeringNotes2015-12-12
- andete / SMS Power, 2015-04-09 sine and EXP tables: https://www.smspower.org/Development/YM2413ReverseEngineeringNotes2015-04-09
- andete / SMS Power, 2015-03-11 volume measurements: https://www.smspower.org/Development/YM2413ReverseEngineeringNotes2015-03-11
- Yamaha YM2413 application manual, operator and DAC section: https://www.waveguide.se/?article=46&file=ym2413am.pdf

## Candidate correspondence with the Rust operator

The production serial operator retains two distinct domains:

- `operator_output`: the raw signed EXP-stage result.
- `channel_output`: `operator_output >> 3`, the channel-output latch consumed by the rhythm routing.

The current operator's negative branch uses bitwise one's-complement negation (`!`), matching the sign convention described by andete's model. In that representation, the negative-zero token is numerically `-1`; the most-negative 9-bit channel-output representation can be `-256`.

This makes the following a strong *candidate reconstruction* of the 9-bit output-level code:

```text
candidate_output_code = 255 - channel_output
```

Examples:
- positive zero (`channel_output = 0`) -> code 255;
- negative zero (`channel_output = -1`) -> code 256;
- positive full scale (`channel_output = 255`) -> code 0;
- negative full scale (`channel_output = -256`) -> code 511.

The expression yields the complete 9-bit unsigned range 0..511 and keeps the two central zero encodings distinct. This aligns the Rust operator's one's-complement output representation with the arithmetic used by the published measured-output model.

## Evidence boundary

This is **not yet proof of the physical internal DAC input word**. The andete model predicts measured analog output levels and reports an exact match to those measurements; it does not probe the internal digital bus between the operator and DAC. The Yamaha manual confirms a 9-bit DAC but does not specify the binary input encoding or expose the latch contents. The published reverse-engineering die topology remains a derivative-chip hypothesis.

Consequently:

1. Record `255 - channel_output` as a candidate measured-output-code mapping, not as a confirmed raw DAC bus encoding.
2. Do not silently replace the existing signed measurement-domain `Ym2413DacCode` with a raw-word type.
3. Do not conflate `Ym2413SerialOperatorSample::channel_output` (one's-complement bit-pattern interpreted as i16) with the signed measured amplitude token currently retained by `Ym2413DacCode`.
4. Keep the output service's current explicit-input boundary until a directed test establishes the operator-to-code correspondence against the pinned Nuked arithmetic and the measured-output model.
5. If implemented for experimentation, expose it as an explicitly named candidate conversion and preserve the original signed operator value in traces. Do not claim physical DAC-word validation.

## Next experiment

Add a non-production comparator mode that emits, for each serial carrier/rhythm output event:

- raw `operator_output` i16;
- `channel_output` i16;
- candidate `255 - channel_output` u16;
- signed measurement-domain interpretation, including separate `-0` and `+0`;
- corresponding Nuked `ch_out` and the published model's predicted output value where derivable.

The comparator must test the complete range, including both zero encodings and the negative extreme, then report any out-of-domain or clipping behavior. It must not feed oracle-derived values into Rust. Only after exact arithmetic correspondence is demonstrated should the candidate become a separately versioned conversion API. Physical DAC input encoding must remain marked unverified until a direct digital capture or equivalent independent evidence exists.

## Decision

The previous statement that no operator-to-output arithmetic candidate exists is now too strong. A credible measured-output mapping candidate has been identified. The raw physical DAC input word, its electrical latch timing, and analog transfer remain unproven. No production code was changed in this investigation.


## Bounded implementation increment — 2026-11-21

The measured-domain conversion is now represented by
`Ym2413DacCode::from_operator_channel_output(i16)` in
`native/ym2413/src/rhythm_output.rs`.

- Accepts only the operator channel-output one's-complement domain `-256..=255`.
- Decodes positive values directly.
- Decodes negative values by one's-complement magnitude, preserving `-1`
  as the distinct negative-zero token and mapping `-256` to measured
  amplitude `-255`.
- Rejects values outside the documented 9-bit one's-complement domain.
- Does not return a raw DAC word and does not alter any output-service call
  site; integration remains a separate timing/ownership task.

An exhaustive unit test checks all 512 bit-pattern values and proves a
one-to-one mapping to the signed measured domain, including both zero
encodings and both full-scale endpoints. The test is committed, but exact-head
CI is still required before this helper is considered validated.

This increment supersedes the earlier sentence above that said no production
code was changed: a standalone conversion API and tests were added; the
serial output service is still not wired into the PG/core pipeline.
