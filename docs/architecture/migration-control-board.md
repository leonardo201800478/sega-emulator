> **Estado corrente — 2026-10-29 (substitui snapshots históricos abaixo):** `main` está em `5d29b1790aa7e8d7af47de9ee2563a1c978b62d7`, após merge da [PR #157](https://github.com/leonardo201800478/sega-emulator/pull/157). SG-1000: marco funcional aprovado no escopo manual informado pelo mantenedor; CPU Z80 consolidada; os seis workflows do SHA candidato passaram. Não há arquivos alterados em `native/sms_core/` nessa PR. Precisão ciclo a ciclo permanece pendente. **Foco ativo: SMS**, com auditorias #158 (opcodes/semântica) e #159 (estado/interrupções/temporalidade). Os gates antigos deste documento são registros históricos de migração e precisam ser interpretados em conjunto com este estado atualizado; não use checkboxes antigos sem correlacioná-los ao código atual.

> **Marco funcional SG-1000 (2026-10-28):** concluído no escopo validado pelo mantenedor. PR [#141](https://github.com/leonardo201800478/sega-emulator/pull/141) integrada à `main` no commit `f51c0fac8dfd850a761e65de48709a2e7002d11d`; issue [#136](https://github.com/leonardo201800478/sega-emulator/issues/136) fechado. O mantenedor reportou PASS manual para os oito jogos afetados e informou que os demais jogos continuam funcionando como antes. Testes diferenciais e alguns checks Python continuam NOT RUN; workflows para o SHA de merge não foram confirmados. Isso não declara precisão ciclo a ciclo nem compatibilidade universal.
> **Estado vigente (2026-10-27):** hardware SG-1000 ativo exclusivamente em Rust. O código Python antigo de hardware, os testes/ferramentas dependentes e os workflows históricos foram arquivados sob `legacy/`; não são importados nem executados pelo projeto ativo. Este documento mantém observações históricas quando indicadas.

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
- [x] Enumerate and replace production consumers of the Python machine/device APIs.
- [ ] Identify all Cargo manifests, lockfiles, Maturin configuration, scripts, and workflows that reference native paths.
- [x] Record baseline test status before changing runtime behavior. The Rust SG-1000 workspace workflow passed on main SHA `632f2f69e7a9e6d9a81b19a4f994bb1ce42eb951` (Windows; fmt, Clippy, workspace tests): [run #131](https://github.com/leonardo201800478/sega-emulator/actions/runs/40442082201). This validates existing tests only, not full SG-1000 fidelity.

### Gate 1 — native crate layout
- [ ] Relocate `z80_alu`, `z80_bus`, and PyO3 adapter with no source loss.
- [ ] Update all path dependencies, workspace membership/exclusions, lockfiles, scripts, workflows, and docs atomically.
- [ ] Pass `cargo metadata --locked --format-version 1`.
- [ ] Pass `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, and `cargo test --workspace --locked`.
- [ ] Build the extension in Release mode and prove the imported extension path.

### Gate 2 — standalone SG-1000 runtime
- [x] CPU, memory/bus, I/O, VDP, PSG/audio, and machine coordination are Rust types independent of PyO3.
- [ ] Add independent regression vectors before behavior fixes.
- [ ] Compare CPU/flags/PC/SP/cycles, memory/I/O, VDP state/framebuffer, PSG samples, and repeatability.
- [ ] No Python callbacks in instruction, memory-byte, pixel, or sample loops.

### Gate 3 — production boundary and legacy isolation
- [x] Qt/PySide6 uses only the thin native API for emulation.
- [x] Missing native runtime fails explicitly; no silent Python-core fallback.
- [x] Moved old Python cores into `legacy/python_reference/` after replacing production imports.
- [x] Active tests/workflows do not import `legacy/`; the legacy tree is excluded from the package.
- [x] Pygame is archived; Qt is the only active GUI.

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
- **Status atualizado:** Python production cores are archived; the Qt application uses the Rust-only facade. Cycle-accurate timing remains an open fidelity goal.

- **Historical snapshot (2026-10-27):** [PR #139 — Bind PyO3 SG-1000 runtime to consolidated Rust core](https://github.com/leonardo201800478/sega-emulator/pull/139) was open and Draft at that time; this line records the state then, not the current PR state.
- **Architecture:** the Qt application uses a Rust-only PyO3 facade; CPU, memory/map, I/O, VDP, PSG and machine coordination are implemented in `native/sg1000_core`.
- **Legacy isolation:** Python hardware implementation, dependent tests/tools and Python-only audit workflows live under `legacy/`, outside the installed package and active test/workflow paths.
- **Validation still required:** all applicable workflows must pass on the same candidate SHA; Release/Maturin build and Qt integration must be verified; the eight ROMs from issue #136 and comparison against ARES remained untested at that historical PR snapshot. The eight-game manual result was later reported by the maintainer and recorded on 2026-10-28; differential comparison remains NOT RUN.
- **Fidelity boundary:** execution advances by instruction with nominal T-states. The crate is not declared cycle-accurate or universally compatible.
- Shared crate relocation and future platforms are separate scope; do not expand this PR into SMS/Game Gear or later consoles.

## Change log
- 2026-10-27: PR #126 expanded the native SG-1000 slice with a PyO3-free TMS9918A, VDP raster advancement, SN76489 clock/sample integration, and Z80 immediate IN/OUT routing. Before merge, verify the Rust workspace, Python CI, SMS fast validation, Z80 differential/extended audits, Qt backend validation, and sequential SG-1000 validation on the same final candidate SHA; see `sg1000-native-consolidation.md`.
- 2026-10-27: Step 1 added a pure-Rust SG-1000 I/O boundary for controller aliases and timestamped PSG write capture. It is intentionally not a complete CPU-port/VDP/audio-scheduler integration; see `sg1000-rust-io-step1.md`.
- 2026-10-28: SG-1000 functional milestone recorded after PR #141 merged into main. The eight-game manual result is maintainer-reported; differential testing and additional Python checks remain NOT RUN. The Master System is the next milestone; preserve SG-1000 regressions.
- 2026-10-27: Initial control board added. No source files moved and no tests claimed as passing.
- 2026-10-27: Corrected the excluded shared-core scaffold's workspace registration and sibling dependency paths so the manifest can be validated independently.
