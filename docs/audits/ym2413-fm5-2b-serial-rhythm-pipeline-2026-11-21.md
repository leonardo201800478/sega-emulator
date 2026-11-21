# YM2413 FM-5.2B — serial Rhythm Mode integration

**Data:** 2026-11-21
**Repository:** `leonardo201800478/SUN`
**Parent:** PR #1154 — FM-5.2A serial rhythm LFSR cadence
**Work branch:** `feat/ym2413-fm5-2b-rhythm-operator-routing`
**Reference implementation:** Nuked-OPLL `1269cf5a783b65583b50fa2464d08be75830aaa0`
**Status:** implemented in a draft stacked PR; not merged to `main`.

## Scope delivered

FM-5.2B connects the existing Rhythm Mode decoder/configuration to the serial PG/operator service. It does not claim complete YM2413 audio output.

### Serial ownership and parameter routing

The existing six-cycle ownership map is reused:

| Core cycle | EG rhythm selector | Voice/operator |
|---:|---|---|
| 11 | BassDrumFirst | Bass Drum modulator |
| 12 | HiHat | Hi-Hat single operator |
| 13 | TomTom | Tom single operator |
| 14 | BassDrumSecond | Bass Drum carrier |
| 15 | SnareDrum | Snare Drum single operator |
| 16 | TopCymbal | Top Cymbal single operator |

The selector is resolved through `decode_rhythm_synthesis`; the implementation does not create a second operator bank or a second clock owner. The existing `Ym2413Core` remains the source of serial cycle, EG state, and the post-EG service boundary.

Rhythm PreparePatch1/PreparePatch2 latches use the selected voice/operator configuration, channel FNUM/BLOCK, patch multiplier/AM/PM/envelope fields, feedback, and the channel's rhythm register. HH and TOM retain the special instrument-nibble TL selection; other voices select patch TL or rhythm volume according to Nuked's `mcsel` cadence.

### PG phase capture and phase address generation

The implementation follows the pinned Nuked `OPLL_PhaseGenerate` sequence:

- Cycle 13 captures HH phase bits 2, 3, 7 and 8 from the current PG phase.
- Cycle 17 captures TC phase bits 3 and 5 when post-shift `rm_enable & 0x80` is set.
- Cycle 13 forms the HH phase address from the captured HH/TC bits and the current pre-DoRhythm LFSR bit.
- Cycle 16 forms the SD phase address from HH bit 8 and the current pre-DoRhythm LFSR bit.
- Cycle 17 forms the TC phase address from the captured phase mix and the fixed low address bit.

The boolean equations and phase-address constants are kept in one helper and covered by directed tests. The phase latches are part of the PG pipeline snapshot.

### Operator cycle overrides and output retention

The serial operator receives the actual post-shift `rm_enable` byte, not a single Rhythm Mode boolean. This preserves transition behavior:

- Bit 7 overrides `ismod1` at cycles 15 and 16 and selects the retained rhythm output path.
- Bit 6 overrides `ismod2` at cycles 13 and 14.
- Bit 6 overrides `ismod3` at cycles 16 and 17.

Five independent delayed output latches are retained in Nuked order: HH, TOM, BD, SD and TC. Source cycles are 15, 16, 17, 0 and 1; return cycles are 2, 3, 4, 8 and 9. They are included in the serial operator snapshot.

### LFSR order

The PG phase path samples the current LFSR bit before the post-EG noise step. `clock_rhythm_noise` still advances once per core-owned slot after the EG boundary, irrespective of RHY mode, matching the pinned Nuked call order. No additional clock or scheduler was introduced.

If no explicit noise seed is supplied, the service continues to gate the rhythm-owned PG/operator path rather than fabricate a reset seed or noise bit. This is a deliberate unresolved hardware-state boundary, not a claim that unseeded percussion is synthesized.

## Snapshot compatibility

The new retained state changes snapshot schema versions:

- Serial operator state: version 3 (five rhythm channel output latches).
- PG pipeline state: version 4 (HH/TC phase latches).
- PG serial service state: version 9 (nested state schema).

Restore validation continues to reject incompatible versions and invalid retained output amplitudes. The seeded 18-slot test verifies snapshot restoration and equivalence between uninterrupted execution and execution resumed from a mid-rotation snapshot.

## Validation evidence and limits

At code HEAD `6aed8b627b9b84b4459da1b0e65ffeb730b9fe35`:

- Rust CI #9825: **SUCCESS**. Change-impact classification, documentation hygiene, workflow validation, Rustfmt, Clippy, workspace unit/integration tests, observability Clippy and observability tests passed. Unrelated platform/runtime gates were skipped by impact classification.
- Nuked PG differential #361: **SUCCESS**.
  - Isolated operator differential: all five rhythm output latches compared against Nuked after each serial call; effective `rm_enable` was captured immediately before `OPLL_Operator`.
  - Integrated Rhythm Mode characterization: 147,456 calls, all 18 serial cycles, individual percussion triggers, simultaneous voices, release and retrigger. The fixture uses Table III-9 frequency values and the dispatcher regression asserts the documented physical slot ownership.
  - Strict control/timing state matched: cycle order, LFO/EG global timers, key-on control, channel/slot identity, `rm_enable`, `rm_select`, rhythm control and the 23-bit LFSR state.
  - All five retained output latches (HH/TOM/BD/SD/TC) became nonzero in both traces and had no reported candidate divergence.
- Nuked Rhythm reset differential #284: **SUCCESS**.
- External Nuked oracle adapter #457: **SUCCESS**.
- Nuked EG characterization #534: the 38-configuration AR/DR/KSR sweep was still running at the time of this audit update.
- Hardware/chip validation: **NOT RUN**.

The same PG workflow also passed the integrated PG service, all 18 accumulated phases, KEY-ON phase reset and pinned MAME/ymfm arithmetic audit.

The integrated Rhythm Mode characterization deliberately separates strict serial/control/timing evidence from synthesis fields derived from the un-certified rhythm patch candidates. It does not claim full synthesis equality with Nuked or physical chip accuracy.

Candidate-dependent differences reported over the 147,456-call trace:

| Field | Rows different | First call | Nuked | Rust candidate |
|---|---:|---:|---:|---:|
| `c_fb` | 8,092 | 1,814 | 0 | 7 |
| `c_dm` | 64,732 | 1,814 | 8 | 9 |
| `op_calc_c_fb` | 8,092 | 1,815 | 0 | 7 |
| `op_calc_c_dm` | 64,731 | 1,815 | 8 | 9 |
| `c_sl` | 8,092 | 1,815 | 4 | 6 |
| `eg_sl` | 8,092 | 1,816 | 4 | 6 |
| `eg_state` | 15,984 | 79,001 | array divergence | array divergence |

These differences are reported, not normalized away. In particular, the first feedback/waveform/sustain-level differences occur as the rhythm patch profile is selected. They must be reconciled against physical patch evidence before the candidate table is certified. The zero LFSR register used in the trace matches Nuked's software reset state only; it is not evidence for the physical YM2413 power-on seed.

## Hardware reference alignment

The serial dispatcher regression now asserts the rhythm ownership documented
in Table III-9 of the [YM2413 application manual](https://www.smspower.org/maxim/Documents/YM2413ApplicationManual):
one-based physical slots 13/16 = BD modulator/carrier, 14 = HH, 15 = TOM,
17 = SD and 18 = TC. The integrated oracle fixtures use the manual's rhythm
frequency setup: R#16/17/18 = 0x20/0x50/0xC0 and R#26/27/28 =
0x05/0x05/0x01, keeping melodic KEY-ON bits clear.

The [SMS Power reverse-engineering notes of 2015-12-16](https://www.smspower.org/Development/YM2413ReverseEngineeringNotes2015-12-16)
explicitly describe the six rhythm-ROM decoder lines as a best guess and state
that rhythm behavior had not yet been reverse-engineered in that work. This is
why the current patch table remains a candidate and why its differences from
Nuked are reported rather than forcibly normalized.

## Explicitly not delivered

- Full integrated synthesis equality for the rhythm patch candidates; the current oracle characterization reports candidate-dependent PG/EG/operator differences rather than treating them as a pass.
- Physical certification of the rhythm patch table, LFSR reset seed and analog behavior.
- Final MO/RO pin routing/retention and the nine-channel output accumulation.
- Conversion from the current operator-level result to a raw nine-bit DAC word.
- PCM generation, PSG+FM mixing, or resampling.
- SMS F0–F2 register/bus integration or runtime audio integration.
- Physical validation of reset seed, patch ROM values, or analog DAC behavior.

The rhythm patch values remain software candidates with documented provenance; they are not presented as a hardware-certified internal ROM dump.

## Next technical gates

1. Reconcile the reported `c_fb`, `c_dm`, `c_sl` and downstream EG-state differences using physical patch evidence. Do not rewrite the candidate table merely to force parity with Nuked.
2. Validate the rhythm patch profile and LFSR reset seed against hardware evidence; retain the explicit-seed gate until then.
3. Resolve and validate MO/RO pin timing and rhythm output routing without collapsing serial events into a sample-domain approximation.
4. Define and test the raw DAC code and PCM sample boundary.
5. Integrate the FM sample path into SMS only after the serial output is independently validated; preserve MachineScheduler as the sole timing authority.
6. Add aggregated Save State and continuous-versus-block equivalence at the SMS machine boundary.

No merge was performed or authorized by this work.


## External Nuked operator differential gate

The pinned Nuked-OPLL workflow now instruments the effective `rm_enable`
latch immediately before `OPLL_Operator` (after the per-call shift and
`DoModeWrite`). A dedicated all-five-voice register sequence drives the
external YM2413 reference. The Rust replay consumes that exact boundary latch
and compares the serial operator/feedback state plus retained HH, TOM, BD, SD
and TC outputs after every `OPLL_Clock` call.

The gate requires both rhythm override bits (0x40 and 0x80) to occur and each
of the five retained Nuked outputs to become nonzero. This is an isolated
operator/output-latch differential: it does not independently validate Rust
PG/EG/noise generation, final MO/RO mixing, DAC/PCM conversion, or physical
chip timing. The broader PG/EG and rhythm-reset workflows remain separate
gates.


## FM-5.2C investigation — andete evidence and candidate reconciliation (2026-11-21)

Sources were retrieved at the requested immutable andete revision
`4893d1c09faa0450b5d3355f19b7682d72822501`:

- `results/rhythm/rhythm.md`
- `results/lfsr/lfsr.txt`
- `results/20151212/fb.txt`
- `results/20151216/ksl.txt`
- `results/20151231/envelope.txt`
- `results/rhythm/sweep.cc`
- `results/rhythm/model6.cc`
- `results/20150209.txt`

The source is measurement-driven engineering work, but it is explicit about uncertainty. It is not a complete dump of the YM2413 rhythm ROM.

### Evidence that is usable

1. **BD is a two-operator sound.** The measured capture-derived custom patch is
   `01 01 18 0F DF F8 6A 6D`. The measured BD and custom-patch waveforms are
   described as a close match, not an exact match: the custom patch is delayed
   by one sample and has a slightly higher peak. The article attributes part
   of the amplitude difference as potentially caused by the shared EG counter
   and the one-sample alignment. Therefore this is a strong candidate, not a
   certified ROM image.

2. **Feedback belongs to the modulator path.** The feedback measurements
   report FB=0 as no feedback and fit a model using the average of the previous
   two modulator outputs, shifted by the FB code. The application-manual
   modulation-index table gives the exponential FB scale. The BD candidate's
   R#3 value `0x0F` therefore supports FB=7 for the BD modulator. It does
   not justify copying that modulator feedback field into the BD carrier slot.

3. **Per-slot patch data is not one undifferentiated BD patch.** The andete
   rhythm analysis separates BD modulator and carrier, HH, TOM, SD and TCY
   operator roles. Its comparison table for Nuke.YKT likewise has six
   separate rhythm entries. In the pinned Nuked source
   `1269cf5a783b65583b50fa2464d08be75830aaa0`, the BD-carrier entry has
   FB=0 and DM=0, while the BD-modulator entry has FB=7 and DM=1.
   The previous Rust latch builder reused the merged BD patch's FB/DM values
   for both BD slots. FM-5.2C now restricts those two modulator-specific
   latches to `BassDrumFirst`; the BD-carrier slot receives FB=0 and DM=0.
   This is a correction to per-slot field ownership, not a claim that the
   complete internal rhythm ROM has been physically dumped.

4. **Rhythm noise cadence is better constrained than its seed.** The 2018
   LFSR note derives the decimated sequence polynomial
   `x^23 + x^9 + x^8 + x + 1`. Its 2020 update explains that the physical
   generator advances 18 times per 72-clock rotation, using the slot-rate
   recurrence `x^23 + x^9 + 1`. HH consumes the current noise bit, the
   generator advances three times, SD consumes the resulting bit, then it
   advances fifteen times. The note also flags interaction with Test Mode
   bit 1. This supports the current slot-rate/call-order model; it does not
   identify the physical power-on seed.

5. **KSL and envelope measurements remain useful but do not settle the
   rhythm patch disagreement.** The KSL article provides measured attenuation
   points and confirms the YM2413 scale ordering 0, 1.5, 3 and 6 dB/octave.
   The envelope article gives `internal sustain level = 8 * SL`, measures
   DAMP as base rate 12, and explicitly leaves the DAMP-to-ATTACK threshold
   near -45 dB uncertain. These findings do not license changing a rhythm
   operator's SL merely to match a software reference.

6. **MO/RO and DAC evidence has a boundary.** The 2015 capture note describes
   separate MO and RO analog pins and a calibrated measurement path intended
   to reconstruct a 9-bit digital output. It reports 511 observed levels
   from -255 through +255, with -256 absent. This establishes the measured
   output domain, not the exact internal operator-to-DAC bit mapping or the
   analog transfer curve. The rhythm article reports that percussion outputs
   are repeated and measured as identical at their two time positions. The
   suggested 9-by-5 shift-register topology comes from an FHB013 derivative
   die shot, not a YM2413 die shot; it remains a structural hypothesis.

### Remaining candidate divergence — do not normalize

The pinned Nuked revision and the andete article are not identical sources
for every patch byte. In particular, the article's named Nuke.YKT rhythm
table shows the SD carrier SL byte as `0x68` (SL=6), while the pinned
`opll.c` rhythm table at the requested commit selects SL=4 for the SD
carrier. The Rust capture-derived candidate remains SL=6. The measured
candidate and the pinned software table therefore disagree by two SL codes.
No physical evidence in the retrieved files resolves this version-specific
difference. **Keep the Rust candidate at SL=6 and preserve the differential
mismatch.**

The earlier integrated trace's `c_sl`/ `eg_sl` differences and later
`eg_state` divergence must remain visible. The available material does not
prove that every EG-array mismatch is caused solely by SD SL; that requires
an isolated per-voice trace and should be tested separately. No global patch
table rewrite is justified.

### Code change and directed regression

Commit `c9a5f73176ee34e585c5b912f4b705256ba84afd` introduced
`rhythm_operator_patch2_latches` and a six-selector regression. It ensures
that FB=7 and DM=1 from the merged BD capture candidate are latched only for
the BD-modulator selector; BD carrier and the other four single-operator
rhythm selectors use FB=0 and DM=0. The subsequent documentation-only
commit qualifies the evidence boundary.

The change does not alter any candidate AR/DR/RR/SL values, the LFSR seed,
the LFSR polynomial, phase equations, output retention timing, or DAC mapping.

### FM-5.2C status

- **Resolved at the software field-ownership level:** BD modulator-only FB/DM
  propagation into the BD-carrier serial slot.
- **Still open:** SD carrier SL (candidate 6 versus pinned Nuked 4), its
  downstream EG-state divergence, exact physical rhythm patch ROM, and
  hardware validation.
- **Not claimed:** full patch-ROM reconciliation or FM-5.2C completion.

Next, rerun the isolated and integrated Nuked differentials on the new code
HEAD. If the FB/DM rows converge, retain the SD SL/EG mismatches as separate
diagnostics. Only then proceed to FM-5.3 MO/RO pin-event routing and repeated
output retention, keeping MO and RO as independent serial outputs.


### Post-change validation — PG differential #364

The directed per-slot FB/DM correction was exercised by the pinned Nuked PG
workflow on code commit `c9a5f73176ee34e585c5b912f4b705256ba84afd`.

- Workflow #364: **SUCCESS**.
- Integrated Rhythm Mode: 147,456 calls, all 18 serial cycles.
- Strict serial/control/LFSR/timing comparison: PASS.
- The integrated report no longer lists `c_fb` or `c_dm` in
  `candidate_patch_differences`; those two divergences are resolved for this
  test sequence.
- Remaining integrated candidate differences:
  - `c_sl`: 8,092 rows; first call 1,815; Nuked=4, Rust=6.
  - `eg_sl`: 8,092 rows; first call 1,816; Nuked=4, Rust=6.
  - `eg_state`: 15,984 rows; first call 79,001; the complete arrays differ.
- The comparator reports `CHARACTERIZED_WITH_PATCH_CANDIDATE_DIVERGENCE`,
  not full synthesis equivalence.

The test's 147,456-call trace uses the explicit Nuked software-reset seed
candidate 0. It is not evidence for the physical YM2413 power-on seed.

The first CI run for the code commit (#9828) failed at `cargo fmt --all
-- --check`, solely on formatting in `pg_serial.rs`. The formatting was
corrected in commit `6249ee472cde2f9f3615d5680d7ad0ddcdc3ff7e`. That later
commit has not yet received a new exact-head CI result; do not report CI as
passing on the updated branch until a fresh run confirms it.

The Nuked EG characterization workflow #537 was still running at the time of
this audit update. Its final result must be checked before closing the
characterization gate.

The previous “Next, rerun...” instruction is superseded: PG differential #364
has now run and verified the FB/DM correction. The next work item remains
isolating the SD SL/EG candidate difference, not changing the candidate table
without new hardware evidence.


## FM-5.3 investigation and output-service increment — 2026-11-21

The original andete rhythm analysis at commit
`4893d1c09faa0450b5d3355f19b7682d72822501` gives a measured 18-slot output
schedule. Each slot is four input clocks; one full rotation is 72 input clocks
(about 49.7 kHz at 3.57 MHz). The measured table is:

| Physical slot | RO output | MO output |
|---:|---|---|
| 0 | HH repeat | — |
| 1 | TOM repeat | — |
| 2 | BD repeat | — |
| 3 | — | melodic channel 0 |
| 4 | — | melodic channel 1 |
| 5 | — | melodic channel 2 |
| 6 | SD repeat | — |
| 7 | TC repeat | — |
| 8 | — | — |
| 9 | — | melodic channel 3 |
| 10 | — | melodic channel 4 |
| 11 | — | melodic channel 5 |
| 12 | — | — |
| 13 | HH original | — |
| 14 | TOM original | — |
| 15 | BD original | — |
| 16 | SD original | — |
| 17 | TC original | — |

Andete reports that the five repeated percussion values are identical to
their original outputs, even when parameters change between the two
occurrences. The 9-by-5 shift-register topology suggested by the FHB013
derivative die shot is not treated as proof of the YM2413's physical layout;
the implementation models only the observed behavior.

The 2015 measurement note reports separate MO and RO analog pins and a
9-bit digital output domain with 511 observed levels (-255..+255). The
MSX Resource Center reverse-engineering report independently states that
each selected output level remains active for three input clocks before the
pin returns to its neutral position. This supports the existing three-active,
one-neutral pulse-width contract. It does not determine the sub-clock phase
or neutral voltage.

### Existing output components and new composition

The following components already existed on `main` and were not duplicated:

- `rhythm_output.rs`: versioned five-value retention, original slots 13..17,
  repeated slots 0/1/2/6/7, and explicit uninitialized-repeat results.
- `output.rs`: independent MO/RO routing, measured pulse width and event
  validation.

The FM-5.3 branch adds `Ym2413RhythmOutputService`, which composes those
components into one serial slot boundary:

- accepts an explicit measured-domain level for MO only at slots 3/4/5/9/10/11;
- accepts original percussion levels only at slots 13..17;
- emits retained duplicate values at slots 0/1/2/6/7 without recomputation;
- returns MO and RO independently in `Ym2413RhythmOutputFrame`;
- snapshots the retention values and cursor with a versioned state;
- rejects invalid slot/input placement without mutating the retention state.

The service accepts `Ym2413DacCode` only. It does not derive this value from
the signed operator intermediate, create a raw nine-bit DAC word, combine
MO+RO, generate PCM, choose a sub-clock pulse phase, or advance the master
clock. The caller must invoke it once per completed slot under the existing
core/Scheduler timing authority.

### FM-5.3 status

**Implemented as a standalone serial output-service boundary; not yet
integrated into Ym2413Core or SMS audio.** The remaining gates are:

- run the new six-MO/five-rhythm routing and snapshot/restore tests in CI;
- verify the output event schedule against the measured table and the pinned
  oracle's pin mux without treating Nuked as physical proof;
- define the handoff from the future operator-to-DAC converter to
  `Ym2413DacCode` without inferring a raw code from the signed intermediate;
- integrate the service into the core's slot boundary only when that
  measured-domain source exists;
- retain unresolved sub-clock pulse phase and neutral electrical level as
  explicit limitations.

No PCM or DAC conversion is included in this increment.


### Source-quality qualification: sweep.cc and model6.cc

The two C++ files are engineering tools/models, not independent ROM dumps:

- `sweep.cc` searches model state and parameters to minimize squared error
  against a previously captured `gold` vector. It includes random search
  for large parameter spaces and exposes candidate variables such as the
  noise state, phase offsets and phase-update ordering. Most instrument
  register sweep rows are commented out. Its best-fit output is therefore a
  model-fit result conditional on its fixture and assumptions, not a
  direct measurement of an internal YM2413 register or ROM byte.
- `model6.cc` is an experimental behavioral model. It explicitly marks
  phase update/reset ordering as uncertain ("seems better"), contains TODOs
  for the actual HH/TCY patches and phase sources, and initializes its
  instrument table from an earlier software implementation with a
  verification TODO. Its HH/SD/TCY Boolean equations are useful hypotheses
  and cross-checks, not proof of the physical truth table.
- `results/20150209.txt` describes a measurement pipeline that used a
  shaping board and ADC. It documents noise, nonlinearities, peak fitting,
  calibration drift and uncertain assignments for ADC samples halfway
  between reconstructed output levels. The measured -255..+255 domain is
  useful, but the document explicitly presents itself as work in progress.

Accordingly, the FM-5.3 service uses the measured output slot schedule and
verbatim repeated measurement-domain values. It does not import the
experimental model's operator-to-output conversion or infer raw DAC bits.
The three-active-clock pulse claim is cross-checked against the 2015
MSX Resource Center reverse-engineering report:
https://www.msx.org/pt-br/photoshoots/ym2413-reverse-engineering-progress


### Exact-head validation — FM-5.3 service code commit 4af0e9a

The following GitHub Actions results apply to code HEAD
`4af0e9ad6a0980e8c53fe65d8c224610c46017e0`:

- CI #9841: **SUCCESS**. Formatting, workspace Clippy, workspace unit and
  integration tests, observability-feature Clippy/tests, and documentation
  hygiene completed successfully. The new MO/RO service unit tests are part
  of the workspace test run.
- PG differential #377: **SUCCESS**. The serial operator, all five rhythm
  latches, AM LFO, integrated FM-3 and integrated Rhythm Mode comparisons
  completed. The Rhythm Mode report remains
  `CHARACTERIZED_WITH_PATCH_CANDIDATE_DIVERGENCE`, with only:
  - `c_sl`: 8,092 rows, first call 1,815, Nuked=4/Rust=6;
  - `eg_sl`: 8,092 rows, first call 1,816, Nuked=4/Rust=6;
  - `eg_state`: 15,984 rows, first call 79,001.
  No `c_fb` or `c_dm` entry remains in the integrated Rhythm Mode
  `candidate_patch_differences` object.
- Rhythm reset differential #300: **SUCCESS**.
- EG characterization #550: still running its final 38-configuration
  AR/DR/KSR sweep at the time of this update. All earlier capture, reset,
  single-/four-channel replay, KSR, and sequencer replay steps completed
  successfully. Do not claim the complete characterization run until its
  final sweep and artifact-retention steps finish.

These are software-oracle and regression results. They do not certify the
physical YM2413 patch ROM, power-on LFSR seed, analog MO/RO levels, or raw DAC
mapping.

## DAC level-count correction and signed-zero retention — 2026-11-21

The earlier statement that the YM2413 had 511 measured output levels and that -256 was absent relied on `results/20150209.txt` alone. That note was explicitly corrected by `results/20150305.txt` at the same andete commit: 16x oversampling resolved 512 peaks, including two close central peaks interpreted as -0 and +0. `results/20150311.txt` further identifies both central values in captured output. The author's suggested idealized collapse of the two zeros is tentative. The 2015-02-09 count must not be treated as the final measurement.

FM-5.3 now preserves the two measured central-level tokens in the rhythm retention type, without claiming a raw 9-bit DAC encoding. See `docs/audits/ym2413-dac-output-evidence-2026-11-21.md` for source-by-source qualification and remaining unknowns.


### Post signed-zero FM-5.3 validation — PG differential #388

The latest output-retention code commit
`27650ff687c9495415644bca1db30585ffed21b9` passed the PG differential
workflow #388:

- 147,456 calls compared across all 18 serial cycles.
- Strict serial/control/LFSR/timing comparison: PASS.
- Integrated Rhythm Mode status remains
  `CHARACTERIZED_WITH_PATCH_CANDIDATE_DIVERGENCE`.
- The integrated candidate-difference object contains only:
  - `c_sl`: 8,092 rows, first call 1,815, Nuked=4/Rust=6.
  - `eg_sl`: 8,092 rows, first call 1,816, Nuked=4/Rust=6.
  - `eg_state`: 15,984 rows, first call 79,001.
- No `c_fb` or `c_dm` candidate difference remains in the integrated
  Rhythm Mode result.
- Rhythm reset differential #311: SUCCESS.
- CI #9852: SUCCESS, including workspace tests and observability runtime
  tests; the negative-zero retention tests are included in the workspace
  suite.
- EG characterization #561 remained in progress at this update.

This is software differential evidence. It does not establish the physical
SD sustain ROM, raw DAC word, or analog output.
