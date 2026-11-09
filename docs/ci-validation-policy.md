# CI Validation Policy

**Scope:** all project phases and platforms (P0–P9).

The CI pipeline is impact-based: it runs the smallest validation set that covers the changed code and its real dependencies. A change must not trigger unrelated host, graphics, or audio builds merely because those components exist in the repository. The path classifier uses `predicate-quantifier: some-with-excludes` whenever a filter combines positive and negated patterns; without that mode, negated patterns can match unrelated files and cause false-positive jobs. PSG, VDP, and SDL3 input paths have dedicated package-level jobs; these jobs replace root-workspace regression for isolated changes in those components.

## Validation matrix

| Change area | Required automated validation |
| --- | --- |
| Z80 CPU, ALU, bus, SMS/SG-1000 CPU integration | Package-scoped rustfmt, Clippy, unit and integration tests for `sega-z80-cpu`, `sega-z80-alu`, `sega-z80-bus`, `sega-sms-core`, and `sega-sg1000-core`. CPU/ALU/bus integration sources in both console cores and Z80-specific SMS tests are classified into this focused path. The job also checks out SingleStepTests/Z80 at pinned revision `ebe1875d48f374bcfd4b505d8eb8ee751568b5f7` and executes the 32,000 DD/FD half-register ALU vectors, 2,000 DD/FD SCF/CCF Q-latch vectors, and 2,000 ED77/ED7F reserved-opcode vectors against the resumable Z80 (36,000 vectors total); it compares architectural state and aggregate T-states, not electrical pin traces. The upstream corpus is Ares-derived and is not a silicon oracle. Do not run unrelated workspace or host-platform builds for Z80-only changes.
| Other root-workspace Rust components | Root workspace formatting, Clippy, unit and integration tests |
| PSG / audio integration | Package-scoped rustfmt, Clippy, and tests for `sega-sn76489`, `sega-audio-runtime`, `sega-sms-core`, and `sega-sg1000-core`; Windows WASAPI/XAudio2 and Linux audio-output validation for the host/dependency fan-out. |
| VDP / video integration | Package-scoped rustfmt, Clippy, and tests for `sega-video`, `sega-sms-core`, and `sega-sg1000-core`; Windows D3D12/DXGI validation for video-core/runtime changes. |
| SDL3 input integration | Package-scoped rustfmt, Clippy, and tests for `sega-input-runtime`, `sega-sms-core`, and `sega-sg1000-core`; Windows SDL3 input validation for input-runtime changes. |
| Machine Scheduler / temporal core | Root workspace validation plus deterministic Scheduler stress replay when `native/machine_scheduler` or `native/temporal_core` changes; audio and application consumers are validated when their dependency graph is affected. |
| Native egui application and SMS worker integration | Windows-targeted application format, Clippy, and tests run for the application and its non-Z80 runtime dependencies: SMS runtime/core, video, audio, input, Scheduler/temporal core, and observability. Z80-only CPU/ALU/bus integration sources and Z80-specific tests in SMS core are excluded from this app fan-out so a CPU-only correction does not start the Windows host build. |
| PowerShell tooling | Parse only the changed `.ps1` scripts |
| Documentation text (Markdown, MDX, RST, TXT) | `git diff --check` for whitespace and patch hygiene only; no Rust build, Clippy, or tests |


## Dependency fan-out

1. Shared contracts and libraries must trigger validation for their actual consumers. In particular, `native/observability` fans out to video, audio, input, and application consumers; `native/machine_scheduler` and `native/temporal_core` fan out to scheduler stress, audio runtime, and application consumers. PSG, VDP, and input console-integration source files are excluded from the generic workspace classifier only when their dedicated focused job covers them.
2. A change spanning multiple components runs the union of those components' required suites.
3. Changes to global Cargo configuration, the root workspace manifest/lockfile, root `build.rs`, Rust toolchain configuration, common build scripts, or CI workflows trigger broad regression.
4. A component-local manifest or lockfile change triggers that component and its real dependents, not every unrelated Windows host validation by default.
5. A Z80-only change runs the focused Z80/SMS/SG-1000 package suite (format, Clippy, unit and integration tests). It does not trigger root-workspace regression, D3D12, audio, SDL3, or tooling jobs unless the changed paths independently require them. If the patch also changes shared root-workspace code, manifests, lockfiles, toolchain/configuration, or CI workflows, run the broader validations selected by those paths.
6. D3D12, WASAPI/XAudio2, and SDL3 checks are independent. They run only when their code or an affected integration requires them.

## Determinism and hardware validation

- Scheduler stress replay is required for Scheduler/temporal-core changes and may be explicitly requested with the `scheduler-stress-short` or `scheduler-stress-long` PR label.
- Physical hardware checks (GPU presentation, audible WASAPI/XAudio2 output, and real gamepad behavior) are recorded separately. A successful compile or headless test is not evidence of physical operation.
- Do not add unrelated platform builds as a substitute for targeted tests.

## Retained project-specific gates

The standalone `native/audio_output` and `native/sega_app` workspaces retain their own lockfiles and receive targeted Windows x64 checks. The ZEXDOC/ZEXALL external conformance suites remain **NOT RUN** until their independent Rust harness is completed; existing semantic, bus, microsequencing and temporal tests do not replace them.

## Result classification and merge gate

Every applicable suite is classified as `PASS`, `FAIL`, `BLOCKED`, or `NOT RUN`. A skipped suite is never reported as a pass.

The aggregate `Rust quality and unit regression` check explicitly verifies expected applicability: required jobs must conclude `success`, while non-applicable jobs must conclude `skipped`. Unexpected executions and missing required executions both fail the aggregate gate. For Z80-only changes, the focused package job is required and the root workspace job is explicitly not applicable. Manual dispatch runs the classifier job successfully with the path-filter step omitted, so dependent manual jobs are not blocked by a skipped prerequisite. A PR is eligible for merge only when all required checks pass on the exact final HEAD, with no unresolved correctness findings or merge conflicts.
