# egui Frontend Layout and Diagnostics Contract

Status: initial UI consolidation for the SMS host.

## Menu organization

The native frontend exposes one consistent menu bar:

- **Sistema** — identifies the active machine, exposes SMS territory/video standard, and lists systems not wired to this frontend as disabled.
- **Arquivo** — ROM selection, recent ROMs, close machine and exit.
- **Vídeo** — aspect ratio and FPS overlay visibility.
- **Áudio** — audio enablement, WASAPI/XAudio2 backend and volume.
- **Controles** — SDL3 device selection and the currently implemented keyboard bindings.
- **Opções** — fullscreen, footer visibility, machine lifecycle controls, and FPS HUD visibility, size (small/medium/large), and position (four viewport corners).
- **DEBUG** — opt-in diagnostic logging and optional runtime telemetry.

Settings applied only when constructing the machine (currently SMS region/video standard) are labelled as taking effect on the next ROM load.

## Video presentation and FPS

The SMS frame remains presented by the dedicated D3D12 child HWND. The FPS indicator is a separate, transparent, mouse-pass-through egui viewport positioned over the video viewport. It has no panel fill and uses light-green text. The Options menu exposes visibility, three HUD sizes, and four corner anchors; the default remains medium at the bottom-right. The overlay geometry is calculated against the actual video viewport rectangle, not the full application window.

FPS is calculated from the D3D12 session's presented-frame sequence, sampled at 500 ms intervals. It does not use egui repaint frequency or monitor refresh as a proxy for emulation output.

## DEBUG collection policy

DEBUG is off by default. Toggling it controls the observability writer and optional worker telemetry. With DEBUG off, the worker does not sample or emit per-frame timing traces, accumulate instruction totals, query audio output metric snapshots, or inspect replaced-frame diagnostics. The machine frame number and presented-frame sequence remain available as runtime/presentation state; they are not optional diagnostic histories.

Diagnostic histories and writer queues must remain bounded. UI callbacks may send coarse-grained commands and render snapshots only; they must never own or advance machine hardware or scheduler time.

## Current limitations

- Only the Master System runtime is connected to this native frontend.
- Keyboard bindings are displayed but are not remappable yet.
- SMS region/video standard takes effect on the next ROM load.
- The FPS overlay reports frames presented by the D3D12 session, not the monitor scanout rate.
