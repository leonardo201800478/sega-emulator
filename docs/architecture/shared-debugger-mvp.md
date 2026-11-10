# Shared Debugger MVP — Architecture and Delivery Plan

Status: SMS adapter implementation in progress; runtime trace capture and native UI are wired on the feature branch, pending final Release validation
Scope owner: emulator runtime; GUI is a presentation and command surface only.

## Objective

Provide a small, practical diagnostic tool for emulator bug investigations. The first adapter is Sega Master System (SMS); the shared primitives must be reusable by SG-1000, Game Gear, Mega Drive/Genesis and later machines.

This is not intended to become a full-featured debugger or a second execution engine.

## Shared contract

The `sega-debugger-core` crate contains only console-neutral primitives:

- `BreakpointSet`: a bounded set of CPU program-counter addresses.
- `BoundedHistory<T>`: fixed-capacity FIFO history with an explicit dropped-entry counter.

The shared crate must not depend on a CPU, console, renderer, scheduler, GUI toolkit, ROM format or host API. Address interpretation and snapshots remain owned by each machine adapter.

## Machine adapter contract

Each console runtime may expose the following coarse-grained operations when its execution model supports them:

1. pause / resume;
2. execute one complete CPU instruction while preserving scheduler and peripheral timing;
3. add/remove/clear address breakpoints;
4. inspect a read-only machine snapshot;
5. read a bounded recent event history;
6. export a local diagnostic report.

The snapshot should contain only fields supported by that console: CPU architectural state, machine time, memory/mapper summary and video/audio status. Missing hardware fields must be represented as unavailable, not fabricated.

## SMS first adapter

The SMS adapter should expose:

- Z80 PC, SP, AF, BC, DE, HL, IX, IY, I, R, IFF1/IFF2, interrupt mode and HALT state;
- a small memory inspector with a bounded address range per request (maximum 256 bytes);
- Sega/Codemasters mapper kind, selected banks and memory-control latch when available;
- VDP registers and status;
- recent bus events with machine timestamp, operation, address, data, base T-states and WAIT states;
- an instruction breakpoint and single-instruction command.

The existing `SmsTemporalMachine::set_trace_enabled` / `take_trace` API is the event source. The runtime worker drains it after each frame or paused command into a 2,048-event FIFO history. Overflow discards the oldest event and increments the dropped counter. Capture is opt-in and disabling it clears both pending and retained events; the underlying machine trace vector is drained at each worker boundary.

## GUI

A compact Diagnostics window should expose:

- Pause / Continue / Step;
- a hexadecimal PC breakpoint field with Add / Remove / Clear;
- CPU and hardware summary;
- memory address + short hex/ASCII view;
- recent events and dropped-event count (trace capture is opt-in; the GUI refreshes live snapshots at 10 Hz while running);
- export report to a user-selected local file.

The GUI sends commands and renders snapshots only. It must not execute instructions, advance machine time, decode hardware state or own hardware timing.

## Cross-console extension

A new console adds an adapter that maps its native CPU/machine state into the shared debugger presentation contract. It must not duplicate breakpoint storage or bounded-history behavior. CPU-specific register names and memory maps remain adapter metadata.

## Safety and privacy

- Trace capture is opt-in and bounded.
- Diagnostic files are local and user-requested.
- Never include ROM bytes, BIOS bytes, copyrighted instruction dumps or full memory images in GitHub issues/PRs.
- Do not change emulation behavior to make debugging easier.
- Breakpoints stop only at documented instruction boundaries; no mid-cycle pause is implied.
- Step must complete one instruction through the owning scheduler, including all required bus and peripheral events.

## Acceptance gates

- Unit tests for breakpoint uniqueness, capacity, ordering and removal.
- Unit tests proving bounded history never exceeds capacity and counts discarded entries.
- SMS integration tests for step, breakpoint stop/resume, snapshots and bounded trace draining;
- runtime trace test proving capture is disabled by default, events are exposed in order, and disabling clears the history.
- GUI tests for command wiring where practical.
- Required repository gates: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --locked`.
- Classify every suite as PASS, FAIL, BLOCKED or NOT RUN; skipped suites are not PASS.
