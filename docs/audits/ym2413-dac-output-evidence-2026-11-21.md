# YM2413 DAC/output evidence — 2026-11-21

## Sources and correction to the earlier level count

The immutable andete revision
`4893d1c09faa0450b5d3355f19b7682d72822501` contains:

- `results/20150209.txt`
- `results/20150305.txt`
- `results/20150311.txt`

The 2015-02-09 note initially reported 511 distinct levels and inferred
`-255..0..+255`, because its original ADC histogram could not resolve the
two central peaks. The 2015-03-05 follow-up explicitly corrects that result:
16x oversampling resolves **512 distinct peaks**, with two unusually close
central peaks. The author tentatively interprets them as `-0` and `+0`,
and says that an idealized model may map both to one zero. The 2015-03-11
follow-up independently discusses both `-0` and `+0` and reports that at
volume settings 14 and 15 the signal takes the four distinct values
`-1, -0, +0, +1`.

Therefore the old statement “the YM2413 has only 511 measured DAC values and
-256 is absent” is superseded. The evidence supports a 9-bit DAC resolution
and 512 measured output peaks, including two separate central measured
levels interpreted as signed zeros. The author's proposed idealized collapse
of those two levels is explicitly tentative, not a demonstrated raw digital
encoding.

The 2015-03-05 note also describes a 32x16 switch matrix and thermometer
decoders inferred from a die image. The SMS Power YM2413 development index
warns that the die shots referenced by these notes are from an FHB013
derivative rather than a confirmed YM2413 die. Treat that internal DAC
topology as a hypothesis; it is not required for the externally measured
dual-zero observation.

## What is and is not established

### Supported by the sources

- Yamaha's application manual specifies a built-in 9-bit DAC.
- Andete's oversampled physical output measurements resolve 512 peaks.
- The two central peaks are separately observed and described as `-0` and
  `+0` in the 2015-03-05 and 2015-03-11 notes.
- The measured volume response includes four distinct values
  `-1, -0, +0, +1` at volume settings 14 and 15.
- MO and RO are separate analog output pins.

### Still unknown

- The exact binary 9-bit word presented internally to the DAC.
- The bit-level encoding of the two central zero states.
- A proven conversion from the serial operator's one's-complement `i16`
  intermediate to the DAC input word.
- The precise sub-clock timing of the DAC input latch relative to the four
  clocks of each serial slot.
- The neutral electrical level and the analog source-follower/load transfer.
- Whether the two measured zero peaks should remain distinct in an idealized
  digital model or be collapsed at a later analog/output abstraction.

The 512 measured peaks do not, by themselves, reveal the raw DAC input bit
pattern. In particular, no shift, mask, sign extension or reinterpretation of
the operator's signed `i16` value is justified by these notes.

## FM-5.3 implementation consequence

The previous `Ym2413DacCode` wrapper represented only signed amplitude
`-255..=+255` and therefore collapsed the two central measured levels. The
FM-5.3 retention layer needs to preserve observed values verbatim, even while
the raw DAC encoding remains unknown.

The current implementation now represents:
- `Ym2413DacCode::new(0)`: the positive-zero measured-level token;
- `Ym2413DacCode::negative_zero()`: the separately observed negative-zero
  measured-level token;
- `Ym2413DacCode::value()`: signed amplitude projection (both zeros return
  numeric zero);
- `Ym2413DacCode::is_negative_zero()`: preserves the distinction.

This is a measurement-domain representation only. It is **not** a raw
nine-bit DAC word and does not convert from the operator intermediate.

Because the serialized retention payload changed semantically, the rhythm
retention and composed MO/RO service snapshot versions were advanced to 2.
Directed tests verify that negative zero remains distinct from positive zero,
survives the original-to-repeat RO path, and survives snapshot/restore.

## Decision

- Correct the historical “511 levels / -256 absent” wording; it reflects the
  superseded 2015-02-09 measurement.
- Preserve both measured central levels in the serial retention representation.
- Do not infer or implement the raw 9-bit DAC word yet.
- Continue FM-5.4 investigation using the original physical captures and
  Yamaha application manual; keep the operator-to-DAC conversion blocked
  until its arithmetic and temporal boundary are independently established.

References:
- andete `results/20150209.txt`, `results/20150305.txt`,
  `results/20150311.txt` at commit
  `4893d1c09faa0450b5d3355f19b7682d72822501`.
- SMS Power, “YM2413 Reverse Engineering Notes 2015-03-05”:
  https://www.smspower.org/Development/YM2413ReverseEngineeringNotes2015-03-05
- SMS Power, “YM2413 Reverse Engineering Notes 2015-03-11”:
  https://www.smspower.org/Development/YM2413ReverseEngineeringNotes2015-03-11
- Yamaha, YM2413 Application Manual, II-1 and III-4:
  https://www.waveguide.se/?article=46&file=ym2413am.pdf
- SMS Power YM2413 development index (die-shot provenance note):
  https://www.smspower.org/Development/YM2413


## Implementation validation update

The dual-zero retention increment is present at code commit
`27650ff687c9495415644bca1db30585ffed21b9`.

- CI #9852: SUCCESS — Rustfmt, workspace Clippy, workspace tests,
  observability Clippy/tests and documentation hygiene.
- PG differential #388: SUCCESS — 147,456 calls, strict serial/control/
  LFSR/timing PASS; candidate differences remain limited to SD SL/EG.
- Rhythm reset differential #311: SUCCESS.

The new tests verify that positive and negative zero are distinct measured
tokens and that negative zero survives the original HH output, delayed RO
repeat, and version-2 snapshot/restore. This validates the retention
representation only; it does not validate raw nine-bit code encoding or
operator-to-DAC conversion.
