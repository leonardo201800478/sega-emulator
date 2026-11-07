# ADR 0001 — Native Vulkan presentation backend

- Status: Accepted for implementation
- Date: 2026-11-06
- Scope: native Rust video presentation; no emulated hardware or Scheduler changes

## Context

The current Windows shell presents frames through a dedicated D3D12 child HWND and a D3D12 mailbox session. Physical validation exposed independent defects in producer sequence continuity, viewport sizing, resize and fullscreen lifecycle. The frontend must also support modern post-processing shaders, optional HDR output and adaptive presentation without coupling those features to console hardware timing.

The current presentation implementation is too tightly coupled to D3D12 for a second backend to be added safely by duplicating the application shell.

## Decision

1. Implement the primary renderer with the native Vulkan API through `ash`; do not use `wgpu` as the runtime graphics API.
2. Use Vulkan as the default backend when a compatible Vulkan loader, surface, physical device, graphics/present queue and swapchain can be initialized.
3. Retain the existing D3D12 implementation as an explicit fallback backend during migration and for systems where Vulkan initialization fails.
4. Put backend selection and lifecycle behind a backend-neutral presentation interface. The SMS worker publishes immutable `VideoFrame` objects through the existing mailbox contract and must not know which graphics API consumes them.
5. Use SPIR-V shader modules for Vulkan. WGSL may be considered as an authoring format only if a deterministic Naga-to-SPIR-V build path is validated; it must not introduce wgpu into the runtime presentation path.
6. HDR is optional and disabled by default. SDR remains the baseline output. HDR10 output requires explicit user opt-in, supported surface formats/color spaces, verified display capability and correct metadata. Unsupported HDR modes must fall back to SDR without failing video initialization.
7. VRR is capability-driven and optional. Presentation pacing must never change emulated Master Clock, Scheduler event order, CPU progress or audio/video synchronization.

## Native Vulkan implementation

- Bindings: `ash` 0.38.x; dynamic loader via `Entry::load` so the executable can report a missing Vulkan loader and fall back to D3D12.
- Window integration: `ash-window` 0.13.x with `raw-window-handle` 0.6, matching the app's current handle version.
- Required baseline: Vulkan instance, Win32 surface, physical-device and queue-family selection, logical device, `VK_KHR_swapchain`, swapchain lifecycle, image acquisition, command submission, presentation and orderly teardown.
- The Vulkan backend must own its own native surface/swapchain lifecycle. A native HWND cannot be simultaneously attached to active Vulkan and D3D12 swapchains; fallback must retire and destroy the failed backend before creating the alternate backend for that window.
- Device loss, surface loss, resize/out-of-date and minimized/zero-size windows must be handled as explicit states, not panics.
- Unsafe Vulkan calls must be isolated inside the native backend module with documented invariants and narrow safe wrappers. No unsafe code is permitted in the emulator core/runtime.

## HDR policy

- SDR is the default and reference path.
- HDR10 is optional and requires a compatible presentation color space (for example, BT.2020/PQ), a defined linear-light processing pipeline and explicit metadata handling.
- `VK_EXT_hdr_metadata` supplies static SMPTE ST 2086 and CTA-861.3 metadata. It does not itself provide HDR10+ dynamic metadata.
- HDR10+ dynamic metadata is not a release requirement until an interoperable OS/driver/display path is identified and tested end-to-end. Do not label static HDR10 as HDR10+.
- Any HDR shader/tone-map path must be validated with test patterns and measured output, not screenshots alone.

## Aspect-ratio policy

The viewport offers 1:1, 4:3 and 16:9 display modes. The default for SMS is 4:3. The selected viewport is fitted uniformly inside the available client area; it must never be stretched to fill an incompatible client rectangle. The GPU scales the source frame to the selected presentation rectangle. Integer scaling may be added separately and must not alter the selected aspect ratio.

## VRR and pacing

- Query actual surface/present capabilities and use only supported presentation modes.
- FIFO is the safe fallback. Mailbox/immediate or tearing-capable modes are opt-in only when supported and appropriate.
- Measure presentation timing independently from emulated frame count.
- Never skip machine execution or change Scheduler timing to match monitor refresh.
- Do not claim VRR support merely because Vulkan is active; verify the display, driver, compositor, surface mode and observed presentation behavior.

## Migration sequence

1. Stabilize the current mailbox contract and viewport geometry.
2. Introduce a backend-neutral presentation interface without changing the SMS worker contract.
3. Implement Vulkan instance/surface/device/swapchain initialization and a diagnostic clear/present path.
4. Implement frame upload and presentation with explicit synchronization and resource lifetime.
5. Make Vulkan the default when initialization succeeds; retain D3D12 fallback.
6. Add shader pipeline and SDR post-processing.
7. Add optional HDR10 output and metadata; keep it disabled by default.
8. Validate resize, minimize/restore, fullscreen, ROM replacement, device/surface loss and shutdown on Windows hardware.
9. Evaluate VRR and HDR10+ separately; unsupported capabilities remain unavailable rather than emulated or advertised.

## Acceptance gates

- Rust fmt, Clippy with warnings denied, workspace tests and Windows x64 Release build pass.
- Vulkan validation layers report no synchronization, lifetime or swapchain errors in development tests.
- D3D12 fallback is exercised with Vulkan loader unavailable and with Vulkan device/surface initialization failure.
- Repeated ROM replacement preserves monotonically increasing mailbox sequences and never requires restarting the application.
- 1:1, 4:3 and 16:9 maintain their selected geometry in windowed, maximized and fullscreen states.
- HDR remains off by default; SDR output is unchanged when HDR is disabled.
- Presentation tests prove that rendering/present pacing cannot mutate machine time.
- No merge until CI is green for the exact final HEAD and Windows physical validation is recorded.
