# ADR-Z80-001 — Resumable Z80 execution and timed bus transactions

> **Policy note (2026-11-05):** this proposal is subordinate to [ADR-0016](../architecture/adr/0016-observable-fidelity-and-proportional-timing.md). A resumable/microsequenced execution path is conditional, not a universal requirement. Implement it only for a supported console/game/peripheral interaction whose observable result requires suspension between instruction/bus phases. The atomic executor remains the default for operations without observable intermediate effects.

- Status: PROPOSED
- Date: 2026-11-02
- Scope: `native/z80_cpu`, `native/z80_bus`, and future SMS / SG-1000 machine adapters
- Depends on: shared Z80 extraction in PR #398
- Does not authorize: replacing the current executor, changing opcode semantics, integrating a nominal timestamp as a physical bus edge, or removing either machine's compatibility facade

## Context

The shared Z80 currently exposes `Z80::step<B: CpuBus>()`. It runs an entire instruction synchronously, performs all bus accesses inside that call, and returns an aggregate nominal T-state count. The `CpuBus` trait returns read data immediately and has no suspension, WAIT, or scheduler handshake.

The SMS machine therefore executes the complete instruction before advancing its devices. Its PSG write queue uses a nominal instruction-relative offset for `OUT (n),A`; that offset is diagnostic/coarse timing, not a physical bus transaction. The same architectural limitation exists in the SG-1000 machine.

If a supported platform/game interaction requires observing a bus operation before the instruction retires, the atomic interface alone is insufficient for that specific path. In that case the CPU must be suspendible at the minimum required bus boundary, allow the machine to advance relevant devices to the corresponding timestamp, and resume after the transaction completes. This is not a requirement to microsequence every instruction.

## Hardware timing constraints from the Zilog Z80 CPU User Manual (UM008011-0816)

The timing chapter defines instructions as ordered machine cycles (M-cycles), each made of T-state clock periods:

- M1 opcode fetch is normally four T-states, with documented variants of five or six T-states; it includes the refresh portion.
- Ordinary memory read and write cycles are normally three T-states, before any external WAIT extension.
- I/O read and write cycles include one automatically inserted WAIT state; additional WAIT states may extend the operation.
- An accepted maskable interrupt starts a special M1 acknowledge cycle using IORQ rather than the ordinary MREQ fetch indication; two WAIT states are automatically inserted.
- WAIT is sampled during the documented cycle timing and can extend the bus operation. A WAIT condition is not equivalent to adding a fixed delay after an instruction.
- The manual describes NMI recognition at a machine-cycle boundary; interrupt acceptance must not be approximated as an arbitrary host callback during an instruction.

These are CPU-family timing rules. The SMS and SG-1000 adapters must still provide console-specific memory/I/O decode, interrupt lines, and clock ratios. Any silicon/profile-specific deviations require an explicit source and tests.

## Decision

Where a demonstrated gameplay dependency requires resumable bus operations, provide a resumable execution interface in the shared Z80 crate and keep the atomic `step()` path for operations that do not require temporal suspension. Both paths must preserve one semantic implementation and be compared where they overlap. This proposal does not authorize universal microsequencing or deprecating the atomic path.

The resumable path is a CPU state machine, not an instruction decoder that runs ahead and replays recorded bus accesses. It must not perform a read/write side effect before the corresponding bus phase has reached its scheduled completion point.

### Required execution model

1. `begin_instruction()` starts or resumes the current architectural instruction only when no bus request is outstanding.
2. The CPU exposes the next micro-operation / bus request. It may be a fetch, memory read, memory write, I/O read, I/O write, interrupt acknowledge, refresh phase, or internal T-state interval.
3. The machine adapter schedules the request against the shared master timeline and advances the CPU clock domain through the scheduler.
4. The bus adapter returns read data or write completion only at the correct transaction completion edge. Until then, the CPU remains suspended with a stable request identity and address/data.
5. WAIT is represented as an extension of the active bus cycle. The CPU does not retire the request or advance its micro-operation index while WAIT is asserted.
6. On completion, the CPU consumes the response, updates its internal sequencer state, and exposes the next operation.
7. Instruction retirement occurs only after every bus and internal phase has completed. Aggregate nominal T-state totals remain diagnostics, not the source of machine time.

### Contract shape (conceptual; names may change during implementation)

- `Z80MicroState`: serializable continuation state for prefix decode, opcode/operand latches, temporary ALU values, effective address, pending read/write, interrupt entry, refresh, and instruction retirement.
- `Z80BusRequest`: stable request ID, operation kind, address, optional write data, base M-cycle duration, and phase metadata.
- `Z80BusResponse`: matching request ID, read data where applicable, completion status, and elapsed wait-state count.
- `Z80RunState`: whether the CPU is ready, waiting for a bus response, or has retired an instruction.
- A machine scheduler owns all emulated timestamps. The CPU sequencer owns no wall clock, timer, or private machine clock.

The public API must make invalid transitions unrepresentable or return a typed error: duplicate completion, response for the wrong request, advancing while a response is pending, or retiring with an unfinished bus cycle.

## Micro-operation coverage

The first implementation slice must cover the primitives needed to prove the mechanism, not claim complete Z80 cycle accuracy:

1. NOP and register-only operations (M1 plus internal phases where specified).
2. Immediate operand fetch (e.g. `LD A,n`).
3. Memory read and write (e.g. `LD A,(HL)`, `LD (HL),A`).
4. Stack write/read sequencing (PUSH/POP or CALL/RET).
5. I/O read/write with the mandatory I/O WAIT state.
6. Prefix fetch sequencing (CB, ED, DD, FD and indexed-CB), with refresh accounting.
7. HALT refresh behavior.
8. NMI and IM 0/1/2 acknowledge/entry sequencing, including automatic interrupt-acknowledge waits.

Conditional branches, block instructions, undocumented opcodes, and all indexed corner cases must be added in explicit slices. No opcode family is considered covered merely because its aggregate T-state total matches.

## Invariants

- No CPU bus side effect is committed before its scheduled bus phase.
- Every completed instruction has the same architectural result as the reference executor for the same initial CPU and memory/input state, except for differences explicitly identified as existing reference bugs and deferred to a separate semantic-correction change.
- The sum of completed M-cycle T-states, internal T-states, and inserted WAIT states equals the elapsed CPU-domain clocks.
- A machine timestamp never moves backwards; a response cannot be applied to a different request.
- CPU, VDP, PSG, and machine I/O observe a single deterministic event ordering.
- No callback per T-state crosses the Rust/Python FFI boundary.
- Save states include the in-flight micro-operation, pending request identity, wait count, and clock-domain phase.
- The legacy executor remains available until both SMS and SG-1000 pass differential and integration gates.

## Validation plan

### Unit / primitive tests

- For each primitive explicitly assigned to the temporal gameplay path, validate exact M-cycle sequence/duration and the documented completion point of external side effects.
- For temporal bus requests, zero/one/multiple WAIT states preserve request identity and suspend/resume correctly when the target platform exposes WAIT.
- Model automatic I/O/interrupt-acknowledge waits only in profiles where the CPU/platform contract requires them.
- Prefix fetches and refresh increments must not be duplicated across suspension when that path is active.
- Save/restore during a pending temporal bus request produces the same continuation and final state.
- Operations not assigned to a temporal path remain valid through the atomic executor with exact semantics and nominal timing.

### Differential tests

Where both paths implement the same temporal-relevant operation, run the atomic and resumable executors from identical snapshots and deterministic scripts. Compare architectural registers/latches, memory and port side effects, interrupt/HALT state and nominal duration. Do not require a resumable-path counterpart for atomic-only instructions; record the exact overlap and any intentional timing-model differences.

### Machine integration gates

- SMS and SG-1000 adapters validate functional address decode, open-bus behavior and interrupt wiring. WAIT and detailed bus timing are added only for a real, evidenced consumer.
- Register device clock domains with the Scheduler only for components participating in the selected temporal path, using verified ratios/phases.
- Apply PSG writes at a completed I/O bus phase when gameplay/audio evidence requires that timing; otherwise preserve the defined atomic write semantics and nominal offset.
- Scheduler stress and deterministic trace replay are required for a temporal integration change; SKIPPED is not PASS.
- The atomic path is not deprecated merely because a temporal path exists. Any future removal requires a separate ADR, demonstrated need and compatibility evidence.

## Non-goals

- No JIT, speculative execution, instruction batching that skips bus phases, or frame-skip.
- No timing implementation in Python.
- No changes to SMS VDP profile selection, SG-1000 TMS9918A behavior, PSG polynomial/profile semantics, mapper behavior, or opcode flags in this structural milestone.
- No assertion of TEMPORAL_PASS from unit tests alone.

## References

- Zilog, *Z80 CPU User Manual*, UM008011-0816, “Timing” chapter: machine cycles, instruction fetch, memory read/write, I/O cycles, interrupt request/acknowledge, and WAIT-state timing.
- Repository implementation at PR #398: `native/z80_cpu/src/cpu.rs` and `native/z80_bus/src/lib.rs`.
- Current machine adapters: `native/sms_core/src/machine.rs` and `native/sg1000_core/src/machine.rs`.
