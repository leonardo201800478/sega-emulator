# CI validation policy — Rust-only migration

## Required Rust gates

The pull-request workflow validates the active root Cargo workspace with the pinned Rust toolchain:

- `cargo metadata --locked --format-version 1 --no-deps`;
- `cargo fmt --all -- --check`;
- `cargo clippy --workspace --all-targets --locked -- -D warnings`;
- `cargo test --workspace --locked`.

The standalone `native/audio_output` and `native/sega_app` workspaces retain their own lockfiles and receive Windows x64 checks until they are consolidated under `rust/`.

## Windows host validation

The Windows workflow validates the egui shell, the SMS temporal runtime with D3D12 enabled, the video runtime, and WASAPI/XAudio2 output. Compile/Clippy success is not a substitute for physical-device validation. GPU presentation, actual audio output, input latency and shutdown behavior must be classified separately.

The PowerShell tooling job parses the maintained scripts. The scripts invoke Cargo directly; no language runtime or binding build is required.

## Removed legacy validation

The Python/Qt/PyO3 jobs and their workflows were removed together with the corresponding source tree and extension crate. Their former test results are historical only and are not part of the current gate.

The Python orchestration for external Z80 conformance was removed. A low-level Rust test binary remains in the SMS core but is not yet an independent, complete harness: it still depends on SMS compatibility exports and requires the upstream-suite acquisition/assembly/preflight pipeline. ZEXDOC/ZEXALL are **NOT RUN** until that harness is relocated to the shared Z80 crate and completed in Rust. Existing Rust semantic, bus, microsequencing and temporal tests remain active; they do not replace those external suites.

## Result classification

Every required suite is reported as PASS, FAIL, BLOCKED or NOT RUN. Skipped, cancelled, unavailable or unexecuted checks never count as PASS. Results are valid only for the exact commit SHA being reviewed. A green Linux workspace does not imply Windows host or physical-device validation.
