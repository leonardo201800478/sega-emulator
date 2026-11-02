# ADR-0013 — Snapshot/Restore Validation Evidence

Validation ledger captured 2026-11-02. GitHub is the source of truth; every PASS below is tied to the exact implementation SHA and workflow run shown.

## Repository baseline

- Main branch SHA: `0da3098893cce4d80f7a22291d00488e4e7d0f9b`
- Main was not modified.
- Superseded implementation PRs #339, #341, #343–#350 and #352–#354 were closed after their changes were represented in the consolidated #358 branch; none was merged.
- #351 (ADR-0014), #355 (documentation reconciliation), #356 (technical dossier/handoff) and #358 (consolidated Scheduler stack) remain open Draft PRs at the time of this ledger update.
- PR #358 targets `main`; exact head and CI evidence are recorded below. No PR was merged.

## Exact-SHA CI ledger

| PR | Scope | Implementation SHA | Workflow run | Rust fmt | Clippy | Rust tests | Ruff fmt | Ruff lint | Pytest | Notes |
|---|---|---|---:|---|---|---|---|---|---|---|
| #343 | Scheduler CI baseline | `170f03bba3a36b0bc45329a374491ebfb6dfdd61` | 41355467670 | PASS | PASS | PASS | PASS | PASS | PASS | Supporting dependency |
| #344 | Clock registration contract | `0c5fcda97cc5a99c43a89b55a0fe56e1d2f3d5ac` | 41355902030 | PASS | PASS | PASS | PASS | PASS | PASS | Supporting dependency |
| #345 | Exact clock-edge time | `ff628cb479dc70b6b7a6b8cc734eed5007bf306d` | 41356120284 | PASS | PASS | PASS | PASS | PASS | PASS | Supporting dependency |
| #346 | Delta-event budget | `ec2bd69d32409490901d30dc58ff94779eda061b` | 41356994362 | PASS | PASS | PASS | PASS | PASS | PASS | Supporting dependency |
| #347 | ADR-0013 documentation | `fc3fb1e014324f025fb05447e29e7ac8b2c890b0` | 41358326800 | PASS | PASS | PASS | PASS | PASS | PASS | Documentation |
| #348 | E1 — Snapshot DTOs | `02f71b830ddd47156eb69d78de7b8107f851ec41` | 41360589305 | PASS | PASS | PASS | PASS | PASS | PASS | E1 |
| #349 | E2 — ClockDomain restore | `356a7eb4a022e898b87c04c1ca61140056f47d14` | 41361087461 | PASS | PASS | PASS | PASS | PASS | PASS | E2 |
| #350 | E3 — EventQueue restore | `d84e588a470c09655ae3c424a094d9f2c98e9a58` | 41368745114 | PASS | PASS | PASS | PASS | PASS | PASS | E3 implementation SHA; branch head `a2e21ca310f7b73e95d4d75700ca1e032cf1cc07` is a documentation-only reconciliation commit and has no separate exact-head workflow |
| #351 | ADR-0014 synchronization | `2b577e96e467d170e6eef537a51cee4eced8ee70` | 41369576783 | PASS | PASS | PASS | PASS | PASS | PASS | Exact current head |
| #352 | E4 — Atomic Scheduler restore | `9c506be48982c20c98977e71aef71eb8f1cf3031` | 41369507172 | PASS | PASS | PASS | PASS | PASS | PASS | E4 implementation SHA; branch head `c5f3c76f1fdb833576153f22da8d66a4107f1f0b` contains documentation-only reconciliation and has no separate exact-head workflow |
| #353 | E5 — Versioned state codec | `a33065fe14c09ee1840f5c9abb7e9ff0730457d2` | 41370192970 | PASS | PASS | PASS | PASS | PASS | PASS | Implementation SHA |
| #353 | E5 — Final PR head | `839fdaa25137a91916057c585a7cd81a9bcdb305` | 41370298546 | PASS | PASS | PASS | PASS | PASS | PASS | Exact current head |
| #354 | E6 — Differential replay tests | `f724479832fc0e3f051c37ec23b226056858019c` | 41370554220 | PASS | PASS | PASS | PASS | PASS | PASS | Implementation SHA |
| #354 | E6 — Pre-ledger head | `e89fe8619e5f7798e3b4e44f29c5297cef7f1d6b` | 41370622762 | PASS | PASS | PASS | PASS | PASS | E6 code and ADR head immediately before adding this evidence ledger |
| #358 | Consolidated Scheduler stack + stress harness | `ca349d304942d105fcef4bf133be8be2c79c8ed6` | 41377566361 | PASS | PASS | PASS | PASS | PASS | PASS | Exact PR head; stress profile job SKIPPED on PR event; SHORT_PASS/LONG_PASS NOT RUN |

The #354 evidence-ledger commit was `11378882ea73a7307430dcb52389709fdea2a509`, validated by workflow `41370823497` on that exact SHA. The current consolidated PR #358 head `ca349d304942d105fcef4bf133be8be2c79c8ed6` passed all six ordinary gates in workflow `41377566361`; the opt-in stress job was skipped because this was a pull-request event.

## Required workflow gates

The successful runs above executed these checks:

- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets --locked -- -D warnings`
- `cargo test --workspace --locked`
- `python -m ruff format --check .`
- `python -m ruff check .`
- `python -m pytest`

No warning was accepted as a pass. A failed earlier candidate is not promoted: PR #352 head `1251af366db0687753a7ca5bc465051a28f7a27e` failed Clippy and was corrected at implementation SHA `9c506be48982c20c98977e71aef71eb8f1cf3031`. PR #353 intermediate candidates also had formatting failures; the final implementation and final PR head both passed.

## Component status

| Component | Implementation | QUICK_PASS | SHORT_PASS | LONG_PASS | INTEGRATION_PENDING | TEMPORAL_PASS |
|---|---|---|---|---|---|---|
| E1 DTOs | IMPLEMENTED | PASS | NOT RUN | NOT RUN | YES | NOT CLAIMED |
| E2 ClockDomain | IMPLEMENTED | PASS | NOT RUN | NOT RUN | YES | NOT CLAIMED |
| E3 EventQueue | IMPLEMENTED | PASS | NOT RUN | NOT RUN | YES | NOT CLAIMED |
| E4 Scheduler atomic restore | IMPLEMENTED | PASS | NOT RUN | NOT RUN | YES | NOT CLAIMED |
| E5 Versioned codec | IMPLEMENTED | PASS | NOT RUN | NOT RUN | YES | NOT CLAIMED |
| E6 Differential replay | IMPLEMENTED | PASS | NOT RUN | NOT RUN | YES | NOT CLAIMED |
| E7 Evidence consolidation | IMPLEMENTED | PASS | NOT RUN | NOT RUN | YES | NOT CLAIMED |

## Limitations and remaining work

- SHORT_PASS and LONG_PASS smoke/stress campaigns were not run as part of this snapshot/restore stack.
- No independent hardware oracle or electrical timing validation was performed.
- E6 proves deterministic equivalence for the explicit scheduler test scenarios; it does not prove correctness of device timing or full machine integration.
- E7 records CI and code evidence. It does not imply merge approval.
- `TEMPORAL_PASS` remains unclaimed until broader temporal integration and independent oracles exist.

## Branch and PR safety

- No direct commit to `main`.
- No force-push or destructive reset.
- No merge or PR closure.
- PR #358 remains Draft for review; superseded PRs listed above are closed, not merged.


---

## Addendum — consolidated Scheduler and multi-domain evidence (2026-11-02)

This addendum supersedes the earlier #358 head/check references above where they differ. Historical rows remain as provenance for the earlier candidate SHAs; they must not be interpreted as evidence for the current heads.

### Current PR stack

| PR | Role | Base | Head SHA | State |
|---|---|---|---|---|
| [#358](https://github.com/leonardo201800478/sega-emulator/pull/358) | Consolidated Temporal Core + Machine Scheduler, ADRs, evidence and stress harness | `main` | `0d8ad97e79ac50c4687e1400dd5bb434ca5f3607` | Open, Draft, not merged |
| [#359](https://github.com/leonardo201800478/sega-emulator/pull/359) | Multi-domain advancement/replay tests | #358 branch | `a544fe367f67c1f6c6d622efcdb528e222ab53c3` | Open, Draft, not merged |

The #359 branch is stacked on #358. Its tests cover integer `MachineTime` boundaries only. Do not retarget, rebase, merge, or close either PR without an explicit dependency plan and authorization.

### Exact-SHA validation

**PR #358 — head `0d8ad97e79ac50c4687e1400dd5bb434ca5f3607`**

- Ordinary CI workflow [41378338137](https://github.com/leonardo201800478/sega-emulator/actions/runs/41378338137): Rust fmt, Clippy with `-D warnings`, Rust workspace tests, Python Ruff format, Ruff lint and pytest all PASS on this exact SHA.
- SHORT workflow [41379180394](https://github.com/leonardo201800478/sega-emulator/actions/runs/41379180394): the stress test stage completed successfully for 4 fixed seeds × 20,000 events = 80,000 events under 512 MiB; 1 test passed. The overall workflow was **cancelled**, and the Rust workspace test stage did not complete. Therefore record SHORT_PASS only for the stress stage, not for the workflow as a whole.
- LONG workflow [41379203192](https://github.com/leonardo201800478/sega-emulator/actions/runs/41379203192): all three jobs succeeded. The stress harness processed 16 fixed seeds × 250,000 events = 4,000,000 events under a 2 GiB limit; 1 stress test passed. LONG_PASS applies to this harness and exact SHA.

**PR #359 — head `a544fe367f67c1f6c6d622efcdb528e222ab53c3`**

- CI workflow [41381005224](https://github.com/leonardo201800478/sega-emulator/actions/runs/41381005224): Python lint/regression and Rust quality/unit regression jobs succeeded on this exact SHA. The Scheduler stress job was skipped by the PR workflow condition; no stress result is implied for #359.
- `multi_domain_replay.rs` adds:
  - three rational clock domains registered out of ID order;
  - duplicate event timestamps and explicit phase/priority ordering;
  - fixed expected dispatch token order `[0, 1, 2, 4, 3, 5, 6]`;
  - uninterrupted versus repeated snapshot/restore comparison;
  - irregular idle chunks checked at intermediate timestamps against an independent integer-rational arithmetic oracle (not `ClockDomain`);
  - atomic preflight overflow assertion for the exact `SchedulerError::Clock(ClockError::ArithmeticOverflow)` variant, including unchanged exported state and pending event.
- These tests establish software-level multi-domain advancement/replay properties at integer machine timestamps. They do not establish sub-tick dispatch, device safe points, bus arbitration or hardware timing.

### Stress-harness interpretation

The #358 SHORT/LONG harness schedules events directly using `MachineTime`. Registered clock domains are persisted and advanced by scheduler dispatch, but the stress workload does not generate events by stepping independent device clocks. Consequently, the stress results validate deterministic event replay/restore under the stated workloads, not end-to-end cross-domain device interactions. PR #359 separately adds focused multi-domain advancement coverage, but still does not integrate hardware devices.

### Current component status

| Area | Implementation | QUICK_PASS | SHORT_PASS | LONG_PASS | Integration | TEMPORAL_PASS |
|---|---|---|---|---|---|---|
| Temporal Core / Scheduler E1–E7 | IMPLEMENTED in PR stack | PASS on recorded exact SHAs | #358 stress stage PASS; workflow cancelled | PASS on #358 exact SHA | INTEGRATION_PENDING | NOT CLAIMED |
| Multi-domain advancement/replay tests | IMPLEMENTED in #359 | CI PASS on exact head | NOT RUN | NOT RUN | INTEGRATION_PENDING | NOT CLAIMED |
| SMS/Z80/bus/VDP/PSG integration | NOT IMPLEMENTED by this stack | NOT RUN | NOT RUN | NOT RUN | PENDING | NOT CLAIMED |

### Next engineering milestone

Design and evidence-gate the first real SMS temporal integration boundary. Before implementation:
1. Re-inventory current remote SMS CPU, bus, VDP and PSG execution paths and the current issue #158 ledger.
2. Read the existing SMS architecture/audit documents and relevant ADRs.
3. Establish primary-source evidence for clock relationships, bus-cycle visibility, wait states/contention and interrupt timing. Do not infer physical priorities from the scheduler's ordinal fields.
4. Define a minimal, testable CPU/bus/scheduler contract and acceptance traces, including an independent oracle.
5. Keep device-specific timing out of generic Scheduler crates and keep Python outside the timing loop.

No device timing implementation, merge, or `TEMPORAL_PASS` is authorized by this documentation update.
