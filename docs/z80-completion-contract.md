# Z80 Completion Contract — Shared Core

**Status:** Architecture/acceptance contract for the Z80-only completion sprint.  
**Implementation branch:** `feat/z80-complete-architectural-state`  
**Starting point:** `feat/z80-sms-jp-hl-ex-de-hl` (PR #429 head `4165c157b4246c0c1c928cded74b5f52a3f3c5bd`).  
**Scope boundary:** shared Rust Z80 CPU and its Scheduler-facing transaction protocol. No VDP, PSG, SG-1000, Game Gear, or Mega Drive expansion until the Z80 completion gate is met.

## 1. Architectural invariants

1. The existing atomic `Z80::step()` remains available and is not deleted, replaced, or weakened during the migration.
2. Atomic and resumable execution consume/export one common architectural-state DTO. Diagnostic traces, counters used only for host telemetry, and continuation internals are not hardware registers and must not be conflated with it.
3. The microsequencer never reads/writes memory or I/O directly. It emits typed bus requests; the machine bus/Scheduler supplies completion data and elapsed time.
4. No instruction may commit a read-dependent architectural result before the matching transaction completion.
5. Any instruction continuation safe point must survive export/restore with exact phase, opcode/prefix context, pending request identity, WAIT count, M-cycle/T-state position, operands already sampled, and captured effective addresses/write payloads.
6. No undocumented behavior may be inferred from names or generic Z80 assumptions when the atomic executor and validated vectors already define project behavior. Divergences are recorded and investigated, not silently normalized.
7. The Scheduler owns emulated time. Differential functional equality does not, by itself, establish electrical pin, bus arbitration, peripheral, or platform temporal fidelity.

## 2. Common architectural-state DTO

The common state contract must cover at least:

- Main register bank: AF, BC, DE, HL, SP, PC.
- Alternate bank: AF', BC', DE', HL'.
- Index registers: IX, IY.
- Interrupt/refresh registers: I, R.
- Interrupt state: IFF1, IFF2, IM 0/1/2, EI inhibit/delay state.
- Internal architectural latches: WZ/MEMPTR, Q and previous-Q, P/V interrupt-edge latch.
- Control state: HALT, latched NMI request, latched INT request/data.
- Any additional CPU-visible/internal state demonstrated by atomic behavior or independent reference vectors.

Private recursion guards, trace buffers, instruction counters and host diagnostic data stay outside the architectural DTO. Injected interrupt opcodes or prefix continuation are represented as resumable continuation state when the sequencer needs them, not dropped.

Both atomic and resumable cores must be able to import/export this DTO without changing instruction semantics. Snapshot serialization must include the DTO and the in-flight sequencer continuation; codec versions advance and reject incompatible/corrupt states.

## 3. Resumable transaction contract

Supported operation types must distinguish opcode/prefix acknowledge, memory read, memory write, port read, port write and internal cycles. Each request has a stable identity, address/port, direction, payload (for writes), nominal duration and an explicit completion edge. WAIT extends only the active request and is counted exactly once.

The bus/Scheduler integration must support:
- exact request identity and exactly-once completion;
- no architectural mutation on rejected IDs or invalid payloads;
- address and write-data capture across suspension;
- explicit internal T-states/cycles without fabricated memory transactions;
- restore validation across CPU phase, request metadata, timing counters and machine deadline;
- deterministic event ordering and stable replay under different chunk sizes.

## 4. Instruction completion scope

The eventual decoder must cover the entire documented Z80 instruction space implemented by the repository's atomic executor, including BASE, CB, ED, DD, FD, DDCB/FDCB, HALT, NMI and INT IM0/1/2. Unassigned ED encodings retain the atomic executor's documented NOP behavior. Repeated prefixes and prefix substitution follow the atomic executor and independent hardware vectors.

Instruction families are delivered in small dependency-ordered PRs:
1. Common architectural state + codec and differential import/export.
2. Remaining BASE data transfers, 16-bit arithmetic, relative/absolute flow, stack, exchange and control operations.
3. CB operations with register and memory destinations.
4. ED arithmetic, special-register transfers, port operations, block transfer/compare/I/O, returns and IM control.
5. DD/FD index substitution, displacement, IX/IY stack/control, DDCB/FDCB.
6. HALT and interrupt acknowledge/entry/return sequences, EI delay and edge cases.
7. Full undocumented flags/latches, refresh sequencing, repeated prefixes and state corner cases.

## 5. Per-PR acceptance

Every opcode/family PR includes:
- table-driven encoding coverage (including aliases and prefix sequences);
- fixed semantic vectors for result, flags and all affected architectural fields;
- differential test: execute same initial DTO and bus transcript through atomic and resumable paths, then compare full architectural DTO, memory writes, port writes, opcode bytes and nominal T-state total;
- transaction-order assertions and WAIT tests for each individual M-cycle;
- snapshot round-trip/restore at every reachable safe point;
- negative restore tests for inconsistent phase, address, payload, prefix context, request ID and timing;
- overflow/error atomicity tests;
- SMS Scheduler replay/determinism tests and SG-1000 regression where existing shared paths apply;
- Rust fmt, Clippy with `-D warnings`, full workspace tests, Python gates and Windows x64 gates.

The Scheduler stress job is PASS only when explicitly executed and successful. SKIPPED is never PASS. Local tests marked NOT RUN remain NOT RUN.

## 6. Completion gates and status

- **FUNCTIONAL_PASS:** every atomic-executor opcode/alias/prefix path has matching sequencer behavior and exhaustive differential/reference evidence.
- **STATE_PASS:** complete architectural state and all safe-point continuations round-trip and restore deterministically.
- **TEMPORAL_PASS:** independently proven machine-time/bus phase/WAIT/interrupt/pin behavior and integrated peripherals. This cannot be inferred from FUNCTIONAL_PASS.

The Z80-only sprint does not advance to other platforms until FUNCTIONAL_PASS and STATE_PASS are achieved. TEMPORAL_PASS remains a separate claim and requires independent timing/electrical evidence. No status is upgraded on the basis of compilation alone.
