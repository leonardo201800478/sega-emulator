# Project completion contract — Sega Emulator

**Status:** permanent Draft pull request; this contract must not be finalized before the entire project is complete.  
**Normative roadmap:** [`docs/project-roadmap.md`](project-roadmap.md). This document does not create a second roadmap, phases, sprint plan, or competing priority order.  
**Architecture and scope:** [`docs/project-scope.md`](project-scope.md).  
**Fidelity policy:** [ADR-0016 — observable fidelity and proportional timing](architecture/adr/0016-observable-fidelity-and-proportional-timing.md).  
**Source of truth:** the latest integrated `main` SHA and its verified CI evidence.

## 1. Purpose and lifecycle

This contract is the project-wide final acceptance gate. It records what must be true before the Sega Emulator can be declared complete. It is intentionally kept in a Draft PR throughout development and must not be merged, closed as completed, or marked ready for review while any mandatory project phase or acceptance criterion remains open.

The contract is not a progress dashboard. Component-level implementation status, evidence, and active work remain in the canonical documents and in their implementation PRs. The roadmap remains the only source of phase order and priorities.

## 2. Non-negotiable architecture

- Hardware execution and hardware state belong to Rust.
- Python/PySide6 is limited to the frontend, launcher, library, configuration, logs, diagnostics, telemetry, and tooling.
- PyO3 exposes coarse-grained host operations; no Python callback per instruction, pixel, sample, or hardware tick.
- Share CPU cores and components when the underlying hardware is compatible; do not duplicate a CPU per console.
- The machine scheduler coordinates causally relevant domains and events. It does not force every component into per-tick or per-pin scheduling.
- No JIT, frame skipping, timing hacks, speculative optimization, or host-controlled hardware timing.
- Save states and replay must preserve the state needed for deterministic continuation.
- Commercial ROMs, proprietary BIOS/firmware, and protected dumps must never be committed.

## 3. Fidelity contract

Functional semantics are mandatory for every supported instruction and operation. The required execution granularity is proportional to observable behavior:

1. **Gameplay-sensitive temporal paths:** preserve order, duration, and interaction points when CPU, memory/bus, mapper, VDP/raster, audio, interrupts, DMA, or peripherals can affect gameplay according to timing.
2. **Atomic deterministic baseline:** operations without observable intermediate effects may execute atomically, while preserving exact results, flags, state transitions, side effects, determinism, and nominal duration where known.
3. M-cycles, T-states, WAIT, arbitration, resumable transactions, and internal phases are required only when a supported console/game/peripheral has a demonstrated dependency on them.
4. Pin-level, transistor-level, analog, and universal electrical simulation are not project completion requirements.

Every increase in temporal detail must identify the affected console/game/revision/peripheral, observable effect, evidence, minimum sufficient granularity, and regression test. Absence of unnecessary microsequencing is not a functional defect.

## 4. Final acceptance gates

The project may be declared complete only when **all** of the following are satisfied on the final integrated `main`:

- [ ] Every phase P0–P9 in the canonical roadmap has met its documented exit criteria, in the prescribed order.
- [ ] Every platform in scope (SG-1000, Master System, Game Gear, Mega Drive/Genesis, Sega CD, 32X and Sega CD 32X) has its required hardware runtime and host integration implemented; deferred or unsupported behavior is explicit rather than silently approximated.
- [ ] Shared CPU cores and reusable components are consolidated without per-console duplicates or parallel legacy hardware runtimes.
- [ ] Functional behavior is covered by unit, integration, diagnostic-ROM, and representative game tests appropriate to each platform.
- [ ] Gameplay-sensitive timing interactions have reproducible evidence and tests at the minimum sufficient temporal granularity. No universal pin-accuracy claim is required.
- [ ] Deterministic execution is demonstrated across continuous execution and chunked/frame execution wherever the public runtime supports both.
- [ ] Save-state save/load and deterministic replay restore all state required for continuation, including relevant device and scheduler state.
- [ ] Cross-domain CPU/video/audio/bus/interrupt/peripheral ordering is validated wherever those domains interact observably.
- [ ] Frontend, PyO3 boundary, packaging, supported host builds, and user-facing workflows meet the canonical roadmap's exit criteria; hardware timing remains outside Python.
- [ ] Compatibility and regression matrices for all supported platforms are current, with no unresolved release-blocking failures.
- [ ] Documentation, ADRs, component status, build instructions, and release notes describe the integrated implementation and no longer contain stale snapshots or contradictory requirements.
- [ ] All required quality gates pass on the exact final `main` SHA:
  ```text
  cargo fmt --all -- --check
  cargo clippy --workspace --all-targets --locked -- -D warnings
  cargo test --workspace --locked
  python -m pytest
  python -m ruff check .
  python -m ruff format --check .
  ```
- [ ] Required platform-specific CI, native build, and hardware/host integration jobs pass on that same SHA. No mandatory suite is SKIPPED, NOT RUN, or BLOCKED at final acceptance.
- [ ] Every open implementation PR required for the release is merged or explicitly rejected/closed with rationale; no required work remains only on an unmerged branch.
- [ ] The final repository audit confirms reproducible dependencies/lockfiles, clean architecture boundaries, no protected ROM/firmware assets, and no unresolved critical or high-severity defects.

Test results must be classified individually as PASS, FAIL, BLOCKED, or NOT RUN. SKIPPED is never PASS. Each result must identify the exact commit SHA and the jobs that actually ran.

## 5. Finalization procedure

When all gates above are satisfied:

1. Re-audit the remote `main`, all open PRs, workflows, release artifacts, and the canonical roadmap.
2. Re-run or verify every required gate against the exact final `main` SHA; rerun any check whose evidence belongs to another SHA.
3. Update the canonical component status and roadmap evidence without creating a parallel roadmap.
4. Complete this checklist with links to the evidence and record the final SHA.
5. Request final review, convert this Draft PR to ready only after the evidence is complete, and merge it only after all repository policies and CI requirements are satisfied.

Until then, this PR remains a permanent, non-mergeable-in-practice Draft completion gate. Its existence does not authorize bypassing normal branch, review, test, or merge policies.
