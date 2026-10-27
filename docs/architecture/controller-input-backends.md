# Low-latency controller input for the Rust core

## Product decision

The SG-1000 default input policy is **Native Low Latency**. Prefer the closest
usable native API for the connected controller instead of forcing every device
through a generic SDL abstraction:

1. **Windows: XInput first** for devices exposed by XInput.
2. **Windows: DirectInput fallback** for legacy devices not available through
   XInput. Device object mapping must be configurable; do not assume a standard
   axis/button order.
3. **SDL: explicit fallback and cross-platform backend**, using semantic
   GameController/Gamepad mappings where available and an explicit mapping
   profile for raw joystick devices.

The public selection policy is represented by
`ControllerBackendPreference::NativeLowLatency`, which is the default. This
is a policy for choosing a backend, not a guarantee that XInput or DirectInput
is universally faster for every controller. The end-to-end result must be
measured on the actual controller/transport/driver combination. Microsoft
documents XInput as a current-state polling API; SDL likewise requires its
controller state to be updated through its event/update mechanism.

## Latency contract

- Poll the selected host backend immediately before the emulated frame (or
  input-latch boundary) that will consume the snapshot; do not wait for a
  Python callback or an unrelated UI timer.
- Keep the polling and OS-specific code in the host adapter. Transfer one small,
  normalized snapshot into Rust and latch it before CPU execution.
- Never poll an OS API, call SDL, enter COM, or invoke Python from a CPU
  instruction, memory access, or bus callback.
- Keep the polling owner explicit. Do not add a polling thread until profiling
  demonstrates a benefit and ownership/synchronization semantics are designed.
- SDL's event pump must be serviced on the thread required by SDL; the adapter
  must not assume that reading cached state updates the physical-device state.
- Track controller connection state. On disconnect, latch a released snapshot
  immediately; throttle discovery of empty XInput slots rather than probing all
  unused slots on every poll.
- Record timestamps around host sample acquisition and machine latching in a
  diagnostic/benchmark mode. Do not put logging or allocation in the normal
  per-frame hot path.

## Core boundary

1. A host adapter polls XInput, DirectInput, or SDL and translates its result to
   semantic pressed/not-pressed buttons.
2. The adapter creates a `ControllerFrame` from
   `native/sg1000_core/src/input.rs`.
3. The host integration applies `frame.apply_to(&mut machine)`, which latches
   both controller bytes through the existing `Sg1000Machine::set_joystick`
   interface.
4. CPU port reads observe the latest latched state. Identical snapshots must
   produce identical machine input state.

## Normalized port contract

The first implementation defines six semantic controls: Up, Down, Left,
Right, Button 1, and Button 2. Bits 0–5 are active-low in that order; bits 6–7
remain high. The released state is `0xFF`. Backend adapters normalize each
backend to pressed=true and must not invert the active-low byte a second time.

This bit assignment remains provisional until cross-checked against the
existing Python implementation and independent SG-1000 hardware/reference
documentation. Do not treat the proposed mapping as final hardware truth before
that verification.

## Backend-specific requirements

- **XInput**: map D-pad and supported buttons from `XINPUT_STATE`; read only
  known-connected user slots at the normal poll cadence, and refresh discovery
  of unused slots periodically. Apply a documented deadzone only if analog
  stick-to-direction mapping is enabled.
- **DirectInput**: acquire and poll devices according to the API's lifecycle,
  handle lost/acquire states, and use a persisted/user-configurable object map.
- **SDL**: prefer semantic GameController/Gamepad mappings; call the appropriate
  update/pump operation at the correct event-loop boundary. Use explicit
  profiles for unmapped raw joystick devices.
- Prevent duplicate enumeration when one physical controller is exposed by
  multiple backends. Preserve a stable device identity where the API permits.
- Keep enumeration, hot-plug, calibration, remapping, errors and backend
  selection in the host adapter. The Rust machine receives normalized snapshots
  only.
- Do not add host polling, OS handles, callbacks, SDL types, or COM interfaces
  to `Sg1000Machine` or its CPU bus.

## Acceptance gates

- Verify the port-bit layout against the existing Python implementation and
  reliable independent SG-1000 documentation/tests.
- Implement XInput-first native selection on Windows, DirectInput legacy
  fallback, and an SDL cross-platform/explicit backend.
- Test synthetic backend samples, each button, chords, disconnect/reconnect,
  remapping, duplicate-device suppression and errors.
- Add Windows CI for XInput/DirectInput and an SDL backend build on a supported
  target. Mark unavailable platform checks NOT RUN, never PASS.
- Test that a sampled state is latched before the next CPU read and that
  repeated identical frames produce identical machine state.
- Benchmark input-sample-to-latch time and end-to-end button-to-visible-response
  latency with a defined device, transport, frame rate, workload, warmup and
  repetitions. Report median and spread; do not infer lower latency from API
  names alone.
- No host polling or callbacks from CPU execution. No dedicated input thread
  without measured evidence and an explicit ownership/synchronization model.

## Current scope of PR #127

This PR introduces the Rust normalization contract, the default native-first
policy, and deterministic tests. It does **not** yet implement actual SDL
polling, XInput API calls, or DirectInput COM enumeration. Those adapters and
the frontend integration must be implemented and validated before this PR can
be considered complete.
