# CI Workflow Verification — 2026-11-07

**Repository:** `leonardo201800478/sega-emulator`
**Baseline `main` SHA audited:** `d48d5c35cd3469bad8f090f6a28094d6339560e8`
**Verification/fix PR:** [#628](https://github.com/leonardo201800478/sega-emulator/pull/628)
**Status:** targeted PR and path-filter verification passed; manual dispatch and post-merge push verification remain pending.

## 1. Consolidated changes in this delivery

| PR | Merge commit | Scope |
| --- | --- | --- |
| [#622](https://github.com/leonardo201800478/sega-emulator/pull/622) | `751c6c62a295c66e82a22c9918282ee29dd8f370` | GitHub Actions hardening: least-privilege permissions, actions pinned to immutable SHAs, checkout with `persist-credentials: false`, and impact-based CI policy. |
| [#624](https://github.com/leonardo201800478/sega-emulator/pull/624) | `40bddb93598517412980de50ba093e2edaa0dc89` | Checkout/history correction for `push` path classification. |
| [#623](https://github.com/leonardo201800478/sega-emulator/pull/623) | `d48d5c35cd3469bad8f090f6a28094d6339560e8` | Restoration of native SMS audio output and input/gamepad integration. |

The audio restoration retains the separate native audio-output workspace, WASAPI/XAudio2 integration, and SDL3 input runtime. The restoration's Windows CI passed; physical device operation is not inferred from compile/headless tests.

## 2. Defects found by impact smoke tests

Initial isolated PR tests against the pre-correction workflow exposed false-positive workspace runs:

| Isolated change | Initial run | Observed behavior | Assessment |
| --- | --- | --- | --- |
| Documentation only | [42375446328](https://github.com/leonardo201800478/sega-emulator/actions/runs/42375446328) | `docs=true` and incorrectly `other_rust=true`; Rust workspace ran. | FAIL — redundant validation |
| Z80-only | [42375452634](https://github.com/leonardo201800478/sega-emulator/actions/runs/42375452634) | `z80=true` and incorrectly `other_rust=true`; both focused and workspace suites ran. | FAIL — redundant validation |
| Audio-output only | [42375458556](https://github.com/leonardo201800478/sega-emulator/actions/runs/42375458556) | `audio=true` and incorrectly `other_rust=true`; Windows audio and workspace suites ran. | FAIL — redundant validation |

**Root cause:** `dorny/paths-filter` was using the default `predicate-quantifier: some` with negated patterns in `other_rust`. Under this mode, a file could match a negated rule and still be included. The upstream action documents `some-with-excludes` as the mode that makes exclusions final.

## 3. Corrections in PR #628

1. Set `predicate-quantifier: some-with-excludes` on the path classifier.
2. Keep the `changes` job successful on `workflow_dispatch`, while omitting only the path-filter step. This prevents dependent jobs from being skipped because their classifier prerequisite was skipped.
3. Make the aggregate gate check applicability explicitly:
   - expected suite: result must be `success`;
   - non-applicable suite: result must be `skipped`;
   - unexpected execution or missing required execution fails the gate.
4. Report non-applicable jobs as `NOT RUN`, never as `PASS`.
5. Update `docs/ci-validation-policy.md`.

## 4. Corrected path-isolation tests

All smoke PRs below were temporary, draft-only, targeted at the corrected workflow branch, and were not merged.

| Scenario | Test PR / run | Classifier result | Jobs selected | Result |
| --- | --- | --- | --- | --- |
| Docs-only | [#625 / 42375680917](https://github.com/leonardo201800478/sega-emulator/actions/runs/42375680917) | `docs=true`; all code-impact filters false | Documentation patch hygiene only | PASS |
| Z80-only | [#626 / 42375691667](https://github.com/leonardo201800478/sega-emulator/actions/runs/42375691667) | `z80=true`, `other_rust=false` | Z80 focused validation only | PASS |
| Audio-only | [#627 / 42375696222](https://github.com/leonardo201800478/sega-emulator/actions/runs/42375696222) | `audio=true`, `other_rust=false` | Windows WASAPI/XAudio2 only | PASS |
| Scheduler | [#629 / 42375772853](https://github.com/leonardo201800478/sega-emulator/actions/runs/42375772853) | `scheduler=true`, `other_rust=true` | Root workspace + deterministic Scheduler stress | PASS |
| PowerShell tooling | [#630 / 42375779186](https://github.com/leonardo201800478/sega-emulator/actions/runs/42375779186) | `tooling=true`; all code filters false | Parser for the changed PowerShell script only | PASS |
| Video + input + app union | [#631 / 42375859295](https://github.com/leonardo201800478/sega-emulator/actions/runs/42375859295) | `video=true`, `input=true`, `app=true`, `other_rust=true` | Root workspace + Windows D3D12/app + SDL3 input | PASS |

### Detailed evidence

- **Z80:** `cargo fmt --all -- --check`, targeted Clippy for `sega-z80-cpu`, `sega-z80-alu`, `sega-z80-bus`, `sega-sms-core`, `sega-sg1000-core`, then targeted tests. Result: **864 passed, 0 failed across 73 test executables**.
- **Audio:** WASAPI/XAudio2 Clippy and tests on `x86_64-pc-windows-msvc`. Result: **5 passed, 0 failed**.
- **Scheduler:** root workspace Clippy/tests plus deterministic stress replay. Result: **2 stress tests passed, 0 failed**; one ordinary non-ignored test was filtered by the explicit `--ignored` stress invocation.
- **PowerShell:** only `scripts/ci_impact_smoke_test.ps1` was parsed; parser returned PASS.
- **Host union:** root workspace, Windows SDL3 input, native egui application Clippy and D3D12/DXGI Clippy/tests all passed.

## 5. Full workflow validation

[PR #628 workflow run 42375595819](https://github.com/leonardo201800478/sega-emulator/actions/runs/42375595819) ran because the PR changes the workflow and therefore activates the broad/full validation path.

Result: **SUCCESS**.

- Change impact classification: PASS
- Rust workspace regression: PASS
- Documentation patch hygiene: PASS
- Windows D3D12 / DXGI and native app: PASS
- Windows WASAPI / XAudio2: PASS
- Windows SDL3 input: PASS
- PowerShell tooling syntax: PASS
- Z80 focused: NOT RUN (not applicable to this change)
- Scheduler stress: NOT RUN (not applicable to this change)
- Aggregate quality gate: PASS

The aggregate job log explicitly printed PASS for each required suite and NOT RUN for non-applicable suites.

## 6. Remaining verification boundaries

| Check | Status | Reason / next action |
| --- | --- | --- |
| PR-triggered impact matrix | PASS | Docs, Z80, audio, Scheduler, tooling and combined host scenarios were exercised. |
| Full PR validation | PASS | PR #628 full path passed. |
| Push event after the latest classifier correction | NOT RUN | Must be verified by the automatic CI run after PR #628 is merged into `main`. Earlier push run [42374351667](https://github.com/leonardo201800478/sega-emulator/actions/runs/42374351667) passed on the prior workflow revision. |
| Manual `workflow_dispatch` default run | NOT RUN | The workflow's manual path was corrected and its expressions parsed by PR CI, but this integration did not have a workflow-dispatch trigger operation. Trigger it from GitHub Actions after merge. |
| Manual lockfile-refresh artifact | NOT RUN | Requires explicit manual dispatch with `refresh_cargo_lockfiles=true`. |
| Physical GPU/audio/gamepad validation | NOT RUN in CI | These require actual host devices and must be recorded separately. |
| ZEXDOC / ZEXALL | NOT RUN | Remains blocked until the Z80 E7 semantic closure and independent conformance harness gate are satisfied. |

## 7. Merge and cleanup state

- Main remains at `d48d5c35cd3469bad8f090f6a28094d6339560e8` while PR #628 is under review.
- PR #628 contains the workflow correction and this verification report. It must be merged only after exact-HEAD checks remain green and merge authorization is confirmed.
- Smoke PRs #625, #626, #627, #629 and #630, plus #631, are validation-only artifacts and must remain unmerged; close them after their final successful runs.
- After PR #628 merges, verify the new `push` run on `main`, then perform the manual-dispatch checks and record their run IDs here.

## 8. Authoritative references

- [dorny/paths-filter — predicate quantifier](https://github.com/dorny/paths-filter#predicate-quantifier)
- [GitHub Actions — job dependencies and skipped jobs](https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-syntax#jobsjob_idneeds)
- [CI validation policy](ci-validation-policy.md)
