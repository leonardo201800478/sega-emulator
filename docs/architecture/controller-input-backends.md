# Host controller integration into the Rust core

## Goal

Keep emulated input state and SG-1000 port encoding in Rust while keeping host
API polling outside the deterministic machine. SDL, XInput, and DirectInput are
host input sources, not hardware emulation devices.

## Boundary

1. A host adapter polls SDL, XInput, or DirectInput on the frontend/event-loop
   side and translates its result to semantic buttons.
2. The adapter produces a `ControllerFrame` from
   `native/sg1000_core/src/input.rs`.
3. The integration boundary applies `frame.port_1()` and `frame.port_2()`
   to `Sg1000Machine::set_joystick(1, byte)` and
   `Sg1000Machine::set_joystick(2, byte)`.
4. CPU port reads continue to observe only the latest latched bytes. Host polling
   must never occur per emulated instruction or from a CPU bus callback.

## Normalized port contract

The first implementation defines six semantic controls: Up, Down, Left,
Right, Button 1, and Button 2. Bits 0–5 are active-low in that order; bits 6–7
remain high. The released state is `0xFF`. Adapter code must normalize each
backend to pressed=true and must not invert the active-low byte a second time.

This bit assignment is the integration contract for the new normalization
module and must be cross-checked against the existing Python implementation and
hardware/reference documentation before it is treated as final hardware truth.

## Backend requirements

- **SDL**: map SDL game-controller semantic buttons when available; fall back to
  joystick axes/buttons only through an explicit mapping profile.
- **XInput**: map D-pad and supported face buttons; apply a documented deadzone
  to analog axes only if analog-to-digital direction mapping is enabled.
- **DirectInput**: map device objects using a persisted/user-configurable
  mapping; do not assume all controllers expose the same object ordering.
- Keep device enumeration, hot-plug, calibration, remapping, and errors in the
  host adapter. The Rust machine receives normalized snapshots only.
- Do not add host polling, OS handles, callbacks, SDL types, or COM interfaces
  to `Sg1000Machine` or its CPU bus.

## Acceptance gates for backend adapters

- Unit tests for active-low encoding, independent player ports, button chords,
  disconnect/reconnect (release all buttons), and invalid device/mapping errors.
- Adapter tests using synthetic samples; no physical controller required in CI.
- Windows CI for XInput/DirectInput feature builds and an SDL feature build on a
  supported target. Report unavailable target checks as NOT RUN, not PASS.
- Integration test proving the next emulated port read observes a latched input
  frame and repeated identical frames produce identical machine state.
- No claim of hardware-accurate controller behavior until the port bit layout
  is confirmed against reliable SG-1000 documentation or independent tests.

## Scope of this PR

This PR introduces the deterministic Rust normalization contract and tests. It
does **not** yet implement SDL polling, XInput API calls, or DirectInput COM
enumeration. Those adapters must be added as separate host-side implementations
against this contract; their platform dependencies must not enter the hardware
core crate.
