# H2 streaming validation

Status: acceptance plan; no production H2 validation has been run  
Baseline: existing SN76489 PSG renderer remains authoritative and default

## 1. Evidence rules

- A passing compile is not proof of temporal correctness.
- The H2 offline study tests do not validate production streaming.
- Do not claim PSG/H2 waveform identity; they are different reconstruction models.
- Compare H2 to the existing offline event-based reference for kernel correctness and compare both renderers on identical digital stimuli for spectral characterization.
- Record commit SHA and exact workflow run for every PASS.
- Physical listening and electrical captures are separate from automated CI.

## 2. Event capture tests

- No event capture allocations/queueing when PSG mode is selected.
- Tone transitions carry the exact PSG clock timestamp.
- Volume writes that alter mixed amplitude emit the correct delta at the write boundary.
- Period writes that do not alter current amplitude do not create false events.
- Noise control reset captures an immediate output-level change only when one occurs.
- Tone/noise transitions on the same internal tick are represented as one same-timestamp group or deterministically sortable events.
- Same-clock bus writes are canonicalized deterministically.
- Event timestamps are monotonic across clock advances and writes.
- Event epoch reset clears pending events and initializes the baseline level.
- Event queue overflow increments a counter and forces safe fallback; no silent event loss.

## 3. Streaming-kernel equivalence

For identical ordered events, compare the streaming output to the offline reference at the same absolute sample indices:
- continuous render;
- block sizes 1, 7, 64, 997, 800, 960 and 16,384 samples;
- clock chunks 1, 7, 16, 997, 16,384 and frame-sized clock advances;
- events immediately before, at and after a block boundary;
- events at the same clock, including cancelling deltas;
- fractional event positions at varied clock/sample phase;
- first output sample and reset at nonzero absolute clock;
- final drain/tail behavior.

The expected condition is bit-identical output when using the same arithmetic/order, or a documented tight numeric tolerance if the implementation intentionally changes operation ordering. Any tolerance must be justified by a measured bound, not chosen to make tests pass.

## 4. Digital stimulus coverage

Use the SMS profile first:
- silence and reset;
- each tone channel individually;
- all three tones simultaneously;
- tone periods 0, 1, 2, 3, 4, 5, 7, 14, 28, 56, 112, 255, 512, 1023;
- writes to low/high period nibbles during playback;
- all attenuation values 0–15 for each channel;
- periodic noise and white noise;
- noise rate 0–3, including tone-2-linked rate;
- noise register reset while divider is mid-phase;
- simultaneous channel/noise transitions;
- volume and period writes at Scheduler dispatch boundaries.

SG-1000 tests must ensure profile-specific LFSR behavior remains unchanged if/when H2 supports that profile. The initial production scope may be SMS only; do not silently claim SG-1000 support.

## 5. Regression and integration

- Compare the existing PSG PCM output before/after the change for identical stimuli; require exact equality.
- Run all SN76489 tests and all audio_runtime tests.
- Run SMS temporal bus and runtime tests, including continuous-versus-chunked Scheduler advancement.
- Verify save/restore state behavior and event-capture state policy.
- Run worker tests for full frames and breakpoint-truncated frames.
- Verify volume, mute and analog DSP before/after renderer selection.
- Verify reset and ROM replacement clear the H2 timeline.
- Verify renderer switching at worker-safe boundaries, with bounded fade/crossfade or documented fade-to-silence behavior.
- Verify initialization failure, invalid event ordering and overflow fall back to PSG without restarting CPAL.
- Verify no CPAL callback allocation, blocking lock or H2 computation.

## 6. Desktop validation (Windows x64)

- Slint menu presents mutually exclusive PSG and H2 choices.
- PSG is selected for existing settings files and fresh installs.
- H2 is visibly marked experimental and selected mode is persisted.
- Changing renderer does not restart CPAL.
- Current renderer status/error is visible.
- DSP controls and volume/mute remain functional and persistent.
- A/B listening with identical ROM, frame, region and volume; record listener notes without treating preference as proof of hardware accuracy.

## 7. Current evidence

| Gate | State |
|---|---|
| Existing offline H2 study | historical tests and CI documented in continuity checkpoint |
| Production event observer | IMPLEMENTED ON FEATURE BRANCH; targeted tests added, NOT RUN |
| Standalone streaming kernel | IMPLEMENTED ON FEATURE BRANCH; tests added, NOT RUN |
| Event-to-PCM runtime integration | NOT IMPLEMENTED |
| Renderer selection command | NOT IMPLEMENTED |
| Slint PSG/H2 selector | NOT IMPLEMENTED |
| Persisted renderer preference | NOT IMPLEMENTED |
| Streaming determinism | NOT RUN |
| PSG bit-exact regression after observer change | NOT RUN |
| Performance / soak test | NOT RUN |
| Windows listening test | NOT RUN |

No production H2 acceptance claim is made by this document.


## 8. Partial implementation evidence

The feature branch now contains opt-in event capture in `Sn76489Core`, exposed through the SMS temporal bus and machine. Added tests cover:
- volume-write delta at the current PSG clock;
- tone transition timestamp at the /16 divider edge;
- event stream invariance under different clock chunking;
- bounded queue overflow signaling;
- reset epoch clearing;
- exact PCM equality between capture-enabled and capture-disabled core runs.

The source changes have been committed, but no local Rust toolchain/test runner was available during this implementation step. Therefore these tests are **added, not passed**. They must be run on CI before the event observer is accepted. No production H2 renderer or UI integration exists yet.


The standalone kernel's new tests include direct output comparison with `psg_h2_model.rs` using identical timestamped deltas, plus segmented/continuous output equivalence. They remain unexecuted and therefore are not PASS evidence.
