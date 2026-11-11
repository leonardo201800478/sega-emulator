# Sega Desktop — R1 Slint / WGPU 30 / Vulkan POC

This crate is an isolated feasibility application for the desktop replatforming decision in ADR 0011. It is not yet the production launcher and does not start the SMS runtime.

## Scope

- Slint 1.18.x with the WGPU 30 integration.
- FemtoVG WGPU renderer selected through Slint's `renderer-femtovg-wgpu` feature.
- WGPU 30 backend restricted to Vulkan through `WGPUSettings.backends = Backends::VULKAN`.
- Rendering notifier obtains Slint's active `wgpu::Device` and `wgpu::Queue`.
- A WGPU-owned RGBA8 texture is imported as `slint::Image` and updated through the same queue.
- Slint composes the imported texture and UI in one window.

The code deliberately does not depend on the runtime, FrameMailbox, CPAL, SDL3, D3D12 or WASAPI/XAudio2.

## Build and checks

Run from the repository root:

```powershell
cargo fmt --manifest-path native/sega_desktop/Cargo.toml -- --check
cargo clippy --release --locked --all-targets --manifest-path native/sega_desktop/Cargo.toml --target x86_64-pc-windows-msvc -- -D warnings
cargo test --release --locked --all-targets --manifest-path native/sega_desktop/Cargo.toml --target x86_64-pc-windows-msvc
cargo build --release --locked --manifest-path native/sega_desktop/Cargo.toml --target x86_64-pc-windows-msvc
```

## Runtime validation

A successful build is not proof of Vulkan presentation. Run the Release executable on Windows x64 with a Vulkan-capable GPU/driver. Confirm:

1. The window opens and reports the Vulkan adapter.
2. The status line confirms Slint and the texture share the renderer's device/queue.
3. The color-bar marker moves continuously in the imported texture.
4. The texture and Slint text are composed in the same window.
5. Renderer teardown releases the imported image; renderer recreation republishes a new texture.
6. Resizing and closing the window do not crash or leave the process running.

If Vulkan is unavailable, the app must fail to initialize rather than silently selecting DX12 or another backend. Record GPU, driver, executable SHA, logs and observations with the validation report.
