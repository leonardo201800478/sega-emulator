# Project toolchain and workspace manager

The repository has a root Cargo workspace and independent native workspaces.
The manager discovers workspace roots with Cargo itself, so each workspace is
validated once even when several member crates contain a Cargo.toml.

## Requirements

- Python 3.14, as declared by pyproject.toml
- Rust stable with rustfmt and clippy
- Cargo and Git on PATH
- Development dependencies installed from .[dev]

Activate the project .venv before using the Windows PowerShell wrapper.

## Commands

    .\scripts\project.ps1 audit
    .\scripts\project.ps1 locks
    .\scripts\project.ps1 locks -RefreshOffline
    .\scripts\project.ps1 validate
    .\scripts\project.ps1 validate -Json artifacts\project-validation.json
    .\scripts\project.ps1 validate -RustOnly

Cross-platform entry point:

    python tools/project_manager.py audit
    python tools/project_manager.py locks
    python tools/project_manager.py locks --refresh-offline
    python tools/project_manager.py validate

## Safety model

- audit is read-only. It lists missing lockfiles and manifests outside active workspace membership without changing them. Unassigned manifests are NOT RUN until their workspace policy is decided.
- locks runs cargo check --locked for every active workspace, including a workspace whose lockfile is absent; Cargo decides whether that absence is valid. It must not modify lockfiles.
- locks --refresh-offline is an explicit mutating operation. It uses
  cargo check --offline, never cargo update, and never accesses the network.
- Every successful refresh is followed by cargo check --locked.
- locks --refresh-offline does not create missing lockfiles; it reports those workspaces as BLOCKED for manual policy review.
- The manager never stages, commits, pushes, merges, deletes or stashes files.
- Tool versions are inventoried, not installed or upgraded implicitly.
- Failures remain FAIL or BLOCKED; no skipped check is counted as PASS.

## Scope and limitations

Every active Cargo workspace receives independent formatting, Clippy and
test invocations. Manifests that are not included in any declared workspace
are reported separately as NOT RUN, rather than silently treated as active
crates. This includes standalone native workspaces such as native/z80_core
and native/audio_output.

validate is comprehensive and may take time. Use audit for a fast inventory
and locks to investigate lockfile consistency. Windows-specific D3D12,
WASAPI/XAudio2 compilation and conditional deterministic Scheduler stress
remain governed by GitHub Actions. A local host run cannot replace those jobs.

The first version reports tool versions rather than pinning/upgrading them.
Toolchain pinning is a separate reviewed change because current CI selects
Rust stable and Python 3.14.
