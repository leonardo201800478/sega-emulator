# SMS Debugger Reconstruction

Status: reconstruction started; this branch establishes the opt-in boundary. The debugger is not complete.

## Non-negotiable runtime boundary

- `SmsTemporalWorker` remains the sole owner of `SmsTemporalMachine`, Z80, MachineScheduler, bus, VDP, PSG and emulated time.
- The debugger never creates a second machine and never executes hardware instructions itself.
- Debugger collection/analysis must run on a dedicated host thread, separate from the emulation worker.
- The emulation worker may publish bounded, owned diagnostic records through a non-blocking channel only when the user enables Debugger mode. It must never wait for the collector.
- A full channel drops diagnostic records and increments an observable dropped-record counter. It must not stall the scheduler.
- No debugger trace, detailed telemetry, diagnostic report or log file is enabled by default.

## User consent and persistence

- The global diagnostics consent is disabled on every application start and is never restored from preferences. A replacement ROM worker may inherit the current in-session consent, but no consent is inherited across application sessions.
- Enabling the global Coleta de logs option in the Debugger creates one `DiagnosticsConsent` capability passed to both the central logger and SMS worker. It authorizes logs, metrics, snapshots and traces; the trace toggle is a secondary control and cannot enable capture without this capability.
- Enabling it may collect bounded records in memory only; it must not create directories or files.
- Filesystem access is allowed only after a separate explicit Export command and a user-selected destination.
- Revoking consent stops publication/timers, disables worker trace at a command boundary, drops the collector, clears pending/retained records and host pacing buffers, and disables detailed runtime telemetry. Re-enabling starts with empty buffers.
- Ordinary runtime error reporting required for application stability is not a diagnostic trace and must not implicitly activate collection or persistence.

## Diagnostic domains

### Boot and CPU
- Z80 architectural registers and interrupt/HALT state at capture boundaries.
- Instruction address/opcode bytes and decoded instruction text only after a cycle-safe instruction-boundary trace source is available.
- Bus read/write and I/O events with machine timestamp, address, data, operation, base T-states and WAIT states.
- Mapper bank/latch transitions, reset/interrupt events and scheduler timestamps.

### Video and sprites
- VDP register/status transitions and scanline/dot position.
- VRAM/CRAM writes with machine timestamps.
- Sprite attribute-table changes and evaluated sprite entries (Y, X, pattern, attributes).
- Per-scanline sprite count, overflow/collision status and hardware-limit mode.
- Frame identity and rendering configuration so captures can be correlated with a reproduced frame.

### Audio (when explicitly requested)
- PSG register writes, channel/noise state snapshots and audio underrun counters.
- Audio diagnostics remain separately bounded and must not change audio rendering or pacing.

Unavailable fields must be reported as unavailable, never inferred or fabricated. Captures must not include ROM/BIOS bytes or full game memory dumps.

## Delivery sequence

1. **Opt-in correction (this branch):** remove unconditional runtime diagnostics activation; couple detailed telemetry to explicit Debugger-mode activation; preserve trace-disabled default and rollback UI state if activation fails.
2. **Collector isolation:** introduce a dedicated collector thread and bounded non-blocking event transport. Keep the machine and scheduler in the SMS worker.
3. **Boot trace adapter:** expose supported CPU/bus/mapper/interrupt events at instruction or bus boundaries without changing execution semantics.
4. **VDP/sprite adapter:** expose actual VDP evaluation and VRAM/CRAM events from existing hardware paths; do not reconstruct undocumented state in the GUI.
5. **Debugger UI:** Slint presents snapshots and collector buffers only; no hardware logic, cycle stepping or file I/O in GUI callbacks.
6. **Explicit export:** serialize versioned reports on a separate I/O task only after user confirmation.
7. **Validation:** test disabled-by-default, enable/disable lifecycle, bounded overflow, collector backpressure, no disk writes before export, determinism with debugger off/on, and Windows Release operation.

## Acceptance gates

- With global diagnostics consent off: trace disabled, snapshots rejected, detailed telemetry disabled, collector inactive, diagnostic buffers cleared, and no diagnostic files/directories created (except the separately specified minimal bootstrap-failure report).
- With Debugger mode on: events arrive in machine-time order; emulation never blocks on collector; overflow is counted.
- Disabling: no new records are published and all retained capture data is cleared.
- Export: no file is written until the user invokes Export; report contains a schema version, capture configuration and dropped counts.
- Debugger on/off produces identical CPU state, scheduler ticks, frame sequence and audio/video emulation outputs for the same input replay.
- Boot and sprite diagnostics are not declared complete until each domain has a real source in the hardware runtime and focused integration tests.
