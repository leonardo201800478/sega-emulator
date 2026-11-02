# ADR-0013 — deterministic scheduler snapshot and restore

- **Status:** In progress — E1–E4 implemented in stacked draft PRs; E5–E7 pending.
- **Date:** 2026-11-01.
- **Baseline:** PR #346, branch `feat/temporal-event-order-dimensions`, SHA `ec2bd69d32409490901d30dc58ff94779eda061b`.
- **Main baseline:** `0da3098893cce4d80f7a22291d00488e4e7d0f9b`; this main revision does not contain `native/machine_scheduler`.
- **Depends on:** ADR-0002, ADR-0010, ADR-0012 and the scheduler work in PRs #343–#346.

## 1. Context and progress ledger

The temporal foundation has been developed as a linear, reviewable stack. The following work is present in the audited branch, not yet integrated into `main`:

| Step | Commit | Delivered contract | Evidence/status |
|---|---|---|---|
| PR #343 | `170f03bba3a36b0bc45329a374491ebfb6dfdd61` | Clock-domain registration with configurable initial phase; legacy registration remains phase zero; phase validation is atomic. | CI reported SUCCESS on the candidate SHA. |
| PR #344 | `0c5fcda97cc5a99c43a89b55a0fe56e1d2f3d5ac` | Exact rational `ClockEdgeTime` calculations and checked edge-index/overflow behavior. This is a calculation API, not sub-tick dispatch. | CI reported SUCCESS on the candidate SHA. |
| PR #345 | `ff628cb479dc70b6b7a6b8cc734eed5007bf306d` | Configurable `DeltaEventPolicy`; default budget 1024 dispatches per integer timestamp; blocked event remains pending. | CI reported SUCCESS on the candidate SHA. |
| PR #346 | `ec2bd69d32409490901d30dc58ff94779eda061b` | Separate `HardwarePhase` and `PlatformPriority`; lexicographic event ordering; causal regression checks; ADR-0012. | GitHub Actions run `41356994362` SUCCESS on this exact SHA. PR #346 remains open, Draft, and unmerged. |
| PR #348 — E1 | `02f71b830ddd47156eb69d78de7b8107f851ec41` | Versioned DTOs for clock domains, event queue and scheduler; exact `u128` wire contract documented. | GitHub Actions run `41360589305` SUCCESS on this exact SHA. PR #348 remains open, Draft, and unmerged. |
| PR #349 — E2 | `356a7eb4a022e898b87c04c1ca61140056f47d14` | Clock-domain origin history retained; DTO export/import; checked arithmetic validation; scheduler clock-domain export/restore and continuation tests. | GitHub Actions run `41361087461` SUCCESS on this exact SHA. PR #349 remains open, Draft, and unmerged. |
| PR #350 — E3 | `d84e588a470c09655ae3c424a094d9f2c98e9a58` | Canonical EventQueue export/restore; preserves pending sequence values, allocator, and complete dispatched watermark including sequence; validates full causal key, allocator bounds, and uniqueness against the dispatched sequence. | GitHub Actions run `41368745114` SUCCESS on this exact SHA; all six required gates passed. |
| PR #352 — E4 | `9c506be48982c20c98977e71aef71eb8f1cf3031` | Atomic full-scheduler export/restore; canonical clock-domain DTOs; cross-validates time, event watermark, dispatch history, pending-event bounds, schema version and delta policy. | CI run `41369507172` SUCCESS on this exact implementation SHA; independent review pending. |

The independent audit dated 2026-11-01 examined the actual #346 branch and found no snapshot/restore implementation, no Serde support in the native crates, and no snapshot tests. Its local targeted test run passed 16 `temporal_core` tests and 18 `machine_scheduler` tests using Rust 1.85.1. The auditor did not run `fmt` or `clippy`; those local checks are therefore NOT RUN. The GitHub Actions success above is separate evidence and must not be confused with the auditor's local run.

The original audit verdict was **READY WITH CONDITIONS**. E1–E4 are implemented as candidates in stacked PRs with exact-SHA CI evidence. E5–E7 remain unfinished; the complete scheduler snapshot/restore contract is not yet closed and `TEMPORAL_PASS` is not claimed.

## 2. Problem

A snapshot must preserve not merely the visible pending events, but every piece of scheduler state that can affect future observable behavior.

The audit demonstrated that rebuilding a scheduler through its public scheduling API is not equivalent to restoring it: causal-regression protection can be lost, pending events can receive new sequence numbers, and the delta-event budget can reset. The clock-domain snapshot type also omits the clock ratio and is not a complete persistence format.

## 3. Decision

### 3.1 Versioned persistence DTOs, separate from live runtime types

Do not add blanket `Serialize`/`Deserialize` derives to live scheduler types as the persistence design.

Introduce explicit, versioned state DTOs:

- `ClockDomainState`
- `EventQueueState<T>`
- `SchedulerState<T>`

The DTOs are the persistence contract. Runtime structs remain responsible for protecting their own invariants through constructors and controlled state-import methods.

The top-level format must contain a `format_version`. Unknown versions must be rejected explicitly. Payload serialization has its own codec/version contract and must not be assumed to be stable merely because `T` implements a serialization trait.

### 3.2 Canonical queue representation

Serialize pending events as a canonical vector sorted by the complete key:

`(MachineTime, HardwarePhase, PlatformPriority, stable_sequence)`

Do not serialize the internal `BinaryHeap` layout as a persistence contract.

`EventQueueState<T>` must preserve at least:

- canonical pending entries, including payload and assigned sequence;
- `next_sequence`;
- `last_dispatched` causal watermark.

On restore, the queue itself validates the state and reconstructs its heap. Producers must not choose or rewrite assigned sequence values during ordinary scheduling.

**Sequence scope clarification:** the audit found that `next_sequence` belongs to an `EventQueue`, not to the entire machine or all queues in the runtime. In this ADR, “stable sequence” means monotonically assigned within that queue. Documentation must not describe it as process-global or machine-global.

### 3.3 Complete clock-domain state

`ClockDomainState` must preserve:

- domain ID;
- exact `ClockRatio` numerator and denominator;
- current integer `MachineTime`;
- accumulated local cycles;
- current rational phase/residue;
- registration/start timestamp;
- initial phase at registration.

The existing `ClockDomainSnapshot` must not be silently repurposed as the persistence DTO: it omits the ratio and does not contain enough information to validate the full domain history.

Restore must validate the ratio and phase through checked constructors. In particular, the phase must satisfy `phase < denominator`.

Where the stored start and initial phase are available, validate the arithmetic consistency relation using checked integer arithmetic:

`total_cycles * denominator + phase == (now - start) * numerator + initial_phase`

The implementation must handle underflow/overflow as a typed restore error, not wrap or normalize invalid values silently.

### 3.4 Complete scheduler and delta-budget state

`SchedulerState<T>` must preserve:

- current `now`;
- all clock-domain states in canonical ID order;
- event-queue state;
- `DeltaEventPolicy`;
- `last_dispatch_timestamp`;
- `dispatched_at_timestamp`.

The causal watermark and delta counters are required state. A full historical dispatch log is not required: future scheduling/dispatch decisions depend on the retained causal watermark, timestamp and delta count, not on a complete event history.

Do not reject a state solely because `dispatched_at_timestamp` exceeds the currently configured delta limit: the existing API permits reducing the limit after dispatches have already occurred. Restore must preserve that valid state and reproduce the same subsequent blocking behavior.

### 3.5 Atomic restore

Provide restoration as construction of a new instance, conceptually:

`restore(dto) -> Result<MachineScheduler<T>, RestoreError>`

The operation must:

1. decode the DTO;
2. validate individual fields;
3. validate cross-component invariants;
4. reconstruct domains and queue in temporary values;
5. return a fully valid scheduler only after all checks pass.

A failed restore must not expose or leave behind a partially restored scheduler. Prefer a typed error identifying the violated invariant.

### 3.6 Exact integer representation

All temporal quantities must remain exact. No `f32` or `f64` representation is allowed for timestamps, ratios, phases, cycle counters or rational residues.

The persistence wire representation for every `u128` temporal quantity is fixed-width **16-byte unsigned big-endian**. This applies to `MachineTime` tick counts, `total_cycles`, `phase` and `initial_phase`. The codec must map this representation without signed or floating-point conversion and reject truncated or oversized values. The outer container/framing remains a codec concern, but the 128-bit scalar representation is fixed by this ADR. In-memory DTOs retain native `u128` values; they do not depend on a serializer's native `u128` support.

### 3.7 Current boundary of ClockEdgeTime

Do not integrate `ClockEdgeTime` into the event queue as part of this snapshot milestone.

Today the queue dispatches on integer `MachineTime`; `ClockEdgeTime` is an exact rational edge-time calculation API, not a sub-tick scheduler. If rational timestamps later become part of the queue key, that change requires a separate architecture decision, ordering semantics, delta-cycle definition and snapshot format migration/version.

## 4. Restore invariants

A restored scheduler must satisfy, at minimum:

1. `now` is a valid integer machine timestamp.
2. Every domain has non-zero ratio numerator and denominator.
3. Every domain phase is strictly less than its denominator.
4. Every domain time equals scheduler `now`, because all registered domains currently advance together.
5. Domain IDs are unique; serialized domain ordering is canonical.
6. Every pending event timestamp is greater than or equal to `now`.
7. Pending event sequences are unique and each is less than `next_sequence`.
8. Pending event ordering is canonical by the full four-part key.
9. `last_dispatched`, when present, is not later than `now`.
10. No pending event at the last-dispatched timestamp regresses the dispatched `(hardware_phase, platform_priority)` pair.
11. The queue causal watermark and scheduler delta timestamp agree where both represent the last dispatch timestamp.
12. Delta counters and policy are preserved exactly; a counter above a subsequently reduced limit remains representable.
13. All arithmetic checks use checked operations and fail without partial restoration.
14. Snapshot → restore → continue produces the same observable event outputs, clock advances, errors and final exported state as uninterrupted execution.

Do not invent additional physical hardware ordering rules in this validation layer. These are software-state invariants only.

## 5. Implementation sequence

The next implementation work is deliberately split into small, reviewable slices:

1. **E0 — Freeze and stabilize the base.** Reconfirm PRs #341–#347, their heads, dependency chain and CI. Because #341–#346 are still open and unmerged, do not assume the stack is integrated. Freeze the verified #346 SHA as the temporal implementation baseline and stack Etapa E after the documentation PR #347. Do not rebase or merge the stack without a separate maintainer decision. If any parent changes, stop and reconcile before continuing.
2. **E1 — State model.** Add the DTO types and document their version/codec contract. No runtime behavior changes yet.
3. **E2 — Clock-domain export/restore.** Implement complete `ClockDomainState`, checked reconstruction and arithmetic invariant tests.
4. **E3 — Queue export/restore.** Implement canonical vector export, preserve sequence allocator and causal watermark, validate duplicates and reconstruct the heap.
5. **E4 — Scheduler restore.** Preserve delta policy/counters and validate cross-component invariants atomically.
6. **E5 — Codec.** Add versioned encode/decode and exact `u128` handling; reject unknown versions and truncated/corrupt inputs.
7. **E6 — Differential replay.** Prove equivalence against uninterrupted execution, including deterministic randomized operation sequences with fixed seeds.
8. **E7 — CI and evidence.** Run all required Rust and Python checks on the exact candidate SHA and record PASS/FAIL/BLOCKED/NOT RUN accurately.

No implementation slice should be combined with unrelated device, CPU, VDP, PSG, mapper or FFI work.

## 6. Required tests and acceptance criteria

The snapshot milestone is not complete until tests cover:

- basic snapshot/restore continuation;
- multiple pending events, including identical timestamps;
- independent ordering dimensions and stable sequence preservation;
- clock ratios including 3/2, 7/3 and 1/3 with non-zero initial phase;
- snapshots immediately before and at the delta budget boundary;
- reducing the delta limit after dispatch and restoring that state;
- deterministic differential replay with fixed-seed generated operations;
- multiple snapshots from one execution, each restored independently;
- isolated invalid-state cases for every restore invariant;
- truncated/corrupt encoded inputs;
- sequence exhaustion and arithmetic overflow before and after restore;
- canonical DTO equality/encoding for the same logical scheduler state reached through different clock-domain registration orders and different internal heap layouts, while preserving the exact assigned event sequence numbers. Do not require equality when event insertion order differs: insertion sequence is observable tie-break state and must be preserved.

The principal acceptance criterion is exact observable equivalence: the same dispatched events, the same `ClockAdvance` outputs, the same errors at the same operation, and the same final exported state.

## 7. Explicitly deferred

- Sub-tick event dispatch and rational timestamps in the queue.
- Assigning SMS/SG-1000 hardware phase or platform priority values without primary documentation or independent measurement.
- Full dispatch-history persistence.
- Cryptographic checksums and long-term cross-version migration beyond the initial versioned format.
- Serialization of CPU, VDP, PSG, mapper or other device payloads beyond defining the payload codec boundary required by this scheduler DTO.

These deferrals do not weaken the exactness requirements of the current integer-time scheduler.

## 8. Validation and project status

At the time this ADR was prepared:

- PR #346 is open, Draft, unmerged, and its head is `ec2bd69d32409490901d30dc58ff94779eda061b`.
- GitHub Actions workflow run `41356994362` reports SUCCESS for that exact SHA.
- The main branch remains `0da3098893cce4d80f7a22291d00488e4e7d0f9b` and does not contain `native/machine_scheduler`.
- The independent audit's targeted local tests passed (16 temporal-core + 18 scheduler tests); its local `fmt` and `clippy` were NOT RUN.
- E1 state-model DTOs are defined in `temporal_core::state` (`EventQueueState` / `EventQueueWatermark`) and `machine_scheduler` (`ClockDomainState` / `SchedulerState`). E1 exact-SHA CI: PASS (`41360589305`).
- E2 clock-domain export/restore is implemented in PR #349. It retains registration origin and initial phase, validates the checked rational-history equation, and rejects timestamp mismatch/invalid history before inserting the domain.
- E2 exact-SHA CI: PASS (`41361029624`) on `356a7eb4a022e898b87c04c1ca61140056f47d14`; Rust formatting, Clippy with `-D warnings`, workspace tests, Python Ruff format/check and pytest all passed.
- E3 queue export/restore is implemented in PR #350. The watermark preserves the complete dispatched key, including sequence; restore validates pending events against that full key and rejects watermark sequences outside the allocator range. Regression tests cover same-timestamp/phase/priority sequence rollback and a valid later sequence. A follow-up audit added rejection of pending events that reuse the dispatched watermark sequence, even when their timestamp is later. Final E3 candidate `d84e588a470c09655ae3c424a094d9f2c98e9a58` passed exact-SHA run `41368745114` (all six required gates).
- E4 full scheduler atomic restore is implemented in PR #352 at `9c506be48982c20c98977e71aef71eb8f1cf3031`; exact-SHA CI run `41369507172` reports SUCCESS. Independent semantic review is pending.
- Codec and full differential replay: NOT IMPLEMENTED / NOT RUN.
- `TEMPORAL_PASS`: NOT CLAIMED.
- Cycle-accurate SMS timing: NOT CLAIMED.

Required checks for every implementation candidate remain:

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
python -m pytest
python -m ruff check .
python -m ruff format --check .
```

No PR is to be merged without explicit maintainer authorization. No direct commits to `main`, force-pushes or destructive resets are permitted.

## 9. Exit gate

The scheduler snapshot milestone may be marked implemented only when the DTO/restore contract is complete, invalid states are rejected atomically, all differential replay tests pass, and the required CI checks pass on the exact candidate SHA.

It must not be called `TEMPORAL_PASS` until broader temporal integration evidence exists. Software determinism is not proof of electrical or cycle-accurate SMS fidelity.
