# CI validation policy

The CI workflow selects validation jobs from the files changed by a pull request.
This reduces unrelated runner work without weakening validation for the affected
project domains.

## Required classification

The `Change impact classification` job runs for every pull request. It classifies
changes into these domains:

| Domain | Trigger paths | Validation |
| --- | --- | --- |
| Rust | Rust sources, Cargo manifests/lockfiles, toolchain and Cargo config | Rust formatting, Clippy with warnings denied, lockfile audit, targeted SMS/SG-1000 tests and full Cargo workspace tests |
| Python | Python sources/tests, Python project and test configuration, dependency lock | Ruff format, Ruff lint, Python regression suite and runtime imports |
| Qt | Qt frontend, Qt/controller integration tests and Python runtime dependencies | Windows Qt/Python smoke test |
| PowerShell tooling | `scripts/**/*.ps1`, project manager and tooling tests | Windows PowerShell parser validation |
| PyO3/native bridge | Native crates participating in the Python/native machine bridge | Windows extension build and import |
| Windows native media | D3D12 video and WASAPI/XAudio2 audio crates | Windows-target compile and native unit tests |
| Scheduler | Machine scheduler and temporal core | Enables the optional scheduler stress profile when requested |

Changes to workflow definitions, root build/dependency manifests, toolchain
configuration, or shared project tooling trigger the broad validation set.

## Execution policy

- Pull requests run only the validation domains selected by the classifier.
- Pushes to `main` run the full CI matrix as post-merge regression.
- Manual lockfile refresh remains an explicit workflow-dispatch operation.
- Scheduler stress remains opt-in for ordinary scoped changes (short/long profile
  or the corresponding PR label) because it is intentionally resource-intensive.
- Broad validation (`full`) automatically runs the bounded short Scheduler stress
  profile so workflow/toolchain/shared-contract changes exercise the temporal gate.
- The Python job uses the `uv` cache keyed by `pyproject.toml` and `uv.lock`; a
  separate pip cache is omitted because that job installs dependencies through uv.
- Manual Z80 conformance and Qt/native integration workflows remain available
  for milestone, architecture, and release validation.
- A skipped domain means it was not applicable to that change; it is not recorded
  as a passing test suite. The classifier itself must pass for every PR.

## Safety requirements

A domain must be added to the classifier whenever new crates, runtime boundaries,
or platform-specific components are introduced. Shared contracts (scheduler,
temporal core, public interfaces, serialization formats, and workspace manifests)
must continue to trigger broad validation.

The CI selection is an optimization of *which* checks run, not a reduction in
the acceptance criteria for the affected code. Changes with architectural or
cross-domain impact must expand the test scope even if the diff is small.
