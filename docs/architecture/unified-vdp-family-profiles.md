# Unified VDP family profiles — SG-1000, SMS and Game Gear

**Status:** proposed architecture, implementation not yet migrated  
**Branch:** `architecture/unified-vdp-family-profiles-20261114`  
**Base:** `76795360896bfc8651ecf86f0a85ce7c18597875`

## Context

The repository currently has two active video implementations with different hardware lineages:

- `native/sg1000_core/src/vdp.rs`: `Tms9918A`, a TMS9918A-compatible device with legacy display modes, 16 KiB VRAM, 256x192 output and the TMS sprite/status model.
- `native/video_core/src/sms/vdp.rs`: `SmsVdp`, the SMS 315-5124/315-5246 family implementation, including Mode 4, 16 KiB VRAM, 32-byte SMS CRAM and raster state.
- `native/gg_core` is currently only a scaffold; it has no VDP implementation.

The SMS core already re-exports the shared SMS VDP from `sega-video`. The SG-1000 VDP remains implemented in its machine crate. This ADR defines how to converge the device architecture without cloning VDP source files or replacing validated behavior prematurely.

## Decision

Use one video-device API and explicit hardware profiles, but retain distinct rendering engines where the silicon behavior is materially different.

The public model is a profile-selected device, not one oversized renderer with scattered console checks:

```rust
enum VideoHardwareProfile {
    Sg1000(Tms9918Profile),
    Sms(SmsVdpProfile),
    GameGear(GameGearVdpProfile),
}

enum VideoDevice {
    Tms9918A(Tms9918A),
    Mode4(Mode4Vdp),
}
```

Names are illustrative until the API is implemented. A `VideoDevice` exposes coarse-grained operations needed by machine integration (ports, `advance(tstates)`, interrupt/status queries and completed-frame access). The device remains backend hardware; the GUI and presentation host receive completed frames only.

### Engine ownership

1. **TMS9918 engine:** SG-1000 native legacy modes (Text, Graphics I, Graphics II and Multicolor), TMS status/interrupt behavior, 32 SAT entries and four-sprites-per-line behavior. The SMS legacy-mode compatibility path may reuse this engine only after a behavior-by-behavior comparison proves equivalence.
2. **Mode 4 engine:** SMS1, SMS2 and Game Gear Mode 4 rendering and raster behavior. SMS1/SMS2 remain profiles of the same engine. Game Gear reuses Mode 4 tile/sprite logic but supplies its own color-RAM semantics and LCD viewport/output profile.
3. **Shared device contract:** common lifecycle, timing position, frame publication, port access abstraction and diagnostics where semantics genuinely match. Do not force TMS and Mode 4 to share incompatible register, status, memory or sprite semantics.

This is a single VDP subsystem with multiple hardware engines, not three copied VDP implementations.

## Hardware profiles

### SG-1000 — TMS9918A-compatible profile

- Native TMS9918A-compatible register and two-port protocol.
- 16 KiB VRAM in the current model.
- 256x192 active image.
- Legacy TMS display modes; do not route SG-1000 through SMS Mode 4.
- 32 sprite attribute entries, with the TMS four-sprites-per-scanline limit and TMS overflow/collision semantics.
- TMS fixed palette and TMS-specific name/color/pattern table addressing.
- Preserve current VDP read-ahead buffer, control latch, status clearing and interrupt behavior.
- Keep regional timing as a separate profile dimension; do not infer PAL/NTSC solely from console identity.

The current `Tms9918A` implementation is the behavioral baseline. Migration must preserve its public machine contract and tests before the old implementation is removed.

### SMS1 / SMS2 — Mode 4 profiles

- Continue using `SmsVdpProfile` and `SmsVdpRevision`.
- SMS1 (315-5124) and SMS2 (315-5246) are profiles, not duplicate files.
- Preserve revision-specific SAT addressing, sprite quirks and supported height modes.
- Keep the current 6-bit SMS CRAM representation and SMS output dimensions.
- The in-progress sprite temporal work remains specific to the Mode 4 engine; do not declare it complete merely because profiles exist.

### Game Gear — Mode 4 + handheld output profile

- Reuse the Mode 4 tile, scroll and sprite engine; do not copy `sms/vdp.rs`.
- Use a Game Gear color-RAM implementation: 32 entries, 12 bits per color, two bytes per entry (64 bytes total), rather than SMS's 32-byte 6-bit CRAM.
- Keep the VDP's internal raster coordinate space separate from the LCD viewport.
- Internal rendering space: 256x192; visible LCD aperture: 160x144, centered (48 pixels cropped horizontally and 24 vertically) as the initial documented profile.
- Cropping is an output aperture, not a reduction in VDP work: pixels, sprites and tile fetches outside the LCD aperture still participate in hardware evaluation and status effects.
- The Game Gear profile must define the exact CRAM byte-write/latch rules, LCD aperture mapping, palette conversion and any handheld-specific I/O/status differences. These must be verified against the official Game Gear hardware manual before implementation.
- Do not model Game Gear as merely an SMS framebuffer resized to 160x144.

## Proposed module organization

Keep one crate as the owner of VDP hardware:

```text
native/video_core/src/
  lib.rs
  device.rs             # VideoDevice facade / common contract
  profile.rs            # top-level hardware profile selection
  tms9918/              # one TMS engine, not one copy per console
    mod.rs
    profile.rs
    vdp.rs
  mode4/                # one Mode 4 engine for SMS1/SMS2/Game Gear
    mod.rs
    profile.rs
    vdp.rs
    color.rs             # profile-specific CRAM/palette semantics
    output.rs            # viewport/output aperture, including Game Gear
```

This is a target layout, not a mandate to move files in one large refactor. Existing paths remain until staged migration and compatibility tests pass. Avoid a second `vdp.rs` for Game Gear or SG-1000. Do not create thin duplicate modules merely to mirror console names.

## Migration sequence

### VDP-0 — characterization (no behavior change)
- Inventory SG-1000 and SMS public APIs, tests, timing assumptions and machine call sites.
- Record all current TMS9918A and Mode 4 behavior in profile-specific test fixtures.
- Identify shared TMS routines in SMS legacy mode and SG-1000; only consolidate routines with matching semantics.
- Establish Game Gear CRAM and aperture test vectors from the official manual and independent test ROMs.

### VDP-1 — common device contract
- Add the common profile/device contract in `sega-video`.
- Keep both existing engines intact behind adapters.
- Add construction tests for SG-1000, SMS1, SMS2 and Game Gear profiles.
- No machine scheduler, CPU, PSG, GUI or frame-pacing changes.

### VDP-2 — SG-1000 engine ownership migration
- Move/adapt the existing TMS9918A engine into `sega-video` without changing its semantics.
- Make `sg1000_core` consume/re-export the shared device.
- Prove equivalence with the existing SG-1000 VDP tests, frame hashes and timing/chunking tests before deleting the old source.
- Keep any machine-specific wrapper limited to wiring and compatibility API.

### VDP-3 — Game Gear profile
- Add Game Gear CRAM and LCD-aperture behavior to the Mode 4 engine through profile-owned strategies.
- Preserve internal 256x192 raster coordinates and hardware side effects outside the visible 160x144 aperture.
- Validate CRAM low/high byte sequencing, auto-increment, color conversion, aperture edges, sprite clipping and status effects.
- Add Game Gear ROM/test coverage before claiming compatibility.

### VDP-4 — convergence and regression
- Route SG-1000, SMS1, SMS2 and Game Gear through the same `VideoDevice` construction boundary.
- Remove only code proven redundant.
- Run full workspace CI and all system-specific ROM/debugger suites.
- Verify deterministic equivalence under different `advance()` chunk sizes.

## Non-goals and invariants

- No hardware logic in Slint, WGPU presentation or host audio.
- No per-instruction GUI callbacks.
- No replacement of the MachineScheduler as temporal authority.
- No change to Z80, PSG, input, frame pacing or runtime orchestration as part of this VDP modeling task.
- No blanket pixel/dot pipeline without evidence. Use the smallest temporal granularity that reproduces observable hardware behavior.
- No deletion of the current SG-1000 VDP until equivalence is demonstrated.
- No claim of full Game Gear or SG-1000 compatibility from profile construction alone.

## Validation gates

- TMS9918A: all existing SG-1000 tests remain green; test all four legacy modes, table masking, sprite evaluation/overflow/collision, read buffer, status and VBlank/IRQ timing.
- Mode 4 SMS: preserve SMS1/SMS2 regression coverage and the active sprite-evaluation work.
- Game Gear: verify 32x12-bit CRAM (64-byte address space), byte-pair write semantics, palette conversion, 160x144 aperture mapping and side effects outside the aperture.
- All profiles: reset/clone/state equality, frame-ready behavior, raster position and chunking invariance.
- Hardware claims require a primary reference or independent implementation/test ROM; unverified fields remain explicitly unvalidated.

## References

- Sega Game Gear Hardware Reference Manual (Rev. 1): https://segaretro.org/images/1/16/Sega_Game_Gear_Hardware_Reference_Manual.pdf
- SMS Power — Game Gear Official Docs: https://www.smspower.org/Development/GGOfficialDocs
- SMS Power — VDP modes and legacy TMS modes: https://www.smspower.org/Development/Modes
- TI TMS9918A data manual: https://computers.baffa.tec.br/pages/datasheet/TMS9918A_TMS9928A_TMS9929A_Video_Display_Processors_Data_Manual_Nov82.pdf
- TMS9918A behavior reference (Sean Young): https://github.com/cbmeeks/TMS9918/blob/master/tms9918a.txt
- Existing repository implementations: `native/sg1000_core/src/vdp.rs`, `native/video_core/src/sms/vdp.rs`, `native/video_core/src/sms/profile.rs`.
