# CI validation policy

The required CI path is Rust-first and must validate the native product without
installing or executing the legacy Python application. Python, Qt and PyO3
validation remains available only as an explicit, transitional diagnostic.

## Required validation

| Domain | Trigger | Required validation |
| --- | --- | --- |
| Rust | Every pull request and push to `main` | Cargo workspace metadata/lockfile checks, rustfmt, Clippy with warnings denied, targeted SMS/SG-1000 tests and full Cargo workspace tests |
| Native application | Changes to `native/sega_app/**` and application manifests | Windows x64 eframe check, Clippy and headless tests |
| Windows native media | Required CI | D3D12/video-runtime compile and tests; WASAPI/XAudio2/audio-output compile and tests |
| Scheduler | Scheduler/temporal changes, broad changes, or explicit stress label/profile | Deterministic stress profile; skipped profiles remain NOT RUN |
| PowerShell tooling | Changes to project PowerShell scripts | Windows PowerShell parser validation |

The required Rust CI validates the active Cargo workspaces explicitly with
`cargo metadata --locked`. It must not invoke `tools/project_manager.py` or
install Python merely to audit Cargo manifests or lockfiles.

## Transitional legacy validation

The following checks are no longer required on ordinary pull requests or pushes:

- Python Ruff formatting and lint;
- Python pytest collection and regression;
- PySide6/Qt runtime import and smoke tests;
- PyO3/Maturin extension build, import and Python integration tests.

They remain available through the CI workflow's manual
`workflow_dispatch` input `run_legacy_python_validation`. This preserves a
diagnostic path while the Rust application reaches functional parity, without
making the legacy frontend a gate for new Rust-only changes.

A normal PR that does not explicitly dispatch the legacy validation records
those suites as **NOT RUN**, never PASS. A manually dispatched run must be
evaluated against its exact SHA and reported separately from required Rust CI.

Do not remove Python modules, PySide6, PyO3, Maturin, their lock/configuration,
or their historical tests merely because their CI checks became opt-in. Their
removal remains gated by the corresponding Rust replacement, parity tests,
migration inventory and the approved removal milestone.

## Execution policy

- Pull requests and pushes to `main` run the required Rust/native validation.
- Legacy Python/Qt/PyO3 validation runs only when a maintainer explicitly
  dispatches CI with `run_legacy_python_validation=true`.
- Cargo lockfile refresh remains a separate manual operation.
- Scheduler stress remains opt-in for ordinary scoped changes and is enabled
  for scheduler/broad changes or explicit labels/profiles.
- A skipped or unrequested validation is NOT RUN, not PASS.
- CI results are valid only for the exact commit SHA whose checks completed.

## Safety requirements

The CI selection is an optimization of the migration's validation path, not
permission to claim feature parity. Rust changes must still run the relevant
Rust, Windows-native and deterministic Scheduler tests. Hardware-dependent
D3D12, audio endpoint, input, display and gameplay checks remain separate
manual acceptance gates where the hosted runner cannot prove physical behavior.

Python checks may be removed permanently only after the Python runtime and
all active consumers have been replaced and the migration inventory confirms
that no production path depends on Python, PySide6, PyO3 or Maturin.
