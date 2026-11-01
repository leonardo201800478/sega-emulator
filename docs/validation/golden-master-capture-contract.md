# Golden Master Capture Contract

**Status:** PROPOSED — documentation-only contract  
**Scope:** SG-1000 and Master System (SMS), extensible to later Sega systems  
**Initial implementation:** PR A; no runtime, timing, or frontend behavior changes

## 1. Purpose

Define a reproducible, auditable format for capturing and comparing emulator executions. A capture is evidence tied to an exact emulator revision, machine configuration, input replay, and media identity.

The contract separates three claims:

1. **Determinism:** independent executions with identical declared inputs produce identical outputs.
2. **Baseline integrity:** an execution matches an explicitly approved reference capture.
3. **Hardware fidelity:** outputs agree with an independent hardware trace, test ROM, or trusted reference implementation under a documented comparison method.

A deterministic result is not, by itself, evidence of hardware accuracy.

## 2. Non-goals and constraints

- This contract does not implement a scheduler, alter CPU execution, or add cycle-accurate behavior.
- Python is an orchestration and serialization layer only. Hardware state, timing, memory access, and audio/video generation remain in Rust.
- No callback from Rust to Python per instruction, bus access, pixel, or audio sample.
- Commercial ROMs, BIOS images, save files containing copyrighted data, and raw memory dumps must not be committed.
- A capture tool may read user-selected local ROM/BIOS files. It records their names (sanitized where necessary), sizes, and SHA-256 digests, never their bytes.
- Captures from different emulator commits, build profiles, host backends, machine regions, or media digests are not silently treated as equivalent.

## 3. Capture bundle layout

A capture bundle is a directory or archive with this logical layout:

```text
capture/
  manifest.json
  inputs/
    replay.json
  checkpoints/
    cpu.jsonl
  video/
    frame-000000.rgb
    frame-000100.rgb
  audio/
    pcm-s16le-44100-mono.pcm
  comparison/
    report.json
```

Only artifacts actually produced by the selected scenario are present. The manifest lists every artifact and its byte length and SHA-256. Large binary artifacts may be stored as CI artifacts or in approved external artifact storage; the repository stores manifests, small test fixtures, and documentation, not commercial media or large binary dumps.

## 4. Manifest schema (version 1)

The following is an illustrative JSON instance. Values containing `...` are placeholders and are not valid digests.

```json
{
  "schema_version": 1,
  "capture_id": "sms-homebrew-boot-001",
  "system": "sms",
  "emulator": {
    "repository": "leonardo201800478/sega-emulator",
    "commit": "<full-git-sha>",
    "dirty": false,
    "build_profile": "release",
    "rustc": "<rustc-version>",
    "python": "<python-version>",
    "extension": {
      "name": "sega_z80_core",
      "version": "<extension-version>",
      "sha256": "<sha256>"
    }
  },
  "media": {
    "cartridge": {
      "name": "homebrew-test.sms",
      "size_bytes": 32768,
      "sha256": "<sha256>",
      "license_or_provenance": "project-owned-homebrew"
    },
    "bios": null
  },
  "machine": {
    "region": "usa",
    "video_standard": "ntsc",
    "sample_rate_hz": 44100,
    "audio_format": "s16le",
    "audio_channels": 1,
    "video_format": "rgb888",
    "visible_width": 256,
    "visible_height": 192
  },
  "scenario": {
    "id": "sms-homebrew-boot-001",
    "version": 1,
    "frames_requested": 600,
    "frames_completed": 600,
    "input_replay_sha256": "<sha256>",
    "checkpoint_policy": "frame-boundary-and-explicit-events"
  },
  "results": {
    "final_frame_number": 600,
    "final_pc": 4660,
    "total_instructions": 123456,
    "total_tstates": 987654,
    "artifacts": [
      {
        "path": "video/frame-000600.rgb",
        "media_type": "application/octet-stream",
        "format": "rgb888",
        "size_bytes": 147456,
        "sha256": "<sha256>"
      },
      {
        "path": "audio/pcm-s16le-44100-mono.pcm",
        "media_type": "application/octet-stream",
        "format": "s16le",
        "sample_rate_hz": 44100,
        "channels": 1,
        "size_bytes": 0,
        "sha256": "<sha256>"
      },
      {
        "path": "checkpoints/cpu.jsonl",
        "media_type": "application/x-ndjson",
        "format": "z80-register-checkpoints-v1",
        "size_bytes": 0,
        "sha256": "<sha256>"
      }
    ]
  }
}
```

The example illustrates the field structure, not current capabilities of either native binding. Producers must omit unknown fields rather than invent values. A capture with `dirty: true` may be useful diagnostically but cannot become an approved baseline.

## 5. Input replay

Input replay is a deterministic, timestamped sequence in machine time. Version 1 records frame-boundary events until a machine-time event API is available; it must not pretend that frame-level input reproduces sub-frame electrical timing.

Each event contains:
- monotonically increasing sequence number;
- frame index at which it is applied;
- logical controller/port identifier;
- complete normalized button state (including released buttons);
- optional scenario marker.

The replay file includes schema version, system, initial input state, event list, and SHA-256 in the manifest. Host keyboard polling, SDL events, wall-clock timestamps, and GUI focus are not replay inputs.

## 6. CPU checkpoints

Checkpoint records are observations, not save states. At minimum, a Z80 checkpoint records:
- checkpoint sequence and frame index;
- PC, AF, BC, DE, HL, alternate register sets, IX, IY, SP;
- I, R, interrupt mode, IFF1/IFF2, HALT state, EI-delay state;
- total instruction and T-state counters exposed by the runtime.

Where the runtime does not expose a field, mark the checkpoint schema capability as unavailable; do not synthesize it. The first implementation must document whether snapshots are taken at a stable instruction boundary. It must not claim sub-instruction bus state unless that state is actually exposed.

Checkpoint cadence is scenario-defined and fixed in the scenario version. Recommended points are initial state, each frame boundary, and named events such as interrupt entry or BIOS handoff. A checkpoint's time basis (frame boundary, instruction boundary, or explicit event) is mandatory.

## 7. Video artifacts

Video capture records raw framebuffer bytes with explicit pixel format, dimensions, row stride, visible region, and frame number. SHA-256 is calculated over the exact byte sequence written to the artifact.

Comparisons require matching pixel format, dimensions, stride, and color interpretation. A mismatch in metadata is an incompatible capture, not a pixel mismatch. PPM may be generated as a human-viewable derivative, but the canonical comparison uses raw bytes.

## 8. Audio artifacts

Audio capture records the PCM emitted by the emulated chip/mixer before host device playback. The producer must state:
- signedness and endianness;
- sample width;
- sample rate;
- channel count and channel ordering;
- exact sample count and the capture interval.

Host output latency, resampling by the operating system, device volume, and DAC behavior are outside the PCM artifact. VGM command logs are useful diagnostic artifacts but are not PCM and must not be substituted for PCM capture.

If the current Rust runtime cannot expose the produced PCM without copying large buffers or changing timing, that is a capability gap for the follow-up native diagnostics work; this contract does not authorize moving synthesis to Python.

## 9. Comparison rules

The comparator emits a machine-readable report with:
- both manifest identifiers and emulator/media digests;
- compatibility decision and reasons;
- per-artifact expected/actual digest and byte length;
- exact equality result;
- first differing byte offset when applicable;
- video mismatch coordinates when dimensions and pixel format are compatible;
- audio mismatch sample/channel index when PCM metadata is compatible;
- first divergent CPU checkpoint and differing register fields.

Default policy is strict byte equality for identical capture configurations. No implicit tolerance, normalization, resampling, image filtering, or audio alignment is permitted. Any tolerance-based comparison must be a separately named metric with rationale, parameters, and raw strict-comparison result retained.

A missing artifact is NOT RUN or INCOMPLETE, not PASS. An incompatible format is INCOMPATIBLE, not PASS.

## 10. Two-run determinism protocol

For each scenario:
1. Start two fresh machine instances from the same declared initial configuration.
2. Use the same ROM/BIOS digests and exact input replay.
3. Use the same emulator commit, build profile, extension build, and machine configuration.
4. Run the same requested frame/event sequence.
5. Capture video, PCM, and CPU checkpoints independently.
6. Compare the two captures with strict equality.
7. Preserve both manifests and the comparison report.

A run must not reuse mutable machine state, output files, audio ring-buffer cursors, or prior execution counters from the other run. If the host runtime cannot guarantee independent instances, the limitation must be recorded.

## 11. Baseline approval and provenance

A baseline is approved only when:
- source revision is a full Git SHA and the checkout is clean;
- all required build/test gates for that revision are recorded;
- media identity and lawful provenance are documented;
- scenario and input replay are versioned;
- two independent runs are byte-identical;
- known limitations and unavailable observables are explicit;
- an independent reference is identified for any hardware-fidelity claim.

Baseline artifacts are immutable. A change to emulator code, scenario, replay, ROM/BIOS digest, output format, or relevant build configuration creates a new candidate; it does not overwrite the old baseline.

## 12. Status vocabulary

Use these statuses consistently:

- `IMPLEMENTED`: capability exists in the code.
- `QUICK_PASS`: focused tests pass.
- `SHORT_PASS`: bounded regression suite passes.
- `LONG_PASS`: extended suite passes.
- `INTEGRATION_PENDING`: implementation exists but integration evidence is incomplete.
- `TEMPORAL_PASS`: reserved for validated temporal behavior against suitable traces/oracles; never infer from frame-level determinism.

For capture results:
- `PASS`: all declared assertions for the run passed.
- `FAIL`: a declared assertion failed.
- `BLOCKED`: execution could not proceed due to an external prerequisite.
- `NOT RUN`: not executed.
- `INCOMPATIBLE`: inputs/artifact metadata do not permit a valid comparison.

## 13. Implementation sequence

This contract is documentation-only. Follow-up work is intentionally separated:

A. **Native diagnostics API:** stable, read-only snapshots and access to already-produced framebuffer/PCM buffers, with explicit ownership/lifetime and no per-instruction Python callbacks.

B. **Capture harness:** Python orchestration for scenario selection, replay, metadata, artifact hashing, and report generation. No hardware emulation logic.

C. **SG-1000 and SMS adapters:** use the same manifest/replay/comparison contract while preserving system-specific machine behavior.

D. **Scenario corpus:** begin with project-owned homebrew and legally redistributable test programs. Add external test ROMs only after license/provenance review.

E. **Golden baseline approval:** only after reproducibility, provenance, and independent-oracle criteria are met.

## 14. Acceptance criteria for PR A

- [ ] Contract defines manifest, replay, CPU checkpoint, video, audio, and comparison semantics.
- [ ] Determinism, baseline integrity, and hardware fidelity are explicitly distinct.
- [ ] No ROM/BIOS payload or copyrighted dump is introduced.
- [ ] No runtime, dependencies, or execution behavior are changed.
- [ ] Follow-up implementation work is separated into reviewable PRs.
- [ ] Documentation reviewed against the current SG-1000/SMS APIs before implementation begins.
