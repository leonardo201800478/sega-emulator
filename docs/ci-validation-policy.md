# CI Validation Policy

**Scope:** all project phases and platforms (P0–P9).

The CI pipeline is impact-based: it runs the smallest validation set that covers the changed code and its real dependencies. A change must not trigger unrelated host, graphics, or audio builds merely because those components exist in the repository. The path classifier uses `predicate-quantifier: some-with-excludes` whenever a filter combines positive and negated patterns; without that mode, negated patterns can match unrelated files and cause false-positive jobs.

## Validation matrix

| Change area | Required automated validation |
| --- | --- |
| Z80 CPU, ALU, bus, SMS/SG-1000 CPU integration | Package-scoped rustfmt, Clippy, unit and integration tests for `sega-z80-cpu`, `sega-z80-alu`, `sega-z80-bus`, `sega-sms-core`, and `sega-sg1000-core`. CPU/ALU/bus integration sources in both console cores and Z80-specific SMS tests are classified into this focused path. Do not run unrelated workspace or host-platform builds for Z80-only changes. |
| Other root-workspace Rust components | Root workspace formatting, Clippy, unit and integration tests |
| Machine Scheduler / temporal core | Root workspace validation plus deterministic Scheduler stress replay when `native/machine_scheduler` or `native/temporal_core` changes |
| Video core / VDP and video runtime | Root workspace validation. Windows D3D12/DXGI validation runs for `native/video_core`, `native/video_runtime`, and the shared observability dependency. |
| Audio chips / DSP / audio runtime | Root workspace validation. Windows WASAPI/XAudio2 and Linux audio-output validation run for `native/sn76489`, `native/audio_runtime`, `native/audio_output`, Scheduler/temporal dependencies used by audio runtime, and shared observability. |
| SDL3 input runtime | Root workspace validation plus Windows SDL3 input validation. Shared observability changes also validate this consumer. |
| Native egui application and SMS worker integration | Windows-targeted application format, Clippy, and tests run for the application and its non-Z80 runtime dependencies: SMS runtime/core, video, audio, input, Scheduler/temporal core, and observability. Z80-only CPU/ALU/bus integration sources and Z80-specific tests in SMS core are excluded from this app fan-out so a CPU-only correction does not start the Windows host build. |
| PowerShell tooling | Parse only the changed `.ps1` scripts |
| Documentation text (Markdown, MDX, RST, TXT) | `git diff --check` for whitespace and patch hygiene only; no Rust build, Clippy, or tests |


## Dependency fan-out

1. Shared contracts and libraries must trigger validation for their actual consumers. In particular, `native/observability` fans out to video, audio, input, and application consumers; `native/machine_scheduler` and `native/temporal_core` fan out to audio runtime and application consumers.
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
