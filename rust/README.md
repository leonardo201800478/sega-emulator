# Rust workspace

This directory is the canonical destination for the Sega Emulator Rust product.

> **Migration state: boundary defined; source relocation not yet performed.**
> The existing `native/` tree remains the current source location until the
> dependency-aware relocation is completed and validated. Do not treat this
> directory as a buildable workspace yet.

## Target layout

```text
rust/
├── Cargo.toml                 # single canonical workspace
├── Cargo.lock                 # single canonical lockfile
├── apps/
│   └── sega-app/              # egui/eframe application shell
├── crates/
│   ├── z80-bus/
│   ├── z80-cpu/
│   ├── z80-alu/
│   ├── temporal-core/
│   ├── machine-scheduler/
│   ├── sn76489/
│   ├── audio-runtime/
│   ├── video-core/
│   ├── video-runtime/
│   ├── input-runtime/
│   └── sms-core/
├── hosts/
│   └── audio-output/          # host de áudio nativo em transição; substituição CPAL prevista em R3
├── runtime/
│   └── sms-runtime/           # pure-Rust worker and session lifecycle
└── tests/
    └── system/                # Rust-only integration and determinism tests
```

## Product boundary

- Rust is the only implementation and test language in the active product.
- egui/eframe owns presentation only; it never advances hardware clocks.
- The SMS temporal runtime is the only active machine runtime in this first
  migration milestone.
- The shared Z80, Scheduler, temporal core, SN76489, SMS VDP and host
  transports remain single implementations.
- No PyO3, Maturin, Python, PySide6, Qt, Python test runner, or Python
  packaging artifacts belong in the resulting product.
- Future console crates are not part of the active workspace until the SMS
  runtime is executable and validated.

## Relocation rules

1. Move source trees by preserving Git history where possible and update every
   manifest path, workspace membership, lockfile, CI path, build command and
   documentation reference in the same coherent change.
2. Extract the SMS worker from the PyO3-bound crate before removing that crate.
   The extracted worker must be the sole worker implementation.
3. Split the hybrid SMS core by dependency, not by filename: retain memory,
   mapper, I/O and VDP wrappers only where the temporal path consumes them.
4. Do not carry forward atomic SMS/SG-1000 runtimes, Python reference code,
   Python tests, Qt UI, bindings or compatibility fallbacks.
5. Recreate essential coverage as Rust tests against actual public contracts;
   do not mechanically translate Python test syntax or delete a contract
   before its Rust replacement exists.
6. Keep hardware-dependent Windows checks distinct from headless tests.
7. Do not claim a move or validation until the exact candidate SHA passes the
   required Rust and Windows CI gates.

## Current blockers

- `SmsTemporalWorker` is still located in `native/z80_core`, a PyO3 crate.
- `native/sms_core` still mixes temporal and atomic modules.
- The root workspace still includes the atomic SG-1000 core.
- The app and audio-output crates still have standalone workspace/lockfile
  boundaries.
- CI and repository tooling still contain Python, Qt and PyO3 workflows.
