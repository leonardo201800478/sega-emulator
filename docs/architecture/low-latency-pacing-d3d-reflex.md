# Host frame pacing, native graphics APIs and presentation synchronization

**Status:** Incremental implementation; Reflex SDK integration remains a separate gated workstream.  
**Scope:** Windows Qt/PySide6 frontend and SG-1000 presentation.  
**Base:** `c1107c88929aee9114fcc1b4450996f9f15bedea`.

## Goals

- Keep emulated machine time independent from host monitor refresh and GPU presentation.
- Reduce host-side frame pacing drift without advancing hardware time from wall-clock APIs.
- Offer Qt Quick's native Vulkan, Direct3D 11 and Direct3D 12 render paths, with an explicit Qt raster fallback.
- Evaluate NVIDIA Reflex through official integration points without claiming support when the SDK or runtime capability is unavailable.
- Preserve deterministic Rust hardware state and keep OS/GPU APIs out of `native/sg1000_core`.

## Timing domains

1. **Emulated time:** Z80 T-states and VDP/PSG advancement remain authoritative and deterministic. A nominal NTSC SG-1000 frame is approximately 59.92 Hz using the current 3,579,545 Hz clock and 228 × 262 nominal T-states.
2. **Host emulation pacing:** the Qt host schedules work around the nominal frame period. The pacer now maintains an absolute cadence; when a callback is late it skips missed host slots rather than executing catch-up bursts.
3. **Presentation:** Qt Quick and the display driver control when the latest completed framebuffer is presented. A 59.92 Hz emulation stream can therefore be displayed on a 144 Hz monitor with repeated presentation of the latest frame; the monitor refresh must never alter emulated T-states.

This is not a claim of cycle-accurate CPU/VDP/PSG synchronization. It also does not guarantee a specific click-to-photon latency; that requires measurements on a real Windows/GPU/display configuration.

## Native graphics API selection

The Qt Quick scene graph must select its graphics API before the first `QQuickWindow` is created. The driver preference is therefore applied at application startup; changing it in the Drivers dialog persists the choice and requires an application restart. Supported choices are Vulkan, Direct3D 11, Direct3D 12, and Qt raster fallback. Vulkan remains the default. The Video menu offers VSync, VRR, G-SYNC and Off. VSync/Off request swap intervals of 1/0; VRR and G-SYNC are system-managed states and selecting them does not enable them in Windows or the NVIDIA Control Panel. Changes require restart and need Windows validation for each backend. The effective API is verified and logged after scene-graph initialization; initialization failure falls back to raster.

This intentionally uses Qt's existing native rendering backend rather than adding a second Rust swapchain behind a Qt Quick surface. The Rust hardware core remains independent of the graphics API. Qt's renderer abstraction is already part of the current frontend; bypassing it would require a separate native-window/swapchain integration and ownership/synchronization contract.

Direct3D support alone does **not** provide a third-party shader injection system. A post-processing/shader pipeline needs its own design: shader format and compilation, pass ordering, texture lifetime, resize handling, frame synchronization, user-supplied asset validation and license/security policy. Do not promise compatibility with arbitrary ReShade/game shaders from the API selector alone.

## NVIDIA Reflex feasibility

- NVIDIA's public Reflex SDK documentation describes integration through the SDK/Streamline, with NVAPI and a C++ development environment. A Rust application could call a small native adapter over FFI, but this is an optional Windows/NVIDIA host feature, not part of the emulated hardware core.
- Reflex Low Latency integration needs correct frame/input/simulation/render markers and must be validated with actual latency measurements. It is not equivalent to setting a driver flag globally.
- NVIDIA's public developer page currently says to sign up for notification when Reflex Frame Warp becomes available for general integration. Reflex 2 Frame Warp is therefore **not claimed as generally integrable** by this change. Do not expose a Reflex 2 toggle until an applicable public SDK/runtime is available and tested.
- Until the official SDK and its redistribution/licensing requirements are confirmed for CI and release packaging, Reflex remains an explicitly unsupported/NOT RUN capability. The emulator must function normally without NVIDIA hardware or SDK files.

Official reference: [NVIDIA Reflex SDK](https://developer.nvidia.com/performance-rendering-tools/reflex).

## Acceptance criteria

- FramePacer tests prove immediate first frame, fixed cadence under late callbacks, no catch-up bursts, reset behavior and invalid-period rejection.
- Qt driver dialog lists only implemented choices and states that graphics API changes require restart.
- The requested API is set before creating the first Qt Quick window and the effective API is logged/checked.
- Tests cover the mapping of persisted API names to Qt's API enums and the existing raster fallback.
- Python formatting/lint/tests and relevant Qt offscreen tests pass on the exact PR SHA.
- Windows manual validation records the effective API, monitor refresh/VRR state, emulation FPS, frame-time percentiles and latency measurement method.
- Reflex is reported as BLOCKED/NOT RUN until SDK access, runtime availability and a reproducible latency test exist. No latency reduction is claimed without before/after evidence.

## Not included in this slice

- A cycle-exact Z80/VDP/PSG scheduler or changes to nominal hardware clocks.
- Reflex 2 Frame Warp integration.
- A custom Rust-owned Direct3D/Vulkan swapchain.
- Arbitrary third-party shader/ReShade compatibility.
- Guaranteed click-to-photon latency targets.

## Validation status

The code changes in this PR have not been executed in a local Windows environment by the authoring agent. GitHub Actions results on the PR candidate SHA are the required validation evidence; skipped or missing checks are not PASS.
