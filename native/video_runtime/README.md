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

This is a host/transfer smoke test, not an SMS1 VDP validation or a GPU performance benchmark.
The example intentionally runs no emulation and does not involve Python, PyO3 or Qt.
