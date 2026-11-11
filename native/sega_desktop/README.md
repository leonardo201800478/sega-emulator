# Sega Desktop — R2 SMS / Slint / WGPU 30 / Vulkan

The Slint host presents real SMS VDP frames from the Rust runtime. R2.2 migrates the legacy menu groups and ports worker operations and UI settings that are safe before R3/R4.

## Menu migration

- Sistema: SMS region selection (USA, Brasil, Europa, Japão); other Sega systems remain visible but disabled until their runtimes are integrated.
- Arquivo: load raw `.sms` / `.bin` ROMs or a `.zip` containing exactly one supported ROM entry; reopen recent ROMs, close ROM and exit.
- Vídeo: 4:3, 1:1 and 16:9 presentation modes preserve the selected ratio while the window is resized; unused viewport space is letterboxed. The source frame remains 256×192. The optional sprite-limit compatibility mode is persisted separately from hardware state.
- Áudio: menu retained; physical output, backend selection and volume remain disabled until R3 (CPAL 0.18.2).
- Controles: keyboard/gamepad entries retained; physical input and remapping remain disabled until R4 (SDL3).
- Opções: status-footer visibility. FPS size/position remain pending. Fullscreen is available from the UI and F11.
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

The ROM argument is optional. Without it, use Arquivo > Procurar or enter an absolute path manually. The native Windows file-selection dialog filters `.sms`, `.bin` and `.zip`. A ZIP must contain exactly one non-directory `.sms` / `.bin` entry; archives are read in memory, with limits on archive size, entry count and decompressed ROM size. No archive-controlled path is extracted to disk. The selected region and sprite compatibility preference are applied to each newly loaded machine. Recent ROMs are persisted in the desktop settings directory. Clicking the video viewport resumes a paused machine; F11 toggles fullscreen.

## Validation

```powershell
cargo fmt --manifest-path native/sega_desktop/Cargo.toml -- --check
cargo clippy --release --locked --all-targets --manifest-path native/sega_desktop/Cargo.toml --target x86_64-pc-windows-msvc -- -D warnings
cargo test --release --locked --all-targets --manifest-path native/sega_desktop/Cargo.toml --target x86_64-pc-windows-msvc
cargo build --release --locked --manifest-path native/sega_desktop/Cargo.toml --target x86_64-pc-windows-msvc
```

A successful build does not prove physical Vulkan presentation. Run on Windows x64 and verify real VDP output, all enabled menu commands, ROM replacement/close, pause/resume/reset/step-frame, aspect-ratio modes during continuous resize, video-click resume from pause, F11 fullscreen toggle, ZIP ROM selection and rejection cases, sprite-limit toggling, trace capture, and clean shutdown.


## R2.3 — persistent frontend preferences

- Persists presentation-only preferences in `%APPDATA%\\Sega Emulator\\slint_ui_settings.txt`: SMS region, aspect ratio, FPS visibility, footer visibility and FPS text size.
- Persists up to ten recent ROM paths in `%APPDATA%\\Sega Emulator\\recent_roms.txt`, newest first. Selecting a recent entry uses the same safe worker handoff as manual ROM loading.
- Recent ROM history and preferences are saved after UI changes and on application shutdown. They do not contain or serialize machine state.
- FPS text size offers small, medium and large values. HUD corner placement remains pending because the current Slint composition does not yet have a separately positioned overlay surface.
