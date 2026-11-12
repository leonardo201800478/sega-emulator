# P0 — Physical capture runbook: paired video-only and audio-active baselines

**Issue:** [#755](https://github.com/leonardo201800478/sega-emulator/issues/755)

**Current implementation scope:** SMS frame production, mailbox consumption, RGB conversion, WGPU texture upload, and OS presentation telemetry. CPAL physical audio is integrated and normally initialized automatically when an SMS worker is created. For this video-only capture, set `SEGA_EMULATOR_VIDEO_ONLY=1` before launching the desktop; this skips CPAL session creation while preserving digital PSG generation and its normal contribution to frame processing/pacing. Gameplay input and input-to-frame latency remain outside this capture.

## Preconditions

- Windows 11 x64, Release build, Vulkan backend.
- Record tested commit SHA, GPU and driver, Windows build, monitor resolution/refresh/VRR state, power mode, ROM SHA-256, region and exact in-game scene.
- Use a continuously changing scene so repeated frames can be distinguished. Keep the ROM and scene identical across repetitions.
- Do not add a frame limiter, change V-Sync/G-SYNC between repetitions, or alter MachineScheduler/region clocks.
- The Slint 16 ms timer is mailbox polling only, not a display refresh target.
- Run both modes defined below. In video-only mode, set `SEGA_EMULATOR_VIDEO_ONLY=1` before launch and confirm `R3_AUDIO skipped: SEGA_EMULATOR_VIDEO_ONLY enabled`; if it reports `CPAL output opened`, invalidate that run.
- In audio-active mode, leave `SEGA_EMULATOR_VIDEO_ONLY` unset, confirm `R3_AUDIO CPAL output opened`, confirm `R3_AUDIO_HEALTH configured=1`, confirm `consumed_frames` increases and `output_error=None`, and verify audible PSG output at the selected physical endpoint. If CPAL is unavailable or silent, mark the audio-active run invalid rather than treating it as an audio-on sample.
- No controller input, keyboard gameplay input, or input-to-frame latency is part of either test mode.
- Digital PSG generation remains part of the emulated machine and must not be disabled or bypassed. The absence of physical sound output does not remove PSG state or its existing role in runtime frame processing/pacing.

## Video-only launch mode

The desktop reads `SEGA_EMULATOR_VIDEO_ONLY` when creating each SMS worker. Accepted true values are `1` and `true` (case-insensitive). Unset, `0`, and other values retain normal CPAL initialization. Apply the variable to the same PowerShell process that launches `sega-desktop.exe`; do not set it after the worker has been created. The flag bypasses only the physical CPAL session: PSG emulation and sample generation remain active.

For audio-active runs, leave the variable unset. `R3_AUDIO CPAL output opened` records the negotiated sample rate, channels, sample format, fallback flag and queue capacity. The one-second `R3_AUDIO_HEALTH` record reports configured state, queue occupancy/high-water, accepted/consumed/dropped frames, underrun frames/events, dropped blocks, flushed frames and any session error. These counters are cumulative snapshots; calculate per-window deltas from consecutive records. `consumed_frames` increasing proves callback consumption, not audible sound, so also verify audible output at the selected device. Do not change endpoint, volume, sample rate preference, Windows audio enhancements or exclusive/shared mode between paired runs.

## Capture matrix

| ID | Region | Window state | Audio mode | Duration | Repetitions |
|---|---|---|---|---:|---:|
| P0-V | USA / NTSC | Visible and focused | Video-only | 1 min | 1 |
| P0-A | USA / NTSC | Visible and focused | CPAL active | 1 min | 1 |
| P1-V | USA / NTSC | Visible and focused | Video-only | 5 min | 3 |
| P1-A | USA / NTSC | Visible and focused | CPAL active | 5 min | 3 |
| P2-V | Europe / PAL | Visible and focused | Video-only | 5 min | 2 |
| P2-A | Europe / PAL | Visible and focused | CPAL active | 5 min | 2 |
| P3-V | USA / NTSC | Visible but unfocused | Video-only | 5 min | 1 |
| P3-A | USA / NTSC | Visible but unfocused | CPAL active | 5 min | 1 |

This paired matrix totals 62 minutes, excluding startup and warm-up: 31 minutes per audio mode. Run the two modes in alternating pairs (V then A, or A then V chosen before the experiment and kept consistent) to reduce time-of-day and thermal drift. Keep ROM SHA-256, region, scene, window geometry, focus state, display settings and system load matched within each pair. Use the exact same ROM and scene for audio-on/off; choose a continuously changing scene with audible PSG activity. P3 is a separate host-focus experiment and must not be merged into baseline statistics.

For the first causal comparison, prioritize P1-V and P1-A: three paired five-minute NTSC captures. Continue to P2 and P3 only after confirming valid logs and PresentMon data in both P1 modes.

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
2. **SMS worker:** completed-frame interval p50/p95/p99 and p95−p50 jitter; step duration; video publication; `audio_submit_p50/p95/p99`; frame processing; requested and actual pacing wait; PSG sample count. Compare video-only and CPAL-active distributions without assuming that the audio submit path is equivalent when the session is absent.
3. **Mailbox/host:** p50/p95/p99 and jitter for timer poll intervals and consumed-frame intervals; p50/p95/p99 for mailbox residence, mailbox take, RGB conversion, `Queue::write_texture` and total Slint callback; polls, empty polls, consumed frames, sequence gaps and producer replacements. Host distributions are rolling windows of up to 512 observations; each record includes sample counts.
4. **Physical audio output (CPAL-active runs):** negotiated endpoint rate/channels/format and fallback flag from `R3_AUDIO`; `R3_AUDIO_HEALTH` cumulative accepted/consumed/dropped frames, dropped blocks, queue high-water, underrun frames/events, flushed frames and output error. Derive interval deltas between consecutive one-second records. A healthy capture requires no callback/session error, no unexplained drops/underruns, increasing consumed frames and audible PSG output. Keep audio metrics separate from video presentation metrics.
5. **OS presentation:** PresentMon `MsBetweenPresents`, `MsBetweenDisplayChange`, dropped-frame and present-mode fields where available; correlate using QPC ticks/frequency.

Do not interpret `empty_polls` as physical repeated frames: it only means no new frame was ready when the Slint timer polled. Do not call a sequence gap or mailbox replacement a defect in isolation: the mailbox is latest-frame-wins. Compare producer cadence, consumer cadence and physical display events together. Do not equate a present event with photons emitted by the panel; optical validation requires a photodiode or equivalent external measurement.

A run is valid only if it has the full requested duration, correct PID, non-empty telemetry and PresentMon data, matching ROM/region/scene metadata, and no unrelated system load or configuration changes. Report p50/p95/p99, min/max, mean, standard deviation, and raw CSV/log files. No physical run is considered PASS until its raw evidence is reviewed.

## Out of scope

- Acoustic waveform/latency measurement at the speaker output. Audible confirmation is a functional check, not a calibrated latency measurement; use loopback capture or an external audio interface only if a later investigation requires acoustic timing.
- Keyboard/gamepad mapping, SDL3, controller latency or input-driven test scenarios.
- Changes to CPU, Scheduler, VDP, PSG, bus, WAIT states, or regional clocks.
- Any claim that this instrumentation fixes the FPS symptom or proves physical panel scanout.
- Treating video-only and audio-active captures as a single statistical population; analyze each mode separately before comparing paired results.
