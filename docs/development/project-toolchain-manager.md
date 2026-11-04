# Project toolchain and workspace manager

The repository has a root Cargo workspace and independent native workspaces.
The manager discovers workspace roots with Cargo itself, so each workspace is
validated once even when several member crates contain a Cargo.toml.

## Requirements

- Python 3.15.2, as declared by pyproject.toml
- Rust 1.99.1 pinned in rust-toolchain.toml with rustfmt and clippy
- Cargo and Git on PATH
- Development and Qt dependencies synchronized from the committed uv.lock

Install/synchronize the locked Python environment with:

    uv sync --locked --extra dev --extra qt

Activate the project .venv before using the Windows PowerShell wrapper.

## Commands

    .\scripts\project.ps1 audit
    .\scripts\project.ps1 locks
    .\scripts\project.ps1 locks -RefreshOffline
    .\scripts\project.ps1 validate
    .\scripts\project.ps1 validate -Json artifacts\project-validation.json
    .\scripts\project.ps1 validate -RustOnly

Cross-platform entry point:

    uv run --locked python tools/project_manager.py audit
    uv run --locked python tools/project_manager.py locks
    uv run --locked python tools/project_manager.py locks --refresh-offline
    uv run --locked python tools/project_manager.py validate

## Safety model

- audit is read-only. It lists missing lockfiles and manifests outside active workspace membership without changing them. Unassigned manifests are NOT RUN until their workspace policy is decided.
- locks runs cargo check --locked for every active workspace, including a workspace whose lockfile is absent; Cargo decides whether that absence is valid. It must not modify lockfiles.
- locks --refresh-offline is an explicit mutating operation. It uses
  cargo check --offline, never cargo update, and never accesses the network.
- Every successful refresh is followed by cargo check --locked.
- locks --refresh-offline does not create missing lockfiles; it reports those workspaces as BLOCKED for manual policy review.
- The manager never stages, commits, pushes, merges, deletes or stashes files.
- Python dependencies are pinned directly in pyproject.toml and transitively in uv.lock. The required uv version is pinned in [tool.uv].
- Failures remain FAIL or BLOCKED; no skipped check is counted as PASS.

## Scope and limitations

The root Cargo workspace and the independent native/audio_output and
native/z80_core workspaces receive their own lockfile validation. Formatting,
Clippy and tests are run for the applicable Rust workspace members.

Manifests outside the declared active workspaces are reported separately as
NOT RUN, rather than silently treated as active crates. This currently includes
native/32x_core, native/gg_core, native/md_core and native/scd_core. They remain
scaffolds for future platform phases and are intentionally not activated by
this toolchain migration.

validate is comprehensive and may take time. Use audit for a fast inventory
and locks to investigate lockfile consistency. Windows-specific D3D12,
WASAPI/XAudio2 compilation and conditional deterministic Scheduler stress
remain governed by GitHub Actions. A local host run cannot replace those jobs.

## Controlled dependency upgrades

The project pins Python 3.15.2, Rust 1.99.1 and uv 0.13.7.
Direct Python dependencies are pinned in pyproject.toml and the complete
resolved Python environment is recorded in uv.lock. CI installs the dev and
Qt extras with uv sync --locked and runs commands through uv run --locked.
Do not bypass the lockfile in CI.

Cargo lockfiles are generated artifacts and must never be edited manually.
For an explicit online dependency-refresh branch, update each independent
workspace separately:

```powershell
cargo update --workspace
cargo update --manifest-path native/audio_output/Cargo.toml
cargo update --manifest-path native/z80_core/Cargo.toml
```

The root workspace has exact pins for its direct Serde and Windows dependencies;
Cargo may update only compatible transitive versions. The standalone Z80
workspace resolves PyO3 0.30.1 for Python 3.15.2. The audio-output workspace
has its own lockfile and must be resolved independently.

After updating, inspect every lockfile diff, then run:

```powershell
uv lock --check --python 3.15
python tools/project_manager.py locks
.\scripts\project.ps1 validate
```

Do not use locks -RefreshOffline as a substitute for an online version
upgrade: that command intentionally runs cargo check --offline from the
local cache and never performs cargo update.
