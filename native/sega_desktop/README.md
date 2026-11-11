# Sega Desktop — R2 SMS / Slint / WGPU 30 / Vulkan

The Slint host presents real SMS VDP frames from the Rust runtime. R2.2 migrates the legacy menu groups and ports worker operations and UI settings that are safe before R3/R4.

## Menu migration

- Sistema: SMS region selection (USA, Brasil, Europa, Japão); other Sega systems remain visible but disabled until their runtimes are integrated.
- Arquivo: load ROM by absolute path (.sms / .bin), reopen the most recent ROM in this session, close ROM and exit.
- Vídeo: original 4:3, 1:1 and 16:9 presentation modes. The source frame remains 256×192.
- Áudio: menu retained; physical output, backend selection and volume remain disabled until R3 (CPAL 0.18.2).
- Controles: keyboard/gamepad entries retained; physical input and remapping remain disabled until R4 (SDL3).
- Opções: status-footer visibility. FPS size/position remain pending; fullscreen and FPS visibility are implemented.
- DEBUG: request an immutable Z80/runtime snapshot and enable/disable bounded scheduler/bus trace capture. Full debugger viewport remains a follow-up.

## Runtime contract

- SmsTemporalWorker owns the SMS machine and its execution thread.
- MachineScheduler remains the sole authority for emulated time.
- Start/Pause/Stop/Reset/Step-frame and ROM replacement are coarse-grained worker commands.
- Completed immutable VideoFrame values are published through FrameMailbox.
- The UI timer consumes only the newest pending frame; it never advances emulation.
- RGB888 is converted to tightly packed RGBA8 only at the presentation boundary.
- Slint/WGPU uses the renderer's own Vulkan device and queue.
- No CPAL or SDL3 integration is included in R2.

## Run

From the repository root on Windows x64:

```powershell
cargo run --release --locked --manifest-path native/sega_desktop/Cargo.toml -- [path\to\game.sms] [usa|brazil|europe|japan]
```

The ROM argument is optional. Without it, enter the absolute path in Arquivo > Carregar ROM. The selected region is applied when a ROM is loaded. The File > Browse command opens the native Windows file-selection dialog and filters `.sms` / `.bin` ROMs. The path field remains available for manual entry. Recent ROMs are persisted in the desktop settings directory.

## Validation

```powershell
cargo fmt --manifest-path native/sega_desktop/Cargo.toml -- --check
cargo clippy --release --locked --all-targets --manifest-path native/sega_desktop/Cargo.toml --target x86_64-pc-windows-msvc -- -D warnings
cargo test --release --locked --all-targets --manifest-path native/sega_desktop/Cargo.toml --target x86_64-pc-windows-msvc
cargo build --release --locked --manifest-path native/sega_desktop/Cargo.toml --target x86_64-pc-windows-msvc
```

A successful build does not prove physical Vulkan presentation. Run on Windows x64 and verify real VDP output, all enabled menu commands, ROM replacement/close, pause/resume/reset/step-frame, aspect-ratio modes, trace capture, and clean shutdown.


## R2.3 — persistent frontend preferences

- Persists presentation-only preferences in `%APPDATA%\\Sega Emulator\\slint_ui_settings.txt`: SMS region, aspect ratio, FPS visibility, footer visibility and FPS text size.
- Persists up to ten recent ROM paths in `%APPDATA%\\Sega Emulator\\recent_roms.txt`, newest first. Selecting a recent entry uses the same safe worker handoff as manual ROM loading.
- Recent ROM history and preferences are saved after UI changes and on application shutdown. They do not contain or serialize machine state.
- FPS text size offers small, medium and large values. HUD corner placement remains pending because the current Slint composition does not yet have a separately positioned overlay surface.
