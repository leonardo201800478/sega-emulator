# SMS Telemetry HUD and Native D3D12 Composition

## Scope

The native SMS HUD presents emulation-side telemetry from the Rust-owned
`SmsTemporalWorker`. It is a presentation-only consumer: it does not advance
the CPU, VDP, Scheduler, master clock, or frame execution.

## Metric source and semantics

The HUD reads `SmsTemporalWorker::performance_metrics()`:

- `emulation_fps`: completed SMS frame intervals per wall-clock second,
  measured by the runtime worker.
- `frame_time_ms`: wall-clock interval between consecutive completed SMS
  frames.
- `input_to_frame_ms`: host input detection to the first completed SMS frame
  that incorporates that input.
- `completed_frames`: monotonically increasing completed-frame count for the
  current telemetry instance.

The HUD does not sample egui repaint frequency, DXGI Present, GPU throughput,
monitor refresh rate, or a fixed 60 Hz target. It refreshes its presentation
text every 250 ms; this cadence does not participate in emulated time.

A breakpoint-truncated instruction sequence is not a completed frame. Runtime
telemetry and video publication must only advance after the VDP completes a
frame. Pending input latency remains pending until the next complete frame.

## Native composition

The D3D12 video surface is a child HWND of the eframe root HWND. The root uses
`WS_CLIPCHILDREN`, so egui parent painting is not relied upon to cover the
video child. The telemetry HUD is a second child HWND, a sibling of the D3D12
surface, with these styles:

- Window styles: `WS_CHILD | WS_CLIPSIBLINGS`.
- Extended styles: `WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_NOACTIVATE`.
- Transparency: `SetLayeredWindowAttributes` with a black color key.
- Z-order: `SetWindowPos(HWND_TOP)` within the same parent child-window band.
- Activation: `SW_SHOWNOACTIVATE` and `WM_MOUSEACTIVATE -> MA_NOACTIVATE`.
- Hit testing: `WM_NCHITTEST -> HTTRANSPARENT`.

The HUD is never a top-level window and never uses `HWND_TOPMOST` or the
`WS_EX_TOPMOST` style. It cannot remain above another application. Its lifetime
is subordinate to the eframe parent and it is destroyed before the D3D12 child
window.

The child is created on the parent HWND's owning GUI thread. Its bounds are
computed from the egui video viewport in parent-client coordinates and
converted from points to physical pixels using `Context::pixels_per_point()`.
Bounds are recalculated when the video viewport, window size, or DPI scale
changes. Fullscreen uses the same parent-client geometry path.

The transparent layered child uses `WS_EX_TRANSPARENT` and explicit transparent
hit testing so mouse input reaches the underlying video child. It does not take
focus and does not change desktop z-order.

## Vector toolbar icons

The main execution toolbar and debugger controls share
`native/sega_app/src/icons.rs`. Icons are drawn with egui vector primitives
(lines, circles, rectangles, and filled polygons); no Unicode glyph is used as
an icon substitute. The shared icon set includes play, pause, stop, reset,
step, trace, add, remove, and breakpoint state.

## Validation contract

Automated Windows validation must verify:

- The HUD HWND is a direct child of the eframe root HWND.
- Required layered, transparent, non-activating child styles are present.
- `WS_EX_TOPMOST` is absent.
- The HUD is first in the parent's child z-order above the D3D12 video child.
- Resize and DPI changes update physical-pixel bounds.
- Hiding, fullscreen transitions, parent destruction, and shutdown preserve
  child-window lifetime and z-order.
- The HUD does not consume mouse input.
- Runtime telemetry tests distinguish complete frames from breakpoint stops.

A successful compile or HWND-style assertion does not prove final visual
composition on every Windows compositor/DPI combination. Manual presentation
validation remains a separate acceptance item after automated gates pass.
