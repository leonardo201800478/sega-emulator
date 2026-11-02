# ADR-0013 — Snapshot/Restore Validation Evidence

Validation ledger captured 2026-11-02. GitHub is the source of truth; every PASS below is tied to the exact implementation SHA and workflow run shown.

## Repository baseline

- Main branch SHA: `0da3098893cce4d80f7a22291d00488e4e7d0f9b`
- Main was not modified.
- PRs #343–#354 remain open and Draft; no PR was merged or closed.
- All PRs #343–#354 reported mergeable at the final inventory, except mergeability may be recalculated by GitHub after subsequent branch updates.

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
| #354 | E6 — Pre-ledger head | `e89fe8619e5f7798e3b4e44f29c5297cef7f1d6b` | 41370622762 | PASS | PASS | PASS | PASS | PASS | PASS | E6 code and ADR head immediately before adding this evidence ledger |
| #354 | E7 — Final evidence-ledger head | `11378882ea73a7307430dcb52389709fdea2a509` | 41370823497 | PASS | PASS | PASS | PASS | PASS | PASS | Exact PR head containing the consolidated ledger; all six jobs succeeded |

The evidence ledger was committed in PR #354 head `11378882ea73a7307430dcb52389709fdea2a509`; workflow `41370823497` completed successfully on that exact SHA, with both Rust and Python jobs successful. The ledger row above records that exact-head result.

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
- PRs remain Draft for review.
