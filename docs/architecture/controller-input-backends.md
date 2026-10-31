# Controller Input Backends — Legacy Frontend Path

> **Migration status:** the current Python XInput/PySDL3 implementation is a compatibility baseline, not the target architecture. The reusable host-independent contract is now defined in [Native Input Runtime](native-input-runtime.md).

## Current implementation retained during migration

- XInput is currently polled through Python ctypes.
- SDL3 Gamepad and raw Joystick support is currently accessed through PySDL3.
- The Qt frontend samples these backends immediately before machine frame execution.
- SG-1000 and SMS receive normalized snapshots through their existing native APIs.
- XInput currently takes precedence as a group. In mixed XInput/non-XInput setups, SDL-only devices can be omitted. This is a known compatibility limitation, not desired behavior.

These paths must remain operational until native backends have equivalent behavior and hardware validation. Do not remove or duplicate their profile, mapping, hotplug, and device-selection behavior during the port.

## Target architecture

See [Native Input Runtime](native-input-runtime.md) for the cross-core data model, GameInput/SDL3 strategy, device identity, timestamping, allocation rules, compatibility matrix, and acceptance gates.

The intended Windows backend is Microsoft GameInput, with native SDL3 as fallback. The two backends must feed one device registry and deduplicate the same physical controller. They must not compete as separate player groups.

## Latency contract

- Poll the newest available reading at the latest safe boundary before emulation advances.
- Do not poll inside CPU instructions, memory accesses, bus callbacks, or audio/video inner loops.
- Do not add a dedicated polling thread without a measured benefit and explicit ownership/synchronization design.
- Distinguish device-to-runtime capture, runtime-to-emulated-frame application, and button-to-photon latency.
- API marketing claims and software timestamps are not physical latency measurements.

## Existing core contract

The SG-1000 and SMS hardware cores remain host-independent. They accept explicit input snapshots and do not call Windows, SDL, Qt, or Python APIs. Console-specific electrical layouts, active-low behavior, light-phaser timing, and future peripheral protocols remain inside the corresponding core/scheduler layers.
