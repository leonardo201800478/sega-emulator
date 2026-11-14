# Unified VDP family migration

**Status:** migration in progress — architecture baseline
**Priority:** SMS1 → SMS2 → SG-1000 → Game Gear
**Target crate:** `native/video_core` (`sega-video`)
**Migration branch:** `feat/unified-vdp-migration-sms1-first-20261114`

## 1. Objective

Converge the console video devices behind one shared VDP subsystem without flattening distinct ASIC behavior into one renderer and without replacing validated implementations before equivalence is demonstrated.

The migration covers four system profiles in this strict implementation order:

1. **SMS1 — Sega 315-5124**: first target and compatibility baseline for the Mode 4 engine.
2. **SMS2 — Sega 315-5246**: same Mode 4 engine, with an explicit ASIC profile for revision-specific behavior.
3. **SG-1000 — TMS9918A-compatible**: migrate the existing TMS engine into the shared crate only after SMS1/SMS2 foundations are stable.
4. **Game Gear**: last target; reuse Mode 4 logic but provide dedicated handheld color-RAM and LCD-output behavior.

The order is a delivery priority, not permission to approximate one ASIC with another. Each profile retains its own documented capabilities and unknowns.

## 2. Architecture decision

One shared crate owns the VDP subsystem and its public device boundary. It contains two specialized hardware engines:

- **TMS9918A engine** — native SG-1000 legacy modes and TMS-specific status, sprite, memory and register semantics.
- **Mode 4 engine** — SMS1, SMS2 and Game Gear tile/sprite/raster behavior, with profile-specific strategies for ASIC and output differences.

The common boundary is an integration contract, not a common implementation of incompatible registers or memory semantics. It will expose coarse-grained device operations needed by machine integration (port access, advancement to a machine-provided boundary, interrupt/status queries, reset/state and completed-frame access). The GUI and presentation host receive completed frames only.

Target ownership:

```text
native/video_core/src/
  lib.rs
  device.rs             # common VDP device contract / facade
  profile.rs            # four system profiles and engine selection
  tms9918/              # one TMS9918A engine
  mode4/                # one Mode 4 engine for SMS1/SMS2/GG
    profile.rs          # ASIC/system-specific behavior
    color.rs            # profile-owned CRAM and palette semantics
    output.rs           # raster-to-output aperture mapping
```

This is a target organization, not a mandate for a bulk file move. Existing modules remain in place until their replacement is proven equivalent. Avoid console-specific copies of `vdp.rs`.

## 3. Hybrid raster model — preserve and evolve existing work

Continue the hybrid Scanline + Dot/Pixel model already being developed in the recent raster/sprite work. Do not introduce a competing renderer or a second timing architecture.

- **Scanline remains the predominant execution/composition unit.** Normal advancement and rendering should operate in bounded line spans, retaining the current scanline-oriented path where no observable mid-line distinction is required.
- **Dot/Pixel precision is selective.** Use finer raster segmentation only for hardware-observable events whose position within a line changes output or status (for example, a register/VRAM/CRAM write or a fetch/evaluation boundary when the target ASIC behavior is established).
- At a sub-line event, finish/commit the raster span preceding the event using the old state, then apply the event and continue the remaining span with the new state. Do not retroactively apply a later write to pixels that precede it.
- Sprite evaluation, sprite-number buffering, sprite attribute/pattern fetch and pixel composition are distinct phases when evidence requires that separation. The Mode 4 sprite work in PR #814 is a structural first step; it does not yet prove the temporal buffer or exact evaluation boundary.
- The VDP consumes elapsed time and event boundaries from its owning machine. It does not create an independent master clock, schedule host deadlines, or use GUI frame pacing as hardware time.
- Hardware-specific latches, fetch rules and status timing remain profile/engine responsibilities. Do not assume one revision's phase applies to another.

The current mainline audit in PR #813 documented the present gap: the legacy SMS path composes a whole scanline at line completion and applies VRAM/CRAM writes immediately. The migration must close this gap incrementally with tests and evidence; this document does not claim that dot-level temporal behavior is already complete.

## 4. System profiles and responsibilities

### 4.1 SMS1 — first implementation target

- ASIC identity: Sega 315-5124.
- Mode 4 engine.
- Preserve the SMS1-specific SAT address masking and documented sprite behavior.
- Preserve the existing 16 KiB VRAM and 32-byte, 6-bit SMS CRAM model.
- Start from the SMS1 profile and tests already present in `native/video_core/src/sms/profile.rs`.
- Do not import SMS2 extra-height modes or Game Gear palette/output behavior into SMS1.
- Complete raster visibility, sprite evaluation/fetch phases and status timing only against evidence-backed boundaries.

### 4.2 SMS2 — second target

- ASIC identity: Sega 315-5246.
- Reuse the same Mode 4 engine; select behavior through an explicit profile.
- Keep revision-specific capabilities separate (including supported active-height modes and any proven SAT/scroll/status differences).
- Do not fork a second renderer or duplicate SMS VDP source.
- SMS2 is not considered complete merely because the profile can be constructed.

### 4.3 SG-1000 — third target

- TMS9918A-compatible engine; do not route native SG-1000 through Mode 4.
- Integrate the existing native Rust implementation from `native/sg1000_core/src/vdp.rs` into `sega-video`, adapting it to the project's current universal VDP device/profile interface.
- The SG-1000 machine must consume the shared Rust device through that universal interface; do not create a parallel SG-1000 VDP API or preserve a separate active implementation indefinitely.
- Preserve Text, Graphics I, Graphics II and Multicolor modes, 16 KiB VRAM, 32-entry SAT, four-sprites-per-line behavior, TMS status/interrupt and read-ahead/control-latch behavior.
- Python is not part of the SG-1000 runtime, integration, dependencies, or implementation path. Historical Python material, if consulted at all, is documentation/reference material only; it is not code to port or embed.
- Remove the old Rust source only after behavioral, frame-hash, timing/chunking and machine-level equivalence gates pass.

### 4.4 Game Gear — final target

- Reuse the Mode 4 engine; do not copy the SMS renderer.
- Add a dedicated Game Gear profile and color-RAM strategy: 32 entries, 12-bit color, two-byte entry semantics (64-byte address space).
- Keep internal raster coordinates distinct from the visible LCD aperture.
- Initial output target: internal 256×192 raster and visible 160×144 LCD aperture. The proposed centered crop (48 pixels horizontally and 24 vertically) remains a profile mapping to be verified, not a reason to clip hardware evaluation.
- Pixels, sprites and status effects outside the visible LCD aperture must still participate in the internal hardware evaluation where the hardware requires it.
- Verify byte ordering/latching, auto-increment, palette conversion, viewport edges and handheld-specific I/O/status behavior before declaring compatibility.

## 5. Migration phases

### VDP-M0 — characterize current implementations

- Inventory the active SMS1/SMS2 Mode 4 engine, native Rust SG-1000 TMS implementation, universal VDP interface, public APIs, machine call sites, tests and timing assumptions.
- Do not introduce Python into the SG-1000 runtime or dependency graph.
- Preserve existing behavior as profile-specific regression fixtures.
- Record which current SMS legacy paths are genuinely equivalent to TMS behavior; share only proven-equivalent routines.
- Identify all existing scanline/dot timing work in recent PRs and carry it forward rather than restarting it.
- No hardware behavior changes in this phase.

### VDP-M1 — consolidate Mode 4 for SMS1

- Treat the existing `sega-video` SMS VDP as the sole Mode 4 implementation.
- Complete the SMS1 profile as the first supported target.
- Continue the independent sprite-evaluation work in PR #814; retain its tests and temporal limitations explicitly.
- Introduce only the smallest shared device/profile boundary required by machine integration.
- Keep the active runtime adapter until the new boundary demonstrates equivalence.

### VDP-M2 — add SMS2 profile

- Add/verify SMS2-specific behavior in the same Mode 4 engine.
- Add differential profile tests for every capability that differs from SMS1.
- No copied renderer and no assumptions from unverified overscan/raster behavior.

### VDP-M3 — migrate TMS9918A ownership

- Move/adapt the existing SG-1000 engine into `sega-video`.
- Keep a compatibility wrapper in `sg1000_core` if required by current call sites.
- Prove equivalence before removing the original implementation.

### VDP-M4 — Game Gear profile

- Add Game Gear CRAM and LCD output aperture to Mode 4 through profile-owned strategies.
- Preserve internal raster and hardware side effects outside the LCD aperture.
- Add independent Game Gear tests/ROM coverage before claiming compatibility.

### VDP-M5 — converge machine construction and regression

- Route SMS1, SMS2, SG-1000 and Game Gear through one profile-selected VDP construction boundary.
- Remove only code proven redundant.
- Run full workspace and system-specific validation.
- Verify deterministic equivalence across different advancement chunk sizes and save/restore points.

## 6. Temporal and architectural invariants

1. `MachineScheduler` remains the sole authority for master time and causal event ordering.
2. VDP advancement is driven by machine-provided elapsed time/boundaries; it must not own an independent clock.
3. No instruction-level callbacks from the GUI. Machine/CPU/bus integration remains backend Rust.
4. A bus write must be applied at its scheduler boundary. The raster span before it observes old state; the span after it observes new state, subject to the target ASIC's proven latches.
5. Scanline composition remains the default. Dot/Pixel granularity is introduced only where hardware-observable behavior requires it.
6. Presentation, WGPU upload, VSync, CPAL and host pacing cannot alter emulated raster time.
7. Save states must include every in-flight raster span, pending boundary, latch, sprite evaluation buffer and fetch phase required for deterministic continuation.
8. No changes to Z80, PSG, MachineScheduler semantics or desktop presentation as incidental work in this VDP migration.
9. No compatibility claim based only on successful compilation, profile construction or matching screenshots.

## 7. Validation gates

### Shared invariants

- Reset/clone/state equality and save/restore at mid-line and event boundaries.
- Advancement chunking invariance: one large advance and equivalent smaller advances produce identical state, status and frame output.
- Deterministic event ordering and no retroactive pixel changes.
- Frame-ready and raster-position behavior preserved.

### SMS1 / SMS2

- Separate profile test matrices for ASIC-specific register, SAT, sprite, scroll, status and height behavior.
- Sprite SAT order, evaluation boundary, eight-entry buffer, overflow index, X/tile/pattern fetch and collision timing validated independently.
- VRAM/CRAM/pattern writes at selected sub-line boundaries produce expected before/after pixel spans.
- R8/R9 latches, H/V counters, VBlank and HINT/VINT tested with traces and independent vectors.
- Diagnostic ROM execution is recorded as PASS/FAIL/NOT RUN; building a ROM is not execution.

### SG-1000

- All existing native Rust TMS tests remain green through the universal VDP interface.
- No Python runtime, dependency, adapter or implementation is introduced.
- All four TMS modes, table masks, sprite evaluation/overflow/collision, read buffer, status and VBlank/IRQ timing preserved.
- Frame hashes and chunking tests match the pre-migration baseline.

### Game Gear

- 32 × 12-bit CRAM entries, byte-pair semantics, palette conversion and auto-increment.
- 160×144 output aperture mapping and clipping.
- Sprite evaluation and status side effects outside the LCD aperture.
- Handheld-specific I/O/status differences covered by independent references.

### Acceptance discipline

Every PASS must refer to the exact candidate SHA and identify the command/workflow and scope. Missing evidence is NOT RUN or BLOCKED, never inferred PASS. Full temporal accuracy is not declared until the target hardware profile's relevant raster, bus and interrupt traces pass an independent oracle.

## 8. Source lineage and PR handling

- PR #813 is merged and supplies the current SMS raster-fidelity gap analysis and initial fine-scroll correction.
- PR #814 remains an independent Mode 4 sprite-evaluation increment. Keep it separate until its focused code and tests are validated; then consume its merged result in this migration rather than duplicating the patch.
- PR #815 was closed without merge. Its useful architectural decisions are incorporated here: one shared VDP subsystem, two specialized engines, explicit system profiles, no duplicated console VDPs, staged migration, Game Gear CRAM/LCD separation and equivalence-before-removal.
- Historical Scheduler/VDP/D3D12 consolidation documents describe earlier integration states and must not override the current SMS runtime and recent PR evidence.

## 9. Explicit non-goals

- Rewriting the Z80, PSG, MachineScheduler or desktop presentation.
- Replacing scanline rendering wholesale with a full dot pipeline.
- Moving all VDP files in one bulk refactor.
- Treating SG-1000 legacy modes as Mode 4.
- Treating Game Gear as a cropped/resized SMS framebuffer without independent CRAM and aperture semantics.
- Removing existing VDP implementations before equivalence is demonstrated.
