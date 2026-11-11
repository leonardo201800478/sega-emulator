# Sega Desktop — R2 SMS / Slint / WGPU 30 / Vulkan

The desktop host now connects the Rust SMS runtime to the Slint/WGPU 30 presentation path. This is an incremental R2 integration, not the complete launcher.

## Runtime contract

- `SmsTemporalWorker` owns the SMS machine and its execution thread.
- `MachineScheduler` remains the sole authority for emulated time.
- The host sends only coarse-grained start/pause commands.
- Completed immutable `VideoFrame` values are published through `FrameMailbox`.
- The UI timer consumes only the newest pending frame; it never advances emulation.
- RGB888 is converted to tightly packed RGBA8 only at the presentation boundary.
- Slint/WGPU uses the renderer's own Vulkan device and queue.
- No CPAL or SDL3 integration is included in R2.

## Run

From the repository root on Windows x64:

```powershell
cargo run --release --locked --manifest-path native/sega_desktop/Cargo.toml -- path\to\game.sms usa
```

Supported region arguments are `usa`, `brazil`, `europe` and `japan`. The ROM is loaded before the window opens. Use the Iniciar and Pausar controls to send coarse-grained commands to the SMS worker.

## Validation

```powershell
cargo fmt --manifest-path native/sega_desktop/Cargo.toml -- --check
cargo clippy --release --locked --all-targets --manifest-path native/sega_desktop/Cargo.toml --target x86_64-pc-windows-msvc -- -D warnings
cargo test --release --locked --all-targets --manifest-path native/sega_desktop/Cargo.toml --target x86_64-pc-windows-msvc
cargo build --release --locked --manifest-path native/sega_desktop/Cargo.toml --target x86_64-pc-windows-msvc
```

A successful build does not prove physical Vulkan presentation. Run the Release executable on Windows x64 and verify that the VDP image changes with the emulated ROM, start/pause work, resize preserves aspect ratio, and shutdown produces one clean renderer teardown. Record GPU, driver, executable SHA, logs and observations.
