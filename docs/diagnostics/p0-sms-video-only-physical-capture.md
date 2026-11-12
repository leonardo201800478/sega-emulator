# P0 — Physical capture runbook: SMS video-only baseline

**Issue:** [#755](https://github.com/leonardo201800478/sega-emulator/issues/755)

**Current implementation scope:** SMS frame production, mailbox consumption, RGB conversion, WGPU texture upload, and OS presentation telemetry. Physical sound output and gameplay controls are not implemented in the current desktop build and are deliberately excluded from this capture.

## Preconditions

- Windows 11 x64, Release build, Vulkan backend.
- Record tested commit SHA, GPU and driver, Windows build, monitor resolution/refresh/VRR state, power mode, ROM SHA-256, region and exact in-game scene.
- Use a continuously changing scene so repeated frames can be distinguished. Keep the ROM and scene identical across repetitions.
- Do not add a frame limiter, change V-Sync/G-SYNC between repetitions, or alter MachineScheduler/region clocks.
- The Slint 16 ms timer is mailbox polling only, not a display refresh target.
- No physical audio endpoint, audio callback, controller input, keyboard gameplay input, or input-to-frame latency is part of this test.
- Digital PSG generation remains part of the emulated machine and must not be disabled or bypassed. The absence of physical sound output does not remove PSG state or its existing role in runtime frame processing/pacing.

## Capture matrix

| ID | Region | Window state | Duration | Repetitions |
|---|---|---|---:|---:|
| P0 | USA / NTSC | Visible and focused | 1 min | 1 |
| P1 | USA / NTSC | Visible and focused | 5 min | 3 |
| P2 | Europe / PAL | Visible and focused | 5 min | 2 |
| P3 | USA / NTSC | Visible but unfocused | 5 min | 1 |

The five-minute captures total 30 minutes, plus the one-minute smoke test. Keep startup/warm-up separate from steady-state analysis. P3 is a separate host-focus experiment and must not be merged into baseline statistics.

Brazil / PAL-M is not included in this first physical matrix; add it only after NTSC/PAL captures are stable and the exact ROM/runtime profile is verified.

## Correlation with PresentMon

The desktop writes one `R2_FRAME_PACING_HOST` record approximately once per second. The fields `qpc_available`, `qpc_ticks`, and `qpc_frequency` provide a raw QPC marker near the log emission point. Start PresentMon with QPC timestamps enabled and capture the emulator process that owns the presentation path:

```powershell
PresentMon.exe --process_id <PID> --output_file <capture.csv> --qpc_time --write_display_metadata --timed 300 --terminate_after_timed
```

Check that the CSV contains frame rows and QPC timestamps before accepting the capture. Record the process image and PID; if the launcher spawns a child, capture the actual desktop process rather than assuming the launcher owns WGPU presentation.

For Vulkan, do not reject a capture solely because PresentMon reports `Runtime=Other` or lacks a usable `msInPresentAPI` value. Interpret available present/display intervals and dropped-frame metadata according to the PresentMon version and its documented semantics. A QPC marker correlates host telemetry with the trace; it does not make `Queue::write_texture` a physical present timestamp.

## Metrics and acceptance

Analyze separately:

1. **Virtual machine:** machine ticks/frame min, p50, max; expected regional profile; no drift or Scheduler changes.
2. **SMS worker:** completed-frame interval p50/p95/p99; step duration; video publication; processing duration; requested and actual pacing wait.
3. **Mailbox/host:** polls, empty polls, consumed frames, sequence gaps, producer replacements, mailbox residence, conversion duration, queue-write duration.
4. **OS presentation:** PresentMon `MsBetweenPresents`, `MsBetweenDisplayChange`, dropped-frame and present-mode fields where available; correlate using QPC ticks/frequency.

Do not call a sequence gap or mailbox replacement a defect in isolation: the mailbox is latest-frame-wins. Compare producer cadence, consumer cadence and physical display events together. Do not equate a present event with photons emitted by the panel; optical validation requires a photodiode or equivalent external measurement.

A run is valid only if it has the full requested duration, correct PID, non-empty telemetry and PresentMon data, matching ROM/region/scene metadata, and no unrelated system load or configuration changes. Report p50/p95/p99, min/max, mean, standard deviation, and raw CSV/log files. No physical run is considered PASS until its raw evidence is reviewed.

## Out of scope

- Physical sound playback, CPAL/WASAPI/XAudio2 session behavior, PCM endpoint latency and underruns.
- Keyboard/gamepad mapping, SDL3, controller latency or input-driven test scenarios.
- Changes to CPU, Scheduler, VDP, PSG, bus, WAIT states, or regional clocks.
- Any claim that this instrumentation fixes the FPS symptom or proves physical panel scanout.
