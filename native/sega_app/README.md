# Native egui application — SMS runtime integration

`sega-app` is the native Rust/egui frontend for the temporal Sega Master System runtime.

## Current vertical slice

- Opens the native Windows file picker for `.sms` and `.zip` ROMs, while keeping the path field editable and enabling Windows clipboard paste (`eframe/clipboard`). Raw `.bin` paths remain accepted for compatibility.
- Remembers the last selected ROM directory in `%APPDATA%\\Sega Emulator\\last_rom_directory.txt`.
- Loads raw `.sms` / `.bin` files or the only `.sms` entry inside a ZIP archive; archives with zero or multiple SMS entries are rejected explicitly.
- Selects the machine region (USA/NTSC, Brazil/PAL-M, Europe/PAL, Japan/NTSC).
- Creates the existing `SmsTemporalWorker`; it does not implement a second machine or scheduler.
- Exposes coarse-grained start, pause, stop, reset and single-frame commands.
- Connects completed RGB888 VDP frames through the external `FrameMailbox` to the D3D12 child-window presenter.
- Shows frame/instruction/video telemetry and runtime errors.
- Stops and joins the machine worker, detaches its mailbox, then shuts down D3D12 before destroying the child HWND.

## Current limitations

- 7z archives, multi-ROM ZIP selection and library indexing are not implemented.
- Audio output and physical controller input are not yet connected to the app.
- The D3D12 viewport requires Windows x64 and a compatible graphics device.
- CI compilation and headless tests do not prove physical GPU presentation or gameplay compatibility.

## Build

```powershell
cargo fmt --manifest-path native/sega_app/Cargo.toml -- --check
cargo check --locked --manifest-path native/sega_app/Cargo.toml --target x86_64-pc-windows-msvc
cargo test --locked --manifest-path native/sega_app/Cargo.toml --target x86_64-pc-windows-msvc
```

## Ownership and timing

The frontend sends coarse-grained commands to the SMS worker. The worker remains the sole owner of mutable machine state and emulated time. Completed RGB888 frames cross into the presentation thread through an immutable single-slot mailbox; replacing a pending presentation frame never skips CPU, VDP, PSG, Scheduler or audio work. The D3D12 host owns its presentation session and closes its mailbox only after the SMS worker has detached.


## Aspect ratio

The video viewport defaults to the SMS 4:3 presentation ratio. The UI exposes
1:1, 4:3 and 16:9 display modes. The selected viewport is fitted inside the
available client area with a uniform scale; it is never stretched to fill a
window with a different ratio. The native child presentation window follows
the fitted viewport rectangle.
