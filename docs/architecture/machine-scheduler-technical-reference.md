# Machine Scheduler — Technical Reference and Completion Dossier

- **Repository:** `leonardo201800478/sega-emulator`
- **Document purpose:** consolidate the implemented temporal foundation and define the remaining scheduler-finalization work.
- **Audited implementation stack:** PRs #343–#354, ending at `11378882ea73a7307430dcb52389709fdea2a509`.
- **Documentation reconciliation:** PR #355, head `5ff6f84a3d58c0475be7024fe99e247f74565561`.
- **Status:** implementation milestones E1–E7 present in the stacked candidate series; review, stress validation and integration remain pending.
- **Important:** this document describes the code observed on the audited branches. It does not claim that the stack has been merged into `main`.

## 1. Executive summary

The Scheduler work establishes a deterministic, integer-master-time timeline for coordinating rational device clock domains and ordered machine events. The implementation is split into two native Rust crates:

- `native/temporal_core`: reusable time, rational clock and event-queue primitives.
- `native/machine_scheduler`: machine-level orchestration over those primitives.

The Scheduler is deliberately not a device emulator. It does not execute CPU instructions, render VDP pixels, clock PSG/YM chips internally, access cartridge buses, or invoke hardware callbacks. It advances registered clock-domain accounting to event boundaries and returns typed dispatch records to the owning machine loop. Device semantics remain with their respective components.

The implementation stack also adds canonical snapshot DTOs, validated restore, a versioned binary codec and deterministic replay tests. E1–E7 are implemented in the candidate PR stack and have recorded exact-SHA CI evidence. The remaining work is to independently review semantics, run short and long stress campaigns, address findings, and then integrate under the repository's branch/PR policy.

## 2. Architectural boundaries

### 2.1 Ownership

| Layer | Responsibility | Explicitly not responsible for |
|---|---|---|
| `temporal_core` | `MachineTime`, rational clock conversion, `ClockEdgeTime`, event ordering, queue state | Console-specific event meanings or device callbacks |
| `machine_scheduler` | Master machine timeline, clock-domain registry, event dispatch boundary, delta budget, full scheduler snapshots | CPU execution, VDP rendering, bus arbitration semantics, audio synthesis |
| System/machine layer | Register domains, schedule profile-defined events, consume dispatch records, call devices in deterministic order | Duplicating rational clock arithmetic already owned by temporal core |
| Device layer | VDP/CPU/PSG/mapper state and device behavior | Owning the global machine timeline |
| Python frontend | GUI, ROM selection, logs and test tooling | Cycle counting, device timing, scheduler state or bus semantics |

### 2.2 Design constraints

- Native emulation logic remains in Rust.
- Time and cycle arithmetic use integer/rational representations; no floating-point timing.
- No JIT, speculative parallelism, frame-skip timing hacks or premature optimization.
- Event and clock ordering must be deterministic and reproducible.
- Arithmetic uses checked operations and rejects overflow rather than wrapping.
- Scheduler primitives remain generic and do not encode SMS-specific phase assumptions without primary evidence.
- No per-instruction Python callbacks; the Python frontend remains outside the timing loop.

## 3. Source layout and module responsibilities

### `native/temporal_core/src/clock.rs`

Contains:

- `MachineTime`: absolute profile-defined master timestamp backed by `u128`.
- `ClockRatio`: positive rational local cycles per master tick, represented by `u64/u64`.
- `ClockEdgeTime`: exact integer-plus-fractional position of a local clock edge.
- `ClockDomain`: accumulated local cycles and retained fractional phase.
- `ClockDomainSnapshot`: read-only time, cycle count and phase view.
- `ClockError`: explicit invalid-ratio, backwards-time, phase and overflow failures.

The profile must document how its master-time unit maps to physical clocks. `MachineTime` is not inherently a universal frequency.

### `native/temporal_core/src/event.rs`

Contains:

- `HardwarePhase(u16)`: profile-defined ordering stratum at a timestamp.
- `PlatformPriority(u16)`: software dispatch priority within the same hardware phase.
- `EventPriority`: backward-compatible alias for `PlatformPriority`.
- `ScheduledEvent<T>`: timestamp, phase, priority, stable sequence and typed payload.
- `EventQueue<T>`: min-priority behavior implemented over `BinaryHeap`, plus allocator and causal watermark.
- `EventQueueError`: sequence exhaustion, past scheduling, ordering regression and invalid restored state.

The canonical event key is:

`(MachineTime, HardwarePhase, PlatformPriority, insertion_sequence)`

Lower values dispatch first at each ordering dimension. The sequence is the stable tie-breaker for otherwise equal events.

### `native/temporal_core/src/state.rs`

Defines persistence DTOs used by the queue:

- `EventQueueState<T>`: canonical pending event vector, next sequence allocator and optional last-dispatched watermark.
- `EventQueueWatermark`: complete last-dispatched causal key, including sequence.

The DTO captures future-observable ordering state, not the internal `BinaryHeap` memory layout.

### `native/machine_scheduler/src/lib.rs`

Contains:

- `ClockDomainId(u16)`: stable scheduler-local clock identity.
- `ClockAdvance`: newly elapsed cycles, total cycles and resulting phase for a domain.
- `MachineDispatch<T>`: dispatched event plus clock advances up to its timestamp.
- `DeltaEventPolicy`: non-zero maximum dispatch count per integer timestamp; default 1024.
- `SchedulerError`: explicit scheduler, clock, queue and restore errors.
- `MachineScheduler<T>`: current time, event queue, ordered clock registry, delta policy and dispatch history.

### `native/machine_scheduler/src/state.rs`

Defines versioned persistence DTOs:

- `ClockDomainState`: domain ID, rational ratio, current time, total cycles, phase, registration origin and initial phase.
- `SchedulerState<T>`: top-level format version, current time, canonical clock-domain list, event queue state, delta limit and dispatch history.
- `SCHEDULER_STATE_FORMAT_VERSION`: currently 1.

The DTO schema is intentionally separate from live runtime types. Persistence does not depend on blanket Serde derives.

### `native/machine_scheduler/src/codec.rs`

Defines the explicit versioned binary wire format and caller-supplied `EventPayloadCodec<T>`. Scheduler event framing is independent of device payload schemas.

### `native/machine_scheduler/tests/snapshot_replay.rs`

Contains fixed-seed differential replay scenarios using independent restored scheduler instances.

## 4. Time and rational clock model

### 4.1 Master time

`MachineTime` is an absolute unsigned `u128` tick count. It supports checked addition and checked elapsed-duration calculation. Backwards movement is rejected.

The unit is selected by the machine profile. A profile must define the conversion from its master-time tick to the actual hardware clock model. Do not silently assume that one `MachineTime` tick is one CPU cycle.

### 4.2 Clock ratios and phase

A `ClockRatio` represents:

`local_cycles_per_master_tick = numerator / denominator`

The ratio must have positive numerator and denominator. A domain retains:

- registration timestamp;
- initial phase;
- current timestamp;
- total local cycles;
- current fractional phase.

For an elapsed master interval `dt`, numerator `n`, denominator `d`, and prior phase `p`:

`scaled = dt * n + p`

`elapsed_cycles = floor(scaled / d)`

`new_phase = scaled mod d`

All operations are checked. Retaining the remainder makes chunked advancement equivalent to one large advancement, provided the same timeline is traversed.

### 4.3 Exact edge-time calculation is not sub-tick event dispatch

`ClockEdgeTime` represents an exact edge position as whole master ticks plus a reduced fractional component. Its comparison uses exact cross multiplication in `u128`.

This is a calculation API. The current `EventQueue` still stores integer `MachineTime` timestamps. Do not claim that the queue dispatches rational sub-tick events. Likewise, `HardwarePhase` is an ordinal ordering stratum, not a fractional timestamp or electrical phase measurement.

## 5. Event queue ordering and causality

The queue orders events lexicographically by:

1. absolute integer timestamp;
2. hardware phase;
3. platform priority;
4. insertion sequence.

The queue assigns monotonically increasing `u64` sequence numbers. The allocator state is preserved by snapshot/restore.

After dispatch, the queue retains a complete watermark. New events cannot be scheduled before the dispatched timestamp, nor can a same-timestamp event regress the already-dispatched phase/priority/sequence key. This prevents causal rollback at the current timestamp.

Queue restoration validates at least:

- canonical strict ordering of pending keys;
- uniqueness of pending sequences;
- every pending sequence is below `next_sequence`;
- watermark sequence is in allocator range;
- no pending event reuses the watermark sequence;
- no pending key is at or before the dispatched watermark.

The internal heap representation is reconstructed from the canonical vector.

## 6. MachineScheduler contracts

### 6.1 Registration

- `register_clock_domain(id, ratio)` preserves the compatibility zero-phase behavior.
- `register_clock_domain_with_phase(id, ratio, phase)` accepts an explicit initial phase smaller than the denominator.
- Duplicate IDs and invalid phases are rejected without mutation.
- Clock domains are stored in a `BTreeMap`, so observable advancement order is ascending stable ID, independent of registration order.

### 6.2 Scheduling

- `schedule_at` is the compatibility API and uses zero hardware phase.
- `schedule_at_with_phase` accepts separate hardware phase and platform priority.
- Events before current scheduler time are rejected.
- Same-timestamp causal regression is rejected by the event queue.

### 6.3 Dispatch

`dispatch_next`:

1. inspects the next event timestamp;
2. checks the same-timestamp delta-event budget;
3. preflights all clock-domain arithmetic;
4. advances all domains to the event timestamp;
5. pops the event;
6. updates dispatch timestamp/count;
7. returns `MachineDispatch<T>` with per-domain `ClockAdvance` values.

It does not invoke a device callback. The machine layer consumes the returned event and clock advances.

The clock preflight is designed to prevent an arithmetic failure from popping the event or partially advancing domains. Review the preflight/apply boundary carefully during the independent audit.

### 6.4 Idle advancement

`advance_idle_to(target)` is a bounded run-until primitive. It rejects backwards time and refuses to advance across or onto a pending event. It then calculates all domain advances and applies them.

### 6.5 Delta-event budget

The default budget is 1024 dispatches at one integer timestamp. A zero limit is invalid. Exceeding the limit leaves the pending event in the queue. A later timestamp resets the per-timestamp count. Updating the policy does not discard events or erase dispatch history; increasing a limit can resume a blocked queue.

This is a deterministic software safety bound, not a hardware-derived physical limit or event-precedence rule.

## 7. Snapshot and restore: E1–E4

### E1 — State DTOs

Defined explicit state models for clock domains, event queue and scheduler. The model preserves all data that can affect subsequent scheduling and clock advancement, including allocator and watermark state.

### E2 — Clock-domain export/restore

The clock DTO retains ratio, current counters, phase, registration timestamp and initial phase. Restore reconstructs the domain only after validating its exact rational history. A domain restored into an existing scheduler must match the scheduler's current timestamp and must not duplicate an ID.

### E3 — EventQueue export/restore

Export sorts pending events by the complete canonical key and preserves their original insertion sequence, allocator and complete dispatch watermark. Restore rejects noncanonical, duplicate, out-of-range and causally regressive state.

### E4 — Atomic full-scheduler restore

The full scheduler DTO includes all clock domains, queue state, delta policy and dispatch history. Restore validates the format version, non-zero delta limit, dispatch timestamp/count consistency, watermark timestamp, queue events relative to `now`, canonical clock-domain order and clock-domain timestamps/history.

Domains and queue are reconstructed in temporary values before the new scheduler is exposed. Invalid state must not produce a partially restored scheduler.

## 8. Versioned binary codec: E5

The frame format is:

`magic[8] | state_version[u16 BE] | payload_version[u16 BE] | body_length[u64 BE] | body | crc32[u32 BE]`

Key properties:

- 8-byte magic: `SEGSCHD\0`.
- State schema version and event-payload codec version are independent.
- Temporal `u128` values use exactly 16 unsigned big-endian bytes.
- Collection counts and payload lengths use unsigned 32-bit big-endian values.
- The event payload is encoded/decoded by a caller-supplied `EventPayloadCodec<T>`.
- Decoder checks magic, versions, frame/body lengths, option tags, truncation, trailing data, payload boundaries and CRC.
- The codec uses checked length conversions and fallible reservation for the decoded clock-domain collection.

CRC-32/ISO-HDLC detects accidental corruption; it is not cryptographic authentication and does not protect against malicious modification. The codec does not imply that arbitrary device payloads already have a stable schema. Each payload codec must be deterministic and versioned independently.

The top-level scheduler DTO schema version is currently 1. Increment it when the DTO schema changes incompatibly. Change the payload codec version when a payload representation changes independently.

## 9. Differential replay: E6

The replay integration test uses a local fixed-seed PRNG and deterministic scenarios. It creates multiple rational clock domains, registers them out of ID order, schedules events with shared timestamps and distinct ordering dimensions, snapshots at multiple checkpoints, restores independent schedulers and compares continuation.

The comparison covers observable dispatch outcomes, clock advances, sequence allocation, errors and final exported state. It also exercises past-event rejection, delta-budget changes/resumption and arithmetic overflow atomicity.

This is software-level differential evidence for the tested scenarios. It is not an independent hardware oracle, a proof of every possible scheduler state, or proof of cycle-accurate device timing.

## 10. Evidence consolidation: E7

The evidence ledger is:

`docs/evidence/adr-0013-snapshot-restore-validation.md`

It records exact implementation SHAs and workflow IDs for PRs #343–#354. The final PR #354 head was `11378882ea73a7307430dcb52389709fdea2a509`, validated by workflow `41370823497` on that exact SHA. The consolidated PR #358 initially passed the six ordinary gates on `ca349d304942d105fcef4bf133be8be2c79c8ed6` in workflow `41377566361`. Subsequent documentation consolidation commits changed the head; those earlier PASS results do not transfer to the new head. The current head must be checked independently before integration.

The six gates are:

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
python -m pytest
python -m ruff check .
python -m ruff format --check .
```

Earlier candidate failures are not promoted as passing evidence. In particular, the ledger records a Clippy failure in an earlier #352 candidate and formatting failures in intermediate #353 candidates; corrected exact SHAs subsequently passed.

PR #355's ADR/evidence reconciliation and PR #356's technical dossier/handoff have been incorporated into the consolidated PR #358 branch. PR #351's ADR-0014 was also incorporated. These source PRs should be treated as superseded only after their complete contents are verified in #358; the current inventory remains the authority for their open/closed state.

## 11. PR stack and provenance

The historical series was linear/stacked. The superseded PRs were closed after consolidation; #351, #355 and #356 are being absorbed into #358. Re-fetch live branch heads, checks and PR states before acting.

| PR | Scope | Audited head SHA |
|---|---|---|
| #343 | Scheduler CI baseline / clock registration | `170f03bba3a36b0bc45329a374491ebfb6dfdd61` |
| #344 | Exact rational clock-edge time | `0c5fcda97cc5a99c43a89b55a0fe56e1d2f3d5ac` |
| #345 | Delta-event budget | `ff628cb479dc70b6b7a6b8cc734eed5007bf306d` |
| #346 | Hardware phase and platform-priority dimensions | `ec2bd69d32409490901d30dc58ff94779eda061b` |
| #347 | ADR-0013 documentation | `fc3fb1e014324f025fb05447e29e7ac8b2c890b0` |
| #348 | E1 — Snapshot state DTOs | `02f71b830ddd47156eb69d78de7b8107f851ec41` |
| #349 | E2 — ClockDomain snapshot/restore | `356a7eb4a022e898b87c04c1ca61140056f47d14` |
| #350 | E3 — EventQueue snapshot/restore | `a2e21ca310f7b73e95d4d75700ca1e032cf1cc07` |
| #351 | ADR-0014 device synchronization contract (incorporated into #358) | `2b577e96e467d170e6eef537a51cee4eced8ee70` |
| #352 | E4 — Atomic Scheduler restore | `c5f3c76f1fdb833576153f22da8d66a4107f1f0b` |
| #353 | E5 — Versioned scheduler-state codec | `839fdaa25137a91916057c585a7cd81a9bcdb305` |
| #354 | E6/E7 — Differential replay and evidence ledger | `11378882ea73a7307430dcb52389709fdea2a509` |
| #355 | Documentation reconciliation (incorporated into #358) | `5ff6f84a3d58c0475be7024fe99e247f74565561` |
| #356 | Technical dossier and handoff (incorporated into #358) | `bba0dd142042a9f3b7c09a555c014b3a2a7b3e5a` |
| #358 | Consolidated Scheduler stack and stress harness | current head: consult GitHub PR #358 |

The implementation evidence for #350 and #352 is tied to their exact validated implementation SHAs, not necessarily to the later documentation-only branch heads. Consult the evidence ledger for exact SHA/run pairing. Do not infer a passing workflow for a different SHA.

## 12. Known gaps and non-claims

The following are intentionally not complete:

1. `SHORT_PASS` and `LONG_PASS` stress campaigns have not been run for the complete snapshot/restore stack.
2. No independent hardware oracle or electrical timing validation has been used.
3. Rational `ClockEdgeTime` calculation exists, but the event queue remains integer-timestamped; sub-tick dispatch is deferred.
4. Scheduler-to-device temporal integration is pending.
5. No claim of SMS cycle accuracy or `TEMPORAL_PASS` is justified.
6. Device payload schemas are not universally implemented by the scheduler; the codec boundary is generic.
7. The scheduler is not a bus arbiter and does not define electrical contention or device-specific precedence by itself.
8. External semantic review and maintainer integration approval remain pending.

## 13. Required finalization campaign

The next work should be split into three independent review/test tracks and coordinated by a lead:

- **Agent A — semantic and invariant audit:** inspect temporal arithmetic, event causality, delta budget, atomicity, restore validation and codec correctness. Produce findings with file/line references and minimal fixes.
- **Agent B — stress and property-style tests:** design/run SHORT_PASS and LONG_PASS; test deterministic generated operation sequences, boundary values, event storms, sequence exhaustion, overflow, snapshot/restore repetition and codec malformed-input cases. Use bounded, reproducible seeds and record runtime/resource usage.
- **Agent C — independent CI/reproducibility and integration-boundary audit:** verify exact SHA/workflow evidence, run all six gates on the exact candidate, inspect crate/workspace wiring and ensure device-facing contracts remain generic. Confirm that the Scheduler returns dispatch/clock advances without pulling VDP/CPU/PSG execution into its crate.

The lead must reconcile findings, avoid overlapping edits, create small commits on a dedicated branch, and open/update a Draft PR. The three tracks are review roles; they must not be reported as autonomous agents unless the new chat actually has three agent execution contexts.

## 14. Device integration strategy after Scheduler finalization

The agreed scope boundary is:

- **VDP:** full device implementation belongs in its own workstream and must model its internal behavior precisely.
- **Other devices (Z80, PSG, mappers and remaining components):** initially add only the connection to Scheduler-controlled cadence/clock advancement. Their internal timing fidelity and cycle-level behavior are improved in separate, independently scoped stages.
- The machine/system layer is the integration owner: it registers rational clock domains, schedules profile-defined events, consumes `MachineDispatch` and invokes device logic at deterministic boundaries.
- Do not embed VDP rendering or CPU instruction execution into `machine_scheduler`.
- Do not assign SMS hardware phases or event priorities by guesswork. Require primary documentation, measurements or independent reference behavior.

This boundary prevents the Scheduler from becoming coupled to one console device and keeps the VDP implementation from contaminating generic scheduling primitives.

## 15. Definition of done

The Scheduler implementation milestone can be called finalized for integration only when:

- the independent semantic audit has no unresolved critical/high-severity findings;
- SHORT_PASS and LONG_PASS have reproducible PASS results on identified SHAs;
- all six repository gates pass on the exact final candidate SHA;
- snapshot/restore and codec malformed-state tests cover the identified boundaries;
- the PR stack and dependencies are reconciled without accidental merge or history rewriting;
- ADR/evidence ledger identify exact SHA, workflow IDs and limitations;
- the integration contract remains generic and device callbacks remain outside the Scheduler.

Even after this gate, use `INTEGRATION_PENDING` until the machine/device connections are validated. Reserve `TEMPORAL_PASS` for later temporal integration supported by independent oracles. Software determinism alone does not prove cycle accuracy.


---

## 12. Current validation state and next integration boundary (2026-11-02)

This section is the current status overlay. Earlier historical SHAs and PR descriptions in this dossier remain useful provenance, but the live PRs and exact-SHA evidence ledger are authoritative.

### 12.1 Consolidated candidate stack

- PR #358 is open, Draft, targets `main`, and is not merged.
- PR #358 head: `0d8ad97e79ac50c4687e1400dd5bb434ca5f3607`.
- PR #359 is open, Draft, based on the #358 branch, and is not merged.
- PR #359 head: `a544fe367f67c1f6c6d622efcdb528e222ab53c3`.
- The #359 test branch is a dependent stack layer, not an independent candidate against `main`.

### 12.2 Verified CI and stress evidence

For PR #358 head `0d8ad97e79ac50c4687e1400dd5bb434ca5f3607`:
- Workflow 41378338137 passed the six ordinary Rust/Python gates.
- Workflow 41379180394 completed the SHORT stress stage (4 × 20,000 = 80,000 events; 512 MiB; 1 test passed), but the overall workflow was cancelled and its Rust workspace test stage did not complete.
- Workflow 41379203192 completed all jobs successfully, including LONG stress (16 × 250,000 = 4,000,000 events; 2 GiB; 1 stress test passed).

For PR #359 head `a544fe367f67c1f6c6d622efcdb528e222ab53c3`:
- Workflow 41381005224 passed Rust quality/unit regression and Python lint/regression.
- The stress job was skipped by the PR workflow condition.
- The integration test `native/machine_scheduler/tests/multi_domain_replay.rs` verifies integer-timestamp multi-domain advancement, replay/restore, chunk invariance, a fixed dispatch order and atomic overflow handling against an independent arithmetic oracle.

The exact workflow links, caveats and statuses are maintained in `docs/evidence/adr-0013-snapshot-restore-validation.md`.

### 12.3 What the evidence proves—and does not prove

The scheduler stress harness uses direct `MachineTime` event scheduling. It does not model independent device clocks as event producers. The #359 test exercises rational domain advancement through dispatch and idle chunks, but still operates only at integer `MachineTime` boundaries.

Therefore, the current evidence supports deterministic scheduler software behavior for the tested cases. It does **not** establish:
- rational sub-tick event dispatch;
- CPU micro-operation or bus-cycle execution;
- Z80 memory/I/O transaction timestamps;
- SMS bus arbitration, wait states or VDP contention;
- VDP/PSG/YM2413 safe-point integration;
- cycle-accurate SMS execution.

The correct status remains `INTEGRATION_PENDING`; `TEMPORAL_PASS` is not claimed.

### 12.4 Next milestone: SMS temporal integration design

The next work item is an evidence-first design, not immediate device refactoring:
1. Inspect the current remote SMS implementation and exact issue #158 state.
2. Trace the real execution path from Z80 instruction stepping through memory/I/O access, VDP/PSG advancement and interrupt handling.
3. Identify which operations currently advance time in aggregate and which bus effects are visible only after an instruction.
4. Establish authoritative SMS timing references and independent test oracles before assigning any physical clock ratio, phase or priority.
5. Define a minimal interface for CPU execution to yield at bus-visible safe points, while leaving the generic scheduler free of Z80/SMS semantics.
6. Specify deterministic traces and regression criteria for SG-1000 and SMS before implementation.

No timing constant, event phase, bus priority or wait-state rule may be invented to fit the current scheduler API. A design gap requiring sub-tick timestamps must be raised as a separate architectural decision; `HardwarePhase` must not be repurposed as a fractional timestamp.

### 12.5 Finalization audit addendum — 2026-11-02

Two additional candidate PRs extend the validation baseline without changing device integration scope:

- **PR #362** (`a94b096255c880ad703279b1c6c185feb3851414`) adds deterministic interleaved operation replay: scheduling, dispatch, idle advancement, delta-budget changes, snapshot/restore through the versioned codec, and final queue draining. It also checks scheduler-level sequence exhaustion without state mutation. Its six ordinary CI gates passed in workflow `41383394157`; opt-in stress was skipped.
- **PR #363** (`8b2eee8264a2778557689654d0af62c0a0a29dc1`) fixes false intermediate overflow in rational clock conversion. The old code could overflow `elapsed * numerator` or `edge_index * denominator` before division even when the exact result fit in u128. The correction uses quotient/remainder decomposition, shares conversion logic with clock restore, exposes `ClockDomain::preview_advance_to`, and uses that preview in `MachineScheduler::calculate_advances` so preflight and commit share one conversion implementation. It adds large-ratio regressions and exhaustive small-ratio comparisons against integer arithmetic. Its six ordinary CI gates passed in workflow `41383734394`; opt-in stress was skipped.

The operation replay SHORT/LONG profiles introduced by #362 remain NOT RUN. Do not infer their result from the ordinary CI gates or from the older #358 dispatch-only stress result. The exact SHA/workflow matrix is maintained in `docs/evidence/adr-0013-snapshot-restore-validation.md`.

These changes improve software arithmetic correctness and deterministic replay coverage. They do not change the documented integer `MachineTime` EventQueue contract, do not enable sub-tick dispatch, and do not establish hardware timing. Scheduler/device integration remains `INTEGRATION_PENDING`; `TEMPORAL_PASS` remains unclaimed.
### 12.6 Opt-in operation-replay execution — 2026-11-02

The CI workflow now accepts explicit pull-request labels `scheduler-stress-short` and `scheduler-stress-long`. The stress job remains opt-in; ordinary PR CI does not pay the stress cost.

- SHORT on PR #362 head `cce79018427daab57a36808e162492b17bf4303e`, workflow `41384089223`: PASS. The new operation replay completed 8,000 operations (4 seeds × 2,000), and the existing dispatch replay completed 80,000 events (4 seeds × 20,000); both tests passed. All six ordinary gates also passed.
- LONG on the same exact head, workflow `41384147113`: 320,000 operations plus 4,000,000 dispatch events; status remains IN PROGRESS at this addendum's timestamp. No LONG_PASS is claimed until successful completion.
- The arithmetic PR #363's final head is `d6c2e9fbc243208ebe4e761e079934dd95eba98e`; ordinary CI workflow `41384173879` passed all six gates after a documentation-only rustdoc correction.

These software-level stress results do not change the limitations in §12.3: no sub-tick dispatch, CPU/bus transactions, SMS arbitration, VDP/PSG safe points, or hardware timing has been demonstrated.
### 12.7 LONG completion — 2026-11-02

Workflow [41384147113](https://github.com/leonardo201800478/sega-emulator/actions/runs/41384147113) completed SUCCESS on exact PR #362 head `cce79018427daab57a36808e162492b17bf4303e`.

- Operation replay: 16 seeds × 20,000 operations = 320,000 operations verified.
- Dispatch replay: 16 seeds × 250,000 events = 4,000,000 dispatches verified.
- Test binary result: 2 passed, 0 failed; execution time 393.99 seconds.
- All six ordinary gates passed in the same workflow.

Therefore, the deterministic scheduler software stress campaign has SHORT_PASS and LONG_PASS evidence on the documented exact SHAs. This does not prove electrical phase behavior, sub-tick event dispatch, CPU/bus transaction integration, SMS arbitration, VDP/PSG synchronization, or hardware temporal accuracy. Overall scheduler/device integration remains INTEGRATION_PENDING and TEMPORAL_PASS remains NOT CLAIMED.