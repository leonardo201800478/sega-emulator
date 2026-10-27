# Low-latency controller input for SG-1000

## Default backend policy

The Qt frontend uses **Native Low Latency** by default:

1. **Windows XInput** is polled directly through the native XInput DLL when at least one XInput controller is connected.
2. **SDL3** is the fallback when no XInput controller is connected. SDL3 supports mapped gamepads and raw joystick bindings, including legacy and non-XInput devices.
3. The frontend polls input at the frame boundary immediately before calling `machine.run_frame()`; the resulting semantic actions are latched through the existing machine `set_joystick` interface before CPU execution.

XInput is not asserted to be universally faster on every device. The actual button-to-photon latency depends on controller firmware, USB/Bluetooth transport, driver, display refresh, and frame pacing. No physical-device latency measurements are claimed by this PR.

References:
- Microsoft XInputGetState: https://learn.microsoft.com/en-us/windows/win32/api/xinput/nf-xinput-xinputgetstate
- Microsoft XInput polling: https://learn.microsoft.com/en-us/windows/win32/xinput/getting-started-with-xinput
- SDL3 event pumping: https://wiki.libsdl.org/SDL3/SDL_PumpEvents
- SG-1000 controller pinout: https://consolemods.org/wiki/SG-1000:Connector_Pinouts
- SG-1000 port layout discussion: https://nicole.express/2023/neo-geo-eat-your-heart-out.html

## Input and determinism contract

- Host APIs are polled only at the frontend frame/input boundary. There are no OS API calls, SDL calls, or Python callbacks from a CPU instruction, memory access, or bus callback.
- Each backend translates device state into semantic pressed/not-pressed actions. The SG-1000 machine receives one logical active-low snapshot per player before running the next frame.
- The Rust core's `ControllerFrame` is host-independent and deterministic. It performs no polling; it only transfers the normalized snapshot into the machine.
- Each logical controller uses bits 0–3 for Up, Down, Left, Right and bits 4–5 for Button 1 and Button 2. A released control is 1; a pressed control is 0.
- Physical reads are composed by `Sg1000Io`: `0xDC` carries player 1 controls on bits 0–5 and player 2 Up/Down on bits 6–7; `0xDD` carries player 2 Left/Right/Button 1/Button 2 on bits 0–3. Logical aliases `0x00` and `0x01` remain for compatibility with the host-facing machine contract.
- A disconnect releases the input snapshot and raises a disconnect event so the frontend can pause rather than silently continue with a missing expected controller.
- XInput discovery of all four user slots is throttled to 500 ms; known-connected slots are sampled each frame. No background polling thread is created.
- XInput takes precedence as a group when any XInput device is connected, to avoid assigning the same physical controller through XInput and SDL simultaneously. In mixed XInput/non-XInput setups, SDL-only devices are not assigned until all XInput devices are disconnected. This limitation is deliberate and documented.
- SDL3 remains responsible for mapped Gamepad and raw Joystick state when XInput is not present. The existing user-configurable button mapping and analog deadzone are retained.
- No DirectInput COM adapter is implemented or claimed by this PR. A future DirectInput adapter needs its own device lifecycle, configurable object mapping, duplicate-device detection, and Windows CI coverage.

## Implementation boundary

1. `NativeFirstControllerInput` attempts XInput first and delegates to the existing SDL3 backend only when no XInput controller is connected.
2. XInput's native DLL is loaded only on Windows. Missing DLLs and non-Windows platforms safely use the SDL3 fallback.
3. The frontend calls `poll()` immediately before the frame is executed, then applies actions to the machine. The hardware core remains independent of Windows and SDL types.
4. The native Rust `ControllerFrame` API remains a deterministic, host-independent contract. It is not used to call host APIs and does not claim per-instruction polling.

## Validation gates

- Unit-test each active-low bit, button chords, two-player isolation, logical aliases and physical port composition.
- Test XInput button/axis mapping, XInput slot discovery throttling, disconnect/reconnect, SDL fallback, configuration updates and one-shot disconnect events using injected fake APIs.
- Run Windows Rust formatting, Clippy and workspace tests, plus Python formatting/lint and Qt integration tests. Python hardware differential/robustness workflows are archived.
- Physical input latency benchmarking is a separate hardware-dependent validation. It must specify controller, connection transport, frame rate, warmup, workload, repetitions, median and spread; API naming alone is not evidence of lower latency.
- No dedicated input thread without profiling and an explicit ownership/synchronization model.
