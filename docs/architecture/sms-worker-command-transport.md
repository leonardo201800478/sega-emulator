# SMS runtime command transport

## Ownership

The `SmsTemporalWorker` thread remains the sole owner of `SmsTemporalMachine`,
the CPU, Scheduler, VDP, audio state, and machine-time progression. Frontend and
debugger code may hold only the cloneable command handle and owned snapshots.

## Command queue contract

- Host-to-worker commands use a bounded synchronous channel with a capacity of 64.
- Producers use `try_send`; a full queue is reported immediately and does not
  block the UI thread while waiting for capacity.
- Controller state changes are fire-and-forget after queue acceptance; runtime
  application errors are surfaced through worker error telemetry.
- Reply-bearing operations use one-shot bounded reply channels and a 100 ms
  receive deadline. Timeout and worker disconnection are explicit errors.
- The paused worker polls its command receiver at a bounded interval so worker
  shutdown does not depend on all cloned debugger handles being dropped.
- Worker shutdown is also signaled through shared atomic state. The worker
  checks that state at its event-loop boundary before accepting more work.
- Dropping the host handle detaches the join handle after signaling shutdown;
  the worker performs its audio and mailbox cleanup on its own thread, avoiding
  a synchronous join on the GUI thread.

A timed-out reply does not roll back a command already accepted by the worker.
Callers must treat timeout as an unknown completion state and refresh the
worker-owned state before retrying non-idempotent operations.

## Timing telemetry

- Frame timing is recorded only after `step_frame_until` completes an SMS frame.
- Frame time is the host-clock interval between those completion timestamps.
- Emulation FPS is derived from completed SMS frames and expires to zero after
  500 ms without a new completed frame; no NTSC/PAL nominal rate is assumed.
- Controller commands carry a complete button state. When multiple states are
  accepted before a frame completes, the latest accepted state determines the
  measured input timestamp; the pending timestamp is cleared at the first frame
  completion after the worker applies that state.
- Telemetry reads do not advance the Scheduler or machine clock.

## Validation

The Release workflows for the root workspace and SMS runtime must pass on the
exact PR head. Formatting is checked by the pinned Windows workflow. No local
Release validation is claimed when the Rust toolchain is unavailable.

## Frame completion semantics

A debugger breakpoint may stop the worker's frame-step operation at an instruction
boundary before the VDP completes the next frame. That partial execution must not:

- increment completed-frame telemetry or affect SMS emulation FPS/frame time;
- publish an incomplete framebuffer to the presentation mailbox;
- consume a pending input-to-frame sample.

The worker still updates machine-time diagnostics and drains generated audio
through the discard path while stopping at the breakpoint. Input latency remains
pending until the first subsequent complete SMS frame.

