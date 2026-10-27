# Migration control board — Clean Architecture

This board is the source of truth for migration sequencing and evidence. GitHub Actions are validators, not autonomous AI reviewers. A phase advances only when its evidence is recorded against the same candidate SHA.

## Roles (independent validation lanes)

| Lane | Responsibility | Required evidence |
|---|---|---|
| Architecture / dependency audit | Check dependency direction, crate ownership, path consistency, and no duplicate runtime | Cargo tree/metadata, manifest review, import graph |
| Functional regression | Validate behavior changed by a slice | Focused unit/integration tests and regression tests |
| Build / package integrity | Prove the intended native extension is built and imported | locked Cargo checks, Release/Maturin build, extension path assertion |
| Determinism / differential | Compare repeatable checkpoints against independent reference behavior | fixed vectors, first-divergence report, repeated-run comparison |
| Performance / boundary audit | Ensure Python boundary is coarse-grained and compare like-for-like timings | call-boundary inspection and benchmark baseline/candidate |
| Integration / UI | Validate Qt-facing API, ROM lifecycle, reset/input/audio/video, and error handling | integration tests and Windows build evidence |

A CI workflow only validates the commands it actually executes. A green Python workflow is not evidence that Rust compiled; a successful extension build is not evidence of hardware fidelity.

## Phase gates

### Gate 0 — inventory and contracts
- [x] Migration plan recorded in `clean-architecture-migration.md`.
- [ ] Enumerate every consumer of the current Python machine/device APIs.
- [ ] Identify all Cargo manifests, lockfiles, Maturin configuration, scripts, and workflows that reference native paths.
- [x] Record baseline test status before changing runtime behavior. The Rust SG-1000 workspace workflow passed on main SHA `632f2f69e7a9e6d9a81b19a4f994bb1ce42eb951` (Windows; fmt, Clippy, workspace tests): [run #131](https://github.com/leonardo201800478/sega-emulator/actions/runs/40442082201). This validates existing tests only, not full SG-1000 fidelity.

### Gate 1 — native crate layout
- [ ] Relocate `z80_alu`, `z80_bus`, and PyO3 adapter with no source loss.
- [ ] Update all path dependencies, workspace membership/exclusions, lockfiles, scripts, workflows, and docs atomically.
- [ ] Pass `cargo metadata --locked --format-version 1`.
- [ ] Pass `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, and `cargo test --workspace --locked`.
- [ ] Build the extension in Release mode and prove the imported extension path.

### Gate 2 — standalone SG-1000 runtime
- [ ] CPU, memory/bus, I/O, VDP, PSG/audio, and scheduler integrated as pure Rust types independent of PyO3.
- [ ] Add independent regression vectors before behavior fixes.
- [ ] Compare CPU/flags/PC/SP/cycles, memory/I/O, VDP state/framebuffer, PSG samples, and repeatability.
- [ ] No Python callbacks in instruction, memory-byte, pixel, or sample loops.

### Gate 3 — production boundary and legacy isolation
- [ ] Qt/PySide6 uses only the thin native API for emulation.
- [ ] Missing native runtime fails explicitly; no silent Python-core fallback.
- [ ] Move old Python cores into `legacy/python_reference/sega_emulator_reference/` only after all production imports are replaced.
- [ ] Tests import legacy reference modules explicitly; distribution excludes legacy.
- [ ] Decide and document whether Pygame is migrated to the same API or classified as legacy.

### Gate 4 — later platforms
- [ ] Start SMS/Game Gear only after Gate 2 and Gate 3 pass for SG-1000.
- [ ] Mega Drive, Sega CD, and 32X remain out of scope until their prerequisite milestone passes.

## Current verified baseline
- Branch: `refactor/native-core-layout`
- Baseline SHA: `2e93350d2a27dd0f37671ccca1df11b5b6654a55`
- No PR exists at this baseline.
- The shared crates and PyO3 adapter remain in their old directories.
- `native/sega_emu_core` remains a scaffold outside the root workspace; its path dependencies now point to the existing sibling crates. The shared-crate relocation is still pending.
- The Rust SG-1000 and SMS crates remain partial. No fidelity or standalone-runtime acceptance is implied.
- Baseline crate validation: **PASS** on main SHA `632f2f69e7a9e6d9a81b19a4f994bb1ce42eb951` via [Rust SG-1000 Workspace run #131](https://github.com/leonardo201800478/sega-emulator/actions/runs/40442082201): formatting, Clippy, and workspace tests succeeded; logs show 22 SG-1000 tests passed, plus 15 SMS and 7 Z80 ALU tests. This run predates the migration branch's documentation-only commits and validates the same SG-1000 source blobs.
- Historical baseline note (before PR #126): VDP, PSG and machine-wide scheduling were not integrated. The current status is recorded below; the crate is still not a complete standalone emulator.
- Do not move Python production cores to `legacy/` yet: the current application still depends on those modules, and the Rust crate does not replace all production contracts.

## Current migration status — PR #126

- Branch: `refactor/sg1000-native-core-consolidation`.
- The branch internalizes the Z80 ALU and restores the shared `sega-z80-bus` contract used by SG-1000 and SMS.
- `sg1000_core/src/vdp.rs` is now Python-independent: TMS9918A state, VRAM/control ports, scanline/VBlank timing, framebuffer rendering, and sprite helpers live in Rust types.
- `Sg1000Machine` advances the VDP and SN76489 from nominal instruction T-states; CPU `IN A,(n)` and `OUT (n),A` are wired through the machine bus.
- The PSG default clock profile is configurable but provisional; instruction-boundary timing is not cycle-exact.
- PR #126 acceptance is an incremental Rust slice: native-core regressions, workspace quality checks, determinism checks, and review on the final candidate SHA. This PR does not complete Gate 2.
- The standalone gate remains open until the Z80 instruction/prefix/interrupt implementation is complete, hardware clocks and VDP behavior are checked against independent references, and the PyO3/Qt boundary passes Release/Maturin and parity validation. Do not claim standalone completion or move Python production cores to `legacy/` before those gates pass.

## Change log
- 2026-10-27: PR #126 expanded the native SG-1000 slice with a PyO3-free TMS9918A, VDP raster advancement, SN76489 clock/sample integration, and Z80 immediate IN/OUT routing. Before merge, verify the Rust workspace, Python CI, SMS fast validation, Z80 differential/extended audits, Qt backend validation, and sequential SG-1000 validation on the same final candidate SHA; see `sg1000-native-consolidation.md`.
- 2026-10-27: Step 1 added a pure-Rust SG-1000 I/O boundary for controller aliases and timestamped PSG write capture. It is intentionally not a complete CPU-port/VDP/audio-scheduler integration; see `sg1000-rust-io-step1.md`.
- 2026-10-27: Initial control board added. No source files moved and no tests claimed as passing.
- 2026-10-27: Corrected the excluded shared-core scaffold's workspace registration and sibling dependency paths so the manifest can be validated independently.
