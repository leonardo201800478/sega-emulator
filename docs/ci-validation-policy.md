# CI Validation Policy

**Scope:** all project phases and platforms (P0–P9).

The CI pipeline is impact-based: it runs the smallest validation set that covers the changed code and its real dependencies. A change must not trigger unrelated host, graphics, or audio builds merely because those components exist in the repository. The path classifier uses `predicate-quantifier: some-with-excludes` whenever a filter combines positive and negated patterns; without that mode, negated patterns can match unrelated files and cause false-positive jobs.

## Validation matrix

| Change area | Required automated validation |
| --- | --- |
| Z80 CPU, ALU, bus, SMS/SG-1000 CPU integration | Rust formatting; Clippy and tests for `sega-z80-cpu`, `sega-z80-alu`, `sega-z80-bus`, `sega-sms-core`, and `sega-sg1000-core`. CPU-focused validation is selected from Z80 implementation, SMS/SG-1000 temporal integration, and Z80 conformance-test paths. |
| Other root-workspace Rust components | Root workspace formatting, Clippy, unit and integration tests |
| Machine Scheduler / temporal core | Root workspace validation plus deterministic Scheduler stress replay when `native/machine_scheduler` or `native/temporal_core` changes |
| Video core / VDP | Root workspace validation. D3D12/DXGI Windows validation runs only for `native/video_runtime` or the native application (including its SMS runtime dependency). |
| Audio chips / DSP / audio runtime | Root workspace validation where applicable. Windows WASAPI/XAudio2 validation runs for `native/audio_output`, `native/audio_runtime`, or the shared SN76489 dependency. |
| SDL3 input runtime | Root workspace validation plus Windows SDL3 input validation |
| Native egui application and SMS worker integration | Windows-targeted format and Clippy for all targets; changes to `rust/runtime/sms-runtime` also trigger this dependent application build. A separate `cargo test` is added when the application gains its own tests. |
| PowerShell tooling | Parse only the changed `.ps1` scripts |
| Documentation text (Markdown, MDX, RST, TXT) | `git diff --check` for whitespace and patch hygiene only; no Rust build, Clippy, or tests |


## Dependency fan-out

1. Shared contracts and libraries must trigger validation for their actual consumers.
2. A change spanning multiple components runs the union of those components' required suites.
3. Changes to global Cargo configuration, the root workspace manifest/lockfile, Rust toolchain configuration, or CI workflows trigger broad regression.
4. A component-local manifest or lockfile change triggers that component's validation, not a full Windows matrix by default.
5. A Z80-focused change also validates SMS and SG-1000 integration. If unrelated root-workspace code is changed in the same patch, both focused and workspace validation run.
6. D3D12, WASAPI/XAudio2, and SDL3 checks are independent. They run only when their code or an affected integration requires them.

## Determinism and hardware validation

- Scheduler stress replay is required for Scheduler/temporal-core changes and may be explicitly requested with the `scheduler-stress-short` or `scheduler-stress-long` PR label.
- Physical hardware checks (GPU presentation, audible WASAPI/XAudio2 output, and real gamepad behavior) are recorded separately. A successful compile or headless test is not evidence of physical operation.
- Do not add unrelated platform builds as a substitute for targeted tests.

## Retained project-specific gates

The standalone `native/audio_output` and `native/sega_app` workspaces retain their own lockfiles and receive targeted Windows x64 checks. The ZEXDOC/ZEXALL external conformance suites remain **NOT RUN** until their independent Rust harness is completed; existing semantic, bus, microsequencing and temporal tests do not replace them.

## Result classification and merge gate

Every applicable suite is classified as `PASS`, `FAIL`, `BLOCKED`, or `NOT RUN`. A skipped suite is never reported as a pass.

The aggregate `Rust quality and unit regression` check explicitly verifies expected applicability: required jobs must conclude `success`, while non-applicable jobs must conclude `skipped`. Unexpected executions and missing required executions both fail the aggregate gate. Manual dispatch runs the classifier job successfully with the path-filter step omitted, so dependent manual jobs are not blocked by a skipped prerequisite. A PR is eligible for merge only when all required checks pass on the exact final HEAD, with no unresolved correctness findings or merge conflicts.
