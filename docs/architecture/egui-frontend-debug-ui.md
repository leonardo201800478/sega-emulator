# egui Frontend Layout and Diagnostics Contract

Status: initial UI consolidation for the SMS host.

## Menu organization

The native frontend exposes one consistent menu bar:

- **Sistema** — identifies the active machine, exposes SMS territory/video standard, and lists systems not wired to this frontend as disabled.
- **Arquivo** — ROM selection, recent ROMs, close machine and exit.
- **Vídeo** — aspect ratio and FPS overlay visibility.
- **Áudio** — audio enablement, WASAPI/XAudio2 backend and volume.
- **Controles** — SDL3 device selection, remappable keyboard actions, and remappable gamepad buttons. Bindings are persisted under the user's `%APPDATA%/Sega Emulator/` directory.
- **Opções** — fullscreen, footer visibility, and FPS HUD visibility, size (small/medium/large), and position (four viewport corners).
- **DEBUG** — opt-in diagnostic logging and optional runtime telemetry.

Execution controls (start, pause, stop, reset, and step-frame) are shown as icon buttons directly below the menu bar. Buttons are enabled only when the current runtime state supports the command. Settings applied only when constructing the machine (currently SMS region/video standard) take effect on the next ROM load. Automatic region selection remains disabled in this native frontend until a deterministic ROM metadata resolver is integrated; the ROM filename is not used as evidence.

## Video presentation and FPS

The SMS frame remains presented by the dedicated D3D12 child HWND. The FPS indicator is a separate, transparent, mouse-pass-through egui viewport positioned over the video viewport. It has no panel fill and uses light-green text. The Options menu exposes visibility, three HUD sizes, and four corner anchors; the default remains medium at the bottom-right. The overlay geometry is calculated against the actual video viewport rectangle, not the full application window. Its deferred viewport is declared while enabled and uses an always-on-top transparent window so it can remain above the native D3D12 child window.

FPS is calculated from the D3D12 session's presented-frame sequence, sampled at 500 ms intervals. It does not use egui repaint frequency or monitor refresh as a proxy for emulation output.

## Fullscreen behavior

F11 toggles borderless fullscreen. In fullscreen, the menu and execution toolbar are hidden by default and reappear when the pointer enters the top 56 logical points or while a menu popup remains open. The video viewport receives the reclaimed vertical space. Leaving fullscreen restores the normal persistent menu layout.

## Input mapping

Keyboard actions can be rebound by choosing Remapear and pressing the new key. Gamepad actions can be rebound by selecting the physical button after choosing Remapear. Keyboard and gamepad mappings are persisted separately in the per-user application settings directory. Default gamepad hat and analog-axis directional inputs remain available alongside the configurable directional buttons. Mapping changes affect only host input translation; they do not alter emulated hardware or timing.

## DEBUG collection policy

DEBUG is off by default. Toggling it controls the observability writer and optional worker telemetry. With DEBUG off, the worker does not sample or emit per-frame timing traces, accumulate instruction totals, query audio output metric snapshots, or inspect replaced-frame diagnostics. The machine frame number and presented-frame sequence remain available as runtime/presentation state; they are not optional diagnostic histories.

Diagnostic histories and writer queues must remain bounded. UI callbacks may send coarse-grained commands and render snapshots only; they must never own or advance machine hardware or scheduler time.

Menu diagnostics are transition-oriented: menu activation/state changes, close requests, close events, and focus changes are recorded with the menu name or resulting focus state. Per-repaint events (popup requested/rendered/measured and repeated root HWND reads) are intentionally not logged; they add volume without describing a state transition. Native child-window creation and actual bounds changes remain observable.

## Current limitations

- Only the Master System runtime is connected to this native frontend.
- Automatic SMS region resolution is not yet integrated into the native Rust frontend; explicit USA, Brazil/PAL-M, Europe/PAL, and Japan choices remain available.
- SMS region/video standard takes effect on the next ROM load.
- The FPS overlay reports frames presented by the D3D12 session, not the monitor scanout rate.
