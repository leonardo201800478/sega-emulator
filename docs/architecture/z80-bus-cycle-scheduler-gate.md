# Z80 bus-cycle timeline — gate for SMS1 scheduler integration

**Status:** architecture gate / implementation plan  
**Scope:** shared Z80 core and SMS1 machine integration  
**Temporal accuracy:** NOT IMPLEMENTED  
**Purpose:** define what must exist before CPU bus activity can be interleaved with SMS1 raster events.

## 1. Current repository facts

The current code has an instruction-level CPU API:

- `native/sms_core/src/cpu.rs`: `Z80::step()` executes one instruction and returns aggregate nominal T-states.
- `native/z80_bus/src/lib.rs`: `CpuBus` exposes untimed `read_memory`, `write_memory`, `read_port` and `write_port`.
- `Z80BusEvent` and `Z80TStateSample` are observational records, not scheduler callbacks. The source explicitly limits their current emission to selected ED block instruction families.
- `native/sms_core/src/machine.rs`: `SmsMachine::step()` runs the entire instruction and then advances the legacy `SmsVdp` by the returned aggregate cycles.
- `SmsMachineBus::with_psg_queue` captures the instruction's starting T-state. The PSG OUT event uses a nominal +11 T-state note for immediate OUT; the code itself marks full bus-phase timing as a separate milestone.
- `native/sms_core/src/vdp.rs`: the legacy VDP advances in nominal 228-T-state scanline chunks. Its own module header says it is not dot-cycle exact.

These facts mean that a machine-level event queue alone cannot currently interleave Z80 port accesses, VRAM writes and VDP raster transitions at their actual bus phases.

## 2. Why instruction-boundary integration is not acceptable

If `SmsMachine::step()` is followed by a VDP/Scheduler advance for the full instruction duration, all CPU bus effects have already happened before the VDP timeline advances. This creates false ordering around:

- writes to VDP control/data ports near H/V counter transitions;
- writes to VRAM/CRAM during display fetch and sprite evaluation;
- reads of status/counters near latch edges;
- IRQ assertion/deassertion and CPU interrupt sampling;
- WAIT states or bus contention that stall only selected machine cycles.

An instruction-boundary bridge may be deterministic, but it is not cycle-accurate. It must not be described as a temporal implementation or used to claim `TEMPORAL_PASS`.

## 3. Required execution contract

The CPU must become resumable at deterministic bus/micro-operation boundaries. The machine owns the bus and scheduler; the CPU core owns the Z80's instruction and micro-operation state.

Each bus transaction must provide:

| Field | Requirement |
|---|---|
| Instruction identity | PC/opcode/prefix context for diagnostics |
| Relative offset | T-state offset from the start of the current instruction or a monotonic CPU clock cursor |
| Transaction | M1 fetch, memory read/write, I/O read/write, interrupt acknowledge, refresh, internal cycle |
| Address/data | Values driven or sampled at that bus transaction |
| Duration | Documented base T-states, before machine-inserted WAIT extension |
| Commit point | Exact point at which a write takes effect or a read value is sampled |
| Resumable state | CPU can pause and continue without repeating or skipping a bus effect |

The machine then maps the CPU clock domain to `MachineTime`, advances all registered clock domains, dispatches earlier events, performs the bus transaction, and resumes the CPU. No Python callback is involved.

## 4. Required opcode-family coverage

A cycle-timed path cannot be enabled for SMS while only selected opcodes emit timing events. The acceptance matrix must cover:

1. M1 opcode fetch and refresh behavior.
2. Immediate operand fetches and ordinary memory reads/writes.
3. Register-only and internal-cycle instructions.
4. Stack operations, calls, returns, RST and conditional branches.
5. All CB, ED, DD, FD and indexed-CB families.
6. I/O instructions and interrupt acknowledge cycles.
7. HALT refresh cycles, NMI, IM 0/1/2 and EI delay.
8. Repeated/block instructions, including repeat boundaries.
9. CPU WAIT insertion and interrupt sampling at documented boundaries.

Unsupported or not-yet-timed opcodes must remain explicitly outside the timed execution profile; they must not silently fall back to instruction-boundary timing in a cycle-accurate run.

## 5. Proposed staged implementation

### Z0 — trace inventory

- Enumerate every direct CPU bus access and every helper that performs one.
- Map each opcode family to the helper path and aggregate T-state formula.
- Identify current instrumented ED block paths and their oracle provenance.
- Do not change execution behavior.

### Z1 — resumable transaction API

- Define a typed CPU bus transaction/micro-op representation.
- Add an explicit CPU execution cursor and resumable pending operation.
- Preserve the existing instruction-step API for compatibility while the timed path is incomplete.
- Add save-state fields for the in-flight instruction/micro-op once serde support is available.

### Z2 — base instruction timing

- Implement M1, immediate fetch, memory read/write and internal-cycle sequencing for a deliberately bounded opcode subset.
- Verify each transaction offset, address, data and duration against independent traces.
- Keep untimed instructions on the existing path only when running in the explicitly non-cycle-accurate compatibility mode.

### Z3 — full opcode and interrupt coverage

- Extend the timed path across all opcode prefixes and interrupt modes.
- Prove no bus access is duplicated or omitted across pause/resume.
- Compare architectural results against independent Z80 instruction tests and compare bus traces where the oracle provides them.

### Z4 — machine scheduler bridge

- The SMS machine maps CPU T-states to the documented CPU clock domain.
- Each timed bus transaction is interleaved with pending machine events in `MachineScheduler`.
- Device event priorities are supplied by the SMS hardware profile only after evidence.
- VDP status/IRQ and memory side effects are sampled/committed at the proven event timestamp.

### Z5 — SMS1 VDP integration

- Only after Z4 can the SMS1 VDP receive timed register/data accesses and produce raster events.
- Add NTSC/PAL profiles from SMS1 evidence; do not reuse GG, TMS9918 or Genesis phase constants without SMS1 corroboration.
- Add replay/chunk-invariance and external diagnostic-ROM/hardware trace comparisons.

## 6. Acceptance criteria

- The same instruction stream and initial state produce identical CPU state and bus transaction stream regardless of how the outer scheduler divides idle advancement intervals.
- Every transaction is monotonic and ordered; no transaction is retroactively inserted before the CPU cursor.
- Pause/resume at any supported micro-op boundary is deterministic.
- Interrupts are sampled only at documented Z80 boundaries.
- WAIT extension changes machine time and CPU progress without changing the nominal instruction's internal ordering.
- Scheduler advancement cannot skip a pending event.
- SG-1000 regression remains green.
- No timing logic or per-instruction callback moves into Python.
- Formal CPU and hardware trace coverage is recorded separately; no temporal pass is inferred from unit tests alone.

## 7. Explicit non-goals

- No JIT, speculative parallelism, frame skipping or Python-side cycle counting.
- No inferred 228-T-state SMS1 line model.
- No port timestamp guessed from the current instruction's aggregate cycle count.
- No use of legacy `SmsVdp` as a hardware timing oracle.
- No promotion of the existing selected ED traces into complete Z80 bus coverage.

## 8. Relationship to current scheduler work

The generic `MachineScheduler<T>` in `native/temporal_core` owns master time, rational clock domains and deterministic event ordering. It is intentionally not a CPU interpreter or device dispatcher. This document defines the missing CPU-side contract required before the SMS machine can connect its Z80 bus effects to that timeline.
