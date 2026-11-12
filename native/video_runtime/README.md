# Native video runtime

The crate owns immutable frame contracts and the Windows-native presentation backend.
The SMS core and machine scheduler do not depend on this crate's D3D12 module.

## Host feature boundary

`FrameMailbox`, `VideoFrame`, `PixelFormat`, and `FrameQueue` are available without enabling a native presentation API. The legacy Windows D3D12 host is opt-in through the `d3d12-host` Cargo feature, disabled by default. QPC diagnostics are independently gated by `windows_diagnostics`, allowing the Slint `sega-desktop` to use host telemetry without compiling the legacy D3D12 host. The legacy `sega-app` explicitly enables `d3d12-host`; the Slint desktop enables only `windows_diagnostics` and consumes the portable frame contract. The D3D12 smoke examples also require this feature.

## Windows x64 Direct3D 12 smoke test

On a Windows x64 machine with a D3D12-capable adapter and current graphics drivers:

```powershell
cargo run --release --locked -p sega-video-runtime --example d3d12_frame_smoke
```

The example creates a Rust-owned Win32 window, publishes a synthetic 256x192 RGB565 frame
through `FrameMailbox`, expands it to RGBA8888 in the D3D12 upload path, copies it to the
DXGI back buffer and presents it. Close the window to exit.

This is a host/transfer smoke test, not an SMS1 VDP validation or a GPU performance benchmark. Windows CI compiles this example but does not execute it because the runner is headless.
The example intentionally runs no emulation and does not involve Python, PyO3 or Qt.

## Frame delivery policy

The crate exposes two deliberately different delivery contracts:

- `FrameMailbox` is a single-slot, latest-frame mailbox. Publishing a newer frame may replace an older frame that has not yet been consumed. It is suitable only when presentation coalescing is explicitly allowed.
- `FrameQueue` is a bounded FIFO that preserves every accepted frame. A producer blocks when the queue is full and resumes when the presentation consumer removes a frame. This backpressure belongs at a completed-frame host boundary, never inside Scheduler advancement. It can reduce wall-clock throughput but does not alter logical emulated time.

The native runtime integration must choose the lossless queue unless an explicit product-level policy authorizes presentation-frame coalescing. Neither contract may skip machine execution or Scheduler events.

## Rust-owned D3D12 session

On Windows x64, the `D3d12Session` API creates the HWND and D3D12 host on a dedicated Rust worker thread. The worker owns the message pump, consumes the bounded lossless `FrameQueue`, uploads completed immutable `VideoFrame` values, and presents them. The emulation side submits only at a completed-frame boundary; it does not pass graphics objects, wait for GPU fences, or let VSync advance the Scheduler.

Run the session smoke test on a Windows x64 machine with a D3D12-capable adapter:

```powershell
cargo run --release --locked -p sega-video-runtime --example d3d12_session_smoke
```

The example presents one synthetic frame and keeps the worker alive until the native window is closed. It is a thread/lifecycle and transfer smoke test, not a real-game or VDP timing test. Window resize via DXGI stretch, minimize/restore queue preservation, per-monitor DPI notification handling, and one-shot recovery for DXGI_ERROR_DEVICE_REMOVED / DXGI_ERROR_DEVICE_RESET are implemented. Physical-GPU/mixed-DPI validation and machine-to-session wiring remain pending. If the window closes or presentation fails with queued frames, the session closes the queue and reports the number of pending frames canceled; it does not report them as presented.

## Minimize, restore and resize behavior

The D3D12 worker checks the native HWND minimized state before dequeuing. While minimized it continues pumping Win32 messages but retains all accepted frames in the bounded FIFO. If the producer continues, normal queue backpressure applies only at completed-frame submission; no accepted frame is replaced or skipped. Restoring the window resumes FIFO presentation in sequence order.

The swap-chain surface remains at the logical frame dimensions (for SMS, the VDP output size). The existing DXGI `DXGI_SCALING_STRETCH` policy scales that surface to the current client area during window resize; it does not resize the emulated framebuffer or alter Scheduler timing. This is not yet a DPI-awareness implementation or a swap-chain ResizeBuffers path. If shutdown is requested while still minimized, queued frames are explicitly counted as cancelled and the shutdown returns `PendingFramesCancelled`; they are not reported as presented.

## Per-monitor DPI behavior

The native HWND is created while the presentation worker temporarily uses `DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2`, falling back to Per-Monitor V1 if V2 is unavailable. The previous thread DPI context is restored immediately after HWND creation; no process-wide DPI mode is changed. The window procedure enables non-client scaling during `WM_NCCREATE` for the Per-Monitor V1 fallback (PMv2 already enables it automatically) and handles `WM_DPICHANGED` by applying Windows' suggested outer-window rectangle through `SetWindowPos(SWP_NOZORDER | SWP_NOACTIVATE)`. The swap-chain's logical frame dimensions remain unchanged and DXGI stretch scaling continues to fit the client area. Mixed-DPI monitor movement and scale changes still require runtime validation on Windows hardware.

## Responsive waits and device-loss reporting

The D3D12 frame-latency and fence waits use finite 16 ms polling rather than an infinite wait. While waiting, the presentation thread pumps Win32 messages and queries `ID3D12Device::GetDeviceRemovedReason`, preserving the driver's original HRESULT and preventing an indefinitely blocked worker. For `DXGI_ERROR_DEVICE_REMOVED` and `DXGI_ERROR_DEVICE_RESET` only, the session drops the old host/HWND, recreates the D3D12 device and swap chain, and retries the same interrupted completed frame once. `DEVICE_HUNG` and `DRIVER_INTERNAL_ERROR` remain explicit non-recoverable failures. If recreation or retry fails, the session returns `DeviceRecoveryFailed`; frames that cannot be presented are counted as canceled. The session exposes `device_recovery_count()`. Recreating the host also recreates the HWND, so Windows may reposition the window; preserving exact placement remains pending. No Scheduler state is changed or replayed during this recovery path.

## Device removed/reset recovery

The presentation worker attempts one host recreation for each failed frame only when the original reason is `DXGI_ERROR_DEVICE_REMOVED` or `DXGI_ERROR_DEVICE_RESET`. It destroys the old host and HWND first, reuses the registered Win32 class, creates a new HWND/device/swap chain, and retries the exact same immutable `VideoFrame` once. The worker's message pump determines closure from the current HWND rather than posting thread-wide `WM_QUIT` from `WM_DESTROY`, allowing the old HWND to be replaced on the same thread without leaving a stale quit message. Other device-loss reasons are surfaced without automatic retry. If reinitialization or the retry fails, `DeviceRecoveryFailed` preserves the initial HRESULT and reports the recovery error; all non-presentable accepted frames are counted as canceled. The logical machine/Scheduler state is not advanced, rewound, or replayed during recovery.

## Merged implementation status — 2026-11-05

The following implementation slices are merged into `main`: lossless bounded `FrameQueue` (#546), Rust-owned `D3d12Session` (#547), minimize/restore preservation (#550), per-monitor DPI handling (#553), responsive device-loss waits (#555), one-shot removed/reset recovery (#558), and independent SMS1 VDP port-contract tests (#560). The detailed merge/CI record is [the 2026-11-05 integration audit](../../docs/audits/video-runtime-d3d12-session-merge-audit-2026-11-05.md).

Automated CI passed for the merged PR heads. Physical GPU execution, forced device-loss/recovery, mixed-DPI monitor movement, and SMS machine-to-session wiring remain `NOT RUN` / pending. In particular, these merges do not imply that `SmsMachine → MachineScheduler → Sms1Vdp → VideoFrame → D3d12Session` is integrated; the SMS1 temporal gate remains blocked by the T0 evidence gaps documented in the audit.


## Windows API bindings

The D3D12/DXGI host uses the official `windows-rs` bindings from crate `windows = 0.62.2`, pinned exactly in this crate's manifest. The same binding release is used by the XAudio2 host in `sega-audio-output`; there are no separate XAudio2 or D3D12 Rust crates to upgrade independently. The 0.62.2 release is the latest stable `windows` crate version verified on 2026-11-07.
