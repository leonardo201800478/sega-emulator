# Shared Z80 CPU

This crate is the shared home of the Z80 implementation extracted from
`native/sms_core/src/cpu.rs`. The SMS implementation is the source of truth
for this first extraction because it contains the broader currently maintained
instruction coverage and the WZ/MEMPTR, Q/P latch, and indexed-CB behavior.

## Ownership

- `sega-z80-cpu`: CPU architectural state, instruction decoding and semantics.
- `sega-z80-alu`: shared arithmetic/logic primitives.
- `sega-z80-bus`: hardware-neutral CPU bus and diagnostic contracts.
- `sega-sms-core` / `sega-sg1000-core`: machine-specific bus adapters and
  interrupt wiring.
- `sega-machine-scheduler`: sole owner of emulated time at machine composition
  level; the CPU must not advance independent clocks.

## Temporal status

This is an as-is structural extraction. `Z80::step` still executes a complete
instruction and returns nominal aggregate T-states. Bus observation hooks are
diagnostic and do not suspend execution or dispatch scheduler events. This
crate is therefore **not cycle-accurate** and must not be marked TEMPORAL_PASS.

The next temporal increment is a resumable micro-operation state machine with
explicit M-cycle/T-state bus requests, WAIT handling, and machine-scheduler
coordination. Keep the current executor as the differential reference until
the new path passes instruction and bus-trace comparisons.

## Compatibility

The SMS crate retains `cpu` and `alu` module facades re-exporting this crate,
so existing SMS call sites remain source-compatible during the migration.
