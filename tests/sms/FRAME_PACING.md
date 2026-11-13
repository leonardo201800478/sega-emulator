# SMS ROM frame-pacing harness

This harness runs an external SMS ROM through the production `SmsTemporalWorker`
and its real host pacing loop (`recv_timeout`). It is deliberately distinct from
`sms_rom_runner.rs`, which steps `SmsTemporalMachine` offline without host pacing.

## Run

From the repository root:

```powershell
python tests/sms/scripts/run_pacing_rom.py --rom .\artifacts\SMSmemtest.sms --suite-id smsmemtest --suite-revision 12771ed247b93d3a83d0c3cbe7d2d39e3ccb47d9 --region usa --duration-seconds 300 --warmup-seconds 10 --button1-pulse-frames 26,441 > .\artifacts\smsmemtest-pacing-ntsc.jsonl
```

For an interactive ROM that waits for controller input, provide frame-boundary
button pulses. Frame numbers are relative to the post-warm-up reset:

```powershell
python tests/sms/scripts/run_pacing_rom.py --rom .\artifacts\SMSmemtest.sms --suite-id smsmemtest --suite-revision 12771ed247b93d3a83d0c3cbe7d2d39e3ccb47d9 --region usa --duration-seconds 300 --button1-pulse-frames 26,441 > .\artifacts\smsmemtest-pacing-ntsc.jsonl
```

The pinned SMSmemtest build already executed by CI provides a reproducible starting schedule: pulse at frames 26 and 441. In the prior full-machine run, the first prompt was accepted at frame 26, all seven RAM cases were reported at frame 441, the second prompt was accepted at frame 441, and the VRAM result was observed at frame 800. That evidence used ROM SHA-256 `f7654d1dea07ce195e1d57da373a86b3f1ca9869af0d9d7dcb4de6039eb52c71`; verify the SHA emitted by the local build before reusing the schedule. A pulse is asserted at a worker frame boundary and released after the next completed frame. Do not assume that a pulse means the ROM passed a test.

## What this measures

- Actual worker frame interval percentiles (p50/p95/p99).
- Time spent stepping the machine and processing each frame.
- Requested pacing wait, measured wait elapsed, and timeout wake lateness.
- Virtual machine ticks per frame, PSG samples per frame, instruction count,
  frame count, region clock, and run/ROM/emulator provenance.
- One JSONL `sample` record per sampling interval, containing rolling-window
  metrics, plus a final `summary`.

The worker diagnostics window holds at most 512 per-frame samples. Sampling every
second produces overlapping rolling windows; `window_sample_count` and
`total_frame_samples` make that explicit. These are not raw per-frame samples.
Do not reconstruct exact individual frame intervals from overlapping percentiles.

## Instrumentation overhead A/B

Run two captures with identical ROM SHA, emulator build, region, duration, warm-up,
input pulse schedule and host configuration:

```powershell
python tests/sms/scripts/run_pacing_rom.py --rom .\artifacts\SMSmemtest.sms --suite-id smsmemtest --suite-revision 12771ed247b93d3a83d0c3cbe7d2d39e3ccb47d9 --region usa --duration-seconds 300 --warmup-seconds 10 --button1-pulse-frames 26,441 > .\artifacts\smsmemtest-diagnostics-on.jsonl
python tests/sms/scripts/run_pacing_rom.py --rom .\artifacts\SMSmemtest.sms --suite-id smsmemtest --suite-revision 12771ed247b93d3a83d0c3cbe7d2d39e3ccb47d9 --region usa --duration-seconds 300 --warmup-seconds 10 --button1-pulse-frames 26,441 --diagnostics-disabled > .\artifacts\smsmemtest-diagnostics-off.jsonl
```

The diagnostics-off run retains observed FPS, completed frames, total instructions
and virtual time, but stage percentiles are zero because the rolling telemetry is
disabled. Use it only to estimate instrumentation overhead; it cannot identify
wake lateness or step-cost distribution by itself. The `diagnostics_enabled`
field in the start/summary records identifies each condition.

## Isolation boundary

This harness uses the production worker and its real `recv_timeout` pacing, but
intentionally disables video presentation and physical audio output. PSG still
runs inside the temporal machine and samples are drained by the worker. Therefore
it isolates ROM-dependent machine/worker cost and host wake behavior from Slint,
WGPU, Vulkan, the physical display, and CPAL. It does **not** replace the
Windows GUI + PresentMon + CPAL A/B matrix required to close P0.

The Rust `Instant` clock is monotonic. On Windows it is backed by QPC, but this
harness currently emits elapsed nanoseconds rather than raw QPC counter values
or QPC frequency. Use ETW/WPA and PresentMon for physical presentation analysis.

## ROM selection and limits

- SMSmemtest is useful for RAM/VRAM functional checks, but it is interactive.
  Use the evidenced frame pulse schedule `26,441` with the pinned ROM hash above;
  without input it remains at its prompt and is not a meaningful sustained workload.
- SMSTestSuite can cover video, PSG/FM and input behavior after its pinned build
  dependencies and case oracles are established. Its audited revision has no
  explicit license located; use locally only pending clarification.
- SmsVdpTest can provide VDP protocol/counter/sprite workloads. Run only cases
  compatible with the current scanline-granular VDP. Dot/fetch and sub-scanline
  timing cases remain `BLOCKED_CAPABILITY`. Its audited revision has no explicit
  license located; use locally only pending clarification.
- ZEXALL-SMS remains blocked until formal Z80 E7 closure. Do not run it as a
  pacing ROM before that gate.
- Record exact upstream revision, build toolchain, ROM SHA-256, region, emulator
  SHA, duration, warm-up and pulse schedule. Do not commit ROM binaries or logs
  containing copyrighted data.

## Interpretation

Compare one variable at a time with identical ROM SHA, emulator SHA, region,
duration, warm-up, pulse schedule, build profile and host configuration. Use
separate runs for each ROM. For each run inspect:

- `observed_fps` and `frame_interval_p50/p95/p99_ns`;
- `step_frame_*` and `frame_processing_*`;
- `pacing_wait_elapsed_*` and `pacing_timeout_lateness_*`;
- `machine_ticks_per_frame_*` and `audio_samples_*`.

A completed run or `CAPTURED` summary is not a ROM PASS and does not prove
physical presentation cadence. A stable virtual tick count with irregular
worker intervals points away from virtual-clock drift, but does not identify
which host wait/pipeline stage caused it. Preserve JSONL output outside Git.
