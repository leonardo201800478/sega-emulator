# Qt/D3D12 presentation telemetry audit — 2026-11-06

## Scope

This increment makes the native presentation path observable from the minimal Qt control panel without moving frame data, rendering, filtering, or timing into Python.

## Ownership and data boundary

- Rust remains the owner of the SMS machine, completed-frame publication, D3D12 worker, DXGI swap chain, and Present calls.
- Python exposes only coarse read-only telemetry properties from the existing PyO3 machine:
  - presentation worker state;
  - last completed frame sequence published;
  - last frame sequence successfully presented;
  - mailbox replacement count;
  - successful D3D12 device-recovery count;
  - last video-output error.
- Qt polls these counters every 250 ms for display. The timer is GUI telemetry only; it never advances emulated time, requests frames, reads pixels, or invokes per-instruction callbacks.

## Status interpretation

- **D3D12 initialized; waiting for first frame:** the host startup handshake completed, but no frame has been presented yet. This is expected before the user starts emulation.
- **Presented N/M:** Rust has published through sequence M and D3D12 has successfully presented through sequence N. The counters are monotonic for the SMS worker lifetime.
- **Mailbox replacements:** pending presentation frames replaced by newer completed frames. This is presentation back-pressure telemetry, not emulation frame skipping.
- **Device recoveries:** successful D3D12 device recreation events.
- **Presentation error / worker stopped:** the native video path is no longer healthy. Emulation and audio remain independently owned by Rust.

## Shutdown behavior

The ROM-close and window-close paths stop the telemetry timer, stop the machine, attempt to close the D3D12 session, and then close audio. A D3D12 worker error is reported but does not abort the remaining Qt teardown or leave the ROM selected. The same best-effort shutdown behavior applies when replacing a loaded ROM.

## Automated validation

- Qt tests cover initial waiting-for-first-frame status, published/presented sequence reporting, mailbox replacement and device-recovery telemetry, and shutdown after a D3D12 close error.
- These tests use a fake machine and validate the GUI contract only.
- Existing Windows CI validates the D3D12 compile and CPU-side frame conversion tests. It does not create a real Qt HWND and prove GPU presentation.

## Remaining physical validation gate

Run the minimal SMS application on a Windows x64 machine with a real D3D12-capable GPU and a non-commercial test ROM. Confirm:
1. the Qt child HWND is created and remains alive for the entire presentation session;
2. the status changes from waiting to a non-zero presented sequence after emulation starts;
3. the presented sequence advances while the machine runs and remains stable while paused;
4. ROM replacement and window close release the D3D12 worker without a crash;
5. device-loss/error telemetry is surfaced without stopping CPU execution.

Until that run is recorded, physical GPU/display presentation remains NOT RUN. CI success is not a hardware PASS.
