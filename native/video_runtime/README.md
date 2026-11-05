# Native video runtime

The crate owns immutable frame contracts and the Windows-native presentation backend.
The SMS core and machine scheduler do not depend on this crate's D3D12 module.

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
