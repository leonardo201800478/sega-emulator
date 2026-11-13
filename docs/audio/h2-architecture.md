# H2 streaming renderer — architecture and integration contract

Status: design specification; production implementation not yet present  
Target: SMS profile first; PSG remains default  
Branch: `feat/audio-h2-experimental-renderer`

## 1. Current production path (audited)

The current path is:

```text
SmsTemporalMachine / temporal bus
  -> ScheduledPsg (native/audio_runtime/src/psg.rs)
  -> Sn76489Core (native/sn76489/src/psg.rs)
  -> FIR + interpolated PCM sample queue
  -> SmsTemporalWorker::execute_frame
  -> AudioSession::process_samples
  -> AnalogOutputProcessor (optional, post-mixer)
  -> resampler / PCM sink
  -> CPAL
```

The scheduler supplies PSG clock advances and dispatch timestamps. `ScheduledPsg::apply_clock_advance` advances the existing core; `write_at_dispatch` applies register writes at dispatched boundaries. The worker drains the PSG PCM queue once per frame, including the discard path for partial frames. The desktop preferences already persist volume, mute and analog DSP configuration in `%APPDATA%/Sega Emulator/slint_ui_settings.txt`.

The H2 study in `native/sn76489/src/psg_h2_model.rs` is nested under the PSG module's test configuration. It constructs its own PSG in an offline test harness, captures amplitude deltas and renders a complete trace into full-session vectors. It is not a production renderer and is not called by the runtime.

## 2. Required production signal graph

```text
                     Existing SN76489 core
                 (one state, one clock authority)
                              |
                 optional transition observer
                 (disabled by default for PSG)
                              |
                     bounded event handoff
                              |
                     renderer selection
                       /           \
              existing PSG         H2
               FIR/PCM        event reconstruction
                       \           /
                        common PCM blocks
                              |
                         existing mixer
                              |
                      volume / mute policy
                              |
                  optional analog output DSP
                              |
                    existing resampler / sink
                              |
                             CPAL
```

The event observer must be observational only. It must not run a second SN76489 simulation, advance time, change registers, modify divider phase, alter WAIT states or call into Slint/CPAL.

## 3. Event contract

A production event represents the change in the *unfiltered digital PSG mix* at a PSG-clock boundary:

- absolute PSG clock timestamp within the current machine audio epoch;
- signed amplitude delta in the same normalized mix domain returned by the existing core mixer;
- monotonically nondecreasing timestamp;
- deterministic ordering for events sharing a timestamp;
- all same-clock deltas retained until the renderer canonicalizes them;
- reset/epoch transition explicitly clears the renderer timeline and queued events.

Events are generated only when the existing core's observable mixed amplitude changes. The observer must cover both internal divider ticks (tone/noise output transitions) and register writes that immediately change the mixed level (volume writes and noise-generator reset effects). Period writes that do not immediately change amplitude must not invent transitions.

The core's current `advance_clocks_inner` splits incoming PSG clocks at divider edges. A timestamp implementation must use the pre-advance absolute clock plus the exact first-edge offset derived from `clock_divider_phase`; it must not infer timestamps from the host frame number. Register writes occur after the Scheduler has applied the clock advance for that dispatch and therefore use the core's current absolute PSG-clock position.

Capture is opt-in. The default PSG path must not allocate, enqueue or sort events. When H2 is selected, capture is enabled at a safe worker command boundary and initialized from the current mixed level.

### Bounded handoff and overflow

The event handoff must have a documented fixed capacity and an explicit overflow counter. Overflow is a renderer fault, not a reason to silently lose transitions and continue with corrupt audio. On overflow, disable H2, reset its timeline, switch to the existing PSG renderer at the next worker-safe boundary, preserve volume/DSP/CPAL, and publish an actionable status/error. The default PSG path must remain available.

The worker must drain events on every `execute_frame` call, including partial-frame/debugger discard calls. For partial frames, H2 state must still consume the events to keep its timeline aligned, while generated PCM is discarded exactly as the current audio discard path behaves.

## 4. H2 kernel and streaming semantics

The reference study uses:

- 65 discrete taps;
- Kaiser beta 7.85726;
- continuous step response integrated at 1024x oversampling;
- normalized Kaiser-windowed sinc with cutoff 0.46 cycles/sample;
- fractional alignment offset -0.75 sample;
- offline event timestamps converted to a fractional 48 kHz sample position using integer quotient/remainder.

The production implementation must reuse the reference kernel data/algorithm rather than replace H2 with an EQ on PSG PCM. It must preserve the same fractional event alignment and deterministic simultaneous-event canonicalization. If an optimization changes the sampled kernel, response, alignment or output rounding, quantify it against the reference and document it.

The offline reference computes the whole session using arrays sized to the total sample count. Production must instead use:
- precomputed immutable kernel tables;
- a fixed-capacity circular accumulation buffer;
- persistent absolute sample index and rational clock-to-sample phase;
- bounded pending event storage;
- incremental event consumption;
- output only after the event horizon proves a sample cannot be changed by a later event;
- no WAV/filesystem/traces in the render path.

Do not equate the 31.25-sample measured group delay with the required event lookahead. They are different quantities. Derive the exact safe emission horizon from the reference's `first`, `settled`, and `response_to_step` indexing. Preserve the reference output at block boundaries, including the first sample, the final tail and reset behavior. The actual host latency is the sum of renderer algorithmic delay, worker buffering and output queueing; report these separately.

## 5. Renderer API boundary

The runtime-facing abstraction should accept blocks of digital events and an absolute end-clock, then produce a bounded PCM block plus health counters. It should not expose PSG registers or own hardware state.

Conceptual contract:

- `begin_epoch(initial_clock, initial_level)`
- `push_events(events)`
- `render_until(end_clock, output_limit)`
- `reset_epoch(clock, current_level)`
- `metrics()`

The API must be adapted to repository conventions and should not be implemented as callbacks per CPU instruction. It should run on the existing machine worker thread, before the common audio session, not on the CPAL callback.

The renderer selection belongs to the SMS worker/runtime. The GUI sends a coarse-grained `SetAudioRenderer` command. The selected mode is persisted in desktop preferences. A mode transition must occur between worker frame/command boundaries. If both renderers are not simultaneously available for a short overlap, use a bounded fade-to-silence / fade-in transition rather than restarting CPAL or running two complete hardware simulations.

## 6. Isolation and failure policy

- PSG is the default for new and existing installations.
- H2 is opt-in and visibly labeled experimental.
- Enabling H2 may enable event capture; disabling it must stop capture and clear pending events.
- The PSG FIR, sample queue, register behavior and output must remain unchanged.
- The analog DSP remains downstream of renderer selection and independent of H2.
- Volume, mute and analog DSP preferences must survive renderer changes.
- H2 initialization failure, event overflow, invalid timestamp ordering or non-finite output causes safe fallback to PSG and a diagnostic status.
- No CPAL restart is required for renderer changes.
- No locks, heap allocations or H2 work in the device callback.

## 7. Required tests before production use

See `h2-validation.md`. No production H2 claim is permitted until the event observer, streaming kernel, runtime integration and fallback have passed the stated gates on CI. Benchmarks and physical Windows listening are separate evidence categories.


## 8. Implementation checkpoint — bounded event observer

The first production-facing prerequisite has now been added on the feature branch:

- `Sn76489Core` exposes opt-in amplitude-event capture; it is disabled by default.
- Events are recorded from the existing core's real `mix()` before/after internal ticks and bus writes.
- Event timestamps use an absolute PSG-clock counter and the exact /16 divider-edge offset.
- Same-timestamp writes remain separate ordered events for canonicalization by the renderer.
- The observer uses a preallocated bounded `VecDeque`; overflow is sticky, counted and stops further capture instead of silently evicting older events.
- `SmsTemporalBus` and `SmsTemporalMachine` expose coarse-grained enable/disable/drain/status methods. No CPU-instruction callback was added.
- The PSG output-generation code path is otherwise unchanged; capture-disabled operation does not call `mix()` for event observation.

Targeted unit tests have been added for immediate volume transitions, internal tone transitions, chunk-invariant timestamps, explicit bounded overflow, reset epoch behavior and exact PCM equality with capture enabled versus disabled. These tests have **not yet been executed** in this environment.

This is an event-observer prerequisite only. It does not implement the H2 streaming kernel, worker renderer selection, Slint selector, preference persistence or runtime fallback. Do not enable this observer in production UI until those components are complete and the tests pass.


## 9. Implementation checkpoint — standalone streaming kernel

A production-facing renderer type now exists in `native/sn76489/src/psg_h2_streaming.rs` and is exported from the SN76489 crate:

- Precomputes the same 1024x oversampled integrated Kaiser-sinc step response as the offline reference.
- Retains the reference's 0.46 cutoff, Kaiser beta 7.85726, 32-sample half-span and -0.75 fractional alignment.
- Uses two fixed 128-sample accumulation rings; pending storage does not scale with session duration.
- Consumes same-clock event groups with deterministic sorted-delta summation.
- Emits the closed PCM prefix as each event group arrives, keeping the ring bounded even when a worker block contains many events.
- Uses integer quotient/remainder for clock-to-sample positioning.
- Rejects backdated/out-of-horizon events and pending-horizon violations rather than silently corrupting PCM.
- Supports explicit epoch start and final drain.

Tests now compare the streaming output to the original offline H2 reference, verify block-boundary invariance, same-clock ordering and invalid/backdated event rejection. These tests are **added but not executed**.

The streaming renderer is not yet connected to `SmsTemporalWorker`, `AudioSession`, Slint or preferences. The event observer and streaming kernel are independently available, but the game still uses the original PSG path only.
