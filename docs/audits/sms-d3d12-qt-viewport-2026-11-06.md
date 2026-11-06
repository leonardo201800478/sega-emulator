# SMS D3D12 viewport integration audit — 2026-11-06

## Scope

This increment connects the Rust-owned SMS completed-frame mailbox to a Qt-owned native viewport HWND. It does not move pixels, image objects, filters, rendering, or emulation timing into Python.

## Ownership contract

- Qt creates and owns the native child widget and its HWND on the GUI thread.
- The GUI thread obtains the HWND with `QWidget.winId()` only after the window is shown.
- `D3d12MailboxSession::start_for_existing_window` validates the HWND on the calling thread before creating the presentation worker. This matters because Win32 documents that `IsWindow` should not be used by a thread for a window it did not create; the D3D12 worker therefore does not validate or destroy the foreign HWND.
- The D3D12 worker owns the DXGI swap chain, D3D12 device/queue, fences, uploads and Present calls.
- The external-window wrapper never calls `DestroyWindow` and does not pump Qt's message queue. Qt remains responsible for the HWND lifecycle and message dispatch.
- The UI must call `close_video_output()` and wait for it to return before the QWidget is destroyed. The native session then releases DXGI/D3D12 resources while the HWND is still valid.

## Frame path

`SmsTemporalMachine::step_frame` completes in Rust. At that completed-frame boundary, the worker copies the immutable RGB888 framebuffer into `VideoFrame` (256x192, 768-byte row stride) and publishes it to the single-slot `FrameMailbox`. The D3D12 worker consumes the newest pending frame and presents it to the Qt-owned HWND.

Mailbox replacement affects presentation only. It does not skip CPU instructions, VDP work, Scheduler events, audio samples, or emulated time. The sequence counter is monotonic for the lifetime of the SMS worker, including reset and ROM replacement.

## Qt viewport

- `SmsVideoViewport` owns geometry only and letterboxes a native child widget to 4:3.
- `WA_NativeWindow`/ `winId()` ensure a real HWND exists.
- `WA_NativeWindow` provides a real HWND; `WA_NoSystemBackground` and `WA_OpaquePaintEvent` prevent Qt from erasing the swap-chain surface with its normal background paint.
- The DXGI swap chain uses `DXGI_SCALING_STRETCH`; the native child HWND is resized by Qt while the 256x192 logical surface remains unchanged.
- The Python adapter exposes only coarse `open_video_output_in_viewport(hwnd)` and `close_video_output()` calls. No framebuffer getter or per-frame callback is introduced.

## Failure and shutdown

- If D3D12 initialization fails, the Qt control panel reports the error while the SMS machine remains available.
- If mailbox publication fails, video is detached and the worker records a video-specific error; emulation continues.
- The window-close path stops the machine, closes the D3D12 session, then releases audio and the QWidget.
- An HWND is a borrowed handle, not an owned resource. The GUI must keep it alive for the full D3D12 session.

## Validation gates

- Rust formatting, Clippy and workspace regression.
- Standalone PyO3 formatting, locked check, Clippy, tests, Maturin build and native SMS/SG-1000 integration tests.
- Windows x64 D3D12 compile and video-runtime tests, including rejection of a zero external HWND before worker startup.
- Python Ruff and pytest, including Qt viewport 4:3 geometry and lifecycle tests.
- Hardware presentation remains a manual validation gate; CI compilation is not proof of physical GPU/display behavior.
