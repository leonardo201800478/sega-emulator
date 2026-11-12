# P0 — SMS frame-pacing telemetry

**Issue:** [#755](https://github.com/leonardo201800478/sega-emulator/issues/755)

**Scope:** diagnostic instrumentation only; no timing-policy correction is included.

## Decision

Do not change MachineScheduler, Z80/VDP execution, regional clock profiles, or the worker's pacing policy until a reproducible capture identifies the stage responsible for the observed cadence loss.

This change adds bounded telemetry to the existing SMS worker and host presentation path. The desktop enables diagnostics when it creates or replaces an SMS worker. Metrics are observational: they never feed values back into emulated time, instruction execution, event dispatch, or frame production.

## Clock domains and measurement boundaries

The capture keeps these domains separate:

1. **Virtual machine time:** SmsTemporalMachine::now() / Scheduler master ticks. This is the hardware timeline.
2. **Worker wall time:** monotonic Instant measurements around step_frame_until, completed-frame publication, audio submission, total frame work, and the existing pacing wait.
3. **Mailbox boundary:** publication sequence and monotonic publication timestamp, replacement count, host polling, empty polls, consumed sequence gaps, mailbox residence time, and time between consumed frames.
4. **WGPU host boundary:** RGB888→RGBA8 conversion and duration of the Queue::write_texture call.

The Slint Winit backend owns the surface and physical present operation. Therefore this patch explicitly does **not** label queue.write_texture as GPU completion or physical presentation, and logs physical_present_timestamp=unavailable_slint_owned. A future present timestamp requires a supported Slint/backend hook or a separately justified renderer integration; it must not be fabricated from request_redraw.

## Worker metrics

SmsFramePacingMetrics reports a rolling window of at most 512 completed frames:

- wall-clock frame interval p50/p95/p99;
- virtual master ticks advanced per completed frame (min/p50/max);
- step_frame_until duration p50/p95/p99;
- VideoFrame construction + FrameMailbox::publish duration p50/p95/p99;
- physical audio-session submission duration p50/p95/p99 (zero when no session exists);
- total execute_frame processing duration p50/p95/p99;
- calculated worker pacing-wait budget p50/p95/p99;
- actual time spent in the worker's command-queue wait p50/p95/p99.

The window is fixed-capacity and overwritten in place. Percentiles are calculated only when a host requests a snapshot, not on every frame. The first frame has no preceding interval and is excluded from interval percentiles.

The machine-tick delta is recorded independently of host duration. For PAL and NTSC captures, compare its stability and regional expected cadence separately from host frame intervals. A stable virtual tick delta with unstable wall intervals points away from Scheduler progression and toward worker/host scheduling or presentation.

## Scheduler instrumentation granularity

This first pass captures the virtual machine timestamp immediately before and after each completed frame step and records the resulting master-tick delta. It does not call Instant around every individual Scheduler dispatch. The SMS path dispatches at instruction/bus/T-state boundaries; per-dispatch wall-clock instrumentation could add significant observer overhead and contaminate the very pacing being measured.

If the frame-level master-tick delta is inconsistent, or if step_frame duration is unstable while other host stages remain stable, the next diagnostic increment should add sampled/aggregated dispatch counters inside SmsTemporalMachine::dispatch_next (dispatch count, event class, elapsed host time and max/p95 sample), behind the existing diagnostics switch. That deeper instrumentation must remain opt-in and must not affect event order, dispatch budget, or virtual time.

## Host metrics

The desktop emits one R2_FRAME_PACING_HOST summary approximately once per second. It reports both pacing_wait_budget (the requested timeout computed by the worker) and pacing_wait_elapsed (the actual monotonic duration until timeout or command arrival):

- timer callback count and empty mailbox polls;
- average/max timer polling interval and mailbox residence interval;
- consumed frames, sequence gaps, the cumulative producer mailbox replacement counter, and publish-to-consume mailbox residence time;
- average/max RGB888→RGBA8 conversion time;
- average/max Queue::write_texture call time;
- worker p50/p95/p99 frame intervals and p95 stage durations;
- machine clock and ticks-per-frame range.

The host report is deliberately explicit that physical present timestamps are unavailable through the current Slint-owned surface. sequence_gaps is a per-window count and mailbox_replaced_total is cumulative; neither is automatically a defect: the mailbox is latest-frame-wins, so replacements can be expected if the consumer is slower. They must be interpreted alongside produced and consumed cadence.

## Existing pacing policy — audit target, not changed here

The current worker computes a wall-time frame budget from the number of drained PSG samples (sample_count / SMS_TEMPORAL_SAMPLE_RATE) and waits for the remainder after processing. This is an existing pacing policy and is now measured separately as pacing_wait_budget (requested timeout) and pacing_wait_elapsed (actual time spent in recv_timeout, including early command wake-up or host wake-up overshoot); this PR does not assume it is correct or incorrect. In particular, it must not be replaced by a guessed PAL/NTSC sleep, GUI timer, VSync callback, or audio-device callback before comparing measured virtual ticks, generated sample counts, processing cost, and host intervals.

The desktop's PRESENT_INTERVAL remains a mailbox polling interval, not the emulated frame period or a guarantee of physical display cadence.

## Capture procedure

1. Use a Release build on Windows x64 with the Vulkan backend and record the commit SHA, GPU/driver, display mode, region, ROM identity, and whether a physical audio session is configured.
2. Keep the same driver/G-SYNC/V-Sync configuration across comparison runs. Do not use a frame limiter or alter Scheduler settings during the baseline.
3. Run a representative NTSC ROM for at least five minutes; repeat with a representative Europe/PAL ROM for at least five minutes.
4. Save stdout/stderr containing R2_FRAME_PACING_HOST records. Keep the initial startup interval separate from steady state.
5. Compare worker frame interval, virtual ticks/frame, step duration, publish duration, requested/actual pacing wait, mailbox replacements, host consumed intervals, publish-to-consume residence, and queue-write duration.
6. Repeat with the presentation window minimized/occluded only as a separate experiment; do not combine those results with the normal visible baseline.
7. Only after locating the stage, propose a narrowly scoped correction and add deterministic regression tests.

## Interpretation matrix

| Observation | Likely investigation area | Not sufficient to conclude |
|---|---|---|
| Virtual ticks/frame vary | Machine/VDP frame boundary and Scheduler integration | That wall-clock throttling is the fix |
| Virtual ticks/frame stable; step_frame p95 spikes | CPU/VDP work, contention, locks, host thread scheduling | That Scheduler's logical time is wrong |
| step_frame stable; publish p95 spikes | Frame allocation/copy, mailbox lock/ownership | That VRR can fix it |
| Worker cadence stable; consumed cadence/gaps unstable | GUI timer, mailbox consumer, window event-loop starvation | That emulated frames are late |
| Queue-write duration spikes | Upload path, driver submission overhead | Physical GPU completion/present latency |
| All host stages stable but actual display cadence irregular | Slint/Winit/WGPU surface present path and display synchronization | That request_redraw timestamp equals present |
| PAL virtual cadence stable near its profile; host cadence not | Worker pacing and presentation policy | That the PAL hardware clock should be changed |

## Non-goals and invariants

- No changes to MachineScheduler, CPU, VDP, PSG, bus, WAIT states, or regional clock profiles.
- No sleep/throttle in the GUI and no host-driven hardware advancement.
- No frame skipping, interpolation, virtual clock correction, or scheduler-to-vblank coupling.
- No CPAL integration; an audio-submit metric of zero means no physical session work was measured.
- No claim that this PR fixes the P0 symptom. It provides evidence to select and validate the subsequent correction.
