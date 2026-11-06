# Rust workspace boundary and relocation plan

**Date:** 2026-11-06  
**Status:** approved target; implementation not yet migrated  
**Parent decision:** ADR 0010 — Rust-only, SMS temporal first

## 1. Objective

Create one physically separate Rust product tree under `rust/`. The repository
root remains for project governance, documentation, CI, licensing and
non-executable project metadata. Active source code, Cargo manifests,
lockfiles, application code, host adapters and Rust tests move under
`rust/`.

Python is not a second implementation, a reference runtime, a test harness,
a build tool or a fallback. The final repository must contain no Python
source, Python package metadata, Python lockfiles, Python workflows, PyO3 or
Maturin artifacts.

## 2. Target topology

The canonical topology is documented in [rust/README.md](../../rust/README.md)
and has these boundaries:

| Area | Responsibility |
|---|---|
| `rust/apps/sega-app` | egui/eframe shell, menus and presentation |
| `rust/runtime/sms-runtime` | SMS session, worker, coarse-grained lifecycle |
| `rust/crates/z80-*` | one shared Z80 implementation and bus/ALU contracts |
| `rust/crates/machine-scheduler` | sole authority for machine time and event ordering |
| `rust/crates/temporal-core` | shared temporal primitives and serializable state |
| `rust/crates/sn76489` | shared PSG implementation |
| `rust/crates/video-core` | shared SMS VDP implementation |
| `rust/crates/audio-runtime` | scheduled audio production |
| `rust/crates/video-runtime` | frame transport and host presentation boundary |
| `rust/crates/input-runtime` | native input abstraction |
| `rust/hosts/audio-output` | Windows WASAPI/XAudio2 output adapters |
| `rust/tests/system` | Rust-only integration, replay and determinism tests |

No future-platform runtime is activated by this relocation. SMS temporal is the
first and only machine runtime in the active workspace.

## 3. Dependency constraints

The intended runtime dependency direction is:

```text
sega-app (egui/eframe)
    -> application/session API
        -> sms-runtime
            -> sms-core temporal path
                -> shared Z80 + bus
                -> MachineScheduler + temporal-core
                -> SN76489 + audio-runtime
                -> video-core + video-runtime
            -> native input / audio host interfaces
```

The UI sends coarse-grained commands only. It does not call CPU instructions,
advance a clock, schedule hardware events or own machine state.

## 4. Ordered migration

### R0 — establish the boundary

- Define `rust/` as the only destination for active Rust product source.
- Inventory every Rust crate, standalone Cargo workspace, lockfile, CI path,
  documentation link and build script.
- Identify all consumers of the worker and every module in the hybrid SMS core.

### R1 — extract the SMS temporal runtime

- Extract `SmsTemporalWorker` and its required session/host dependencies from
  `native/z80_core` into `rust/runtime/sms-runtime`.
- Remove PyO3 annotations and dependencies from the runtime implementation.
- Preserve exactly one worker, one SMS temporal machine and one Scheduler.
- Add Rust integration tests for lifecycle, input, frame production, PCM
  production, shutdown and deterministic block/continuous execution.

### R2 — isolate the temporal SMS core

- Separate temporal modules from atomic CPU/machine/PSG/VDP modules.
- Preserve only memory, cartridge/mapper, I/O and VDP wrapper code proven to be
  consumed by the temporal path.
- Keep the shared Z80, PSG, VDP and Scheduler implementations unique.
- Do not perform a filename-only move or delete shared dependencies.

### R3 — consolidate the Rust workspace

- Move Rust source, manifests and lockfiles under `rust/`.
- Consolidate the active crates into one Cargo workspace and one canonical
  lockfile where dependency constraints allow.
- Put the app under `rust/apps`, host adapters under `rust/hosts`, and
  machine-independent crates under `rust/crates`.
- Remove obsolete root-level Cargo workspace files only in the same change
  that updates all CI and developer commands.

### R4 — retire legacy implementations

- Remove Python application/runtime, tests, reference code, scripts,
  packaging, lockfiles, Qt frontend, PyO3 adapter, Maturin configuration,
  wheels and Python-specific workflows.
- Remove atomic SMS and SG-1000 implementations from the active workspace.
- The old SG-1000 implementation is not a runtime fallback; a future SG-1000
  must be rebuilt on the shared Z80 after SMS is executable.

### R5 — validate and close

- Run rustfmt, Clippy with warnings denied and all workspace tests.
- Run Windows-target D3D12, WASAPI/XAudio2 and input checks.
- Run synthetic/homebrew SMS integration and deterministic replay tests.
- Classify every required suite as PASS, FAIL, BLOCKED or NOT RUN.
- Do not mark hardware-dependent tests PASS based on headless CI.

## 5. Acceptance gates

The relocation is complete only when:

1. All active Rust code and Cargo metadata live under `rust/`.
2. No Python, PyO3, Maturin, PySide6 or Qt artifacts remain.
3. One Cargo workspace and lockfile define the active product.
4. SMS temporal worker is pure Rust and is the only worker implementation.
5. Only SMS temporal is an active machine runtime.
6. UI is limited to presentation and coarse-grained application commands.
7. Required Rust, Windows and system-integration checks pass on the exact
   candidate SHA.
8. No essential semantic, temporal, snapshot or determinism contract was
   removed without a tested Rust replacement.

## 6. Baseline audit

At the baseline `edbec9272efe87a6a408bac74283d86a91d87b23`:

- 566 tracked blobs were returned by the recursive Git tree.
- 188 blobs were under `native/` or were root Cargo workspace files.
- 141 files matched Python source or Python packaging/configuration patterns.
  This includes `legacy/python_reference`, active `src/` and `tests/`,
  plus the PyO3 crate's own Python packaging metadata.
- Root Cargo workspace still includes `native/sg1000_core` and
  `native/sms_core`; `native/audio_output`, `native/z80_core` and
  `native/sega_app` are separate workspaces.
- `native/sms_core` is hybrid: temporal modules depend on shared memory,
  cartridge, I/O and VDP types in the same crate.
- `SmsTemporalWorker` remains inside `native/z80_core`, which depends on
  PyO3 and exposes both atomic and temporal bindings.
- CI still defines Python, Qt, PyO3, Maturin and PowerShell tooling jobs.

These counts are an inventory of tracked paths, not a claim that all files are
runtime-active.

## 7. Implementation status

This change establishes the destination and migration contract only.
**No Rust source has yet been relocated, no Python files have yet been
removed, and no Cargo validation has been run.** The next implementation
increment must extract the SMS worker into a pure-Rust crate and prove its
contract before removing the PyO3-bound crate.
